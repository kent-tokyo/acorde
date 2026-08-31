//! Deterministic, explainable music analysis over [`acorde_core::Score`].

use acorde_core::{ChordSymbol, KeySignature, NoteAddr, Score, detect_chord, roman_numeral};
use serde::{Deserialize, Serialize};

/// Version of the serialized analysis result contract.
pub const ANALYSIS_SCHEMA_VERSION: u32 = 3;

/// A chord label with source evidence and the rule that produced it.
#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
pub struct ChordLabel {
    pub address: NoteAddr,
    pub chord: ChordSymbol,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub roman_numeral: Option<String>,
    pub confidence: u8,
    pub rule_id: String,
    pub evidence: Vec<NoteAddr>,
}

/// Deterministic output of the chord-analysis pass.
#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
pub struct AnalysisResult {
    pub schema_version: u32,
    pub chords: Vec<ChordLabel>,
    pub intervals: Vec<IntervalObservation>,
    #[serde(default)]
    pub key_estimates: Vec<KeyEstimate>,
}

/// A deterministic key candidate ranked by diatonic pitch coverage.
#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
pub struct KeyEstimate {
    pub key: KeySignature,
    pub covered_pitches: usize,
    pub total_pitches: usize,
    pub confidence: u8,
    pub rule_id: String,
    pub evidence: Vec<NoteAddr>,
}

/// A consecutive melodic interval with addresses for both source notes.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct IntervalObservation {
    pub from: NoteAddr,
    pub to: NoteAddr,
    pub semitones: u8,
    pub diatonic_steps: i8,
    pub rule_id: String,
    pub evidence: Vec<NoteAddr>,
}

/// Analyze every voice that contains at least two pitched notes in a measure.
pub fn analyze_chords(score: &Score) -> AnalysisResult {
    let mut chords = Vec::new();
    for (part_index, part) in score.parts.iter().enumerate() {
        for (staff_index, staff) in part.staves.iter().enumerate() {
            for (measure_index, measure) in staff.measures.iter().enumerate() {
                let key = measure
                    .key_sig
                    .as_ref()
                    .unwrap_or(&score.settings.key_signature);
                for (voice_index, voice) in measure.voices.iter().enumerate() {
                    let pitched: Vec<_> = voice
                        .iter()
                        .enumerate()
                        .filter(|(_, note)| !note.is_rest && !note.pitches.is_empty())
                        .collect();
                    let pitches: Vec<_> = pitched
                        .iter()
                        .flat_map(|(_, note)| note.pitches.iter().cloned())
                        .collect();
                    let Some(chord) = detect_chord(&pitches) else {
                        continue;
                    };
                    let evidence = pitched
                        .iter()
                        .map(|(note_index, _)| NoteAddr {
                            part: part_index,
                            staff: staff_index,
                            measure: measure_index,
                            voice: voice_index,
                            note: *note_index,
                        })
                        .collect();
                    chords.push(ChordLabel {
                        address: NoteAddr {
                            part: part_index,
                            staff: staff_index,
                            measure: measure_index,
                            voice: voice_index,
                            note: pitched[0].0,
                        },
                        roman_numeral: roman_numeral(&chord, key),
                        chord,
                        confidence: 100,
                        rule_id: "pitch-class-template".to_string(),
                        evidence,
                    });
                }
            }
        }
    }
    let intervals = analyze_intervals(score);
    let key_estimates = estimate_keys(score);
    AnalysisResult {
        schema_version: ANALYSIS_SCHEMA_VERSION,
        chords,
        intervals,
        key_estimates,
    }
}

/// Estimate major/minor keys from pitch coverage, preserving tied candidates.
pub fn estimate_keys(score: &Score) -> Vec<KeyEstimate> {
    let mut pitches = Vec::new();
    let mut evidence = Vec::new();
    for (part_index, part) in score.parts.iter().enumerate() {
        for (staff_index, staff) in part.staves.iter().enumerate() {
            for (measure_index, measure) in staff.measures.iter().enumerate() {
                for (voice_index, voice) in measure.voices.iter().enumerate() {
                    for (note_index, note) in voice.iter().enumerate() {
                        if note.is_rest {
                            continue;
                        }
                        pitches.extend(note.pitches.iter());
                        if !note.pitches.is_empty() {
                            evidence.push(NoteAddr {
                                part: part_index,
                                staff: staff_index,
                                measure: measure_index,
                                voice: voice_index,
                                note: note_index,
                            });
                        }
                    }
                }
            }
        }
    }
    if pitches.is_empty() {
        return Vec::new();
    }
    let total_pitches = pitches.len();
    let mut candidates = Vec::with_capacity(30);
    for fifths in -7..=7 {
        for mode in ["major", "minor"] {
            let key = KeySignature {
                fifths,
                mode: mode.to_string(),
            };
            let covered = pitches
                .iter()
                .filter(|pitch| key.contains_pitch(pitch))
                .count();
            candidates.push((key, covered));
        }
    }
    candidates.sort_by(|(left_key, left_score), (right_key, right_score)| {
        right_score
            .cmp(left_score)
            .then_with(|| left_key.fifths.abs().cmp(&right_key.fifths.abs()))
            .then_with(|| left_key.fifths.cmp(&right_key.fifths))
            .then_with(|| left_key.mode.cmp(&right_key.mode))
    });
    let best = candidates[0].1;
    candidates
        .into_iter()
        .take_while(|(_, covered)| *covered == best)
        .map(|(key, covered_pitches)| KeyEstimate {
            key,
            covered_pitches,
            total_pitches,
            confidence: ((covered_pitches * 100) / total_pitches) as u8,
            rule_id: "diatonic-pitch-coverage".to_string(),
            evidence: evidence.clone(),
        })
        .collect()
}

/// Analyze adjacent pitched notes in every voice without inferring missing events.
pub fn analyze_intervals(score: &Score) -> Vec<IntervalObservation> {
    let mut observations = Vec::new();
    for (part_index, part) in score.parts.iter().enumerate() {
        for (staff_index, staff) in part.staves.iter().enumerate() {
            for (measure_index, measure) in staff.measures.iter().enumerate() {
                for (voice_index, voice) in measure.voices.iter().enumerate() {
                    let notes: Vec<_> = voice
                        .iter()
                        .enumerate()
                        .filter_map(|(note_index, note)| {
                            if note.is_rest {
                                None
                            } else {
                                note.pitches.first().map(|pitch| (note_index, pitch))
                            }
                        })
                        .collect();
                    for pair in notes.windows(2) {
                        let (from_index, from) = pair[0];
                        let (to_index, to) = pair[1];
                        let from_addr = NoteAddr {
                            part: part_index,
                            staff: staff_index,
                            measure: measure_index,
                            voice: voice_index,
                            note: from_index,
                        };
                        let to_addr = NoteAddr {
                            part: part_index,
                            staff: staff_index,
                            measure: measure_index,
                            voice: voice_index,
                            note: to_index,
                        };
                        observations.push(IntervalObservation {
                            from: from_addr.clone(),
                            to: to_addr.clone(),
                            semitones: (to.to_midi() - from.to_midi()).unsigned_abs() as u8,
                            diatonic_steps: diatonic_distance(from, to),
                            rule_id: "adjacent-melodic-interval".to_string(),
                            evidence: vec![from_addr, to_addr],
                        });
                    }
                }
            }
        }
    }
    observations
}

fn diatonic_distance(from: &acorde_core::Pitch, to: &acorde_core::Pitch) -> i8 {
    let step_index = |step: &acorde_core::Step| match step {
        acorde_core::Step::C => 0i16,
        acorde_core::Step::D => 1,
        acorde_core::Step::E => 2,
        acorde_core::Step::F => 3,
        acorde_core::Step::G => 4,
        acorde_core::Step::A => 5,
        acorde_core::Step::B => 6,
    };
    (i16::from(to.octave) * 7 + step_index(&to.step)
        - (i16::from(from.octave) * 7 + step_index(&from.step))) as i8
}

/// Return the stable chord spelling as a compact human-readable label.
pub fn chord_name(chord: &ChordSymbol) -> String {
    let suffix = match chord.kind.as_str() {
        "major" => "",
        "minor" => "m",
        "dominant" => "7",
        "major-seventh" => "maj7",
        "minor-seventh" => "m7",
        "diminished" => "dim",
        "diminished-seventh" => "dim7",
        "half-diminished" => "ø7",
        "augmented" => "+",
        _ => chord.kind.as_str(),
    };
    let bass = chord
        .bass
        .as_deref()
        .map_or(String::new(), |bass| format!("/{bass}"));
    format!("{}{suffix}{bass}", chord.root)
}

#[cfg(test)]
mod tests {
    use super::*;
    use acorde_core::{Duration, Note, Pitch, Score, Step};

    #[test]
    fn labels_chord_with_note_addresses_and_roman_numeral() {
        let mut score = Score::default();
        let voice = &mut score.parts[0].staves[0].measures[0].voices[0];
        voice.clear();
        for step in [Step::C, Step::E, Step::G] {
            voice.push(Note::new(Pitch::new(step, 4), Duration::Quarter));
        }
        let result = analyze_chords(&score);
        assert_eq!(result.schema_version, ANALYSIS_SCHEMA_VERSION);
        assert_eq!(result.chords.len(), 1);
        assert_eq!(result.intervals.len(), 2);
        assert!(!result.key_estimates.is_empty());
        assert_eq!(result.chords[0].address.note, 0);
        assert_eq!(result.chords[0].evidence.len(), 3);
        assert_eq!(result.chords[0].roman_numeral.as_deref(), Some("I"));
        assert_eq!(chord_name(&result.chords[0].chord), "C");
    }

    #[test]
    fn interval_observation_preserves_direction_and_evidence() {
        let mut score = Score::default();
        let voice = &mut score.parts[0].staves[0].measures[0].voices[0];
        voice.clear();
        voice.push(Note::new(Pitch::new(Step::C, 4), Duration::Quarter));
        voice.push(Note::new(Pitch::new(Step::G, 4), Duration::Quarter));
        let intervals = analyze_intervals(&score);
        assert_eq!(intervals.len(), 1);
        assert_eq!(intervals[0].semitones, 7);
        assert_eq!(intervals[0].diatonic_steps, 4);
        assert_eq!(intervals[0].evidence.len(), 2);
    }

    #[test]
    fn does_not_invent_label_for_unknown_pitch_set() {
        let mut score = Score::default();
        let voice = &mut score.parts[0].staves[0].measures[0].voices[0];
        voice.clear();
        for step in [Step::C, Step::C] {
            voice.push(Note::new(Pitch::new(step, 4), Duration::Quarter));
        }
        assert!(analyze_chords(&score).chords.is_empty());
    }

    #[test]
    fn preserves_relative_major_minor_key_ambiguity() {
        let mut score = Score::default();
        let voice = &mut score.parts[0].staves[0].measures[0].voices[0];
        voice.clear();
        for step in [
            Step::C,
            Step::D,
            Step::E,
            Step::F,
            Step::G,
            Step::A,
            Step::B,
        ] {
            voice.push(Note::new(Pitch::new(step, 4), Duration::Quarter));
        }
        let estimates = estimate_keys(&score);
        assert!(
            estimates
                .iter()
                .any(|estimate| estimate.key.display_name() == "C major")
        );
        assert!(
            estimates
                .iter()
                .any(|estimate| estimate.key.display_name() == "A minor")
        );
        assert!(estimates.iter().all(|estimate| estimate.confidence == 100));
    }

    #[test]
    fn returns_no_key_for_empty_score() {
        assert!(estimate_keys(&Score::default()).is_empty());
    }
}
