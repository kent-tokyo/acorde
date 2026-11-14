//! Add a single-note bass part beneath an existing score.

use serde::{Deserialize, Serialize};

use super::arrange::{PositionedAccordionChord, accordion_pitch_class, preset_chord_pitch_classes};
use super::chordal_pad::{
    MAX_CHANGES, MAX_GENERATED_NOTES, MAX_MEASURES, MAX_SOURCE_NOTES, emit_span, free_channel, tick,
};
use super::gm::{instrument_range, program_name};
use super::notation::Clef;
use super::repeat::measure_sequence;
use super::score::{Measure, MeasureLength, Note, NoteAddr, Part, Score, Staff};

#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "kebab-case")]
pub enum BassLinePattern {
    RootPulse,
    RootFifth,
    /// Root/fifth pulses with a lower chromatic approach into the next change.
    Approach,
}

#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
pub struct BassLineOptions {
    pub chords: Vec<PositionedAccordionChord>,
    pub pattern: BassLinePattern,
    pub midi_program: u8,
    pub low_midi: u8,
    pub high_midi: u8,
    pub center_midi: u8,
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct BassLineDiagnostic {
    pub code: String,
    pub measure_index: Option<usize>,
}

/// Absolute semitone movement between successive bass attacks.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct BassLineMotionReport {
    pub written_attack_count: usize,
    pub playback_attack_count: usize,
    pub written_motion_cost: u64,
    pub playback_motion_cost: u64,
    pub backward_jump_motion_cost: u64,
    pub backward_jump_transition_count: usize,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct BassLineResult {
    pub score: Option<Score>,
    pub resolved_options: BassLineOptions,
    pub added_part: Option<usize>,
    pub generated_notes: Vec<NoteAddr>,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub motion_report: Option<BassLineMotionReport>,
    pub diagnostics: Vec<BassLineDiagnostic>,
    pub contract_version: u16,
}

struct Change {
    tick: u32,
    bass_class: u8,
    fifth_class: u8,
}

fn reject(mut result: BassLineResult, code: &str, measure: Option<usize>) -> BassLineResult {
    result.score = None;
    result.added_part = None;
    result.generated_notes.clear();
    result.motion_report = None;
    result.diagnostics.push(BassLineDiagnostic {
        code: code.into(),
        measure_index: measure,
    });
    result
}

fn pitch_for(class: u8, options: &BassLineOptions, previous: Option<u8>) -> Option<u8> {
    (options.low_midi..=options.high_midi)
        .filter(|pitch| pitch % 12 == class)
        .min_by_key(|pitch| {
            (
                pitch.abs_diff(previous.unwrap_or(options.center_midi)),
                pitch.abs_diff(options.center_midi),
                *pitch,
            )
        })
}

fn motion_report(score: &Score, attacks: &[Vec<u8>]) -> BassLineMotionReport {
    let written = attacks.iter().flatten().copied().collect::<Vec<_>>();
    let written_motion_cost = written
        .windows(2)
        .map(|pair| u64::from(pair[0].abs_diff(pair[1])))
        .sum();
    let mut report = BassLineMotionReport {
        written_attack_count: written.len(),
        playback_attack_count: 0,
        written_motion_cost,
        playback_motion_cost: 0,
        backward_jump_motion_cost: 0,
        backward_jump_transition_count: 0,
    };
    let mut previous_pitch: Option<u8> = None;
    let mut previous_measure: Option<usize> = None;
    for measure in measure_sequence(score) {
        let backward_jump = previous_measure.is_some_and(|prior| measure <= prior);
        for (index, &pitch) in attacks[measure].iter().enumerate() {
            if let Some(previous) = previous_pitch {
                let cost = u64::from(previous.abs_diff(pitch));
                report.playback_motion_cost += cost;
                if index == 0 && backward_jump {
                    report.backward_jump_motion_cost += cost;
                    report.backward_jump_transition_count += 1;
                }
            }
            previous_pitch = Some(pitch);
            report.playback_attack_count += 1;
        }
        previous_measure = Some(measure);
    }
    report
}

/// Append a monophonic GM bass part. Chords must cover the beginning of the
/// score; unsupported input returns diagnostics and no partially changed score.
pub fn arrange_bass_line(score: &Score, mut options: BassLineOptions) -> BassLineResult {
    options.chords.sort_by(|left, right| {
        left.measure_index
            .cmp(&right.measure_index)
            .then_with(|| left.offset_beats.total_cmp(&right.offset_beats))
    });
    let mut result = BassLineResult {
        score: None,
        resolved_options: options,
        added_part: None,
        generated_notes: Vec::new(),
        motion_report: None,
        diagnostics: Vec::new(),
        contract_version: 1,
    };
    let options = &result.resolved_options;
    let Some(reference) = score.parts.first().and_then(|part| part.staves.first()) else {
        return reject(result, "unsupported-source", None);
    };
    let count = reference.measures.len();
    let source_notes = score
        .parts
        .iter()
        .flat_map(|part| &part.staves)
        .flat_map(|staff| &staff.measures)
        .flat_map(|bar| &bar.voices)
        .fold(0usize, |sum, voice| sum.saturating_add(voice.len()));
    if count == 0
        || count > MAX_MEASURES
        || score.parts.len() > 32
        || source_notes > MAX_SOURCE_NOTES
        || options.chords.is_empty()
        || options.chords.len() > MAX_CHANGES
        || score
            .parts
            .iter()
            .flat_map(|part| &part.staves)
            .any(|staff| staff.measures.len() != count)
    {
        return reject(result, "input-limit", None);
    }
    if !super::validate::validate(score).is_valid() {
        return reject(result, "invalid-source", None);
    }
    if options.midi_program > 127
        || options.low_midi > options.center_midi
        || options.center_midi > options.high_midi
        || options.high_midi > 127
    {
        return reject(result, "invalid-register", None);
    }
    let (instrument_low, instrument_high) = instrument_range(options.midi_program);
    if options.low_midi < instrument_low || options.high_midi > instrument_high {
        return reject(result, "instrument-range", None);
    }
    let Some(channel) = free_channel(score) else {
        return reject(result, "no-midi-channel", None);
    };
    let mut starts = Vec::with_capacity(count + 1);
    starts.push(0u32);
    for measure in 0..count {
        let Some(length) = tick(score.playback_measure_beats(measure)) else {
            return reject(result, "unsupported-rhythm", Some(measure));
        };
        let Some(end) = starts.last().and_then(|start| start.checked_add(length)) else {
            return reject(result, "input-limit", Some(measure));
        };
        if length == 0 || end > 8192 * 32 {
            return reject(result, "input-limit", Some(measure));
        }
        starts.push(end);
    }
    let mut changes = Vec::with_capacity(options.chords.len());
    for placed in &options.chords {
        let measure = placed.measure_index;
        let Some(window) = starts.get(measure..measure.saturating_add(2)) else {
            return reject(result, "chord-position", Some(measure));
        };
        let Some(offset) = tick(placed.offset_beats) else {
            return reject(result, "chord-position", Some(measure));
        };
        let Some(position) = window[0].checked_add(offset) else {
            return reject(result, "chord-position", Some(measure));
        };
        if position >= window[1]
            || changes
                .last()
                .is_some_and(|prior: &Change| prior.tick == position)
        {
            return reject(result, "chord-position", Some(measure));
        }
        let Some(root) = accordion_pitch_class(&placed.chord.root) else {
            return reject(result, "unsupported-chord", Some(measure));
        };
        let Some(tones) = preset_chord_pitch_classes(&placed.chord) else {
            return reject(result, "unsupported-chord", Some(measure));
        };
        let bass = placed
            .chord
            .bass
            .as_deref()
            .map(accordion_pitch_class)
            .unwrap_or(Some(root));
        let Some(bass_class) = bass else {
            return reject(result, "unsupported-chord", Some(measure));
        };
        let Some(fifth_class) = [7, 6, 8]
            .into_iter()
            .map(|interval| (root + interval) % 12)
            .find(|class| tones.contains(class))
        else {
            return reject(result, "unsupported-chord", Some(measure));
        };
        if pitch_for(bass_class, options, None).is_none()
            || pitch_for(fifth_class, options, None).is_none()
        {
            return reject(result, "unplayable-register", Some(measure));
        }
        changes.push(Change {
            tick: position,
            bass_class,
            fifth_class,
        });
    }
    if changes.first().is_none_or(|change| change.tick != 0) {
        return reject(result, "missing-initial-chord", Some(0));
    }
    let mut staff = Staff::new(Clef::Bass);
    let added_part = score.parts.len();
    let mut change_index = 0usize;
    let mut previous_pitch = None;
    let mut generated_events = 0usize;
    let mut attacks = vec![Vec::new(); count];
    for measure in 0..count {
        let Some(source_bar) = reference.measures.get(measure) else {
            return reject(result, "unsupported-source", Some(measure));
        };
        let mut voice = Vec::<Note>::new();
        let mut cursor = starts[measure];
        while cursor < starts[measure + 1] {
            while changes
                .get(change_index + 1)
                .is_some_and(|change| change.tick <= cursor)
            {
                change_index += 1;
            }
            let Some(current) = changes.get(change_index) else {
                return reject(result, "unsupported-chord", Some(measure));
            };
            let next = changes.get(change_index + 1);
            let region_end = next.map_or(starts[measure + 1], |change| {
                change.tick.min(starts[measure + 1])
            });
            let span = (region_end - cursor).min(32);
            if span == 0 {
                return reject(result, "chord-position", Some(measure));
            }
            let slot = (cursor - current.tick) / 32;
            let mut class =
                if options.pattern == BassLinePattern::RootPulse || slot.is_multiple_of(2) {
                    current.bass_class
                } else {
                    current.fifth_class
                };
            if options.pattern == BassLinePattern::Approach
                && let Some(next) = next
                && next.tick - current.tick >= 64
                && cursor >= next.tick.saturating_sub(32)
            {
                class = (next.bass_class + 11) % 12;
            }
            let Some(pitch) = pitch_for(class, options, previous_pitch) else {
                return reject(result, "unplayable-register", Some(measure));
            };
            if !emit_span(&[pitch], span, &mut voice) {
                return reject(result, "unsupported-rhythm", Some(measure));
            }
            if generated_events.saturating_add(voice.len()) > MAX_GENERATED_NOTES {
                return reject(result, "generated-note-limit", Some(measure));
            }
            attacks[measure].push(pitch);
            previous_pitch = Some(pitch);
            cursor += span;
        }
        for note in 0..voice.len() {
            result.generated_notes.push(NoteAddr {
                part: added_part,
                staff: 0,
                measure,
                voice: 0,
                note,
            });
        }
        generated_events += voice.len();
        let meter = reference.meter_at(measure, &score.settings.time_signature);
        let mut bar = Measure::empty(meter.numerator, meter.denominator);
        bar.number = source_bar.number;
        bar.time_sig = source_bar.time_sig.clone();
        bar.time_sig_hidden = source_bar.time_sig_hidden;
        bar.time_symbol = source_bar.time_symbol;
        bar.key_sig = source_bar.key_sig.clone();
        bar.barline_left = source_bar.barline_left.clone();
        bar.barline_right = source_bar.barline_right.clone();
        bar.volta = source_bar.volta.clone();
        let beats = f64::from(starts[measure + 1] - starts[measure]) / 32.0;
        if (beats - meter.total_beats()).abs() > 1e-9 {
            bar.actual_length = MeasureLength::from_beats(beats);
        }
        bar.voices[0] = voice;
        staff.measures.push(bar);
    }
    let mut part = Part::new(program_name(options.midi_program), "Bass");
    part.midi_channel = channel;
    part.midi_program = options.midi_program;
    part.staves.push(staff);
    let mut output = score.clone();
    output.parts.push(part);
    if !super::validate::validate(&output).is_valid() {
        return reject(result, "invalid-output", None);
    }
    result.added_part = Some(added_part);
    result.motion_report = Some(motion_report(score, &attacks));
    result.score = Some(output);
    result
}
