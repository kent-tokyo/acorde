//! Bounded, motif-derived countermelody in a separate GM part.

use serde::{Deserialize, Serialize};

use super::arrange::{PositionedAccordionChord, chord_at_note, preset_chord_pitch_classes};
use super::chordal_pad::{
    MAX_CHANGES, MAX_GENERATED_NOTES, MAX_MEASURES, MAX_SOURCE_NOTES, default_meter_groups,
    emit_rest, emit_span, free_channel, melody_spans, tick,
};
use super::gm::{instrument_range, program_name};
use super::notation::Clef;
use super::score::{Measure, MeasureLength, NoteAddr, Part, Score, Staff};

#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
pub struct CountermelodyOptions {
    pub chords: Vec<PositionedAccordionChord>,
    /// Source part whose first staff and first voice supply the motif.
    pub melody_part_index: usize,
    /// Half-open physical-measure range; the default end is two measures later.
    #[serde(default)]
    pub motif_start_measure: usize,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub motif_end_measure_exclusive: Option<usize>,
    pub midi_program: u8,
    pub low_midi: u8,
    pub center_midi: u8,
    pub high_midi: u8,
    /// Clearance beneath every overlapping source-melody note.
    pub min_melody_gap_semitones: u8,
    /// Authored physical measures that close a melodic phrase. The final
    /// measure sounds only its first metrical group; the next phrase starts
    /// a fresh, inverted traversal of the source contour.
    #[serde(default, skip_serializing_if = "Vec::is_empty")]
    pub phrase_end_measures: Vec<usize>,
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct CountermelodyDiagnostic {
    pub code: String,
    pub measure_index: Option<usize>,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct CountermelodyResult {
    pub score: Option<Score>,
    pub resolved_options: CountermelodyOptions,
    pub added_part: Option<usize>,
    pub generated_notes: Vec<NoteAddr>,
    /// Bounded interval contour derived from the source motif. Every second
    /// traversal inverts its direction while chord tones ground the pitches.
    pub motif_intervals: Vec<i8>,
    pub attack_count: usize,
    pub diagnostics: Vec<CountermelodyDiagnostic>,
    pub contract_version: u16,
}

fn fail(
    mut result: CountermelodyResult,
    code: &str,
    measure: Option<usize>,
) -> CountermelodyResult {
    result.score = None;
    result.added_part = None;
    result.generated_notes.clear();
    result.motif_intervals.clear();
    result.attack_count = 0;
    result.diagnostics.push(CountermelodyDiagnostic {
        code: code.into(),
        measure_index: measure,
    });
    result
}

fn motif(score: &Score, options: &CountermelodyOptions, end: usize) -> Option<Vec<i8>> {
    let staff = score.parts.get(options.melody_part_index)?.staves.first()?;
    let mut pitches = Vec::new();
    for measure in &staff.measures[options.motif_start_measure..end] {
        for note in measure.voices.first()? {
            if note.is_rest || note.is_cue || note.is_grace {
                continue;
            }
            if note.is_unpitched || note.pitches.len() != 1 || note.pitches[0].microtone_cents != 0
            {
                return None;
            }
            pitches.push(note.pitches[0].to_midi());
            if pitches.len() == 8 {
                break;
            }
        }
        if pitches.len() == 8 {
            break;
        }
    }
    let intervals = pitches
        .windows(2)
        .map(|pair| (pair[1] - pair[0]).clamp(-7, 7) as i8)
        .collect::<Vec<_>>();
    (intervals.len() >= 2 && intervals.iter().any(|interval| *interval != 0)).then_some(intervals)
}

#[derive(Clone, Copy)]
struct ContourPosition {
    attack: usize,
    phrase: usize,
    authored_phrases: bool,
}

fn choose_pitch(
    tones: &[u8],
    options: &CountermelodyOptions,
    previous: Option<u8>,
    position: ContourPosition,
    intervals: &[i8],
    ceiling: Option<i16>,
) -> Option<u8> {
    let target = if let Some(previous) = previous.filter(|_| position.attack > 0) {
        let slot = (position.attack - 1) % intervals.len();
        let traversal = (position.attack - 1) / intervals.len();
        let inverted = if position.authored_phrases {
            !position.phrase.is_multiple_of(2)
        } else {
            !traversal.is_multiple_of(2)
        };
        let interval = i16::from(intervals[slot]) * if inverted { -1 } else { 1 };
        i16::from(previous) + interval
    } else {
        i16::from(options.center_midi)
    };
    (options.low_midi..=options.high_midi)
        .filter(|pitch| tones.contains(&(pitch % 12)))
        .filter(|pitch| ceiling.is_none_or(|limit| i16::from(*pitch) <= limit))
        .min_by_key(|pitch| {
            (
                (i16::from(*pitch) - target).abs(),
                pitch.abs_diff(options.center_midi),
                *pitch,
            )
        })
}

/// Build a sparse two-bar-scale answer from a source motif. Even metrical
/// groups sound; odd groups rest. Chord changes split a sounding group and
/// choose a fresh chord tone. The source score is never modified.
pub fn arrange_countermelody(score: &Score, options: CountermelodyOptions) -> CountermelodyResult {
    arrange_countermelody_with_activity(score, options, None)
}

pub(crate) fn arrange_countermelody_with_activity(
    score: &Score,
    mut options: CountermelodyOptions,
    activity: Option<&[bool]>,
) -> CountermelodyResult {
    options.chords.sort_by(|left, right| {
        left.measure_index
            .cmp(&right.measure_index)
            .then_with(|| left.offset_beats.total_cmp(&right.offset_beats))
    });
    options.phrase_end_measures.sort_unstable();
    let mut result = CountermelodyResult {
        score: None,
        resolved_options: options,
        added_part: None,
        generated_notes: Vec::new(),
        motif_intervals: Vec::new(),
        attack_count: 0,
        diagnostics: Vec::new(),
        contract_version: 1,
    };
    let options = result.resolved_options.clone();
    let Some(reference) = score.parts.first().and_then(|part| part.staves.first()) else {
        return fail(result, "unsupported-source", None);
    };
    let count = reference.measures.len();
    let source_notes = score
        .parts
        .iter()
        .flat_map(|part| &part.staves)
        .flat_map(|staff| &staff.measures)
        .flat_map(|measure| &measure.voices)
        .fold(0usize, |sum, voice| sum.saturating_add(voice.len()));
    if count == 0
        || count > MAX_MEASURES
        || score.parts.len() > 32
        || source_notes > MAX_SOURCE_NOTES
        || options.chords.is_empty()
        || options.chords.len() > MAX_CHANGES
        || options.phrase_end_measures.len() > MAX_CHANGES
        || activity.is_some_and(|measures| measures.len() != count)
        || options.melody_part_index >= score.parts.len()
        || score
            .parts
            .iter()
            .flat_map(|part| &part.staves)
            .any(|staff| staff.measures.len() != count)
    {
        return fail(result, "input-limit", None);
    }
    if !super::validate::validate(score).is_valid() {
        return fail(result, "invalid-source", None);
    }
    if options
        .phrase_end_measures
        .iter()
        .any(|index| *index >= count)
        || options
            .phrase_end_measures
            .windows(2)
            .any(|pair| pair[0] == pair[1])
    {
        return fail(result, "invalid-phrase-end", None);
    }
    let end = options
        .motif_end_measure_exclusive
        .unwrap_or(options.motif_start_measure.saturating_add(2).min(count));
    if options.motif_start_measure >= end || end > count {
        return fail(result, "motif-range", Some(options.motif_start_measure));
    }
    let Some(intervals) = motif(score, &options, end) else {
        return fail(
            result,
            "insufficient-motif",
            Some(options.motif_start_measure),
        );
    };
    if options.midi_program > 127
        || options.low_midi > options.center_midi
        || options.center_midi > options.high_midi
        || options.high_midi > 127
        || options.min_melody_gap_semitones > 24
    {
        return fail(result, "invalid-register", None);
    }
    let (instrument_low, instrument_high) = instrument_range(options.midi_program);
    if options.low_midi < instrument_low || options.high_midi > instrument_high {
        return fail(result, "instrument-range", None);
    }
    let Some(channel) = free_channel(score) else {
        return fail(result, "no-midi-channel", None);
    };
    let mut bar_ticks = Vec::with_capacity(count);
    let mut bar_starts = Vec::with_capacity(count + 1);
    bar_starts.push(0u32);
    let mut total_beats = 0.0;
    for measure in 0..count {
        let beats = score.playback_measure_beats(measure);
        total_beats += beats;
        let Some(ticks) = tick(beats) else {
            return fail(result, "unsupported-rhythm", Some(measure));
        };
        if ticks == 0 || total_beats > 8192.0 {
            return fail(result, "input-limit", Some(measure));
        }
        let Some(next) = bar_starts.last().and_then(|start| start.checked_add(ticks)) else {
            return fail(result, "input-limit", Some(measure));
        };
        bar_starts.push(next);
        bar_ticks.push(ticks);
    }
    let spans = match melody_spans(score, options.melody_part_index, &bar_starts) {
        Ok(spans) => spans,
        Err(measure) => return fail(result, "unsupported-melody-clearance", Some(measure)),
    };
    let mut previous_position = None;
    for placed in &options.chords {
        let measure = placed.measure_index;
        let Some(&bar_length) = bar_ticks.get(measure) else {
            return fail(result, "chord-position", Some(measure));
        };
        let Some(offset) = tick(placed.offset_beats) else {
            return fail(result, "chord-position", Some(measure));
        };
        let Some(position) = bar_starts[measure].checked_add(offset) else {
            return fail(result, "chord-position", Some(measure));
        };
        if offset >= bar_length
            || previous_position.is_some_and(|previous| position <= previous)
            || preset_chord_pitch_classes(&placed.chord).is_none()
        {
            return fail(result, "unsupported-chord-or-position", Some(measure));
        }
        previous_position = Some(position);
    }
    if previous_position.is_none()
        || options.chords[0].measure_index != 0
        || options.chords[0].offset_beats != 0.0
    {
        return fail(result, "missing-initial-chord", Some(0));
    }
    let mut staff = Staff::new(if options.center_midi < 60 {
        Clef::Bass
    } else {
        Clef::Treble
    });
    let added_part = score.parts.len();
    let mut previous_pitch = None;
    let mut attack_count = 0usize;
    let mut phrase_attack_count = 0usize;
    let mut phrase_index = 0usize;
    let mut generated_events = 0usize;
    for measure in 0..count {
        let active = activity.is_none_or(|measures| measures[measure]);
        if measure > 0 && active && activity.is_some_and(|measures| !measures[measure - 1]) {
            // A silent role section pauses the motif; an entrance starts at its center.
            previous_pitch = None;
            phrase_attack_count = 0;
        }
        let meter = reference.meter_at(measure, &score.settings.time_signature);
        let mut voice = Vec::new();
        if active {
            let Some(unit) = tick(4.0 / f64::from(meter.denominator)) else {
                return fail(result, "unsupported-meter", Some(measure));
            };
            let mut group_starts = vec![0u32];
            for group in default_meter_groups(meter.numerator, meter.denominator) {
                let Some(next) = group_starts
                    .last()
                    .and_then(|start| start.checked_add(u32::from(group) * unit))
                else {
                    return fail(result, "unsupported-meter", Some(measure));
                };
                if next < bar_ticks[measure] {
                    group_starts.push(next);
                } else {
                    break;
                }
            }
            if group_starts.last().copied() != Some(bar_ticks[measure]) {
                group_starts.push(bar_ticks[measure]);
            }
            for (group_index, pair) in group_starts.windows(2).enumerate() {
                let (start, stop) = (pair[0], pair[1]);
                let mut cuts = vec![start];
                cuts.extend(
                    options
                        .chords
                        .iter()
                        .filter(|placed| placed.measure_index == measure)
                        .filter_map(|placed| tick(placed.offset_beats))
                        .filter(|position| *position > start && *position < stop),
                );
                cuts.push(stop);
                for segment in cuts.windows(2) {
                    let (from, to) = (segment[0], segment[1]);
                    let ending = options.phrase_end_measures.binary_search(&measure).is_ok();
                    let emitted = if group_index.is_multiple_of(2) && (!ending || group_index == 0)
                    {
                        let onset = f64::from(from) / 32.0;
                        let Some(chord) = chord_at_note(&options.chords, measure, onset) else {
                            return fail(result, "missing-initial-chord", Some(measure));
                        };
                        let Some(tones) = preset_chord_pitch_classes(chord) else {
                            return fail(result, "unsupported-chord", Some(measure));
                        };
                        let absolute_from = f64::from(bar_starts[measure] + from) / 32.0;
                        let absolute_to = f64::from(bar_starts[measure] + to) / 32.0;
                        let ceiling = spans
                            .iter()
                            .filter(|(start, end, _)| {
                                *start < absolute_to - 1e-9 && *end > absolute_from + 1e-9
                            })
                            .map(|(_, _, midi)| *midi - i16::from(options.min_melody_gap_semitones))
                            .min();
                        let Some(pitch) = choose_pitch(
                            &tones,
                            &options,
                            previous_pitch,
                            ContourPosition {
                                attack: phrase_attack_count,
                                phrase: phrase_index,
                                authored_phrases: !options.phrase_end_measures.is_empty(),
                            },
                            &intervals,
                            ceiling,
                        ) else {
                            return fail(result, "unplayable-counter-register", Some(measure));
                        };
                        previous_pitch = Some(pitch);
                        attack_count += 1;
                        phrase_attack_count += 1;
                        emit_span(&[pitch], to - from, &mut voice)
                    } else {
                        emit_rest(to - from, &mut voice)
                    };
                    if !emitted {
                        return fail(result, "unsupported-rhythm", Some(measure));
                    }
                    if generated_events.saturating_add(voice.len()) > MAX_GENERATED_NOTES {
                        return fail(result, "generated-note-limit", Some(measure));
                    }
                }
            }
        } else if !emit_rest(bar_ticks[measure], &mut voice) {
            return fail(result, "unsupported-rhythm", Some(measure));
        }
        for (note, event) in voice.iter().enumerate() {
            if !event.is_rest {
                result.generated_notes.push(NoteAddr {
                    part: added_part,
                    staff: 0,
                    measure,
                    voice: 0,
                    note,
                });
            }
        }
        generated_events += voice.len();
        let source_bar = &reference.measures[measure];
        let mut bar = Measure::empty(meter.numerator, meter.denominator);
        bar.number = source_bar.number;
        bar.time_sig = source_bar.time_sig.clone();
        bar.time_sig_hidden = source_bar.time_sig_hidden;
        bar.time_symbol = source_bar.time_symbol;
        bar.key_sig = source_bar.key_sig.clone();
        bar.barline_left = source_bar.barline_left.clone();
        bar.barline_right = source_bar.barline_right.clone();
        bar.volta = source_bar.volta.clone();
        let beats = f64::from(bar_ticks[measure]) / 32.0;
        if (beats - meter.total_beats()).abs() > 1e-9 {
            bar.actual_length = MeasureLength::from_beats(beats);
        }
        bar.voices[0] = voice;
        staff.measures.push(bar);
        if options.phrase_end_measures.binary_search(&measure).is_ok() {
            phrase_index += 1;
            phrase_attack_count = 0;
            previous_pitch = None;
        }
    }
    let mut part = Part::new(program_name(options.midi_program), "Counter");
    part.midi_program = options.midi_program;
    part.midi_channel = channel;
    part.staves.push(staff);
    let mut output = score.clone();
    output.parts.push(part);
    if !super::validate::validate(&output).is_valid() {
        return fail(result, "invalid-output", None);
    }
    result.resolved_options.motif_end_measure_exclusive = Some(end);
    result.motif_intervals = intervals;
    result.attack_count = attack_count;
    result.added_part = Some(added_part);
    result.score = Some(output);
    result
}
