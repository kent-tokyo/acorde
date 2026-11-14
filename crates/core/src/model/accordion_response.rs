//! Opt-in, source-addressed responses in written melody rests.

use serde::{Deserialize, Serialize};
use std::cmp::Reverse;
use std::collections::BTreeSet;

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
        let (Some(rest), Some(previous), Some(next)) = (
            melody.get(address.note),
            address
                .note
                .checked_sub(1)
                .and_then(|index| melody.get(index)),
            address
                .note
                .checked_add(1)
                .and_then(|index| melody.get(index)),
        ) else {
            return fail(
                result,
                "response-rest",
                "select a rest between two notes in one measure",
                Some(address.measure),
            );
        };
        if !rest.is_rest
            || rest.is_grace
            || rest.is_cue
            || rest.duration != Duration::Half
            || rest.dot_count != 0
            || rest.tuplet.is_some()
            || ordinary_melody_pitch(previous).is_none()
            || ordinary_melody_pitch(next).is_none()
        {
            return fail(
                result,
                "response-rest",
                "select an ordinary two-beat rest between pitched melody notes",
                Some(address.measure),
            );
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
                let Some(previous) = note_index
                    .checked_sub(1)
                    .and_then(|index| melody.get(index))
                else {
                    return fail(
                        result,
                        "response-rest",
                        "missing preceding melody note",
                        Some(measure_index),
                    );
                };
                let Some(next) = melody.get(note_index + 1) else {
                    return fail(
                        result,
                        "response-rest",
                        "missing following melody note",
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
                let (Some(previous_midi), Some(next_midi)) =
                    (ordinary_melody_pitch(previous), ordinary_melody_pitch(next))
                else {
                    return fail(
                        result,
                        "response-rest",
                        "neighboring melody pitches are unsupported",
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
