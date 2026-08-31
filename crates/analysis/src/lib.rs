//! Deterministic, explainable music analysis over [`acorde_core::Score`].

use acorde_core::{ChordSymbol, KeySignature, NoteAddr, Score, detect_chord, roman_numeral};
use serde::{Deserialize, Serialize};
use std::collections::HashMap;

/// Version of the serialized analysis result contract.
pub const ANALYSIS_SCHEMA_VERSION: u32 = 5;

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
    #[serde(default)]
    pub cadence_candidates: Vec<CadenceCandidate>,
    #[serde(default)]
    pub voice_leading: Vec<VoiceLeadingObservation>,
    #[serde(default)]
    pub satb_diagnostics: Vec<SatbDiagnostic>,
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

/// A cadence transition inferred only from adjacent, explicitly labeled chords.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct CadenceCandidate {
    pub from: NoteAddr,
    pub to: NoteAddr,
    pub kind: CadenceKind,
    pub confidence: u8,
    pub rule_id: String,
    pub evidence: Vec<NoteAddr>,
}

/// Supported cadence transition families.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub enum CadenceKind {
    Authentic,
    Plagal,
    Deceptive,
    Half,
}

/// Voice-leading observation for two adjacent voices at an aligned event.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct VoiceLeadingObservation {
    pub upper: NoteAddr,
    pub lower: NoteAddr,
    pub upper_motion: i16,
    pub lower_motion: i16,
    pub parallel_perfect: bool,
    pub confidence: u8,
    pub rule_id: String,
    pub evidence: Vec<NoteAddr>,
}

/// A typed SATB constraint finding with source addresses for UI selection.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct SatbDiagnostic {
    pub upper: NoteAddr,
    pub lower: NoteAddr,
    pub kind: SatbDiagnosticKind,
    pub severity: SatbSeverity,
    pub confidence: u8,
    pub rule_id: String,
    pub evidence: Vec<NoteAddr>,
}

/// SATB constraint families reported by the deterministic pass.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub enum SatbDiagnosticKind {
    VoiceCrossing,
    WideSpacing,
    ParallelPerfect,
}

/// User-facing seriousness of a SATB diagnostic.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub enum SatbSeverity {
    Error,
    Warning,
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
pub fn analyze_score(score: &Score) -> AnalysisResult {
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
    let cadence_candidates = analyze_cadences(&chords);
    let voice_leading = analyze_voice_leading(score);
    let satb_diagnostics = analyze_satb_with_voice_leading(score, &voice_leading);
    AnalysisResult {
        schema_version: ANALYSIS_SCHEMA_VERSION,
        chords,
        intervals,
        key_estimates,
        cadence_candidates,
        voice_leading,
        satb_diagnostics,
    }
}

/// Analyze SATB constraints using the same aligned voice events as voice-leading analysis.
pub fn analyze_satb(score: &Score) -> Vec<SatbDiagnostic> {
    let voice_leading = analyze_voice_leading(score);
    analyze_satb_with_voice_leading(score, &voice_leading)
}

fn analyze_satb_with_voice_leading(
    score: &Score,
    voice_leading: &[VoiceLeadingObservation],
) -> Vec<SatbDiagnostic> {
    let mut diagnostics = Vec::new();
    for (part_index, part) in score.parts.iter().enumerate() {
        for (staff_index, staff) in part.staves.iter().enumerate() {
            for (measure_index, measure) in staff.measures.iter().enumerate() {
                for (upper_index, upper_voice) in measure.voices.iter().enumerate() {
                    let Some(lower_voice) = measure.voices.get(upper_index + 1) else {
                        continue;
                    };
                    for note_index in 0..upper_voice.len().min(lower_voice.len()) {
                        let Some(upper) = upper_voice[note_index].pitches.first() else {
                            continue;
                        };
                        let Some(lower) = lower_voice[note_index].pitches.first() else {
                            continue;
                        };
                        let upper_addr = NoteAddr {
                            part: part_index,
                            staff: staff_index,
                            measure: measure_index,
                            voice: upper_index,
                            note: note_index,
                        };
                        let lower_addr = NoteAddr {
                            part: part_index,
                            staff: staff_index,
                            measure: measure_index,
                            voice: upper_index + 1,
                            note: note_index,
                        };
                        let distance = upper.to_midi() - lower.to_midi();
                        if distance < 0 {
                            diagnostics.push(satb_diagnostic(
                                upper_addr.clone(),
                                lower_addr.clone(),
                                SatbDiagnosticKind::VoiceCrossing,
                                SatbSeverity::Error,
                                "satb-voice-crossing",
                            ));
                        } else if distance > 24 {
                            diagnostics.push(satb_diagnostic(
                                upper_addr.clone(),
                                lower_addr.clone(),
                                SatbDiagnosticKind::WideSpacing,
                                SatbSeverity::Warning,
                                "satb-wide-spacing",
                            ));
                        }
                    }
                }
            }
        }
    }
    for observation in voice_leading {
        if observation.parallel_perfect {
            diagnostics.push(satb_diagnostic(
                observation.upper.clone(),
                observation.lower.clone(),
                SatbDiagnosticKind::ParallelPerfect,
                SatbSeverity::Warning,
                "satb-parallel-perfect",
            ));
        }
    }
    diagnostics
}

fn satb_diagnostic(
    upper: NoteAddr,
    lower: NoteAddr,
    kind: SatbDiagnosticKind,
    severity: SatbSeverity,
    rule_id: &str,
) -> SatbDiagnostic {
    SatbDiagnostic {
        evidence: vec![upper.clone(), lower.clone()],
        upper,
        lower,
        kind,
        severity,
        confidence: 100,
        rule_id: rule_id.to_string(),
    }
}

/// Find explicit cadence transitions in the order they occur within each voice.
pub fn analyze_cadences(chords: &[ChordLabel]) -> Vec<CadenceCandidate> {
    let mut candidates = Vec::new();
    let mut previous: HashMap<(usize, usize, usize), &ChordLabel> = HashMap::new();
    for chord in chords {
        let key = (chord.address.part, chord.address.staff, chord.address.voice);
        let Some(previous_chord) = previous.insert(key, chord) else {
            continue;
        };
        let Some(from_roman) = previous_chord.roman_numeral.as_deref() else {
            continue;
        };
        let Some(to_roman) = chord.roman_numeral.as_deref() else {
            continue;
        };
        let from_figure = roman_figure(from_roman);
        let to_figure = roman_figure(to_roman);
        let kind = match (from_figure, to_figure) {
            ("V" | "V7", "I") => CadenceKind::Authentic,
            ("IV", "I") => CadenceKind::Plagal,
            ("V" | "V7", "vi") => CadenceKind::Deceptive,
            (_, "V" | "V7") => CadenceKind::Half,
            _ => continue,
        };
        let evidence = vec![previous_chord.address.clone(), chord.address.clone()];
        candidates.push(CadenceCandidate {
            from: previous_chord.address.clone(),
            to: chord.address.clone(),
            kind,
            confidence: 100,
            rule_id: "roman-numeral-cadence-transition".to_string(),
            evidence,
        });
    }
    candidates
}

fn roman_figure(roman: &str) -> &str {
    roman.find('/').map_or(roman, |index| &roman[..index])
}

/// Check aligned notes in adjacent voices for motion and parallel perfect intervals.
pub fn analyze_voice_leading(score: &Score) -> Vec<VoiceLeadingObservation> {
    let mut observations = Vec::new();
    for (part_index, part) in score.parts.iter().enumerate() {
        for (staff_index, staff) in part.staves.iter().enumerate() {
            for (measure_index, measure) in staff.measures.iter().enumerate() {
                for (upper_index, upper_voice) in measure.voices.iter().enumerate() {
                    let Some(lower_voice) = measure.voices.get(upper_index + 1) else {
                        continue;
                    };
                    let count = upper_voice.len().min(lower_voice.len());
                    for note_index in 0..count {
                        let Some(upper) = upper_voice[note_index].pitches.first() else {
                            continue;
                        };
                        let Some(lower) = lower_voice[note_index].pitches.first() else {
                            continue;
                        };
                        let next_upper = upper_voice[note_index + 1..]
                            .iter()
                            .find_map(|note| note.pitches.first());
                        let next_lower = lower_voice[note_index + 1..]
                            .iter()
                            .find_map(|note| note.pitches.first());
                        let (Some(next_upper), Some(next_lower)) = (next_upper, next_lower) else {
                            continue;
                        };
                        let upper_addr = NoteAddr {
                            part: part_index,
                            staff: staff_index,
                            measure: measure_index,
                            voice: upper_index,
                            note: note_index,
                        };
                        let lower_addr = NoteAddr {
                            part: part_index,
                            staff: staff_index,
                            measure: measure_index,
                            voice: upper_index + 1,
                            note: note_index,
                        };
                        let upper_next_midi = next_upper.to_midi();
                        let lower_next_midi = next_lower.to_midi();
                        let upper_motion = upper_next_midi - upper.to_midi();
                        let lower_motion = lower_next_midi - lower.to_midi();
                        let initial = (upper.to_midi() - lower.to_midi()).unsigned_abs() % 12;
                        let next = (upper_next_midi - lower_next_midi).unsigned_abs() % 12;
                        observations.push(VoiceLeadingObservation {
                            upper: upper_addr.clone(),
                            lower: lower_addr.clone(),
                            upper_motion,
                            lower_motion,
                            parallel_perfect: matches!(initial, 0 | 7)
                                && initial == next
                                && upper_motion != 0
                                && upper_motion.signum() == lower_motion.signum(),
                            confidence: 100,
                            rule_id: "aligned-adjacent-voice-leading".to_string(),
                            evidence: vec![upper_addr, lower_addr],
                        });
                    }
                }
            }
        }
    }
    observations
}

/// Backwards-compatible name for the complete score analysis pass.
pub fn analyze_chords(score: &Score) -> AnalysisResult {
    analyze_score(score)
}

/// Analyze a finite batch in input order.
pub fn analyze_batch(scores: &[Score]) -> Vec<AnalysisResult> {
    scores.iter().map(analyze_score).collect()
}

/// Analyze scores lazily, one result per input score.
pub fn analyze_stream<I>(scores: I) -> impl Iterator<Item = AnalysisResult>
where
    I: IntoIterator<Item = Score>,
{
    scores.into_iter().map(|score| analyze_score(&score))
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

    #[test]
    fn batch_and_stream_preserve_score_order() {
        let scores = vec![Score::default(), Score::default()];
        let batch = analyze_batch(&scores);
        let streamed: Vec<_> = analyze_stream(scores.clone()).collect();
        assert_eq!(batch, streamed);
        assert_eq!(batch.len(), scores.len());
    }

    #[test]
    fn detects_authentic_cadence_from_adjacent_measures() {
        let mut score = Score::default();
        let voice = &mut score.parts[0].staves[0].measures[0].voices[0];
        voice.clear();
        for (step, octave) in [(Step::G, 3), (Step::B, 3), (Step::D, 4)] {
            voice.push(Note::new(Pitch::new(step, octave), Duration::Quarter));
        }
        let voice = &mut score.parts[0].staves[0].measures[1].voices[0];
        voice.clear();
        for step in [Step::C, Step::E, Step::G] {
            voice.push(Note::new(Pitch::new(step, 4), Duration::Quarter));
        }
        let result = analyze_score(&score);
        assert_eq!(result.cadence_candidates.len(), 1);
        assert_eq!(result.cadence_candidates[0].kind, CadenceKind::Authentic);
        assert_eq!(result.cadence_candidates[0].evidence.len(), 2);
    }

    #[test]
    fn flags_parallel_octaves_between_aligned_voices() {
        let mut score = Score::default();
        let measure = &mut score.parts[0].staves[0].measures[0];
        measure.voices[0].clear();
        measure.voices[1].clear();
        for (upper, lower) in [(Step::C, Step::C), (Step::D, Step::D)] {
            measure.voices[0].push(Note::new(Pitch::new(upper, 4), Duration::Quarter));
            measure.voices[1].push(Note::new(Pitch::new(lower, 3), Duration::Quarter));
        }
        let observations = analyze_voice_leading(&score);
        assert_eq!(observations.len(), 1);
        assert!(observations[0].parallel_perfect);
        assert_eq!(observations[0].evidence.len(), 2);
        let diagnostics = analyze_satb(&score);
        assert_eq!(diagnostics.len(), 1);
        assert_eq!(diagnostics[0].kind, SatbDiagnosticKind::ParallelPerfect);
        assert_eq!(diagnostics[0].severity, SatbSeverity::Warning);
    }

    #[test]
    fn reports_voice_crossing_as_error() {
        let mut score = Score::default();
        let measure = &mut score.parts[0].staves[0].measures[0];
        measure.voices[0].clear();
        measure.voices[1].clear();
        measure.voices[0].push(Note::new(Pitch::new(Step::C, 3), Duration::Quarter));
        measure.voices[1].push(Note::new(Pitch::new(Step::C, 4), Duration::Quarter));
        let diagnostics = analyze_satb(&score);
        assert_eq!(diagnostics.len(), 1);
        assert_eq!(diagnostics[0].kind, SatbDiagnosticKind::VoiceCrossing);
        assert_eq!(diagnostics[0].severity, SatbSeverity::Error);
    }
}
