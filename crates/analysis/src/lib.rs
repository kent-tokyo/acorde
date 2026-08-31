//! Deterministic, explainable music analysis over [`acorde_core::Score`].

use acorde_core::{ChordSymbol, KeySignature, NoteAddr, Score, detect_chord, roman_numeral};
use serde::{Deserialize, Serialize};
use std::collections::{BTreeMap, HashMap};
use thiserror::Error;

/// Version of the serialized analysis result contract.
pub const ANALYSIS_SCHEMA_VERSION: u32 = 7;

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
    /// Deterministic fingerprint of the canonical input score content.
    #[serde(default)]
    pub score_fingerprint: String,
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
    #[serde(default)]
    pub motifs: Vec<MotifPattern>,
    #[serde(default)]
    pub phrase_boundaries: Vec<PhraseBoundary>,
}

impl AnalysisResult {
    /// Return a cache key that invalidates when either the input or result schema changes.
    pub fn cache_key(&self) -> String {
        format!(
            "analysis-v{}-{}",
            self.schema_version, self.score_fingerprint
        )
    }

    /// Check whether this result was produced from the supplied score.
    pub fn matches_score(&self, score: &Score) -> bool {
        self.score_fingerprint == score_fingerprint(score)
    }
}

/// A host- or application-provided deterministic analysis extension.
pub trait AnalysisPass {
    /// Stable identifier used for ordering and persisted result lookup.
    fn id(&self) -> &str;

    /// Run the pass over a score and return its JSON payload.
    fn run(&self, score: &Score) -> serde_json::Value;
}

/// Validation failures for a registered analysis pass set.
#[derive(Debug, Error, Clone, PartialEq, Eq)]
pub enum AnalysisPassError {
    #[error("analysis pass ID must not be empty")]
    EmptyId,
    #[error("duplicate analysis pass ID: {0}")]
    DuplicateId(String),
}

/// Result of one registered extension pass.
#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
pub struct AnalysisPassResult {
    pub pass_id: String,
    pub output: serde_json::Value,
}

/// Run extension passes in stable ID order after validating their identifiers.
pub fn run_analysis_passes(
    score: &Score,
    passes: &[&dyn AnalysisPass],
) -> Result<Vec<AnalysisPassResult>, AnalysisPassError> {
    let mut ordered: Vec<&dyn AnalysisPass> = passes.to_vec();
    for pass in &ordered {
        if pass.id().is_empty() {
            return Err(AnalysisPassError::EmptyId);
        }
    }
    ordered.sort_by(|left, right| left.id().cmp(right.id()));
    for pair in ordered.windows(2) {
        if pair[0].id() == pair[1].id() {
            return Err(AnalysisPassError::DuplicateId(pair[0].id().to_string()));
        }
    }
    Ok(ordered
        .into_iter()
        .map(|pass| AnalysisPassResult {
            pass_id: pass.id().to_string(),
            output: pass.run(score),
        })
        .collect())
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

/// A repeated melodic interval pattern with all matching source occurrences.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct MotifPattern {
    pub signature: Vec<i8>,
    pub occurrences: Vec<MotifOccurrence>,
    pub confidence: u8,
    pub rule_id: String,
}

/// One source span matching a motif pattern.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct MotifOccurrence {
    pub start: NoteAddr,
    pub end: NoteAddr,
    pub evidence: Vec<NoteAddr>,
}

/// A phrase boundary supported by an explicit rest at the end of a measure.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct PhraseBoundary {
    pub address: NoteAddr,
    pub reason: PhraseBoundaryReason,
    pub confidence: u8,
    pub rule_id: String,
    pub evidence: Vec<NoteAddr>,
}

/// Evidence categories for phrase boundaries.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub enum PhraseBoundaryReason {
    RestTermination,
}

/// Hand-verified expected counts for one analysis benchmark fixture.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize, Default)]
pub struct BenchmarkExpectation {
    pub chords: usize,
    pub intervals: usize,
    pub key_estimates: usize,
    pub cadence_candidates: usize,
    pub voice_leading: usize,
    pub satb_diagnostics: usize,
    pub motifs: usize,
    pub phrase_boundaries: usize,
}

/// Predicted category counts used by the benchmark report.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize, Default)]
pub struct AnalysisCounts {
    pub chords: usize,
    pub intervals: usize,
    pub key_estimates: usize,
    pub cadence_candidates: usize,
    pub voice_leading: usize,
    pub satb_diagnostics: usize,
    pub motifs: usize,
    pub phrase_boundaries: usize,
}

/// An analysis category that can be compared in a benchmark report.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
pub enum BenchmarkCategory {
    Chords,
    Intervals,
    KeyEstimates,
    CadenceCandidates,
    VoiceLeading,
    SatbDiagnostics,
    Motifs,
    PhraseBoundaries,
}

/// A category-level benchmark mismatch.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
pub struct BenchmarkFailure {
    pub category: BenchmarkCategory,
    pub expected: usize,
    pub predicted: usize,
    pub missing: usize,
    pub excess: usize,
}

/// One benchmark fixture and its hand-verified expectation.
#[derive(Debug, Clone)]
pub struct BenchmarkCase<'a> {
    pub name: &'a str,
    pub score: &'a Score,
    pub expected: BenchmarkExpectation,
}

/// Precision, recall, and explanation-completeness for one benchmark case.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct BenchmarkCaseReport {
    pub name: String,
    pub predicted: AnalysisCounts,
    pub expected: BenchmarkExpectation,
    pub precision_percent: u8,
    pub recall_percent: u8,
    pub explanation_completeness_percent: u8,
    pub failures: Vec<BenchmarkFailure>,
}

/// Aggregate results for a deterministic benchmark suite.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct BenchmarkSuiteReport {
    pub cases: Vec<BenchmarkCaseReport>,
    pub case_count: usize,
    pub passed_case_count: usize,
    pub failed_case_count: usize,
    pub precision_percent: u8,
    pub recall_percent: u8,
    pub explanation_completeness_percent: u8,
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
    let motifs = analyze_motifs(score);
    let phrase_boundaries = analyze_phrase_boundaries(score);
    AnalysisResult {
        schema_version: ANALYSIS_SCHEMA_VERSION,
        score_fingerprint: score_fingerprint(score),
        chords,
        intervals,
        key_estimates,
        cadence_candidates,
        voice_leading,
        satb_diagnostics,
        motifs,
        phrase_boundaries,
    }
}

/// Return a deterministic, non-cryptographic fingerprint for a canonical score.
pub fn score_fingerprint(score: &Score) -> String {
    let mut value = serde_json::to_value(score).unwrap_or_default();
    remove_generated_ids(&mut value);
    let bytes = serde_json::to_vec(&value).unwrap_or_default();
    let hash = fnv1a64(&bytes);
    format!("fnv1a64-{hash:016x}")
}

fn remove_generated_ids(value: &mut serde_json::Value) {
    match value {
        serde_json::Value::Object(object) => {
            object.remove("id");
            for child in object.values_mut() {
                remove_generated_ids(child);
            }
        }
        serde_json::Value::Array(values) => {
            for child in values {
                remove_generated_ids(child);
            }
        }
        _ => {}
    }
}

/// Return the current schema-versioned cache key without running the analysis passes.
pub fn analysis_cache_key(score: &Score) -> String {
    format!(
        "analysis-v{}-{}",
        ANALYSIS_SCHEMA_VERSION,
        score_fingerprint(score)
    )
}

fn fnv1a64(bytes: &[u8]) -> u64 {
    bytes.iter().fold(0xcbf29ce484222325u64, |hash, byte| {
        (hash ^ u64::from(*byte)).wrapping_mul(0x100000001b3)
    })
}

impl AnalysisCounts {
    fn from_result(result: &AnalysisResult) -> Self {
        Self {
            chords: result.chords.len(),
            intervals: result.intervals.len(),
            key_estimates: result.key_estimates.len(),
            cadence_candidates: result.cadence_candidates.len(),
            voice_leading: result.voice_leading.len(),
            satb_diagnostics: result.satb_diagnostics.len(),
            motifs: result.motifs.len(),
            phrase_boundaries: result.phrase_boundaries.len(),
        }
    }

    fn total(self) -> usize {
        self.chords
            + self.intervals
            + self.key_estimates
            + self.cadence_candidates
            + self.voice_leading
            + self.satb_diagnostics
            + self.motifs
            + self.phrase_boundaries
    }

    fn explained(self, result: &AnalysisResult) -> usize {
        result
            .chords
            .iter()
            .filter(|item| !item.evidence.is_empty())
            .count()
            + result
                .intervals
                .iter()
                .filter(|item| !item.evidence.is_empty())
                .count()
            + result
                .key_estimates
                .iter()
                .filter(|item| !item.evidence.is_empty())
                .count()
            + result
                .cadence_candidates
                .iter()
                .filter(|item| !item.evidence.is_empty())
                .count()
            + result
                .voice_leading
                .iter()
                .filter(|item| !item.evidence.is_empty())
                .count()
            + result
                .satb_diagnostics
                .iter()
                .filter(|item| !item.evidence.is_empty())
                .count()
            + result
                .motifs
                .iter()
                .map(|item| {
                    item.occurrences
                        .iter()
                        .filter(|occurrence| !occurrence.evidence.is_empty())
                        .count()
                })
                .sum::<usize>()
            + result
                .phrase_boundaries
                .iter()
                .filter(|item| !item.evidence.is_empty())
                .count()
    }
}

/// Run one offline benchmark case using count-based hand-verified annotations.
pub fn benchmark_case(case: &BenchmarkCase<'_>) -> BenchmarkCaseReport {
    let result = analyze_score(case.score);
    let predicted = AnalysisCounts::from_result(&result);
    let expected = case.expected;
    let matched = predicted.chords.min(expected.chords)
        + predicted.intervals.min(expected.intervals)
        + predicted.key_estimates.min(expected.key_estimates)
        + predicted
            .cadence_candidates
            .min(expected.cadence_candidates)
        + predicted.voice_leading.min(expected.voice_leading)
        + predicted.satb_diagnostics.min(expected.satb_diagnostics)
        + predicted.motifs.min(expected.motifs)
        + predicted.phrase_boundaries.min(expected.phrase_boundaries);
    let expected_total = AnalysisCounts {
        chords: expected.chords,
        intervals: expected.intervals,
        key_estimates: expected.key_estimates,
        cadence_candidates: expected.cadence_candidates,
        voice_leading: expected.voice_leading,
        satb_diagnostics: expected.satb_diagnostics,
        motifs: expected.motifs,
        phrase_boundaries: expected.phrase_boundaries,
    }
    .total();
    let predicted_total = predicted.total();
    let failures = [
        (BenchmarkCategory::Chords, expected.chords, predicted.chords),
        (
            BenchmarkCategory::Intervals,
            expected.intervals,
            predicted.intervals,
        ),
        (
            BenchmarkCategory::KeyEstimates,
            expected.key_estimates,
            predicted.key_estimates,
        ),
        (
            BenchmarkCategory::CadenceCandidates,
            expected.cadence_candidates,
            predicted.cadence_candidates,
        ),
        (
            BenchmarkCategory::VoiceLeading,
            expected.voice_leading,
            predicted.voice_leading,
        ),
        (
            BenchmarkCategory::SatbDiagnostics,
            expected.satb_diagnostics,
            predicted.satb_diagnostics,
        ),
        (BenchmarkCategory::Motifs, expected.motifs, predicted.motifs),
        (
            BenchmarkCategory::PhraseBoundaries,
            expected.phrase_boundaries,
            predicted.phrase_boundaries,
        ),
    ]
    .into_iter()
    .filter_map(|(category, expected, predicted)| {
        if expected == predicted {
            return None;
        }
        Some(BenchmarkFailure {
            category,
            expected,
            predicted,
            missing: expected.saturating_sub(predicted),
            excess: predicted.saturating_sub(expected),
        })
    })
    .collect();
    BenchmarkCaseReport {
        name: case.name.to_string(),
        predicted,
        expected,
        precision_percent: percentage(matched, predicted_total),
        recall_percent: percentage(matched, expected_total),
        explanation_completeness_percent: percentage(predicted.explained(&result), predicted_total),
        failures,
    }
}

/// Run benchmark cases in input order; no filesystem or network access is used.
pub fn run_benchmark(cases: &[BenchmarkCase<'_>]) -> Vec<BenchmarkCaseReport> {
    cases.iter().map(benchmark_case).collect()
}

/// Run a benchmark suite and aggregate its case-level metrics.
pub fn run_benchmark_suite(cases: &[BenchmarkCase<'_>]) -> BenchmarkSuiteReport {
    let reports = run_benchmark(cases);
    let case_count = reports.len();
    let passed_case_count = reports
        .iter()
        .filter(|report| report.failures.is_empty())
        .count();
    let failed_case_count = case_count.saturating_sub(passed_case_count);
    let precision_total: usize = reports
        .iter()
        .map(|report| usize::from(report.precision_percent))
        .sum();
    let recall_total: usize = reports
        .iter()
        .map(|report| usize::from(report.recall_percent))
        .sum();
    let explanation_total: usize = reports
        .iter()
        .map(|report| usize::from(report.explanation_completeness_percent))
        .sum();
    let metric_denominator = case_count.saturating_mul(100);
    let aggregate_metric = |total: usize| {
        if case_count == 0 {
            0
        } else {
            percentage(total, metric_denominator)
        }
    };
    BenchmarkSuiteReport {
        cases: reports,
        case_count,
        passed_case_count,
        failed_case_count,
        precision_percent: aggregate_metric(precision_total),
        recall_percent: aggregate_metric(recall_total),
        explanation_completeness_percent: aggregate_metric(explanation_total),
    }
}

fn percentage(numerator: usize, denominator: usize) -> u8 {
    match numerator.saturating_mul(100).checked_div(denominator) {
        Some(value) => value.min(100) as u8,
        None => 100,
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

/// Find repeated three-note melodic interval patterns, resetting at rests.
pub fn analyze_motifs(score: &Score) -> Vec<MotifPattern> {
    let mut groups: BTreeMap<(usize, usize, usize, Vec<i8>), Vec<MotifOccurrence>> =
        BTreeMap::new();
    for (part_index, part) in score.parts.iter().enumerate() {
        for (staff_index, staff) in part.staves.iter().enumerate() {
            let Some(first_measure) = staff.measures.first() else {
                continue;
            };
            for (voice_index, _) in first_measure.voices.iter().enumerate() {
                let mut segment = Vec::new();
                let mut segments = Vec::new();
                for (measure_index, measure) in staff.measures.iter().enumerate() {
                    for (note_index, note) in measure.voices[voice_index].iter().enumerate() {
                        let Some(pitch) = note.pitches.first() else {
                            if segment.len() >= 3 {
                                segments.push(std::mem::take(&mut segment));
                            } else {
                                segment.clear();
                            }
                            continue;
                        };
                        if note.is_rest {
                            if segment.len() >= 3 {
                                segments.push(std::mem::take(&mut segment));
                            } else {
                                segment.clear();
                            }
                            continue;
                        }
                        segment.push((
                            NoteAddr {
                                part: part_index,
                                staff: staff_index,
                                measure: measure_index,
                                voice: voice_index,
                                note: note_index,
                            },
                            pitch.to_midi(),
                        ));
                    }
                }
                if segment.len() >= 3 {
                    segments.push(segment);
                }
                for segment in segments {
                    for window in segment.windows(3) {
                        let signature = vec![
                            (window[1].1 - window[0].1) as i8,
                            (window[2].1 - window[1].1) as i8,
                        ];
                        let occurrence = MotifOccurrence {
                            start: window[0].0.clone(),
                            end: window[2].0.clone(),
                            evidence: window.iter().map(|(address, _)| address.clone()).collect(),
                        };
                        groups
                            .entry((part_index, staff_index, voice_index, signature))
                            .or_default()
                            .push(occurrence);
                    }
                }
            }
        }
    }
    groups
        .into_iter()
        .filter(|(_, occurrences)| occurrences.len() >= 2)
        .map(|((_, _, _, signature), occurrences)| MotifPattern {
            signature,
            occurrences,
            confidence: 100,
            rule_id: "repeated-three-note-interval-pattern".to_string(),
        })
        .collect()
}

/// Report measure-ending rests as explicit, conservative phrase boundaries.
pub fn analyze_phrase_boundaries(score: &Score) -> Vec<PhraseBoundary> {
    let mut boundaries = Vec::new();
    for (part_index, part) in score.parts.iter().enumerate() {
        for (staff_index, staff) in part.staves.iter().enumerate() {
            for (measure_index, measure) in staff.measures.iter().enumerate() {
                for (voice_index, voice) in measure.voices.iter().enumerate() {
                    let Some((note_index, note)) = voice.iter().enumerate().next_back() else {
                        continue;
                    };
                    if !note.is_rest {
                        continue;
                    }
                    let address = NoteAddr {
                        part: part_index,
                        staff: staff_index,
                        measure: measure_index,
                        voice: voice_index,
                        note: note_index,
                    };
                    boundaries.push(PhraseBoundary {
                        address: address.clone(),
                        reason: PhraseBoundaryReason::RestTermination,
                        confidence: 100,
                        rule_id: "measure-ending-rest".to_string(),
                        evidence: vec![address],
                    });
                }
            }
        }
    }
    boundaries
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
    fn analysis_has_stable_score_fingerprint() {
        let score = Score::default();
        let repeated = analyze_score(&score);
        assert_eq!(repeated.score_fingerprint, score_fingerprint(&score));
        assert_eq!(
            repeated.score_fingerprint,
            analyze_score(&score).score_fingerprint
        );

        let mut changed = score.clone();
        changed.metadata.title = "Changed".to_string();
        assert_ne!(repeated.score_fingerprint, score_fingerprint(&changed));
    }

    #[test]
    fn fingerprint_uses_fnv1a_byte_order() {
        assert_eq!(fnv1a64(b"hello"), 0xa430d84680aabd0b);
    }

    #[test]
    fn cache_key_includes_schema_and_score_identity() {
        let result = analyze_score(&Score::default());
        assert!(result.cache_key().starts_with("analysis-v7-fnv1a64-"));
        assert_eq!(result.cache_key(), analysis_cache_key(&Score::default()));
        let mut changed = result.clone();
        changed.schema_version = 8;
        assert_ne!(result.cache_key(), changed.cache_key());
    }

    #[test]
    fn analysis_result_rejects_a_different_score() {
        let score = Score::default();
        let result = analyze_score(&score);
        assert!(result.matches_score(&score));
        let mut changed = score.clone();
        changed.metadata.title = "Changed".to_string();
        assert!(!result.matches_score(&changed));
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

    #[test]
    fn finds_repeated_melodic_motif_with_source_spans() {
        let mut score = Score::default();
        let voice = &mut score.parts[0].staves[0].measures[0].voices[0];
        voice.clear();
        for step in [Step::C, Step::D, Step::E, Step::G, Step::A, Step::B] {
            voice.push(Note::new(Pitch::new(step, 4), Duration::Quarter));
        }
        let motifs = analyze_motifs(&score);
        assert_eq!(motifs.len(), 1);
        assert_eq!(motifs[0].signature, vec![2, 2]);
        assert_eq!(motifs[0].occurrences.len(), 2);
        assert_eq!(motifs[0].occurrences[0].evidence.len(), 3);
    }

    #[test]
    fn reports_measure_ending_rest_as_phrase_boundary() {
        let mut score = Score::default();
        let voice = &mut score.parts[0].staves[0].measures[0].voices[0];
        voice.clear();
        voice.push(Note::new(Pitch::new(Step::C, 4), Duration::Quarter));
        voice.push(Note::rest(Duration::Quarter));
        let boundaries = analyze_phrase_boundaries(&score);
        assert!(boundaries.iter().any(|boundary| {
            boundary.reason == PhraseBoundaryReason::RestTermination
                && boundary.address.measure == 0
                && boundary.address.note == 1
        }));
    }

    #[test]
    fn benchmark_reports_perfect_scores_for_hand_verified_empty_fixture() {
        let score = Score::default();
        let cases = [BenchmarkCase {
            name: "empty-score",
            score: &score,
            expected: BenchmarkExpectation {
                phrase_boundaries: 4,
                ..BenchmarkExpectation::default()
            },
        }];
        let reports = run_benchmark(&cases);
        assert_eq!(reports.len(), 1);
        assert_eq!(reports[0].precision_percent, 100);
        assert_eq!(reports[0].recall_percent, 100);
        assert_eq!(reports[0].explanation_completeness_percent, 100);
        assert!(reports[0].failures.is_empty());
    }

    #[test]
    fn benchmark_reports_category_level_failure_details() {
        let score = Score::default();
        let cases = [BenchmarkCase {
            name: "under-annotated-score",
            score: &score,
            expected: BenchmarkExpectation {
                phrase_boundaries: 5,
                ..BenchmarkExpectation::default()
            },
        }];
        let reports = run_benchmark(&cases);
        assert_eq!(
            reports[0].failures,
            vec![BenchmarkFailure {
                category: BenchmarkCategory::PhraseBoundaries,
                expected: 5,
                predicted: 4,
                missing: 1,
                excess: 0,
            }]
        );
    }

    #[test]
    fn benchmark_suite_aggregates_case_status_and_metrics() {
        let score = Score::default();
        let cases = [
            BenchmarkCase {
                name: "passing",
                score: &score,
                expected: BenchmarkExpectation {
                    phrase_boundaries: 4,
                    ..BenchmarkExpectation::default()
                },
            },
            BenchmarkCase {
                name: "failing",
                score: &score,
                expected: BenchmarkExpectation {
                    phrase_boundaries: 5,
                    ..BenchmarkExpectation::default()
                },
            },
        ];
        let suite = run_benchmark_suite(&cases);
        assert_eq!(suite.case_count, 2);
        assert_eq!(suite.passed_case_count, 1);
        assert_eq!(suite.failed_case_count, 1);
        assert_eq!(suite.cases.len(), 2);
        assert_eq!(suite.precision_percent, 100);
        assert_eq!(suite.recall_percent, 90);
        assert_eq!(suite.explanation_completeness_percent, 100);
    }

    struct TestPass(&'static str);

    impl AnalysisPass for TestPass {
        fn id(&self) -> &str {
            self.0
        }

        fn run(&self, _score: &Score) -> serde_json::Value {
            serde_json::json!({ "pass": self.0 })
        }
    }

    #[test]
    fn extension_passes_run_in_stable_id_order() {
        let score = Score::default();
        let beta = TestPass("beta");
        let alpha = TestPass("alpha");
        let results = run_analysis_passes(&score, &[&beta, &alpha]).unwrap();
        assert_eq!(
            results
                .iter()
                .map(|result| result.pass_id.as_str())
                .collect::<Vec<_>>(),
            ["alpha", "beta"]
        );
        assert_eq!(results[0].output["pass"], "alpha");
    }

    #[test]
    fn extension_passes_reject_empty_and_duplicate_ids() {
        let score = Score::default();
        let empty = TestPass("");
        assert_eq!(
            run_analysis_passes(&score, &[&empty]),
            Err(AnalysisPassError::EmptyId)
        );
        let first = TestPass("same");
        let second = TestPass("same");
        assert_eq!(
            run_analysis_passes(&score, &[&first, &second]),
            Err(AnalysisPassError::DuplicateId("same".to_string()))
        );
    }
}
