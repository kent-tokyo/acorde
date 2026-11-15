//! Two independently timed, chord-grounded inner voices in one added part.

use serde::{Deserialize, Serialize};

use super::arrange::{PositionedAccordionChord, chord_at_note, preset_chord_pitch_classes};
use super::chord_proposals::fingerprint;
use super::chordal_pad::{
    MAX_CHANGES, MAX_GENERATED_NOTES, MAX_MEASURES, MAX_SOURCE_NOTES, emit_rest, emit_span,
    free_channel, melody_spans, tick,
};
use super::gm::{instrument_range, program_name};
use super::notation::Clef;
use super::score::{Measure, MeasureLength, NoteAddr, Part, Score, Staff};

pub const INNER_VOICES_CONTRACT_VERSION: u16 = 1;

#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "kebab-case")]
pub enum InnerVoicePattern {
    Held,
    FirstHalf,
    SecondHalf,
    QuarterPulse,
    OffbeatPulse,
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
#[serde(deny_unknown_fields)]
pub struct InnerVoiceLine {
    pub pattern: InnerVoicePattern,
    pub low_midi: u8,
    pub center_midi: u8,
    pub high_midi: u8,
    /// Maximum distance from this line's previous sounding pitch, including a
    /// new phrase entry. Zero permits a repeated pitch only.
    pub max_leap_semitones: u8,
}

#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
#[serde(deny_unknown_fields)]
pub struct InnerVoicesRequest {
    pub chords: Vec<PositionedAccordionChord>,
    pub melody_part_index: usize,
    pub midi_program: u8,
    pub lower: InnerVoiceLine,
    pub upper: InnerVoiceLine,
    pub min_voice_gap_semitones: u8,
    pub min_melody_gap_semitones: u8,
    /// Empty means active throughout. Otherwise one flag per physical bar.
    #[serde(default, skip_serializing_if = "Vec::is_empty")]
    pub active_measures: Vec<bool>,
    /// At a marked ending, each line rests after its first sounding slot.
    #[serde(default, skip_serializing_if = "Vec::is_empty")]
    pub phrase_end_measures: Vec<usize>,
}

#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
#[serde(deny_unknown_fields)]
pub struct ResolvedInnerVoices {
    pub contract_version: u16,
    pub engine_version: String,
    pub source_fingerprint: String,
    pub request: InnerVoicesRequest,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct InnerVoicesResult {
    pub score: Score,
    pub resolved: ResolvedInnerVoices,
    pub added_part: usize,
    /// Addresses use voice 0 for the lower line and voice 1 for the upper.
    pub generated_notes: Vec<NoteAddr>,
    pub attack_counts: [usize; 2],
}

#[derive(Debug, thiserror::Error)]
pub enum InnerVoicesError {
    #[error("invalid or oversized source score")]
    InvalidSource,
    #[error("invalid inner-voice request")]
    InvalidRequest,
    #[error("invalid chord at measure {0}")]
    InvalidChord(usize),
    #[error("unsupported rhythm at measure {0}")]
    UnsupportedRhythm(usize),
    #[error("melody clearance is unsupported at measure {0}")]
    UnsupportedMelody(usize),
    #[error("no MIDI channel is available")]
    NoMidiChannel,
    #[error("no playable pitch for voice {voice} at measure {measure}")]
    Unplayable { voice: usize, measure: usize },
    #[error("generated-note limit exceeded")]
    GeneratedNoteLimit,
    #[error("invalid generated score")]
    InvalidOutput,
    #[error("unsupported inner-voice contract")]
    UnsupportedContract,
    #[error("inner-voice source mismatch")]
    SourceMismatch,
}

fn valid_line(line: &InnerVoiceLine, instrument_low: u8, instrument_high: u8) -> bool {
    instrument_low <= line.low_midi
        && line.low_midi <= line.center_midi
        && line.center_midi <= line.high_midi
        && line.high_midi <= instrument_high
        && line.max_leap_semitones <= 24
}

fn is_sounding(pattern: InnerVoicePattern, from: u32, bar_ticks: u32) -> bool {
    match pattern {
        InnerVoicePattern::Held => true,
        InnerVoicePattern::FirstHalf => from < bar_ticks / 2,
        InnerVoicePattern::SecondHalf => from >= bar_ticks / 2,
        InnerVoicePattern::QuarterPulse => from % 32 < 16,
        InnerVoicePattern::OffbeatPulse => from % 32 >= 16,
    }
}

fn rhythm_cuts(pattern: InnerVoicePattern, bar_ticks: u32) -> Vec<u32> {
    let mut cuts = vec![0, bar_ticks];
    match pattern {
        InnerVoicePattern::Held => {}
        InnerVoicePattern::FirstHalf | InnerVoicePattern::SecondHalf => {
            cuts.push(bar_ticks / 2);
        }
        InnerVoicePattern::QuarterPulse | InnerVoicePattern::OffbeatPulse => {
            let mut edge = 16;
            while edge < bar_ticks {
                cuts.push(edge);
                edge += 16;
            }
        }
    }
    cuts
}

fn choose_pitch(
    line: &InnerVoiceLine,
    tones: &[u8],
    previous: Option<u8>,
    ceiling: Option<i16>,
) -> Option<u8> {
    (line.low_midi..=line.high_midi)
        .filter(|pitch| tones.contains(&(pitch % 12)))
        .filter(|pitch| ceiling.is_none_or(|limit| i16::from(*pitch) <= limit))
        .filter(|pitch| previous.is_none_or(|old| pitch.abs_diff(old) <= line.max_leap_semitones))
        .min_by_key(|pitch| {
            (
                previous.map_or(0, |old| pitch.abs_diff(old)),
                pitch.abs_diff(line.center_midi),
                *pitch,
            )
        })
}

pub fn arrange_inner_voices(
    source: &Score,
    mut request: InnerVoicesRequest,
) -> Result<InnerVoicesResult, InnerVoicesError> {
    let Some(reference) = source.parts.first().and_then(|part| part.staves.first()) else {
        return Err(InnerVoicesError::InvalidSource);
    };
    let count = reference.measures.len();
    let source_notes = source
        .parts
        .iter()
        .flat_map(|part| &part.staves)
        .flat_map(|staff| &staff.measures)
        .flat_map(|measure| &measure.voices)
        .fold(0usize, |sum, voice| sum.saturating_add(voice.len()));
    if count == 0
        || count > MAX_MEASURES
        || source.parts.len() > 32
        || source_notes > MAX_SOURCE_NOTES
        || request.melody_part_index >= source.parts.len()
        || source
            .parts
            .iter()
            .flat_map(|part| &part.staves)
            .any(|staff| staff.measures.len() != count)
        || !super::validate::validate(source).is_valid()
    {
        return Err(InnerVoicesError::InvalidSource);
    }
    let (instrument_low, instrument_high) = instrument_range(request.midi_program);
    if !valid_line(&request.lower, instrument_low, instrument_high)
        || !valid_line(&request.upper, instrument_low, instrument_high)
        || request.min_voice_gap_semitones == 0
        || request.min_voice_gap_semitones > 24
        || request.min_melody_gap_semitones > 24
        || i16::from(request.lower.high_midi) + i16::from(request.min_voice_gap_semitones)
            > i16::from(request.upper.low_midi)
        || (!request.active_measures.is_empty() && request.active_measures.len() != count)
        || request.chords.is_empty()
        || request.chords.len() > MAX_CHANGES
        || request.phrase_end_measures.len() > count
    {
        return Err(InnerVoicesError::InvalidRequest);
    }
    request.phrase_end_measures.sort_unstable();
    if request
        .phrase_end_measures
        .iter()
        .any(|index| *index >= count)
        || request
            .phrase_end_measures
            .windows(2)
            .any(|pair| pair[0] == pair[1])
    {
        return Err(InnerVoicesError::InvalidRequest);
    }
    request.chords.sort_by(|a, b| {
        a.measure_index
            .cmp(&b.measure_index)
            .then_with(|| a.offset_beats.total_cmp(&b.offset_beats))
    });
    let channel = free_channel(source).ok_or(InnerVoicesError::NoMidiChannel)?;
    let mut bar_ticks = Vec::with_capacity(count);
    let mut bar_starts = vec![0u32];
    for measure in 0..count {
        let ticks = tick(source.playback_measure_beats(measure))
            .filter(|value| *value > 0)
            .ok_or(InnerVoicesError::UnsupportedRhythm(measure))?;
        let next = bar_starts
            .last()
            .and_then(|start| start.checked_add(ticks))
            .ok_or(InnerVoicesError::InvalidSource)?;
        if next > 8192 * 32 {
            return Err(InnerVoicesError::InvalidSource);
        }
        bar_ticks.push(ticks);
        bar_starts.push(next);
    }
    let spans = melody_spans(source, request.melody_part_index, &bar_starts)
        .map_err(InnerVoicesError::UnsupportedMelody)?;
    let mut previous_position = None;
    for placed in &request.chords {
        let measure = placed.measure_index;
        let Some(&bar_length) = bar_ticks.get(measure) else {
            return Err(InnerVoicesError::InvalidChord(measure));
        };
        let offset = tick(placed.offset_beats).ok_or(InnerVoicesError::InvalidChord(measure))?;
        let position = bar_starts[measure] + offset;
        if offset >= bar_length
            || previous_position.is_some_and(|previous| position <= previous)
            || preset_chord_pitch_classes(&placed.chord).is_none()
        {
            return Err(InnerVoicesError::InvalidChord(measure));
        }
        previous_position = Some(position);
    }
    if request.chords[0].measure_index != 0 || request.chords[0].offset_beats != 0.0 {
        return Err(InnerVoicesError::InvalidRequest);
    }
    let fingerprint = fingerprint(source).ok_or(InnerVoicesError::InvalidSource)?;
    let lines = [&request.lower, &request.upper];
    let mut staff = Staff::new(if request.upper.center_midi < 60 {
        Clef::Bass
    } else {
        Clef::Treble
    });
    let mut previous = [None, None];
    let mut attack_counts = [0, 0];
    let mut generated_notes = Vec::new();
    let added_part = source.parts.len();
    let mut generated_events = 0usize;
    for measure in 0..count {
        let meter = reference.meter_at(measure, &source.settings.time_signature);
        let original = &reference.measures[measure];
        let mut bar = Measure::empty(meter.numerator, meter.denominator);
        bar.number = original.number;
        bar.time_sig = original.time_sig.clone();
        bar.time_sig_hidden = original.time_sig_hidden;
        bar.time_symbol = original.time_symbol;
        bar.key_sig = original.key_sig.clone();
        bar.barline_left = original.barline_left.clone();
        bar.barline_right = original.barline_right.clone();
        bar.volta = original.volta.clone();
        let beats = f64::from(bar_ticks[measure]) / 32.0;
        if (beats - meter.total_beats()).abs() > 1e-9 {
            bar.actual_length = MeasureLength::from_beats(beats);
        }
        let active = request.active_measures.is_empty() || request.active_measures[measure];
        if measure > 0
            && active
            && !request.active_measures.is_empty()
            && !request.active_measures[measure - 1]
        {
            previous = [None, None];
        }
        for (voice_index, line) in lines.iter().enumerate() {
            let mut cuts = rhythm_cuts(line.pattern, bar_ticks[measure]);
            cuts.extend(
                request
                    .chords
                    .iter()
                    .filter(|placed| placed.measure_index == measure)
                    .filter_map(|placed| tick(placed.offset_beats)),
            );
            cuts.sort_unstable();
            cuts.dedup();
            let mut voice = Vec::new();
            let ending = request.phrase_end_measures.binary_search(&measure).is_ok();
            let mut ending_sounded = false;
            for segment in cuts.windows(2) {
                let (from, to) = (segment[0], segment[1]);
                let sounding = active
                    && is_sounding(line.pattern, from, bar_ticks[measure])
                    && (!ending || !ending_sounded);
                if sounding {
                    let chord = chord_at_note(&request.chords, measure, f64::from(from) / 32.0)
                        .ok_or(InnerVoicesError::InvalidChord(measure))?;
                    let tones = preset_chord_pitch_classes(chord)
                        .ok_or(InnerVoicesError::InvalidChord(measure))?;
                    let absolute_from = f64::from(bar_starts[measure] + from) / 32.0;
                    let absolute_to = f64::from(bar_starts[measure] + to) / 32.0;
                    let ceiling = spans
                        .iter()
                        .filter(|(start, end, _)| {
                            *start < absolute_to - 1e-9 && *end > absolute_from + 1e-9
                        })
                        .map(|(_, _, midi)| *midi - i16::from(request.min_melody_gap_semitones))
                        .min();
                    let pitch = choose_pitch(line, &tones, previous[voice_index], ceiling).ok_or(
                        InnerVoicesError::Unplayable {
                            voice: voice_index,
                            measure,
                        },
                    )?;
                    if !emit_span(&[pitch], to - from, &mut voice) {
                        return Err(InnerVoicesError::UnsupportedRhythm(measure));
                    }
                    previous[voice_index] = Some(pitch);
                    attack_counts[voice_index] += 1;
                    ending_sounded = true;
                } else if !emit_rest(to - from, &mut voice) {
                    return Err(InnerVoicesError::UnsupportedRhythm(measure));
                }
            }
            generated_events = generated_events.saturating_add(voice.len());
            if generated_events > MAX_GENERATED_NOTES {
                return Err(InnerVoicesError::GeneratedNoteLimit);
            }
            for (note, event) in voice.iter().enumerate() {
                if !event.is_rest && !event.tie_end {
                    generated_notes.push(NoteAddr {
                        part: added_part,
                        staff: 0,
                        measure,
                        voice: voice_index,
                        note,
                    });
                }
            }
            bar.voices[voice_index] = voice;
        }
        staff.measures.push(bar);
    }
    let mut part = Part::new(program_name(request.midi_program), "Inner");
    part.midi_program = request.midi_program;
    part.midi_channel = channel;
    part.staves.push(staff);
    let mut score = source.clone();
    score.parts.push(part);
    if !super::validate::validate(&score).is_valid() {
        return Err(InnerVoicesError::InvalidOutput);
    }
    Ok(InnerVoicesResult {
        score,
        resolved: ResolvedInnerVoices {
            contract_version: INNER_VOICES_CONTRACT_VERSION,
            engine_version: env!("CARGO_PKG_VERSION").into(),
            source_fingerprint: fingerprint,
            request,
        },
        added_part,
        generated_notes,
        attack_counts,
    })
}

pub fn replay_inner_voices(
    source: &Score,
    resolved: &ResolvedInnerVoices,
) -> Result<InnerVoicesResult, InnerVoicesError> {
    if resolved.contract_version != INNER_VOICES_CONTRACT_VERSION
        || resolved.engine_version != env!("CARGO_PKG_VERSION")
    {
        return Err(InnerVoicesError::UnsupportedContract);
    }
    if fingerprint(source).as_deref() != Some(resolved.source_fingerprint.as_str()) {
        return Err(InnerVoicesError::SourceMismatch);
    }
    let result = arrange_inner_voices(source, resolved.request.clone())?;
    if result.resolved != *resolved {
        return Err(InnerVoicesError::UnsupportedContract);
    }
    Ok(result)
}
