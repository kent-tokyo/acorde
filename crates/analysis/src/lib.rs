//! Deterministic, explainable music analysis over [`acorde_core::Score`].

use acorde_core::{ChordSymbol, NoteAddr, Score, detect_chord, roman_numeral};
use serde::{Deserialize, Serialize};

/// Version of the serialized analysis result contract.
pub const ANALYSIS_SCHEMA_VERSION: u32 = 1;

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
    AnalysisResult {
        schema_version: ANALYSIS_SCHEMA_VERSION,
        chords,
    }
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
        assert_eq!(result.chords[0].address.note, 0);
        assert_eq!(result.chords[0].evidence.len(), 3);
        assert_eq!(result.chords[0].roman_numeral.as_deref(), Some("I"));
        assert_eq!(chord_name(&result.chords[0].chord), "C");
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
}
