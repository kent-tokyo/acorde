//! Opt-in, source-addressed responses in written melody rests.

use serde::{Deserialize, Serialize};
use std::cmp::Reverse;
use std::collections::{BTreeMap, BTreeSet};

use super::accordion_sections::{
    SectionedAccordionOptions, SectionedAccordionResult, arrange_with_sections,
};
use super::arrange::{GuidedArrangeDiagnostic, chord_at_note, preset_chord_pitch_classes};
use super::duration::Duration;
use super::pitch::Pitch;
use super::score::{Note, NoteAddr, Score};

const MAX_FILLS: usize = 512;

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct AccordionResponseOptions {
    /// Addresses of authored half rests in the source melody, not generated score addresses.
    pub rest_notes: Vec<NoteAddr>,
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct AccordionResponseFill {
    pub source_rest: NoteAddr,
    pub generated_note: NoteAddr,
    pub pitch_midi: u8,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct ResponseFilledAccordionResult {
    pub arrangement: SectionedAccordionResult,
    pub resolved_options: AccordionResponseOptions,
    pub fills: Vec<AccordionResponseFill>,
    pub contract_version: u16,
}

fn fail(
    mut result: ResponseFilledAccordionResult,
    code: &str,
    message: &str,
    measure: Option<usize>,
) -> ResponseFilledAccordionResult {
    result.arrangement.score = None;
    result.arrangement.generated_notes.clear();
    result.arrangement.source_notes.clear();
    result.arrangement.resolved_patterns.clear();
    result.arrangement.notated_swing = false;
    result.arrangement.notated_swing_measures.clear();
    result.fills.clear();
    result
        .arrangement
        .diagnostics
        .push(GuidedArrangeDiagnostic {
            code: code.into(),
            message: message.into(),
            measure_index: measure,
        });
    result
}

fn ordinary_melody_pitch(note: &Note) -> Option<i16> {
    if note.is_rest || note.is_unpitched || note.is_grace || note.is_cue || note.pitches.len() != 1
    {
        return None;
    }
    let pitch = &note.pitches[0];
    (pitch.microtone_cents == 0).then(|| pitch.to_midi())
}

fn response_pitch(previous: i16, next: i16, chord_tones: &[u8]) -> Option<u8> {
    (60u8..=84)
        .filter(|midi| chord_tones.contains(&(midi % 12)))
        .filter(|midi| {
            let midi = i16::from(*midi);
            (1..=12).contains(&(next - midi)) && (previous - midi).abs() <= 12
        })
        .min_by_key(|midi| {
            let midi_i16 = i16::from(*midi);
            (
                (previous - midi_i16).abs() + (next - midi_i16).abs(),
                Reverse(*midi),
            )
        })
}

/// Add a one-quarter-note chord-tone answer late in each selected two-beat rest.
/// The melody, chord plan and accompaniment are preserved. All requested rests
/// must be supported; a failure returns no partial score or note mappings.
/// A bar-ending rest may target the first note of the next written measure
/// only when every playback visit continues into that measure. Repeat, volta,
/// navigation, and tied-note ambiguity are rejected with a diagnostic.
pub fn arrange_with_response_fills(
    score: &Score,
    accompaniment: SectionedAccordionOptions,
    options: AccordionResponseOptions,
) -> ResponseFilledAccordionResult {
    let arrangement = arrange_with_sections(score, accompaniment);
    let mut result = ResponseFilledAccordionResult {
        arrangement,
        resolved_options: options,
        fills: Vec::new(),
        contract_version: 1,
    };
    if result.arrangement.score.is_none() || result.resolved_options.rest_notes.is_empty() {
        return result;
    }
    if result.resolved_options.rest_notes.len() > MAX_FILLS {
        return fail(
            result,
            "response-input-limit",
            "at most 512 rests can be selected",
            None,
        );
    }
    let mut selected = BTreeSet::new();
    let mut neighbors = BTreeMap::new();
    let sequence = super::repeat::measure_sequence(score);
    for address in result.resolved_options.rest_notes.clone() {
        if address.part != 0
            || address.staff != 0
            || address.voice != 0
            || !selected.insert((address.measure, address.note))
        {
            return fail(
                result,
                "response-address",
                "select distinct source melody rests",
                Some(address.measure),
            );
        }
        let Some(bar) = score.parts[0].staves[0].measures.get(address.measure) else {
            return fail(
                result,
                "response-address",
                "rest address is outside the source score",
                Some(address.measure),
            );
        };
        let melody = &bar.voices[0];
        let Some(rest) = melody.get(address.note) else {
            return fail(
                result,
                "response-rest",
                "rest address is outside the source melody",
                Some(address.measure),
            );
        };
        let previous = address.note.checked_sub(1).and_then(|i| melody.get(i));
        let next = if let Some(next) = melody.get(address.note + 1) {
            Some(next)
        } else {
            // Only a full, ordinary bar-end rest and an immediately sounding
            // first note in the next written bar have an unambiguous target.
            let staff = &score.parts[0].staves[0];
            let end = melody.iter().map(Note::beats).sum::<f64>();
            let bar_end = staff.measure_beats(address.measure, &score.settings.time_signature);
            let target = address.measure + 1;
            let mut visits = sequence
                .iter()
                .enumerate()
                .filter(|(_, m)| **m == address.measure)
                .peekable();
            let safe = visits.peek().is_some()
                && sequence.len() < staff.measures.len().saturating_mul(16).max(64)
                && visits.all(|(i, _)| sequence.get(i + 1) == Some(&target));
            if (end - bar_end).abs() > 1e-9 || !safe {
                return fail(
                    result,
                    "response-next-ambiguous",
                    "following melody note is ambiguous at the bar boundary",
                    Some(address.measure),
                );
            }
            let next = staff
                .measures
                .get(target)
                .and_then(|m| m.voices.first())
                .and_then(|v| v.first());
            if next.is_some_and(|n| n.tie_end || n.pitch_tie_ends.iter().any(|t| *t)) {
                return fail(
                    result,
                    "response-next-ambiguous",
                    "following melody note is tied from an earlier event",
                    Some(address.measure),
                );
            }
            next
        };
        if !rest.is_rest
            || rest.is_grace
            || rest.is_cue
            || rest.duration != Duration::Half
            || rest.dot_count != 0
            || rest.tuplet.is_some()
            || previous.and_then(ordinary_melody_pitch).is_none()
            || next.and_then(ordinary_melody_pitch).is_none()
        {
            return fail(
                result,
                "response-rest",
                "select an ordinary two-beat rest between pitched melody notes",
                Some(address.measure),
            );
        }
        if let (Some(previous), Some(next)) = (
            previous.and_then(ordinary_melody_pitch),
            next.and_then(ordinary_melody_pitch),
        ) {
            neighbors.insert((address.measure, address.note), (previous, next));
        }
    }
    let chords = &result.arrangement.resolved_options.chords;
    let Some(output) = result.arrangement.score.as_mut() else {
        return result;
    };
    for (measure_index, bar) in output.parts[0].staves[0].measures.iter_mut().enumerate() {
        let melody = &bar.voices[0];
        let mut voice = Vec::new();
        let mut offset = 0.0;
        for (note_index, source_note) in melody.iter().enumerate() {
            if selected.contains(&(measure_index, note_index)) {
                let Some(&(previous_midi, next_midi)) = neighbors.get(&(measure_index, note_index))
                else {
                    return fail(
                        result,
                        "response-rest",
                        "neighboring melody pitches are unsupported",
                        Some(measure_index),
                    );
                };
                let Some(chord) = chord_at_note(chords, measure_index, offset + 1.0) else {
                    return fail(
                        result,
                        "response-harmony",
                        "no chord covers the response onset",
                        Some(measure_index),
                    );
                };
                let Some(tones) = preset_chord_pitch_classes(chord) else {
                    return fail(
                        result,
                        "response-harmony",
                        "response chord is unsupported",
                        Some(measure_index),
                    );
                };
                let Some(pitch_midi) = response_pitch(previous_midi, next_midi, &tones) else {
                    return fail(
                        result,
                        "response-range",
                        "no chord tone connects the neighboring melody notes in the response range",
                        Some(measure_index),
                    );
                };
                voice.push(Note::rest(Duration::Quarter));
                let generated_note = NoteAddr {
                    part: 0,
                    staff: 0,
                    measure: measure_index,
                    voice: 1,
                    note: voice.len(),
                };
                voice.push(Note::new(
                    Pitch::from_midi(pitch_midi, false),
                    Duration::Quarter,
                ));
                result
                    .arrangement
                    .generated_notes
                    .push(generated_note.clone());
                result.fills.push(AccordionResponseFill {
                    source_rest: NoteAddr {
                        part: 0,
                        staff: 0,
                        measure: measure_index,
                        voice: 0,
                        note: note_index,
                    },
                    generated_note,
                    pitch_midi,
                });
            } else {
                let mut rest = Note::rest(source_note.duration.clone());
                rest.dot_count = source_note.dot_count;
                rest.tuplet = source_note.tuplet.clone();
                rest.tuplet_boundary = source_note.tuplet_boundary;
                rest.is_grace = source_note.is_grace;
                rest.is_cue = source_note.is_cue;
                voice.push(rest);
            }
            offset += source_note.beats();
        }
        bar.voices[1] = voice;
    }
    let Some(output) = result.arrangement.score.as_ref() else {
        return result;
    };
    if !super::validate::validate(output).is_valid() {
        return fail(
            result,
            "invalid-output",
            "response-filled score failed structural validation",
            None,
        );
    }
    result
}
