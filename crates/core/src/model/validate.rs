use std::collections::HashMap;
use serde::{Deserialize, Serialize};
use super::gm::instrument_range;
use super::score::Score;

/// A structural error found by [`validate`].
#[derive(Debug, Clone, Serialize, Deserialize)]
pub enum ValidationError {
    /// Beat-count mismatch: the notes in a voice don't fill the time signature.
    BeatCount {
        part: usize,
        staff: usize,
        measure: usize,
        voice: usize,
        expected_beats: f64,
        found_beats: f64,
    },
    /// A note pitch lies outside the practical range for the part's GM instrument.
    OutOfRange {
        part_index: usize,
        staff_index: usize,
        measure_index: usize,
        note_index: usize,
        pitch_midi: u8,
        instrument_range: (u8, u8),
    },
}

/// A non-fatal advisory warning found by [`validate`].
#[derive(Debug, Clone, Serialize, Deserialize)]
pub enum ValidationWarning {
    /// A measure's beat count is less than the time signature (incomplete bar).
    IncompleteBar {
        part: usize,
        staff: usize,
        measure: usize,
        expected_beats: f64,
        actual_beats: f64,
    },
    /// Two volta brackets in the same staff overlap or share the same number.
    OverlappingVolta { part: usize, staff: usize },
    /// A part has no notes across all measures.
    EmptyPart { part: usize },
    /// The same rehearsal mark text appears more than once.
    DuplicateRehearsalMark { mark: String },
}

/// Combined result of [`validate`]: errors that indicate broken structure, plus advisory warnings.
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct ValidationReport {
    pub errors: Vec<ValidationError>,
    pub warnings: Vec<ValidationWarning>,
}

impl ValidationReport {
    /// `true` when there are no errors (warnings may still be present).
    pub fn is_valid(&self) -> bool {
        self.errors.is_empty()
    }
}

/// Check every voice in every measure for structural correctness.
///
/// Checks performed:
/// - **Errors**: beat-count mismatch, out-of-range pitch.
/// - **Warnings**: incomplete bar (underfull voice), overlapping volta brackets,
///   empty parts, duplicate rehearsal marks.
///
/// Multi-rest placeholder measures and empty voices are skipped.
/// Percussion parts (MIDI channel 9) are exempt from pitch-range checks.
pub fn validate(score: &Score) -> ValidationReport {
    let mut errors = Vec::new();
    let mut warnings = Vec::new();

    let mut rehearsal_counts: HashMap<String, usize> = HashMap::new();

    for (pi, part) in score.parts.iter().enumerate() {
        let range = instrument_range(part.midi_program);
        let is_percussion = part.midi_channel == 9;
        let mut part_has_notes = false;

        for (si, staff) in part.staves.iter().enumerate() {
            let mut current_ts = score.settings.time_signature.clone();
            let mut volta_numbers_seen: Vec<u8> = Vec::new();

            for (mi, measure) in staff.measures.iter().enumerate() {
                if let Some(ts) = &measure.time_sig {
                    current_ts = ts.clone();
                }
                if measure.multi_rest_count.is_some() {
                    continue;
                }

                // Rehearsal mark deduplication
                if let Some(ref mark) = measure.rehearsal {
                    let entry = rehearsal_counts.entry(mark.clone()).or_insert(0);
                    *entry += 1;
                }

                // Volta overlap detection
                if let Some(ref volta) = measure.volta {
                    if volta_numbers_seen.contains(&volta.number) {
                        warnings.push(ValidationWarning::OverlappingVolta { part: pi, staff: si });
                    } else {
                        volta_numbers_seen.push(volta.number);
                    }
                }

                let expected = current_ts.total_beats();
                for (vi, voice) in measure.voices.iter().enumerate() {
                    if voice.is_empty() {
                        continue;
                    }
                    let non_rest_count: usize = voice.iter().filter(|n| !n.is_rest).count();
                    if non_rest_count > 0 {
                        part_has_notes = true;
                    }
                    let total: f64 = voice.iter().map(|n| n.beats()).sum();
                    if total > expected + 0.02 {
                        errors.push(ValidationError::BeatCount {
                            part: pi,
                            staff: si,
                            measure: mi,
                            voice: vi,
                            expected_beats: expected,
                            found_beats: total,
                        });
                    } else if total < expected - 0.02 && non_rest_count > 0 {
                        warnings.push(ValidationWarning::IncompleteBar {
                            part: pi, staff: si, measure: mi,
                            expected_beats: expected, actual_beats: total,
                        });
                    }

                    if !is_percussion {
                        let transpose = staff.transpose_semitones;
                        for (ni, note) in voice.iter().enumerate() {
                            if note.is_rest || note.is_grace {
                                continue;
                            }
                            for pitch in &note.pitches {
                                let midi = (pitch.to_midi() + transpose as i16).clamp(0, 127) as u8;
                                if midi < range.0 || midi > range.1 {
                                    errors.push(ValidationError::OutOfRange {
                                        part_index: pi,
                                        staff_index: si,
                                        measure_index: mi,
                                        note_index: ni,
                                        pitch_midi: midi,
                                        instrument_range: range,
                                    });
                                }
                            }
                        }
                    }
                }
            }
        }

        if !part_has_notes {
            warnings.push(ValidationWarning::EmptyPart { part: pi });
        }
    }

    for (mark, count) in &rehearsal_counts {
        if *count > 1 {
            warnings.push(ValidationWarning::DuplicateRehearsalMark { mark: mark.clone() });
        }
    }

    ValidationReport { errors, warnings }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::model::{
        duration::Duration,
        pitch::{Pitch, Step},
        score::{Note, Score},
    };

    #[test]
    fn validate_clean_score_returns_empty_errors() {
        let score = Score::new("T", 120, 4, 4, 0, 1);
        assert!(validate(&score).errors.is_empty());
    }

    #[test]
    fn validate_overfull_measure_returns_error() {
        let mut score = Score::new("T", 120, 4, 4, 0, 1);
        score.parts[0].staves[0].measures[0].voices[0]
            .push(Note::new(Pitch::new(Step::C, 4), Duration::Quarter));
        let report = validate(&score);
        assert!(!report.errors.is_empty());
        assert!(matches!(report.errors[0], ValidationError::BeatCount { measure: 0, voice: 0, .. }));
    }

    #[test]
    fn validate_skips_multi_rest() {
        let mut score = Score::new("T", 120, 4, 4, 0, 1);
        score.parts[0].staves[0].measures[0].multi_rest_count = Some(4);
        score.parts[0].staves[0].measures[0].voices[0].clear();
        assert!(validate(&score).errors.is_empty());
    }

    #[test]
    fn validate_out_of_range_pitch_detected() {
        // Piano (program 0): range 21–108. C9 (midi=120) is out of range.
        let mut score = Score::new("T", 120, 4, 4, 0, 1);
        score.parts[0].midi_program = 0;
        score.parts[0].staves[0].measures[0].voices[0] =
            vec![Note::new(Pitch::new(Step::C, 9), Duration::Whole)];
        let report = validate(&score);
        assert!(report.errors.iter().any(|e| matches!(e, ValidationError::OutOfRange { pitch_midi, .. } if *pitch_midi == 120)));
    }

    #[test]
    fn validate_percussion_channel_skips_range_check() {
        // Channel 9 = percussion; even extreme pitches should not trigger OutOfRange.
        let mut score = Score::new("T", 120, 4, 4, 0, 1);
        score.parts[0].midi_channel = 9;
        score.parts[0].midi_program = 0;
        score.parts[0].staves[0].measures[0].voices[0] =
            vec![Note::new(Pitch::new(Step::C, 9), Duration::Whole)];
        let report = validate(&score);
        assert!(!report.errors.iter().any(|e| matches!(e, ValidationError::OutOfRange { .. })));
    }

    #[test]
    fn validate_in_range_pitch_ok() {
        // Piano C4 (midi=60) is in range 21–108.
        let mut score = Score::new("T", 120, 4, 4, 0, 1);
        score.parts[0].midi_program = 0;
        score.parts[0].staves[0].measures[0].voices[0] =
            vec![Note::new(Pitch::new(Step::C, 4), Duration::Whole)];
        assert!(!validate(&score).errors.iter().any(|e| matches!(e, ValidationError::OutOfRange { .. })));
    }

    #[test]
    fn validate_empty_part_warning() {
        let score = Score::new("T", 120, 4, 4, 0, 1);
        let report = validate(&score);
        assert!(report.warnings.iter().any(|w| matches!(w, ValidationWarning::EmptyPart { part: 0 })));
    }

    #[test]
    fn validate_duplicate_rehearsal_mark_warning() {
        use crate::model::score::Score;
        let mut score = Score::new("T", 120, 4, 4, 0, 2);
        score.parts[0].staves[0].measures[0].rehearsal = Some("A".to_string());
        score.parts[0].staves[0].measures[1].rehearsal = Some("A".to_string());
        let report = validate(&score);
        assert!(report.warnings.iter().any(|w| matches!(w, ValidationWarning::DuplicateRehearsalMark { mark } if mark == "A")));
    }
}
