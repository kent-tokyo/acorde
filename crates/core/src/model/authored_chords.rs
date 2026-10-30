//! Display chord symbols backed by written chords in a companion score.

use super::score::{NoteAddr, Score};
use crate::detect_chord;
use serde::{Deserialize, Serialize};

/// Physical source and target voices. Defaults to lower part 2, staff 1, voice 1
/// and upper part 1, staff 1, voice 1 (all indices are zero-based).
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
#[serde(default)]
pub struct AuthoredChordOptions {
    pub source_part: usize,
    pub source_staff: usize,
    pub source_voice: usize,
    pub target_part: usize,
    pub target_staff: usize,
    pub target_voice: usize,
}

impl Default for AuthoredChordOptions {
    fn default() -> Self {
        Self {
            source_part: 1,
            source_staff: 0,
            source_voice: 0,
            target_part: 0,
            target_staff: 0,
            target_voice: 0,
        }
    }
}

/// A symbol added to a pre-existing target melody note.
#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
pub struct AuthoredChordAnnotation {
    pub source: NoteAddr,
    pub target: NoteAddr,
    pub chord: super::notation::ChordSymbol,
}

/// A source or placement that could not be annotated without guessing.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct AuthoredChordDiagnostic {
    pub code: String,
    pub measure: Option<usize>,
    pub source: Option<NoteAddr>,
}

/// A new score and the provenance of every added symbol. Source and target inputs are unchanged.
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct AuthoredChordResult {
    pub score: Score,
    pub count: usize,
    pub annotations: Vec<AuthoredChordAnnotation>,
    pub diagnostics: Vec<AuthoredChordDiagnostic>,
}

fn voice_at(
    score: &Score,
    part: usize,
    staff: usize,
    measure: usize,
    voice: usize,
) -> Option<&[super::score::Note]> {
    score
        .parts
        .get(part)?
        .staves
        .get(staff)?
        .measures
        .get(measure)?
        .voices
        .get(voice)
        .map(Vec::as_slice)
}

/// Add display symbols only from simultaneous, written lower-hand chords.
///
/// Source and target must have corresponding measure counts. In each measure the first
/// recognized chord labels the first eligible melody note, including a pickup whose bass
/// sounds before the block chord. Later changes go to the first eligible note at or after
/// the source onset. Rests and grace notes are ineligible targets. Identical consecutive
/// harmonies within a measure are suppressed. An inversion alone does not prove a slash
/// bass, so inferred symbols omit `bass`. Existing target symbols always win.
pub fn annotate_from_authored_chords(
    target: &Score,
    source: &Score,
    options: &AuthoredChordOptions,
) -> AuthoredChordResult {
    let mut result = AuthoredChordResult {
        score: target.clone(),
        count: 0,
        annotations: Vec::new(),
        diagnostics: Vec::new(),
    };
    let source_staff = source
        .parts
        .get(options.source_part)
        .and_then(|p| p.staves.get(options.source_staff));
    let target_staff = target
        .parts
        .get(options.target_part)
        .and_then(|p| p.staves.get(options.target_staff));
    let (Some(source_staff), Some(target_staff)) = (source_staff, target_staff) else {
        result.diagnostics.push(AuthoredChordDiagnostic {
            code: "staff-unavailable".into(),
            measure: None,
            source: None,
        });
        return result;
    };
    if options.source_voice >= 4
        || options.target_voice >= 4
        || source_staff.measures.len() != target_staff.measures.len()
    {
        result.diagnostics.push(AuthoredChordDiagnostic {
            code: "voice-or-measure-mismatch".into(),
            measure: None,
            source: None,
        });
        return result;
    }
    for measure_index in 0..target_staff.measures.len() {
        let source_measure = &source_staff.measures[measure_index];
        let target_measure = &target_staff.measures[measure_index];
        if source_measure.number != target_measure.number {
            result.diagnostics.push(AuthoredChordDiagnostic {
                code: "measure-number-mismatch".into(),
                measure: Some(measure_index),
                source: None,
            });
            continue;
        }
        let source_voice = voice_at(
            source,
            options.source_part,
            options.source_staff,
            measure_index,
            options.source_voice,
        )
        .unwrap();
        let target_voice = voice_at(
            target,
            options.target_part,
            options.target_staff,
            measure_index,
            options.target_voice,
        )
        .unwrap();
        let mut melody_time = 0.0;
        let melody: Vec<_> = target_voice
            .iter()
            .enumerate()
            .filter_map(|(index, note)| {
                let onset = melody_time;
                melody_time += note.beats();
                (!note.is_rest && !note.is_grace && !note.is_cue && !note.hidden)
                    .then_some((index, onset))
            })
            .collect();
        if melody.is_empty() {
            continue;
        }
        let mut source_time = 0.0;
        let mut previous: Option<(String, String)> = None;
        let mut first = true;
        for (source_index, note) in source_voice.iter().enumerate() {
            let onset = source_time;
            source_time += note.beats();
            if note.is_rest || note.is_grace || note.is_cue || note.hidden || note.pitches.len() < 3
            {
                continue;
            }
            let source_address = NoteAddr {
                part: options.source_part,
                staff: options.source_staff,
                measure: measure_index,
                voice: options.source_voice,
                note: source_index,
            };
            let Some(mut chord) = detect_chord(&note.pitches) else {
                result.diagnostics.push(AuthoredChordDiagnostic {
                    code: "unrecognized-chord".into(),
                    measure: Some(measure_index),
                    source: Some(source_address),
                });
                continue;
            };
            if !matches!(
                chord.kind.as_str(),
                "major" | "minor" | "dominant" | "diminished" | "augmented"
            ) {
                continue;
            }
            let identity = (chord.root.clone(), chord.kind.clone());
            if previous.as_ref() == Some(&identity) {
                continue;
            }
            let candidate = melody.iter().find(|(index, time)| {
                (first || *time >= onset - 1e-9)
                    && result.score.parts[options.target_part].staves[options.target_staff].measures
                        [measure_index]
                        .voices[options.target_voice][*index]
                        .chord_symbol
                        .is_none()
            });
            let target_index = candidate.map(|(index, _)| *index);
            first = false;
            let Some(target_index) = target_index else {
                result.diagnostics.push(AuthoredChordDiagnostic {
                    code: "no-target-at-or-after-onset".into(),
                    measure: Some(measure_index),
                    source: Some(source_address),
                });
                continue;
            };
            let destination = &mut result.score.parts[options.target_part].staves
                [options.target_staff]
                .measures[measure_index]
                .voices[options.target_voice][target_index];
            chord.bass = None;
            destination.chord_symbol = Some(chord.clone());
            previous = Some(identity);
            result.annotations.push(AuthoredChordAnnotation {
                source: source_address,
                target: NoteAddr {
                    part: options.target_part,
                    staff: options.target_staff,
                    measure: measure_index,
                    voice: options.target_voice,
                    note: target_index,
                },
                chord,
            });
        }
    }
    result.count = result.annotations.len();
    result
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::{Duration, Note, Pitch, Score, Step, to_playback_events};

    fn score_with_written_chords() -> Score {
        let mut score = Score::new("source backed", 120, 4, 4, 0, 1);
        let mut lower = score.parts[0].clone();
        lower.id = "lower".into();
        score.parts.push(lower);
        score.parts[0].staves[0].measures[0].voices[0] = (0..4)
            .map(|_| Note::new(Pitch::new(Step::C, 5), Duration::Quarter))
            .collect();
        let chord = |pitches: [u8; 3]| {
            let mut note = Note::new(Pitch::from_midi(pitches[0], false), Duration::Eighth);
            note.pitches = pitches.map(|pitch| Pitch::from_midi(pitch, false)).to_vec();
            note
        };
        score.parts[1].staves[0].measures[0].voices[0] = vec![
            Note::rest(Duration::Eighth),
            chord([62, 66, 69]),
            chord([60, 64, 67]),
            chord([60, 64, 67]),
        ];
        score
    }

    #[test]
    fn written_lower_chords_annotate_existing_melody_with_source_addresses() {
        let source = score_with_written_chords();
        let target = source.clone();
        let before = serde_json::to_value(&source).unwrap();
        let result = annotate_from_authored_chords(&target, &source, &Default::default());
        assert_eq!(result.count, 2);
        assert_eq!(result.annotations[0].source.note, 1);
        assert_eq!(result.annotations[0].target.note, 0);
        assert_eq!(result.annotations[0].chord.root, "D");
        assert_eq!(result.annotations[1].target.note, 1);
        assert_eq!(result.annotations[1].chord.root, "C");
        assert!(
            result
                .annotations
                .iter()
                .all(|item| item.chord.bass.is_none())
        );
        assert_eq!(serde_json::to_value(&source).unwrap(), before);
        let sounding = |score: &Score| {
            to_playback_events(score, &Default::default())
                .iter()
                .map(|event| {
                    (
                        event.pitch_midi_cents,
                        event.time_beats,
                        event.duration_beats,
                    )
                })
                .collect::<Vec<_>>()
        };
        assert_eq!(sounding(&result.score), sounding(&target));
    }

    #[test]
    fn no_written_lower_chords_or_mismatched_measures_do_not_guess() {
        let mut source = score_with_written_chords();
        let mut solo = source.clone();
        solo.parts.pop();
        let empty = annotate_from_authored_chords(&solo, &solo, &Default::default());
        assert_eq!(empty.count, 0);
        assert_eq!(empty.diagnostics[0].code, "staff-unavailable");
        let extra = source.parts[1].staves[0].measures[0].clone();
        source.parts[1].staves[0].measures.push(extra);
        let mismatch = annotate_from_authored_chords(&solo, &source, &Default::default());
        assert_eq!(mismatch.count, 0);
        assert_eq!(mismatch.diagnostics[0].code, "voice-or-measure-mismatch");
    }

    #[test]
    fn existing_symbol_is_preserved_and_source_rest_does_not_create_a_chord() {
        let source = score_with_written_chords();
        let mut target = source.clone();
        let authored = detect_chord(&[67, 71, 74].map(|midi| Pitch::from_midi(midi, false)))
            .expect("G major triad");
        target.parts[0].staves[0].measures[0].voices[0][0].chord_symbol = Some(authored.clone());
        let result = annotate_from_authored_chords(&target, &source, &Default::default());
        assert_eq!(
            result.score.parts[0].staves[0].measures[0].voices[0][0].chord_symbol,
            Some(authored)
        );
        assert_eq!(result.count, 2);
        assert_eq!(result.annotations[0].target.note, 1);
        assert_eq!(result.annotations[1].target.note, 2);
    }
}
