//! Deterministic, explainable music analysis over [`acorde_core::Score`].

use acorde_core::{
    ChangeHint, ChordSymbol, KeySignature, NoteAddr, Pitch, Score, Step, detect_chord,
    roman_numeral, roman_numeral_figured, transpose_fifths,
};
use serde::{Deserialize, Serialize};
use std::collections::{BTreeMap, HashMap, VecDeque};
use thiserror::Error;

/// Version of the serialized analysis result contract.
pub const ANALYSIS_SCHEMA_VERSION: u32 = 14;

/// A chord label with source evidence and the rule that produced it.
#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
pub struct ChordLabel {
    pub address: NoteAddr,
    pub chord: ChordSymbol,
    /// Canonical human-readable spelling derived from `chord`.
    #[serde(default)]
    pub name: String,
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

/// One complete-analysis category that can change between two results.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
pub enum AnalysisCategory {
    Chords,
    Intervals,
    KeyEstimates,
    CadenceCandidates,
    VoiceLeading,
    SatbDiagnostics,
    Motifs,
    PhraseBoundaries,
}

/// A changed part/staff measure interval used by incremental editor planning.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct AnalysisRegion {
    pub part: usize,
    pub staff: usize,
    pub start_measure: usize,
    pub end_measure: usize,
}

/// Dependency-aware refresh plan for a score edit.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct AnalysisRefreshPlan {
    pub local_categories: Vec<AnalysisCategory>,
    pub global_categories: Vec<AnalysisCategory>,
    pub region: Option<AnalysisRegion>,
    pub context_before: usize,
    pub context_after: usize,
}

/// Build a conservative refresh plan from an engine change hint.
pub fn analysis_refresh_plan(hint: &ChangeHint) -> AnalysisRefreshPlan {
    let all = all_analysis_categories();
    if !hint.layout_dirty && !hint.playback_dirty {
        return AnalysisRefreshPlan {
            local_categories: Vec::new(),
            global_categories: Vec::new(),
            region: None,
            context_before: 0,
            context_after: 0,
        };
    }
    match &hint.scope {
        acorde_core::ChangeScope::Measures {
            part,
            staff,
            start,
            end,
        } => AnalysisRefreshPlan {
            local_categories: vec![
                AnalysisCategory::Chords,
                AnalysisCategory::Intervals,
                AnalysisCategory::CadenceCandidates,
                AnalysisCategory::VoiceLeading,
                AnalysisCategory::SatbDiagnostics,
                AnalysisCategory::PhraseBoundaries,
            ],
            global_categories: vec![AnalysisCategory::KeyEstimates, AnalysisCategory::Motifs],
            region: Some(AnalysisRegion {
                part: *part,
                staff: *staff,
                start_measure: *start,
                end_measure: *end,
            }),
            context_before: 1,
            context_after: 1,
        },
        acorde_core::ChangeScope::Part(_) | acorde_core::ChangeScope::Global => {
            AnalysisRefreshPlan {
                local_categories: Vec::new(),
                global_categories: all,
                region: None,
                context_before: 0,
                context_after: 0,
            }
        }
    }
}

fn all_analysis_categories() -> Vec<AnalysisCategory> {
    vec![
        AnalysisCategory::Chords,
        AnalysisCategory::Intervals,
        AnalysisCategory::KeyEstimates,
        AnalysisCategory::CadenceCandidates,
        AnalysisCategory::VoiceLeading,
        AnalysisCategory::SatbDiagnostics,
        AnalysisCategory::Motifs,
        AnalysisCategory::PhraseBoundaries,
    ]
}

/// Return the complete-analysis categories that a dirty engine hint may affect.
///
/// The mapping is intentionally conservative: any layout or playback dirty hint may alter
/// score content or temporal context, so all categories are returned. A clean hint is known to
/// affect neither analysis input nor context and returns an empty list.
pub fn affected_categories_for_change_hint(hint: &ChangeHint) -> Vec<AnalysisCategory> {
    let plan = analysis_refresh_plan(hint);
    let selected: Vec<_> = plan
        .local_categories
        .into_iter()
        .chain(plan.global_categories)
        .collect();
    all_analysis_categories()
        .into_iter()
        .filter(|category| selected.contains(category))
        .collect()
}

/// Deterministic category-level diff between two analysis results.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct AnalysisDiff {
    pub previous_score_fingerprint: String,
    pub current_score_fingerprint: String,
    pub schema_changed: bool,
    pub changed_categories: Vec<AnalysisCategory>,
}

/// Analysis result and its deterministic diff from a previous result.
#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
pub struct AnalysisEditResult {
    pub analysis: AnalysisResult,
    pub diff: AnalysisDiff,
}

/// One deterministic explanation attached to a source note address.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct AnalysisProvenance {
    pub category: AnalysisCategory,
    pub rule_id: String,
    pub confidence: u8,
    pub evidence: Vec<NoteAddr>,
    /// Canonical display label when the finding is a chord explanation.
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub label: Option<String>,
}

/// Return every analysis finding whose evidence contains the supplied source address.
pub fn analysis_provenance(
    analysis: &AnalysisResult,
    address: &NoteAddr,
) -> Vec<AnalysisProvenance> {
    let mut findings = Vec::new();
    let mut add = |category, rule_id: &str, confidence, evidence: &[NoteAddr]| {
        if evidence.iter().any(|item| item == address) {
            findings.push(AnalysisProvenance {
                category,
                rule_id: rule_id.to_string(),
                confidence,
                evidence: evidence.to_vec(),
                label: None,
            });
        }
    };
    for item in &analysis.chords {
        add(
            AnalysisCategory::Chords,
            &item.rule_id,
            item.confidence,
            &item.evidence,
        );
    }
    for item in &analysis.intervals {
        add(
            AnalysisCategory::Intervals,
            &item.rule_id,
            100,
            &item.evidence,
        );
    }
    for item in &analysis.key_estimates {
        add(
            AnalysisCategory::KeyEstimates,
            &item.rule_id,
            item.confidence,
            &item.evidence,
        );
    }
    for item in &analysis.cadence_candidates {
        add(
            AnalysisCategory::CadenceCandidates,
            &item.rule_id,
            item.confidence,
            &item.evidence,
        );
    }
    for item in &analysis.voice_leading {
        add(
            AnalysisCategory::VoiceLeading,
            &item.rule_id,
            item.confidence,
            &item.evidence,
        );
    }
    for item in &analysis.satb_diagnostics {
        add(
            AnalysisCategory::SatbDiagnostics,
            &item.rule_id,
            item.confidence,
            &item.evidence,
        );
    }
    for item in &analysis.motifs {
        for occurrence in &item.occurrences {
            add(
                AnalysisCategory::Motifs,
                &item.rule_id,
                item.confidence,
                &occurrence.evidence,
            );
        }
    }
    for item in &analysis.phrase_boundaries {
        add(
            AnalysisCategory::PhraseBoundaries,
            &item.rule_id,
            item.confidence,
            &item.evidence,
        );
    }
    for finding in &mut findings {
        if finding.category == AnalysisCategory::Chords {
            finding.label = analysis
                .chords
                .iter()
                .find(|item| item.rule_id == finding.rule_id && item.evidence == finding.evidence)
                .map(|item| item.name.clone());
        }
    }
    findings.sort_by_key(|item| {
        (
            analysis_category_rank(item.category),
            item.rule_id.clone(),
            item.evidence
                .iter()
                .map(|address| {
                    (
                        address.part,
                        address.staff,
                        address.measure,
                        address.voice,
                        address.note,
                    )
                })
                .collect::<Vec<_>>(),
        )
    });
    findings
}

/// A score diff together with the explanations affected at one source address.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct AnalysisChangeExplanation {
    pub diff: AnalysisDiff,
    pub previous: Vec<AnalysisProvenance>,
    pub current: Vec<AnalysisProvenance>,
}

/// Compare two analysis results and explain the findings attached to one source address.
pub fn explain_analysis_change(
    previous: &AnalysisResult,
    current: &AnalysisResult,
    address: &NoteAddr,
) -> AnalysisChangeExplanation {
    AnalysisChangeExplanation {
        diff: diff_analysis(previous, current),
        previous: analysis_provenance(previous, address),
        current: analysis_provenance(current, address),
    }
}

fn analysis_category_rank(category: AnalysisCategory) -> u8 {
    match category {
        AnalysisCategory::Chords => 0,
        AnalysisCategory::Intervals => 1,
        AnalysisCategory::KeyEstimates => 2,
        AnalysisCategory::CadenceCandidates => 3,
        AnalysisCategory::VoiceLeading => 4,
        AnalysisCategory::SatbDiagnostics => 5,
        AnalysisCategory::Motifs => 6,
        AnalysisCategory::PhraseBoundaries => 7,
    }
}

impl AnalysisDiff {
    /// Return whether any schema or analysis category changed.
    pub fn is_empty(&self) -> bool {
        !self.schema_changed && self.changed_categories.is_empty()
    }
}

/// Compare two complete results in stable category order.
pub fn diff_analysis(previous: &AnalysisResult, current: &AnalysisResult) -> AnalysisDiff {
    let mut changed_categories = Vec::new();
    if previous.chords != current.chords {
        changed_categories.push(AnalysisCategory::Chords);
    }
    if previous.intervals != current.intervals {
        changed_categories.push(AnalysisCategory::Intervals);
    }
    if previous.key_estimates != current.key_estimates {
        changed_categories.push(AnalysisCategory::KeyEstimates);
    }
    if previous.cadence_candidates != current.cadence_candidates {
        changed_categories.push(AnalysisCategory::CadenceCandidates);
    }
    if previous.voice_leading != current.voice_leading {
        changed_categories.push(AnalysisCategory::VoiceLeading);
    }
    if previous.satb_diagnostics != current.satb_diagnostics {
        changed_categories.push(AnalysisCategory::SatbDiagnostics);
    }
    if previous.motifs != current.motifs {
        changed_categories.push(AnalysisCategory::Motifs);
    }
    if previous.phrase_boundaries != current.phrase_boundaries {
        changed_categories.push(AnalysisCategory::PhraseBoundaries);
    }
    AnalysisDiff {
        previous_score_fingerprint: previous.score_fingerprint.clone(),
        current_score_fingerprint: current.score_fingerprint.clone(),
        schema_changed: previous.schema_version != current.schema_version,
        changed_categories,
    }
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

/// A deterministic key candidate ranked by duration-weighted diatonic pitch coverage.
#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
pub struct KeyEstimate {
    pub key: KeySignature,
    pub covered_pitches: usize,
    pub total_pitches: usize,
    /// Sum of note durations whose pitches belong to this key candidate.
    #[serde(default)]
    pub weighted_covered_beats: f64,
    /// Sum of pitched-note durations used as the weighting denominator.
    #[serde(default)]
    pub total_duration_beats: f64,
    pub confidence: u8,
    pub rule_id: String,
    pub evidence: Vec<NoteAddr>,
}

/// Opt-in local key estimate for one physical measure and a centered measure window.
#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
pub struct LocalKeyEstimate {
    pub measure: usize,
    pub window_start: usize,
    pub window_end_exclusive: usize,
    pub key: KeySignature,
    /// Pearson correlation with the duration-weighted pitch-class profile, not a probability.
    pub profile_correlation: f64,
    pub rule_id: String,
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
    /// Exact signed upper-voice motion in cents.
    #[serde(default)]
    pub upper_motion_cents: i32,
    /// Exact signed lower-voice motion in cents.
    #[serde(default)]
    pub lower_motion_cents: i32,
    pub parallel_perfect: bool,
    pub confidence: u8,
    pub rule_id: String,
    pub evidence: Vec<NoteAddr>,
}

/// One monophonic written note in a located two-voice comparison.
#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
pub struct QuartetNote {
    pub address: NoteAddr,
    pub pitch: Pitch,
    /// Absolute written onset in quarter-note beats from the score start.
    pub onset_beats: f64,
    pub duration_beats: f64,
}

/// Four sounding notes at two successive aligned events in distinct voices.
///
/// Voice order is source order (part, staff, voice), not pitch height. Chords,
/// rests, unpitched notes, and malformed voice bars do not form quartet notes.
#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
pub struct VoiceLeadingQuartet {
    pub first_before: QuartetNote,
    pub first_after: QuartetNote,
    pub second_before: QuartetNote,
    pub second_after: QuartetNote,
    pub contrary_motion: bool,
    pub similar_motion: bool,
    /// A perfect fifth to a perfect fifth, including octave displacement and
    /// antiparallel motion, using written pitch spelling.
    pub parallel_fifth: bool,
    /// A perfect octave to a perfect octave, excluding unisons.
    pub parallel_octave: bool,
    /// music21's keyless P4/A4/d5/m7 resolution rule on the four written notes.
    pub proper_resolution_without_key: bool,
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
    /// Signed exact pitch distance in cents; unlike `semitones`, this preserves microtones.
    #[serde(default)]
    pub cents: i32,
    pub diatonic_steps: i8,
    pub rule_id: String,
    pub evidence: Vec<NoteAddr>,
}

/// Analyze every voice that contains at least two pitched notes in a measure.
pub fn analyze_score(score: &Score) -> AnalysisResult {
    let chords = analyze_chords_only(score);
    let intervals = analyze_intervals(score);
    let key_estimates = estimate_keys(score);
    let cadence_candidates = analyze_cadences(&chords);
    let voice_leading = analyze_voice_leading(score);
    let satb_diagnostics = analyze_satb_in_region(score, &voice_leading, None);
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

fn analyze_chords_only(score: &Score) -> Vec<ChordLabel> {
    analyze_chords_in_region(score, None)
}

/// One chord heard at a written beat, using notes active across all parts and voices.
#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
pub struct VerticalSonority {
    /// Physical measure in the first available staff. For unequal but
    /// time-aligned bar partitions, this is the reference coordinate rather
    /// than the measure containing `label.address`.
    pub measure: usize,
    /// Beat from the start of that reference measure.
    pub beat: f64,
    pub label: ChordLabel,
}

/// A part/staff whose physical bar cannot safely be aligned with the others.
#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
pub struct VerticalTimelineIssue {
    pub measure: usize,
    pub part: usize,
    pub staff: usize,
    pub kind: VerticalTimelineIssueKind,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum VerticalTimelineIssueKind {
    MissingMeasure,
    DifferentStart,
    DifferentLength,
    InvalidLength,
    InvalidNoteDuration,
    OverfullVoice,
    UnrepresentableConcertPitch,
}

/// Sonorities and physical-bar alignment diagnostics in source order.
#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
pub struct VerticalAnalysisReport {
    pub sonorities: Vec<VerticalSonority>,
    pub timeline_issues: Vec<VerticalTimelineIssue>,
}

/// Opt-in cadence candidates inferred from located vertical sonorities.
#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
pub struct VerticalCadenceReport {
    pub cadence_candidates: Vec<CadenceCandidate>,
    pub timeline_issues: Vec<VerticalTimelineIssue>,
}

fn vertical_measure_starts(score: &Score) -> Vec<Vec<Vec<f64>>> {
    score
        .parts
        .iter()
        .map(|part| {
            part.staves
                .iter()
                .map(|staff| {
                    let mut start = 0.0;
                    (0..staff.measures.len())
                        .map(|index| {
                            let current = start;
                            start += staff.measure_beats(index, &score.settings.time_signature);
                            current
                        })
                        .collect()
                })
                .collect()
        })
        .collect()
}

fn vertical_measure_alignment_issues(
    score: &Score,
    starts: &[Vec<Vec<f64>>],
    measure_index: usize,
) -> Vec<VerticalTimelineIssue> {
    const EPSILON: f64 = 1e-7;
    let mut reference: Option<(f64, f64)> = None;
    let mut issues = Vec::new();
    for (part_index, part) in score.parts.iter().enumerate() {
        for (staff_index, staff) in part.staves.iter().enumerate() {
            let Some(measure) = staff.measures.get(measure_index) else {
                issues.push(VerticalTimelineIssue {
                    measure: measure_index,
                    part: part_index,
                    staff: staff_index,
                    kind: VerticalTimelineIssueKind::MissingMeasure,
                });
                continue;
            };
            let start = starts[part_index][staff_index][measure_index];
            let length = staff.measure_beats(measure_index, &score.settings.time_signature);
            if !start.is_finite()
                || !length.is_finite()
                || length <= 0.0
                || measure
                    .actual_length
                    .is_some_and(|value| value.beats().is_none())
            {
                issues.push(VerticalTimelineIssue {
                    measure: measure_index,
                    part: part_index,
                    staff: staff_index,
                    kind: VerticalTimelineIssueKind::InvalidLength,
                });
                continue;
            }
            for voice in &measure.voices {
                let mut elapsed = 0.0;
                let mut invalid = false;
                for note in voice {
                    let duration = if voice.len() == 1 && note.is_plain_whole_rest() {
                        length
                    } else {
                        note.beats()
                    };
                    if !duration.is_finite() || duration < 0.0 {
                        issues.push(VerticalTimelineIssue {
                            measure: measure_index,
                            part: part_index,
                            staff: staff_index,
                            kind: VerticalTimelineIssueKind::InvalidNoteDuration,
                        });
                        invalid = true;
                        break;
                    }
                    elapsed += duration;
                }
                if !invalid && elapsed > length + EPSILON {
                    issues.push(VerticalTimelineIssue {
                        measure: measure_index,
                        part: part_index,
                        staff: staff_index,
                        kind: VerticalTimelineIssueKind::OverfullVoice,
                    });
                }
            }
            if let Some((reference_start, reference_length)) = reference {
                if (start - reference_start).abs() > EPSILON {
                    issues.push(VerticalTimelineIssue {
                        measure: measure_index,
                        part: part_index,
                        staff: staff_index,
                        kind: VerticalTimelineIssueKind::DifferentStart,
                    });
                }
                if (length - reference_length).abs() > EPSILON {
                    issues.push(VerticalTimelineIssue {
                        measure: measure_index,
                        part: part_index,
                        staff: staff_index,
                        kind: VerticalTimelineIssueKind::DifferentLength,
                    });
                }
            } else {
                reference = Some((start, length));
            }
        }
    }
    issues
}

struct WrittenEvent<'a> {
    start: f64,
    end: f64,
    address: NoteAddr,
    note: &'a acorde_core::Note,
}

/// Shared written-position traversal for analysis passes within one physical bar.
/// A lone plain whole rest occupies the actual bar length, including a pickup.
fn written_measure_events(score: &Score, measure_index: usize) -> Vec<WrittenEvent<'_>> {
    let mut events = Vec::new();
    for (part_index, part) in score.parts.iter().enumerate() {
        for staff_index in 0..part.staves.len() {
            events.extend(written_staff_measure_events(
                score,
                part_index,
                staff_index,
                measure_index,
            ));
        }
    }
    events
}

fn written_staff_measure_events(
    score: &Score,
    part_index: usize,
    staff_index: usize,
    measure_index: usize,
) -> Vec<WrittenEvent<'_>> {
    let Some(staff) = score
        .parts
        .get(part_index)
        .and_then(|part| part.staves.get(staff_index))
    else {
        return Vec::new();
    };
    let Some(measure) = staff.measures.get(measure_index) else {
        return Vec::new();
    };
    let bar_beats = staff.measure_beats(measure_index, &score.settings.time_signature);
    let mut events = Vec::new();
    for (voice_index, voice) in measure.voices.iter().enumerate() {
        let mut beat = 0.0;
        for (note_index, note) in voice.iter().enumerate() {
            let duration = if voice.len() == 1 && note.is_plain_whole_rest() {
                bar_beats
            } else {
                note.beats()
            };
            if !duration.is_finite() || duration < 0.0 {
                continue;
            }
            events.push(WrittenEvent {
                start: beat,
                end: beat + duration,
                address: NoteAddr {
                    part: part_index,
                    staff: staff_index,
                    measure: measure_index,
                    voice: voice_index,
                    note: note_index,
                },
                note,
            });
            beat += duration;
        }
    }
    events
}

fn sounding_transpose(score: &Score, address: &NoteAddr) -> Option<i8> {
    let part = score.parts.get(address.part)?;
    let staff = part.staves.get(address.staff)?;
    let measure = staff.measures.get(address.measure)?;
    let instrument = measure
        .instrument_change
        .as_ref()
        .or(part.instrument.as_ref());
    let channel = instrument.map_or(part.midi_channel, |value| value.midi_channel);
    Some(if channel == 9 {
        0
    } else {
        staff
            .transpose_semitones
            .saturating_add(instrument.map_or(0, |value| value.transpose_semitones))
    })
}

fn concert_key_at_measure(
    score: &Score,
    part_index: usize,
    staff_index: usize,
    measure_index: usize,
) -> Option<KeySignature> {
    let staff = score.parts.get(part_index)?.staves.get(staff_index)?;
    let measure = staff.measures.get(measure_index)?;
    let address = NoteAddr {
        part: part_index,
        staff: staff_index,
        measure: measure_index,
        voice: 0,
        note: 0,
    };
    let semitones = sounding_transpose(score, &address)?;
    if measure.key_sig.is_none()
        && let Some(previous_index) = measure_index.checked_sub(1)
        && sounding_transpose(
            score,
            &NoteAddr {
                measure: previous_index,
                ..address.clone()
            },
        ) != Some(semitones)
    {
        return None;
    }
    let explicit = staff
        .measures
        .iter()
        .take(measure_index + 1)
        .rev()
        .find_map(|measure| measure.key_sig.as_ref());
    // A global score key is not enough to tell whether a transposing staff's
    // written key has already been converted to concert pitch.
    let written = if semitones == 0 {
        explicit.unwrap_or(&score.settings.key_signature)
    } else {
        explicit?
    };
    Some(KeySignature {
        fifths: transpose_fifths(written.fifths, &written.mode, semitones),
        mode: written.mode.clone(),
    })
}

fn agreed_concert_key_at_measure(score: &Score, measure_index: usize) -> Option<KeySignature> {
    let mut agreed: Option<KeySignature> = None;
    for (part_index, part) in score.parts.iter().enumerate() {
        for (staff_index, staff) in part.staves.iter().enumerate() {
            let measure = staff.measures.get(measure_index)?;
            let contributes = measure.voices.iter().flatten().any(|note| {
                (!note.is_rest && !note.is_unpitched && !note.pitches.is_empty())
                    || note.chord_symbol.is_some()
            });
            if !contributes {
                continue;
            }
            let candidate = concert_key_at_measure(score, part_index, staff_index, measure_index)?;
            if let Some(previous) = &agreed
                && previous != &candidate
            {
                return None;
            }
            agreed = Some(candidate);
        }
    }
    agreed
}

fn agreed_concert_key_for_addresses<'a>(
    score: &Score,
    addresses: impl IntoIterator<Item = &'a NoteAddr>,
) -> Option<KeySignature> {
    let mut agreed: Option<KeySignature> = None;
    for address in addresses {
        let candidate =
            concert_key_at_measure(score, address.part, address.staff, address.measure)?;
        if agreed
            .as_ref()
            .is_some_and(|previous| previous != &candidate)
        {
            return None;
        }
        agreed = Some(candidate);
    }
    agreed
}

struct TieStartCandidate {
    address: NoteAddr,
    within_measure_start: f64,
    absolute_end: f64,
}

type TieStartIndex = HashMap<(usize, i32), Vec<TieStartCandidate>>;

fn tie_start_index(score: &Score, starts: &[Vec<Vec<f64>>]) -> TieStartIndex {
    const EPSILON: f64 = 1e-7;
    let mut index: TieStartIndex = HashMap::new();
    for (part_index, part) in score.parts.iter().enumerate() {
        for (staff_index, staff) in part.staves.iter().enumerate() {
            for (measure_index, &measure_start) in
                starts[part_index][staff_index].iter().enumerate()
            {
                let bar_length = staff.measure_beats(measure_index, &score.settings.time_signature);
                for event in
                    written_staff_measure_events(score, part_index, staff_index, measure_index)
                {
                    if event.note.is_rest
                        || event.note.is_unpitched
                        || event.end - event.start <= EPSILON
                        || event.end > bar_length + EPSILON
                        || !measure_start.is_finite()
                    {
                        continue;
                    }
                    let Some(transpose) = sounding_transpose(score, &event.address) else {
                        continue;
                    };
                    for (pitch_index, pitch) in event.note.pitches.iter().enumerate() {
                        if event.note.pitch_tie_start(pitch_index) {
                            let cents = pitch.to_midi_cents() + i32::from(transpose) * 100;
                            index
                                .entry((part_index, cents))
                                .or_default()
                                .push(TieStartCandidate {
                                    address: event.address.clone(),
                                    within_measure_start: event.start,
                                    absolute_end: measure_start + event.end,
                                });
                        }
                    }
                }
            }
        }
    }
    index
}

/// A continuation needs a matching tie start in the same part at the same
/// absolute beat and sounding pitch. It may cross voices or staves. If the
/// source explicitly identifies its predecessor, that location must match.
fn is_tied_continuation(
    score: &Score,
    event: &WrittenEvent<'_>,
    pitch_index: usize,
    starts: &[Vec<Vec<f64>>],
    tie_starts: &TieStartIndex,
) -> bool {
    const EPSILON: f64 = 1e-7;
    if !event.note.pitch_tie_end(pitch_index) {
        return false;
    }
    let Some(pitch) = event.note.pitches.get(pitch_index) else {
        return false;
    };
    let Some(transpose) = sounding_transpose(score, &event.address) else {
        return false;
    };
    let Some(&measure_start) = starts
        .get(event.address.part)
        .and_then(|part| part.get(event.address.staff))
        .and_then(|staff| staff.get(event.address.measure))
    else {
        return false;
    };
    let absolute_start = measure_start + event.start;
    if !absolute_start.is_finite() {
        return false;
    }
    let cents = pitch.to_midi_cents() + i32::from(transpose) * 100;
    let explicit_source = event
        .note
        .pitch_tie_sources
        .get(pitch_index)
        .and_then(Option::as_ref);
    tie_starts
        .get(&(event.address.part, cents))
        .is_some_and(|candidates| {
            candidates.iter().any(|candidate| {
                (candidate.absolute_end - absolute_start).abs() < EPSILON
                    && explicit_source.is_none_or(|source| {
                        source.matches(
                            &event.address,
                            &candidate.address,
                            candidate.within_measure_start - event.start,
                        )
                    })
            })
        })
}

fn has_new_pitched_attack(
    score: &Score,
    event: &WrittenEvent<'_>,
    starts: &[Vec<Vec<f64>>],
    tie_starts: &TieStartIndex,
) -> bool {
    event
        .note
        .pitches
        .iter()
        .enumerate()
        .any(|(index, _)| !is_tied_continuation(score, event, index, starts, tie_starts))
}

/// Use the same written-to-sounding semitone offsets as playback. The
/// original spelling is retained when there is no transposition; otherwise
/// enharmonic spelling follows the concert key used for this analysis pass.
fn concert_pitch(
    score: &Score,
    event: &WrittenEvent<'_>,
    pitch: &Pitch,
    key: &KeySignature,
) -> Option<Pitch> {
    let semitones = sounding_transpose(score, &event.address)?;
    if semitones == 0 {
        return Some(pitch.clone());
    }
    let base =
        (pitch.to_midi_cents() - i32::from(pitch.microtone_cents)) / 100 + i32::from(semitones);
    let step_index = match pitch.step {
        Step::C => 0,
        Step::D => 1,
        Step::E => 2,
        Step::F => 3,
        Step::G => 4,
        Step::A => 5,
        Step::B => 6,
    };
    // Diatonic interval for the chromatic shift. For example, -2 is a major
    // second down and -9 is a major sixth down, preserving C -> Eb for an
    // E-flat instrument even when the global key has no flats.
    const DIATONIC_STEPS: [i32; 12] = [0, 0, 1, 2, 2, 3, 3, 4, 5, 5, 6, 6];
    let shift =
        semitones.div_euclid(12) as i32 * 7 + DIATONIC_STEPS[semitones.rem_euclid(12) as usize];
    let diatonic = i32::from(pitch.octave) * 7 + step_index + shift;
    let step = match diatonic.rem_euclid(7) {
        0 => Step::C,
        1 => Step::D,
        2 => Step::E,
        3 => Step::F,
        4 => Step::G,
        5 => Step::A,
        _ => Step::B,
    };
    let octave = diatonic.div_euclid(7);
    let alter = base - (octave + 1) * 12 - i32::from(step.to_semitone());
    if let (Ok(octave), Ok(alter)) = (i8::try_from(octave), i8::try_from(alter))
        && (-2..=2).contains(&alter)
    {
        return Pitch::try_with_microtone(step, octave, alter, pitch.microtone_cents);
    }
    let mut result = Pitch::from_midi((60 + base.rem_euclid(12)) as u8, key.fifths < 0);
    let octave =
        (base - i32::from(result.step.to_semitone()) - i32::from(result.alter)).div_euclid(12) - 1;
    result.octave = i8::try_from(octave).ok()?;
    result.microtone_cents = pitch.microtone_cents;
    Some(result)
}

/// Infer chords from simultaneous sounding notes, retaining authored chord symbols.
///
/// This opt-in pass groups all parts, staves and voices at each written pitched
/// onset. It avoids treating consecutive notes of a melody as one chord. It does
/// not expand repeats or infer a harmonic rhythm where no note starts. Unequal
/// bar partitions are aligned by absolute written beats when every staff has
/// the same validated total length. Roman numerals use the concert key only
/// when all active contributing staves agree, or when the caller supplies one.
/// Output positions use the first available staff's bars.
pub fn analyze_vertical_sonorities(score: &Score) -> Vec<VerticalSonority> {
    analyze_vertical_sonorities_report(score).sonorities
}

pub fn analyze_vertical_sonorities_report(score: &Score) -> VerticalAnalysisReport {
    analyze_vertical_sonorities_with_key(score, None)
}

/// Infer vertical chords in an independently chosen global key.
///
/// Use this when a corpus has a known or separately estimated global key;
/// local key-signature changes in the score are ignored for this pass.
pub fn analyze_vertical_sonorities_in_key(
    score: &Score,
    key: &KeySignature,
) -> Vec<VerticalSonority> {
    analyze_vertical_sonorities_in_key_report(score, key).sonorities
}

pub fn analyze_vertical_sonorities_in_key_report(
    score: &Score,
    key: &KeySignature,
) -> VerticalAnalysisReport {
    analyze_vertical_sonorities_with_key(score, Some(key))
}

/// Add conservative opt-in chords when notes within a bar arpeggiate a complete
/// tertian chord. A bar start uses its first half, and a later onset may use
/// notes up to the next detected sonority within half a beat. A three-note
/// melody is never promoted by the bar-start rule. Evidence includes the later
/// notes used for each inference.
pub fn analyze_arpeggiated_bars_report(score: &Score) -> VerticalAnalysisReport {
    analyze_arpeggiated_bars_with_key(score, None)
}

/// As [`analyze_arpeggiated_bars_report`], using a caller-supplied concert key.
pub fn analyze_arpeggiated_bars_in_key_report(
    score: &Score,
    key: &KeySignature,
) -> VerticalAnalysisReport {
    analyze_arpeggiated_bars_with_key(score, Some(key))
}

/// Find conservative dominant-to-tonic resolutions from opt-in vertical chords.
///
/// A candidate requires a preceding un-applied dominant, a tonic chord, a
/// sounding bass pitch class to rise by five semitones modulo 12, and the
/// outer voices to finish in the same pitch class. This identifies a supported
/// authentic-cadence *candidate*; it does not distinguish perfect from
/// imperfect authentic cadence or assert a phrase ending.
pub fn analyze_vertical_cadences_report(score: &Score) -> VerticalCadenceReport {
    analyze_vertical_cadences_with_key(score, None)
}

/// As [`analyze_vertical_cadences_report`], with a caller-supplied concert key.
pub fn analyze_vertical_cadences_in_key_report(
    score: &Score,
    key: &KeySignature,
) -> VerticalCadenceReport {
    analyze_vertical_cadences_with_key(score, Some(key))
}

fn is_plain_roman(roman: &str, numeral: &str) -> bool {
    if roman.contains('/') {
        return false;
    }
    roman.strip_prefix(numeral).is_some_and(|suffix| {
        suffix.is_empty() || suffix.starts_with(|character: char| character.is_ascii_digit())
    })
}

fn active_outer_sounding_semitones(score: &Score, measure: usize, beat: f64) -> Option<(i32, i32)> {
    const EPSILON: f64 = 1e-7;
    let mut bass = None;
    let mut top = None;
    for event in written_measure_events(score, measure) {
        if event.start > beat + EPSILON
            || event.end <= beat + EPSILON
            || event.note.is_rest
            || event.note.is_unpitched
        {
            continue;
        }
        let transpose = i32::from(sounding_transpose(score, &event.address)?);
        for pitch in &event.note.pitches {
            let cents = pitch.to_midi_cents() + transpose * 100;
            if cents.rem_euclid(100) != 0 {
                return None;
            }
            let semitone = cents / 100;
            bass = Some(bass.map_or(semitone, |low: i32| low.min(semitone)));
            top = Some(top.map_or(semitone, |high: i32| high.max(semitone)));
        }
    }
    Some((bass?, top?))
}

fn analyze_vertical_cadences_with_key(
    score: &Score,
    key_override: Option<&KeySignature>,
) -> VerticalCadenceReport {
    let report = analyze_arpeggiated_bars_with_key(score, key_override);
    let mut result = VerticalCadenceReport {
        cadence_candidates: Vec::new(),
        timeline_issues: report.timeline_issues,
    };
    // The outer-pitch helper uses each staff's physical measure index.
    if valid_alternate_bar_partition(score, &vertical_measure_starts(score)).is_some() {
        return result;
    }
    let mut sonorities: Vec<_> = report.sonorities.iter().collect();
    sonorities.sort_by(|left, right| {
        left.measure
            .cmp(&right.measure)
            .then_with(|| left.beat.total_cmp(&right.beat))
    });
    for (index, current) in sonorities.iter().enumerate() {
        if result
            .timeline_issues
            .iter()
            .any(|issue| issue.measure == current.measure)
        {
            continue;
        }
        let Some(roman) = current.label.roman_numeral.as_deref() else {
            continue;
        };
        if !is_plain_roman(roman, "I") {
            continue;
        }
        let previous = sonorities[index.saturating_sub(8)..index]
            .iter()
            .rev()
            .find(|previous| {
                current.measure.saturating_sub(previous.measure) <= 1
                    && previous
                        .label
                        .roman_numeral
                        .as_deref()
                        .is_some_and(|label| label != roman)
            });
        let Some(previous) = previous else {
            continue;
        };
        if result
            .timeline_issues
            .iter()
            .any(|issue| issue.measure == previous.measure)
            || !previous
                .label
                .roman_numeral
                .as_deref()
                .is_some_and(|label| is_plain_roman(label, "V"))
        {
            continue;
        }
        let Some((previous_bass, _)) =
            active_outer_sounding_semitones(score, previous.measure, previous.beat)
        else {
            continue;
        };
        let Some((current_bass, current_top)) =
            active_outer_sounding_semitones(score, current.measure, current.beat)
        else {
            continue;
        };
        if (current_bass - previous_bass).rem_euclid(12) != 5
            || (current_top - current_bass).rem_euclid(12) != 0
        {
            continue;
        }
        let mut evidence = previous.label.evidence.clone();
        for address in &current.label.evidence {
            if !evidence.contains(address) {
                evidence.push(address.clone());
            }
        }
        result.cadence_candidates.push(CadenceCandidate {
            from: previous.label.address.clone(),
            to: current.label.address.clone(),
            kind: CadenceKind::Authentic,
            confidence: if current.beat.abs() < 1e-7 { 70 } else { 55 },
            rule_id: "vertical-dominant-tonic-outer-voices".to_string(),
            evidence,
        });
    }
    result
}

fn analyze_arpeggiated_bars_with_key(
    score: &Score,
    key_override: Option<&KeySignature>,
) -> VerticalAnalysisReport {
    const EPSILON: f64 = 1e-7;
    let mut report = analyze_vertical_sonorities_with_key(score, key_override);
    let starts = vertical_measure_starts(score);
    // A physical bar in one staff must not be mistaken for the same time span
    // in another staff when their bar partitions differ.
    if valid_alternate_bar_partition(score, &starts).is_some() {
        return report;
    }
    let tie_starts = tie_start_index(score, &starts);
    let Some(reference) = score.parts.first().and_then(|part| part.staves.first()) else {
        return report;
    };
    for measure_index in 0..reference.measures.len() {
        if report
            .timeline_issues
            .iter()
            .any(|issue| issue.measure == measure_index)
            || report
                .sonorities
                .iter()
                .any(|item| item.measure == measure_index && item.beat.abs() < EPSILON)
        {
            continue;
        }
        let length = reference.measure_beats(measure_index, &score.settings.time_signature);
        if !length.is_finite() || length <= 0.0 {
            continue;
        }
        let window = (length / 2.0).min(2.0);
        let written = written_measure_events(score, measure_index);
        let opening: Vec<_> = written
            .iter()
            .filter(|event| {
                !event.note.is_rest
                    && !event.note.is_unpitched
                    && !event.note.pitches.is_empty()
                    && event.end > event.start + EPSILON
                    && event.start < window - EPSILON
            })
            .collect();
        if opening.len() < 4
            || !opening.iter().any(|event| {
                event.start.abs() < EPSILON
                    && has_new_pitched_attack(score, event, &starts, &tie_starts)
            })
            || opening
                .iter()
                .any(|event| event.start > EPSILON && event.note.chord_symbol.is_some())
        {
            continue;
        }
        let resolved_key = key_override
            .cloned()
            .or_else(|| agreed_concert_key_at_measure(score, measure_index));
        let spelling_key = resolved_key
            .as_ref()
            .unwrap_or(&score.settings.key_signature);
        let Some(pitches): Option<Vec<_>> = opening
            .iter()
            .flat_map(|event| {
                event
                    .note
                    .pitches
                    .iter()
                    .map(|pitch| concert_pitch(score, event, pitch, spelling_key))
            })
            .collect()
        else {
            continue;
        };
        let Some(chord) = detect_chord(&pitches) else {
            continue;
        };
        if !matches!(
            chord.kind.as_str(),
            "major"
                | "minor"
                | "diminished"
                | "augmented"
                | "dominant"
                | "major-seventh"
                | "minor-seventh"
                | "diminished-seventh"
                | "half-diminished"
        ) {
            continue;
        }
        let Some(anchor) = opening.iter().find(|event| event.start.abs() < EPSILON) else {
            continue;
        };
        report.sonorities.push(VerticalSonority {
            measure: measure_index,
            beat: 0.0,
            label: ChordLabel {
                address: anchor.address.clone(),
                name: chord_name(&chord),
                roman_numeral: resolved_key
                    .as_ref()
                    .and_then(|key| roman_numeral_figured(&chord, key)),
                chord,
                confidence: 55,
                rule_id: "bar-arpeggio-template".to_string(),
                evidence: opening.iter().map(|event| event.address.clone()).collect(),
            },
        });
    }
    // A note that begins shortly before an identified vertical chord can be
    // its arpeggiated opening. Complete only when the intervening pitches form
    // an exact tertian template; a chromatic approach note rejects it.
    for measure_index in 0..reference.measures.len() {
        if report
            .timeline_issues
            .iter()
            .any(|issue| issue.measure == measure_index)
        {
            continue;
        }
        let written = written_measure_events(score, measure_index);
        let known: Vec<_> = report
            .sonorities
            .iter()
            .filter(|item| item.measure == measure_index)
            .cloned()
            .collect();
        let mut onsets: Vec<_> = written
            .iter()
            .filter(|event| {
                !event.note.is_rest
                    && !event.note.is_unpitched
                    && !event.note.pitches.is_empty()
                    && event.end > event.start + EPSILON
                    && has_new_pitched_attack(score, event, &starts, &tie_starts)
            })
            .map(|event| event.start)
            .collect();
        onsets.sort_by(f64::total_cmp);
        onsets.dedup_by(|left, right| (*left - *right).abs() < EPSILON);
        for onset in onsets {
            if known.iter().any(|item| (item.beat - onset).abs() < EPSILON) {
                continue;
            }
            let Some(next) = known
                .iter()
                .find(|item| item.beat > onset + EPSILON && item.beat <= onset + 0.5 + EPSILON)
            else {
                continue;
            };
            let opening: Vec<_> = written
                .iter()
                .filter(|event| {
                    !event.note.is_rest
                        && !event.note.is_unpitched
                        && !event.note.pitches.is_empty()
                        && event.end > onset + EPSILON
                        && event.start <= next.beat + EPSILON
                })
                .collect();
            let resolved_key = key_override
                .cloned()
                .or_else(|| agreed_concert_key_at_measure(score, measure_index));
            let spelling_key = resolved_key
                .as_ref()
                .unwrap_or(&score.settings.key_signature);
            let Some(pitches): Option<Vec<_>> = opening
                .iter()
                .flat_map(|event| {
                    event
                        .note
                        .pitches
                        .iter()
                        .map(|pitch| concert_pitch(score, event, pitch, spelling_key))
                })
                .collect()
            else {
                continue;
            };
            let Some(chord) = detect_chord(&pitches) else {
                continue;
            };
            if !matches!(
                chord.kind.as_str(),
                "major"
                    | "minor"
                    | "diminished"
                    | "augmented"
                    | "dominant"
                    | "major-seventh"
                    | "minor-seventh"
                    | "diminished-seventh"
                    | "half-diminished"
            ) {
                continue;
            }
            let Some(anchor) = opening.iter().find(|event| {
                (event.start - onset).abs() < EPSILON
                    && has_new_pitched_attack(score, event, &starts, &tie_starts)
            }) else {
                continue;
            };
            report.sonorities.push(VerticalSonority {
                measure: measure_index,
                beat: onset,
                label: ChordLabel {
                    address: anchor.address.clone(),
                    name: chord_name(&chord),
                    roman_numeral: resolved_key
                        .as_ref()
                        .and_then(|key| roman_numeral_figured(&chord, key)),
                    chord,
                    confidence: 45,
                    rule_id: "forward-completed-sonority".to_string(),
                    evidence: opening.iter().map(|event| event.address.clone()).collect(),
                },
            });
        }
    }
    report.sonorities.sort_by(|left, right| {
        left.measure
            .cmp(&right.measure)
            .then_with(|| left.beat.total_cmp(&right.beat))
    });
    report
}

struct AbsoluteWrittenEvent<'a> {
    start: f64,
    end: f64,
    written: WrittenEvent<'a>,
}

/// A different physical bar partition is usable only when every staff spans
/// the same finite score length and every local note fits its own bar.
fn valid_alternate_bar_partition(
    score: &Score,
    starts: &[Vec<Vec<f64>>],
) -> Option<(usize, usize)> {
    const EPSILON: f64 = 1e-7;
    let (reference_part, reference_staff, reference) = score
        .parts
        .iter()
        .enumerate()
        .flat_map(|(part_index, part)| {
            part.staves
                .iter()
                .enumerate()
                .map(move |(staff_index, staff)| (part_index, staff_index, staff))
        })
        .next()?;
    if reference.measures.is_empty() {
        return None;
    }
    let reference_starts = starts.get(reference_part)?.get(reference_staff)?;
    let reference_last = reference.measures.len() - 1;
    let reference_total = reference_starts.get(reference_last)?
        + reference.measure_beats(reference_last, &score.settings.time_signature);
    if !reference_total.is_finite() || reference_total <= 0.0 {
        return None;
    }
    let mut differs = false;
    for (part_index, part) in score.parts.iter().enumerate() {
        for (staff_index, staff) in part.staves.iter().enumerate() {
            let staff_starts = starts.get(part_index)?.get(staff_index)?;
            let last = staff.measures.len().checked_sub(1)?;
            let total =
                staff_starts.get(last)? + staff.measure_beats(last, &score.settings.time_signature);
            if !total.is_finite() || (total - reference_total).abs() > EPSILON {
                return None;
            }
            differs |= staff.measures.len() != reference.measures.len();
            for (measure_index, measure) in staff.measures.iter().enumerate() {
                let start = *staff_starts.get(measure_index)?;
                let length = staff.measure_beats(measure_index, &score.settings.time_signature);
                if !start.is_finite()
                    || !length.is_finite()
                    || length <= 0.0
                    || measure
                        .actual_length
                        .is_some_and(|actual| actual.beats().is_none())
                {
                    return None;
                }
                if let Some(reference_start) = reference_starts.get(measure_index) {
                    differs |= (start - reference_start).abs() > EPSILON;
                    differs |= (length
                        - reference.measure_beats(measure_index, &score.settings.time_signature))
                    .abs()
                        > EPSILON;
                }
                for voice in &measure.voices {
                    let mut elapsed = 0.0;
                    for note in voice {
                        let duration = if voice.len() == 1 && note.is_plain_whole_rest() {
                            length
                        } else {
                            note.beats()
                        };
                        if !duration.is_finite() || duration < 0.0 {
                            return None;
                        }
                        elapsed += duration;
                    }
                    if elapsed > length + EPSILON {
                        return None;
                    }
                }
            }
        }
    }
    differs.then_some((reference_part, reference_staff))
}

fn analyze_vertical_sonorities_on_absolute_timeline(
    score: &Score,
    starts: &[Vec<Vec<f64>>],
    tie_starts: &TieStartIndex,
    reference_part: usize,
    reference_staff: usize,
    key_override: Option<&KeySignature>,
) -> VerticalAnalysisReport {
    const EPSILON: f64 = 1e-7;
    let reference = &score.parts[reference_part].staves[reference_staff];
    let reference_starts = &starts[reference_part][reference_staff];
    let mut events = Vec::new();
    for (part_index, part) in score.parts.iter().enumerate() {
        for (staff_index, staff) in part.staves.iter().enumerate() {
            for (measure_index, &measure_start) in starts[part_index][staff_index]
                .iter()
                .enumerate()
                .take(staff.measures.len())
            {
                for written in
                    written_staff_measure_events(score, part_index, staff_index, measure_index)
                {
                    events.push(AbsoluteWrittenEvent {
                        start: measure_start + written.start,
                        end: measure_start + written.end,
                        written,
                    });
                }
            }
        }
    }
    let mut key = key_override
        .cloned()
        .unwrap_or_else(|| score.settings.key_signature.clone());
    let mut result = Vec::new();
    let mut timeline_issues = Vec::new();
    for (measure_index, measure) in reference.measures.iter().enumerate() {
        let bar_start = reference_starts[measure_index];
        let bar_end =
            bar_start + reference.measure_beats(measure_index, &score.settings.time_signature);
        if key_override.is_none()
            && let Some(next_key) = &measure.key_sig
        {
            key = next_key.clone();
        }
        let window: Vec<_> = events
            .iter()
            .filter(|event| {
                (event.start < bar_end - EPSILON && event.end > bar_start + EPSILON)
                    || (event.start >= bar_start - EPSILON
                        && event.start < bar_end - EPSILON
                        && event.written.note.chord_symbol.is_some())
            })
            .collect();
        let mut invalid = false;
        for event in &window {
            if event.written.note.is_rest || event.written.note.is_unpitched {
                continue;
            }
            if event
                .written
                .note
                .pitches
                .iter()
                .any(|pitch| concert_pitch(score, &event.written, pitch, &key).is_none())
            {
                timeline_issues.push(VerticalTimelineIssue {
                    measure: event.written.address.measure,
                    part: event.written.address.part,
                    staff: event.written.address.staff,
                    kind: VerticalTimelineIssueKind::UnrepresentableConcertPitch,
                });
                invalid = true;
            }
        }
        if invalid {
            continue;
        }
        let mut authored = Vec::new();
        let mut sounding = Vec::new();
        let mut onsets = Vec::new();
        for event in &window {
            if event.start >= bar_start - EPSILON
                && let Some(chord) = &event.written.note.chord_symbol
            {
                authored.push((event.start, event.written.address.clone(), chord));
                onsets.push(event.start);
            }
            if !event.written.note.is_rest
                && !event.written.note.is_unpitched
                && !event.written.note.pitches.is_empty()
                && event.end - event.start > EPSILON
            {
                sounding.push(*event);
                if event.start >= bar_start - EPSILON
                    && has_new_pitched_attack(score, &event.written, starts, tie_starts)
                {
                    onsets.push(event.start);
                }
            }
        }
        onsets.sort_by(f64::total_cmp);
        onsets.dedup_by(|left, right| (*left - *right).abs() < EPSILON);
        for onset in onsets {
            let authored_here: Vec<_> = authored
                .iter()
                .filter(|(start, _, _)| (*start - onset).abs() < EPSILON)
                .collect();
            let active: Vec<_> = sounding
                .iter()
                .filter(|event| event.start <= onset + EPSILON && event.end > onset + EPSILON)
                .collect();
            let resolved_key = key_override.cloned().or_else(|| {
                agreed_concert_key_for_addresses(
                    score,
                    active
                        .iter()
                        .map(|event| &event.written.address)
                        .chain(authored_here.iter().map(|entry| &entry.1)),
                )
            });
            if !authored_here.is_empty() {
                result.extend(authored_here.into_iter().map(|(_, address, chord)| {
                    VerticalSonority {
                        measure: measure_index,
                        beat: onset - bar_start,
                        label: ChordLabel {
                            address: address.clone(),
                            chord: (*chord).clone(),
                            name: chord_name(chord),
                            roman_numeral: resolved_key
                                .as_ref()
                                .filter(|_| sounding_transpose(score, address) == Some(0))
                                .and_then(|key| roman_numeral_figured(chord, key)),
                            confidence: 100,
                            rule_id: "authored-chord-symbol".to_string(),
                            evidence: vec![address.clone()],
                        },
                    }
                }));
                continue;
            }
            let Some(pitches): Option<Vec<_>> = active
                .iter()
                .flat_map(|event| {
                    event
                        .written
                        .note
                        .pitches
                        .iter()
                        .map(|pitch| concert_pitch(score, &event.written, pitch, &key))
                })
                .collect()
            else {
                continue;
            };
            let Some(chord) = detect_chord(&pitches) else {
                continue;
            };
            let Some(anchor) = active.iter().find(|event| {
                (event.start - onset).abs() < EPSILON
                    && has_new_pitched_attack(score, &event.written, starts, tie_starts)
            }) else {
                continue;
            };
            result.push(VerticalSonority {
                measure: measure_index,
                beat: onset - bar_start,
                label: ChordLabel {
                    address: anchor.written.address.clone(),
                    name: chord_name(&chord),
                    roman_numeral: resolved_key
                        .as_ref()
                        .and_then(|key| roman_numeral_figured(&chord, key)),
                    chord,
                    confidence: 70,
                    rule_id: "vertical-sonority-template".to_string(),
                    evidence: active
                        .iter()
                        .map(|event| event.written.address.clone())
                        .collect(),
                },
            });
        }
    }
    VerticalAnalysisReport {
        sonorities: result,
        timeline_issues,
    }
}

fn analyze_vertical_sonorities_with_key(
    score: &Score,
    key_override: Option<&KeySignature>,
) -> VerticalAnalysisReport {
    const EPSILON: f64 = 1e-7;
    let measure_count = score
        .parts
        .iter()
        .flat_map(|part| part.staves.iter())
        .map(|staff| staff.measures.len())
        .max()
        .unwrap_or(0);
    let mut key = key_override
        .cloned()
        .unwrap_or_else(|| score.settings.key_signature.clone());
    let mut result = Vec::new();
    let mut timeline_issues = Vec::new();
    let starts = vertical_measure_starts(score);
    let tie_starts = tie_start_index(score, &starts);
    if let Some((reference_part, reference_staff)) = valid_alternate_bar_partition(score, &starts) {
        return analyze_vertical_sonorities_on_absolute_timeline(
            score,
            &starts,
            &tie_starts,
            reference_part,
            reference_staff,
            key_override,
        );
    }

    for measure_index in 0..measure_count {
        let issues = vertical_measure_alignment_issues(score, &starts, measure_index);
        if !issues.is_empty() {
            timeline_issues.extend(issues);
            continue;
        }
        if key_override.is_none()
            && let Some(next_key) = score
                .parts
                .iter()
                .flat_map(|part| part.staves.iter())
                .filter_map(|staff| staff.measures.get(measure_index))
                .find_map(|measure| measure.key_sig.as_ref())
        {
            key = next_key.clone();
        }
        let resolved_key = key_override
            .cloned()
            .or_else(|| agreed_concert_key_at_measure(score, measure_index));
        let spelling_key = resolved_key.as_ref().unwrap_or(&key);
        let written = written_measure_events(score, measure_index);
        let mut unrepresentable = Vec::new();
        for event in &written {
            if event.note.is_rest || event.note.is_unpitched {
                continue;
            }
            for pitch in &event.note.pitches {
                if concert_pitch(score, event, pitch, spelling_key).is_none() {
                    unrepresentable.push(VerticalTimelineIssue {
                        measure: measure_index,
                        part: event.address.part,
                        staff: event.address.staff,
                        kind: VerticalTimelineIssueKind::UnrepresentableConcertPitch,
                    });
                }
            }
        }
        if !unrepresentable.is_empty() {
            timeline_issues.extend(unrepresentable);
            continue;
        }
        let mut sounding = Vec::new();
        let mut authored = Vec::new();
        let mut onsets = Vec::new();
        for event in &written {
            if let Some(chord) = &event.note.chord_symbol {
                let authored_key = resolved_key
                    .as_ref()
                    .filter(|_| sounding_transpose(score, &event.address) == Some(0));
                authored.push((
                    event.start,
                    ChordLabel {
                        address: event.address.clone(),
                        chord: chord.clone(),
                        name: chord_name(chord),
                        roman_numeral: authored_key
                            .and_then(|key| roman_numeral_figured(chord, key)),
                        confidence: 100,
                        rule_id: "authored-chord-symbol".to_string(),
                        evidence: vec![event.address.clone()],
                    },
                ));
                onsets.push(event.start);
            }
            if !event.note.is_rest
                && !event.note.is_unpitched
                && !event.note.pitches.is_empty()
                && event.end - event.start > EPSILON
            {
                sounding.push(event);
                if has_new_pitched_attack(score, event, &starts, &tie_starts) {
                    onsets.push(event.start);
                }
            }
        }
        onsets.sort_by(f64::total_cmp);
        onsets.dedup_by(|left, right| (*left - *right).abs() < EPSILON);
        for beat in onsets {
            let authored_here: Vec<_> = authored
                .iter()
                .filter(|(onset, _)| (*onset - beat).abs() < EPSILON)
                .collect();
            if !authored_here.is_empty() {
                result.extend(
                    authored_here
                        .into_iter()
                        .map(|(_, label)| VerticalSonority {
                            measure: measure_index,
                            beat,
                            label: label.clone(),
                        }),
                );
                continue;
            }
            let active: Vec<_> = sounding
                .iter()
                .filter(|event| event.start <= beat + EPSILON && event.end > beat + EPSILON)
                .collect();
            let Some(pitches): Option<Vec<_>> = active
                .iter()
                .flat_map(|event| {
                    event
                        .note
                        .pitches
                        .iter()
                        .map(|pitch| concert_pitch(score, event, pitch, spelling_key))
                })
                .collect()
            else {
                continue;
            };
            let Some(chord) = detect_chord(&pitches) else {
                continue;
            };
            let Some(anchor) = active.iter().find(|event| {
                (event.start - beat).abs() < EPSILON
                    && has_new_pitched_attack(score, event, &starts, &tie_starts)
            }) else {
                continue;
            };
            result.push(VerticalSonority {
                measure: measure_index,
                beat,
                label: ChordLabel {
                    address: anchor.address.clone(),
                    name: chord_name(&chord),
                    roman_numeral: resolved_key
                        .as_ref()
                        .and_then(|key| roman_numeral_figured(&chord, key)),
                    chord,
                    confidence: 70,
                    rule_id: "vertical-sonority-template".to_string(),
                    evidence: active.iter().map(|event| event.address.clone()).collect(),
                },
            });
        }
    }
    VerticalAnalysisReport {
        sonorities: result,
        timeline_issues,
    }
}

/// Analyze chord labels only inside an optional part/staff measure region.
pub fn analyze_chords_in_region(score: &Score, region: Option<&AnalysisRegion>) -> Vec<ChordLabel> {
    let mut chords = Vec::new();
    for (part_index, part) in score.parts.iter().enumerate() {
        for (staff_index, staff) in part.staves.iter().enumerate() {
            for (measure_index, measure) in staff.measures.iter().enumerate() {
                if let Some(region) = region
                    && (part_index != region.part
                        || staff_index != region.staff
                        || measure_index < region.start_measure
                        || measure_index >= region.end_measure)
                {
                    continue;
                }
                let key = measure
                    .key_sig
                    .as_ref()
                    .unwrap_or(&score.settings.key_signature);
                for (voice_index, voice) in measure.voices.iter().enumerate() {
                    let authored: Vec<_> = voice
                        .iter()
                        .enumerate()
                        .filter_map(|(note_index, note)| {
                            note.chord_symbol.as_ref().map(|chord| (note_index, chord))
                        })
                        .collect();
                    if !authored.is_empty() {
                        for (note_index, chord) in authored {
                            let address = NoteAddr {
                                part: part_index,
                                staff: staff_index,
                                measure: measure_index,
                                voice: voice_index,
                                note: note_index,
                            };
                            chords.push(ChordLabel {
                                address: address.clone(),
                                name: chord_name(chord),
                                roman_numeral: roman_numeral(chord, key),
                                chord: chord.clone(),
                                confidence: 100,
                                rule_id: "authored-chord-symbol".to_string(),
                                evidence: vec![address],
                            });
                        }
                        continue;
                    }
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
                        name: chord_name(&chord),
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
    chords
}

/// Recompute only the requested categories and carry forward the remaining categories.
///
/// The caller must provide a result from the immediately preceding score state. Categories with
/// dependencies are expanded conservatively: cadence analysis refreshes chords, and SATB
/// diagnostics refresh voice-leading first. This is intended for editor-local incremental work.
pub fn analyze_selected_categories(
    score: &Score,
    previous: &AnalysisResult,
    categories: &[AnalysisCategory],
) -> AnalysisResult {
    analyze_selected_categories_in_region(score, previous, categories, None)
}

/// Recompute selected categories with an optional bounded region for local passes.
pub fn analyze_selected_categories_in_region(
    score: &Score,
    previous: &AnalysisResult,
    categories: &[AnalysisCategory],
    region: Option<&AnalysisRegion>,
) -> AnalysisResult {
    let selected = |category| categories.contains(&category);
    let chords =
        if selected(AnalysisCategory::Chords) || selected(AnalysisCategory::CadenceCandidates) {
            let refreshed = analyze_chords_in_region(score, region);
            merge_chord_region(&previous.chords, refreshed, region)
        } else {
            previous.chords.clone()
        };
    let intervals = if selected(AnalysisCategory::Intervals) {
        merge_interval_region(
            &previous.intervals,
            analyze_intervals_in_region(score, region),
            region,
        )
    } else {
        previous.intervals.clone()
    };
    let key_estimates = if selected(AnalysisCategory::KeyEstimates) {
        estimate_keys(score)
    } else {
        previous.key_estimates.clone()
    };
    let cadence_candidates = if selected(AnalysisCategory::CadenceCandidates) {
        analyze_cadences(&chords)
    } else {
        previous.cadence_candidates.clone()
    };
    let voice_leading = if selected(AnalysisCategory::VoiceLeading)
        || selected(AnalysisCategory::SatbDiagnostics)
    {
        merge_voice_leading_region(
            &previous.voice_leading,
            analyze_voice_leading_in_region(score, region),
            region,
        )
    } else {
        previous.voice_leading.clone()
    };
    let satb_diagnostics = if selected(AnalysisCategory::SatbDiagnostics) {
        merge_satb_region(
            &previous.satb_diagnostics,
            analyze_satb_in_region(score, &voice_leading, region),
            region,
        )
    } else {
        previous.satb_diagnostics.clone()
    };
    let motifs = if selected(AnalysisCategory::Motifs) {
        analyze_motifs(score)
    } else {
        previous.motifs.clone()
    };
    let phrase_boundaries = if selected(AnalysisCategory::PhraseBoundaries) {
        analyze_phrase_boundaries(score)
    } else {
        previous.phrase_boundaries.clone()
    };
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

fn merge_interval_region(
    previous: &[IntervalObservation],
    refreshed: Vec<IntervalObservation>,
    region: Option<&AnalysisRegion>,
) -> Vec<IntervalObservation> {
    let Some(region) = region else {
        return refreshed;
    };
    let mut merged: Vec<_> = previous
        .iter()
        .filter(|item| {
            !analysis_region_contains(region, &item.from)
                && !analysis_region_contains(region, &item.to)
        })
        .cloned()
        .collect();
    merged.extend(refreshed);
    merged.sort_by_key(|item| {
        (
            item.from.part,
            item.from.staff,
            item.from.measure,
            item.from.voice,
            item.from.note,
            item.to.measure,
            item.to.note,
        )
    });
    merged
}

fn merge_voice_leading_region(
    previous: &[VoiceLeadingObservation],
    refreshed: Vec<VoiceLeadingObservation>,
    region: Option<&AnalysisRegion>,
) -> Vec<VoiceLeadingObservation> {
    let Some(region) = region else {
        return refreshed;
    };
    let mut merged: Vec<_> = previous
        .iter()
        .filter(|item| {
            !analysis_region_contains(region, &item.upper)
                && !analysis_region_contains(region, &item.lower)
        })
        .cloned()
        .collect();
    merged.extend(refreshed);
    merged.sort_by_key(|item| {
        (
            item.upper.part,
            item.upper.staff,
            item.upper.measure,
            item.upper.voice,
            item.upper.note,
            item.lower.voice,
        )
    });
    merged
}

fn merge_chord_region(
    previous: &[ChordLabel],
    refreshed: Vec<ChordLabel>,
    region: Option<&AnalysisRegion>,
) -> Vec<ChordLabel> {
    let Some(region) = region else {
        return refreshed;
    };
    let mut merged: Vec<_> = previous
        .iter()
        .filter(|label| !analysis_region_contains(region, &label.address))
        .cloned()
        .collect();
    merged.extend(refreshed);
    merged.sort_by_key(|label| {
        (
            label.address.part,
            label.address.staff,
            label.address.measure,
            label.address.voice,
            label.address.note,
        )
    });
    merged
}

fn merge_satb_region(
    previous: &[SatbDiagnostic],
    refreshed: Vec<SatbDiagnostic>,
    region: Option<&AnalysisRegion>,
) -> Vec<SatbDiagnostic> {
    let Some(region) = region else {
        return refreshed;
    };
    let mut merged: Vec<_> = previous
        .iter()
        .filter(|diagnostic| {
            !diagnostic
                .evidence
                .iter()
                .any(|address| analysis_region_contains(region, address))
        })
        .cloned()
        .collect();
    merged.extend(refreshed);
    merged.sort_by_key(|diagnostic| {
        (
            diagnostic.upper.part,
            diagnostic.upper.staff,
            diagnostic.upper.measure,
            diagnostic.upper.voice,
            diagnostic.upper.note,
            diagnostic.lower.voice,
            diagnostic.rule_id.clone(),
        )
    });
    merged
}

fn analysis_region_contains(region: &AnalysisRegion, address: &NoteAddr) -> bool {
    address.part == region.part
        && address.staff == region.staff
        && (region.start_measure..region.end_measure).contains(&address.measure)
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

/// Errors returned when configuring the bounded analysis cache.
#[derive(Debug, Error, Clone, PartialEq, Eq)]
pub enum AnalysisCacheError {
    #[error("analysis cache capacity must be greater than zero")]
    ZeroCapacity,
}

/// Counters for measuring analysis-cache reuse.
#[derive(Debug, Clone, Copy, Default, PartialEq, Eq, Serialize, Deserialize)]
pub struct AnalysisCacheStats {
    pub hits: usize,
    pub misses: usize,
}

/// A deterministic, bounded cache for complete analysis results.
///
/// Entries are keyed by the schema-versioned canonical score fingerprint. A score edit therefore
/// naturally misses the old entry without requiring callers to diff JSON or manually invalidate
/// every analysis category. Eviction is insertion-order based rather than hash-map iteration
/// based, keeping behavior reproducible across hosts.
#[derive(Debug, Clone)]
pub struct AnalysisCache {
    capacity: usize,
    entries: BTreeMap<String, AnalysisResult>,
    insertion_order: VecDeque<String>,
    stats: AnalysisCacheStats,
}

impl Default for AnalysisCache {
    fn default() -> Self {
        Self {
            capacity: 16,
            entries: BTreeMap::new(),
            insertion_order: VecDeque::new(),
            stats: AnalysisCacheStats::default(),
        }
    }
}

impl AnalysisCache {
    /// Create a cache with a fixed maximum number of score results.
    pub fn with_capacity(capacity: usize) -> Result<Self, AnalysisCacheError> {
        if capacity == 0 {
            return Err(AnalysisCacheError::ZeroCapacity);
        }
        Ok(Self {
            capacity,
            entries: BTreeMap::new(),
            insertion_order: VecDeque::new(),
            stats: AnalysisCacheStats::default(),
        })
    }

    /// Return the configured maximum number of cached results.
    pub fn capacity(&self) -> usize {
        self.capacity
    }

    /// Return hit/miss counters since construction or the last reset.
    pub fn stats(&self) -> AnalysisCacheStats {
        self.stats
    }

    /// Reset hit/miss counters without removing cached results.
    pub fn reset_stats(&mut self) {
        self.stats = AnalysisCacheStats::default();
    }

    /// Return a cached result for the supplied score, if its canonical content is present.
    pub fn get(&self, score: &Score) -> Option<&AnalysisResult> {
        self.entries.get(&analysis_cache_key(score))
    }

    /// Analyze a score once per cache key, returning a cloned result for ergonomic host use.
    pub fn analyze(&mut self, score: &Score) -> AnalysisResult {
        let key = analysis_cache_key(score);
        if let Some(result) = self.entries.get(&key) {
            self.stats.hits = self.stats.hits.saturating_add(1);
            return result.clone();
        }

        self.stats.misses = self.stats.misses.saturating_add(1);
        let result = analyze_score(score);
        self.insert(key, result.clone());
        result
    }

    /// Analyze scores in input order while reusing results already present in the cache.
    pub fn analyze_batch(&mut self, scores: &[Score]) -> Vec<AnalysisResult> {
        scores.iter().map(|score| self.analyze(score)).collect()
    }

    /// Reclaim a previous editor snapshot and analyze its replacement in one operation.
    pub fn analyze_after_edit(&mut self, previous: &Score, current: &Score) -> AnalysisResult {
        if analysis_cache_key(previous) != analysis_cache_key(current) {
            self.invalidate(previous);
        }
        self.analyze(current)
    }

    /// Re-analyze an edited score and return the category-level diff from the previous result.
    pub fn analyze_after_edit_with_diff(
        &mut self,
        previous_score: &Score,
        previous_result: &AnalysisResult,
        current: &Score,
    ) -> AnalysisEditResult {
        let analysis = self.analyze_after_edit(previous_score, current);
        let diff = diff_analysis(previous_result, &analysis);
        AnalysisEditResult { analysis, diff }
    }

    /// Recompute selected categories after an edit and cache the merged complete result.
    pub fn analyze_selected_after_edit(
        &mut self,
        previous_score: &Score,
        previous_result: &AnalysisResult,
        current: &Score,
        categories: &[AnalysisCategory],
    ) -> AnalysisResult {
        let key = analysis_cache_key(current);
        if let Some(result) = self.entries.get(&key) {
            self.stats.hits = self.stats.hits.saturating_add(1);
            return result.clone();
        }
        self.stats.misses = self.stats.misses.saturating_add(1);
        if analysis_cache_key(previous_score) != key {
            self.invalidate(previous_score);
        }
        let result = analyze_selected_categories(current, previous_result, categories);
        self.insert(key, result.clone());
        result
    }

    /// Recompute a refresh plan, using its region for local passes, and cache the merged result.
    pub fn analyze_after_edit_with_plan(
        &mut self,
        previous_score: &Score,
        previous_result: &AnalysisResult,
        current: &Score,
        plan: &AnalysisRefreshPlan,
    ) -> AnalysisEditResult {
        let categories: Vec<_> = plan
            .local_categories
            .iter()
            .chain(&plan.global_categories)
            .copied()
            .collect();
        let key = analysis_cache_key(current);
        let analysis = if let Some(result) = self.entries.get(&key) {
            self.stats.hits = self.stats.hits.saturating_add(1);
            result.clone()
        } else {
            self.stats.misses = self.stats.misses.saturating_add(1);
            if analysis_cache_key(previous_score) != key {
                self.invalidate(previous_score);
            }
            let result = analyze_selected_categories_in_region(
                current,
                previous_result,
                &categories,
                plan.region.as_ref(),
            );
            self.insert(key, result.clone());
            result
        };
        AnalysisEditResult {
            diff: diff_analysis(previous_result, &analysis),
            analysis,
        }
    }

    /// Apply a change hint, recompute its affected categories, and return the result diff.
    pub fn analyze_after_edit_with_hint(
        &mut self,
        previous_score: &Score,
        previous_result: &AnalysisResult,
        current: &Score,
        hint: &ChangeHint,
    ) -> AnalysisEditResult {
        let plan = analysis_refresh_plan(hint);
        self.analyze_after_edit_with_plan(previous_score, previous_result, current, &plan)
    }

    /// Insert a previously computed result and evict the oldest entry when the cache is full.
    pub fn insert(&mut self, key: String, result: AnalysisResult) {
        if self.entries.contains_key(&key) {
            self.entries.insert(key.clone(), result);
            self.insertion_order.retain(|existing| existing != &key);
        } else {
            self.entries.insert(key.clone(), result);
        }
        self.insertion_order.push_back(key);
        while self.entries.len() > self.capacity {
            if let Some(oldest) = self.insertion_order.pop_front() {
                self.entries.remove(&oldest);
            }
        }
    }

    /// Remove the cached result for a score and return whether an entry was removed.
    pub fn invalidate(&mut self, score: &Score) -> bool {
        let key = analysis_cache_key(score);
        let removed = self.entries.remove(&key).is_some();
        if removed {
            self.insertion_order.retain(|existing| existing != &key);
        }
        removed
    }

    /// Remove all cached results.
    pub fn clear(&mut self) {
        self.entries.clear();
        self.insertion_order.clear();
    }

    /// Return the number of cached results.
    pub fn len(&self) -> usize {
        self.entries.len()
    }

    /// Return whether the cache contains no results.
    pub fn is_empty(&self) -> bool {
        self.entries.is_empty()
    }
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
    analyze_satb_in_region(score, &voice_leading, None)
}

/// Analyze SATB diagnostics inside an optional part/staff measure region.
pub fn analyze_satb_in_region(
    score: &Score,
    voice_leading: &[VoiceLeadingObservation],
    region: Option<&AnalysisRegion>,
) -> Vec<SatbDiagnostic> {
    const EPSILON: f64 = 1e-7;
    let mut diagnostics = Vec::new();
    for (part_index, part) in score.parts.iter().enumerate() {
        for (staff_index, staff) in part.staves.iter().enumerate() {
            for (measure_index, measure) in staff.measures.iter().enumerate() {
                if let Some(region) = region
                    && (part_index != region.part
                        || staff_index != region.staff
                        || measure_index < region.start_measure
                        || measure_index >= region.end_measure)
                {
                    continue;
                }
                let written =
                    written_staff_measure_events(score, part_index, staff_index, measure_index);
                for upper_index in 0..measure.voices.len().saturating_sub(1) {
                    let pair: Vec<_> = written
                        .iter()
                        .filter(|event| {
                            event.address.part == part_index
                                && event.address.staff == staff_index
                                && (event.address.voice == upper_index
                                    || event.address.voice == upper_index + 1)
                        })
                        .collect();
                    let mut onsets: Vec<_> = pair.iter().map(|event| event.start).collect();
                    onsets.sort_by(f64::total_cmp);
                    onsets.dedup_by(|left, right| (*left - *right).abs() < EPSILON);
                    for beat in onsets {
                        let active = |voice_index: usize| {
                            pair.iter()
                                .find(|event| {
                                    event.address.voice == voice_index
                                        && event.start <= beat + EPSILON
                                        && event.end > beat + EPSILON
                                        && !event.note.is_rest
                                        && !event.note.is_unpitched
                                })
                                .copied()
                        };
                        let (Some(upper_event), Some(lower_event)) =
                            (active(upper_index), active(upper_index + 1))
                        else {
                            continue;
                        };
                        let (Some(upper), Some(lower)) = (
                            upper_event.note.pitches.first(),
                            lower_event.note.pitches.first(),
                        ) else {
                            continue;
                        };
                        let upper_addr = upper_event.address.clone();
                        let lower_addr = lower_event.address.clone();
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
        if let Some(region) = region
            && !observation
                .evidence
                .iter()
                .any(|address| analysis_region_contains(region, address))
        {
            continue;
        }
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

/// Locate two-voice quartets across parts, staves, and physical bar lines.
///
/// A voice is identified by its part/staff/voice indices. Only successive
/// simultaneous states are compared, and neither voice may have a gap between
/// them. Each state needs exactly one written pitch per voice. Source order is
/// retained so a caller can pass the same four pitches to another analyzer.
pub fn analyze_voice_leading_quartets(score: &Score) -> Vec<VoiceLeadingQuartet> {
    const EPSILON: f64 = 1e-7;
    let starts = vertical_measure_starts(score);
    let mut voices: BTreeMap<(usize, usize, usize), Vec<QuartetNote>> = BTreeMap::new();
    for (part_index, part) in score.parts.iter().enumerate() {
        for (staff_index, staff) in part.staves.iter().enumerate() {
            for (measure_index, measure) in staff.measures.iter().enumerate() {
                let bar_start = starts[part_index][staff_index][measure_index];
                let bar_length = staff.measure_beats(measure_index, &score.settings.time_signature);
                if !bar_start.is_finite() || !bar_length.is_finite() || bar_length <= 0.0 {
                    continue;
                }
                let written =
                    written_staff_measure_events(score, part_index, staff_index, measure_index);
                for (voice_index, voice) in measure.voices.iter().enumerate() {
                    let events: Vec<_> = written
                        .iter()
                        .filter(|event| event.address.voice == voice_index)
                        .collect();
                    // The shared traversal omits invalid-duration events. Reject the
                    // whole voice bar so an omission cannot shift later onsets.
                    if events.len() != voice.len()
                        || events.iter().any(|event| {
                            event.end > bar_length + EPSILON
                                || event.start < -EPSILON
                                || event.end < event.start - EPSILON
                        })
                    {
                        continue;
                    }
                    let stream = voices
                        .entry((part_index, staff_index, voice_index))
                        .or_default();
                    for event in events {
                        if event.note.is_rest
                            || event.note.is_unpitched
                            || event.note.pitches.len() != 1
                            || event.end <= event.start + EPSILON
                        {
                            continue;
                        }
                        stream.push(QuartetNote {
                            address: event.address.clone(),
                            pitch: event.note.pitches[0].clone(),
                            onset_beats: bar_start + event.start,
                            duration_beats: event.end - event.start,
                        });
                    }
                }
            }
        }
    }

    let streams: Vec<_> = voices
        .into_values()
        .filter(|voice| !voice.is_empty())
        .collect();
    let mut quartets = Vec::new();
    for (first_index, first) in streams.iter().enumerate() {
        for second in streams.iter().skip(first_index + 1) {
            let mut onsets: Vec<_> = first
                .iter()
                .chain(second.iter())
                .map(|note| note.onset_beats)
                .collect();
            onsets.sort_by(f64::total_cmp);
            onsets.dedup_by(|left, right| (*left - *right).abs() < EPSILON);
            let mut first_index = 0;
            let mut second_index = 0;
            let mut previous: Option<(&QuartetNote, &QuartetNote)> = None;
            for beat in onsets {
                while first_index + 1 < first.len()
                    && first[first_index + 1].onset_beats <= beat + EPSILON
                {
                    first_index += 1;
                }
                while second_index + 1 < second.len()
                    && second[second_index + 1].onset_beats <= beat + EPSILON
                {
                    second_index += 1;
                }
                let current =
                    first
                        .get(first_index)
                        .zip(second.get(second_index))
                        .filter(|(a, b)| {
                            a.onset_beats <= beat + EPSILON
                                && b.onset_beats <= beat + EPSILON
                                && a.onset_beats + a.duration_beats > beat + EPSILON
                                && b.onset_beats + b.duration_beats > beat + EPSILON
                        });
                let Some((first_after, second_after)) = current else {
                    previous = None;
                    continue;
                };
                if let Some((first_before, second_before)) = previous
                    && (first_before.address != first_after.address
                        || second_before.address != second_after.address)
                    && first_before.onset_beats + first_before.duration_beats + EPSILON >= beat
                    && second_before.onset_beats + second_before.duration_beats + EPSILON >= beat
                {
                    let first_motion =
                        first_after.pitch.to_midi_cents() - first_before.pitch.to_midi_cents();
                    let second_motion =
                        second_after.pitch.to_midi_cents() - second_before.pitch.to_midi_cents();
                    let similar_motion = first_motion != 0
                        && second_motion != 0
                        && first_motion.signum() == second_motion.signum();
                    let contrary_motion = first_motion != 0
                        && second_motion != 0
                        && first_motion.signum() != second_motion.signum();
                    let parallel_fifth = (similar_motion || contrary_motion)
                        && spelled_perfect_interval(&first_before.pitch, &second_before.pitch, 5)
                        && spelled_perfect_interval(&first_after.pitch, &second_after.pitch, 5);
                    let parallel_octave = (similar_motion || contrary_motion)
                        && spelled_perfect_interval(&first_before.pitch, &second_before.pitch, 8)
                        && spelled_perfect_interval(&first_after.pitch, &second_after.pitch, 8);
                    let proper_resolution_without_key = keyless_quartet_resolution(
                        &first_before.pitch,
                        &first_after.pitch,
                        &second_before.pitch,
                        &second_after.pitch,
                        first_motion,
                        second_motion,
                    );
                    quartets.push(VoiceLeadingQuartet {
                        first_before: first_before.clone(),
                        first_after: first_after.clone(),
                        second_before: second_before.clone(),
                        second_after: second_after.clone(),
                        contrary_motion,
                        similar_motion,
                        parallel_fifth,
                        parallel_octave,
                        proper_resolution_without_key,
                        rule_id: "located-written-voice-quartet".to_string(),
                        evidence: vec![
                            first_before.address.clone(),
                            first_after.address.clone(),
                            second_before.address.clone(),
                            second_after.address.clone(),
                        ],
                    });
                }
                previous = Some((first_after, second_after));
            }
        }
    }
    quartets
}

fn spelled_perfect_interval(a: &Pitch, b: &Pitch, number: u8) -> bool {
    let diatonic = quartet_diatonic_distance(a, b);
    let cents = (a.to_midi_cents() - b.to_midi_cents()).abs();
    match number {
        5 => diatonic % 7 == 4 && cents % 1200 == 700,
        8 => diatonic >= 7 && diatonic % 7 == 0 && cents >= 1200 && cents % 1200 == 0,
        _ => false,
    }
}

fn keyless_quartet_resolution(
    first_before: &Pitch,
    first_after: &Pitch,
    second_before: &Pitch,
    second_after: &Pitch,
    first_motion: i32,
    second_motion: i32,
) -> bool {
    if first_motion == 0 && second_motion == 0 {
        return true;
    }
    let initial_diatonic = quartet_diatonic_distance(first_before, second_before) % 7;
    let initial_cents = (first_before.to_midi_cents() - second_before.to_midi_cents()).abs() % 1200;
    let final_generic = quartet_diatonic_distance(first_after, second_after) % 7 + 1;
    match (initial_diatonic, initial_cents) {
        (3, 500) => first_motion <= 0, // P4: first voice stays or descends.
        (3, 600) => first_motion > 0 && second_motion < 0 && final_generic == 6, // A4.
        (4, 600) => first_motion < 0 && second_motion > 0 && final_generic == 3, // d5.
        (6, 1000) => final_generic == 3, // m7.
        _ => true,
    }
}

fn quartet_diatonic_distance(a: &Pitch, b: &Pitch) -> i32 {
    fn index(step: &Step) -> i32 {
        match step {
            Step::C => 0,
            Step::D => 1,
            Step::E => 2,
            Step::F => 3,
            Step::G => 4,
            Step::A => 5,
            Step::B => 6,
        }
    }
    ((i32::from(a.octave) - i32::from(b.octave)) * 7 + index(&a.step) - index(&b.step)).abs()
}

/// Check simultaneously sounding notes in adjacent voices for motion and parallel intervals.
pub fn analyze_voice_leading(score: &Score) -> Vec<VoiceLeadingObservation> {
    analyze_voice_leading_in_region(score, None)
}

/// Analyze voice-leading observations inside an optional part/staff measure region.
pub fn analyze_voice_leading_in_region(
    score: &Score,
    region: Option<&AnalysisRegion>,
) -> Vec<VoiceLeadingObservation> {
    const EPSILON: f64 = 1e-7;
    let mut observations = Vec::new();
    for (part_index, part) in score.parts.iter().enumerate() {
        for (staff_index, staff) in part.staves.iter().enumerate() {
            for (measure_index, measure) in staff.measures.iter().enumerate() {
                if let Some(region) = region
                    && (part_index != region.part
                        || staff_index != region.staff
                        || measure_index < region.start_measure
                        || measure_index >= region.end_measure)
                {
                    continue;
                }
                let written =
                    written_staff_measure_events(score, part_index, staff_index, measure_index);
                for upper_index in 0..measure.voices.len().saturating_sub(1) {
                    let pair: Vec<_> = written
                        .iter()
                        .filter(|event| {
                            event.address.part == part_index
                                && event.address.staff == staff_index
                                && (event.address.voice == upper_index
                                    || event.address.voice == upper_index + 1)
                        })
                        .collect();
                    let mut onsets: Vec<_> = pair.iter().map(|event| event.start).collect();
                    onsets.sort_by(f64::total_cmp);
                    onsets.dedup_by(|left, right| (*left - *right).abs() < EPSILON);
                    let active = |voice_index: usize, beat: f64| {
                        pair.iter()
                            .find(|event| {
                                event.address.voice == voice_index
                                    && event.start <= beat + EPSILON
                                    && event.end > beat + EPSILON
                                    && !event.note.is_rest
                                    && !event.note.is_unpitched
                                    && !event.note.pitches.is_empty()
                            })
                            .copied()
                    };
                    let mut previous = None;
                    for beat in onsets {
                        let (Some(upper), Some(lower)) =
                            (active(upper_index, beat), active(upper_index + 1, beat))
                        else {
                            previous = None;
                            continue;
                        };
                        if let Some((prev_upper, prev_lower)) = previous {
                            let prev_upper: &WrittenEvent<'_> = prev_upper;
                            let prev_lower: &WrittenEvent<'_> = prev_lower;
                            if prev_upper.end + EPSILON >= beat
                                && prev_lower.end + EPSILON >= beat
                                && (prev_upper.address != upper.address
                                    || prev_lower.address != lower.address)
                            {
                                let (Some(u0), Some(l0), Some(u1), Some(l1)) = (
                                    prev_upper.note.pitches.first(),
                                    prev_lower.note.pitches.first(),
                                    upper.note.pitches.first(),
                                    lower.note.pitches.first(),
                                ) else {
                                    previous = Some((upper, lower));
                                    continue;
                                };
                                let upper_motion_cents = u1.to_midi_cents() - u0.to_midi_cents();
                                let lower_motion_cents = l1.to_midi_cents() - l0.to_midi_cents();
                                let initial =
                                    (u0.to_midi_cents() - l0.to_midi_cents()).unsigned_abs() % 1200;
                                let next =
                                    (u1.to_midi_cents() - l1.to_midi_cents()).unsigned_abs() % 1200;
                                observations.push(VoiceLeadingObservation {
                                    upper: prev_upper.address.clone(),
                                    lower: prev_lower.address.clone(),
                                    upper_motion: u1.to_midi() - u0.to_midi(),
                                    lower_motion: l1.to_midi() - l0.to_midi(),
                                    upper_motion_cents,
                                    lower_motion_cents,
                                    parallel_perfect: matches!(initial, 0 | 700)
                                        && initial == next
                                        && upper_motion_cents != 0
                                        && upper_motion_cents.signum()
                                            == lower_motion_cents.signum(),
                                    confidence: 100,
                                    rule_id: "time-aligned-adjacent-voice-leading".to_string(),
                                    evidence: vec![
                                        prev_upper.address.clone(),
                                        prev_lower.address.clone(),
                                    ],
                                });
                            }
                        }
                        previous = Some((upper, lower));
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
    estimate_key_candidates(score, true)
}

fn estimate_key_candidates(score: &Score, only_best_coverage: bool) -> Vec<KeyEstimate> {
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
                        let duration_beats = note.beats().max(0.0);
                        pitches.extend(note.pitches.iter().map(|pitch| (pitch, duration_beats)));
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
    let total_duration_beats: f64 = pitches.iter().map(|(_, beats)| *beats).sum();
    let mut candidates = Vec::with_capacity(30);
    for fifths in -7..=7 {
        for mode in ["major", "minor"] {
            let key = KeySignature {
                fifths,
                mode: mode.to_string(),
            };
            let covered = pitches
                .iter()
                .filter(|(pitch, _)| key.contains_pitch(pitch))
                .count();
            let weighted_covered_beats = pitches
                .iter()
                .filter(|(pitch, _)| key.contains_pitch(pitch))
                .map(|(_, beats)| *beats)
                .sum::<f64>();
            candidates.push((key, covered, weighted_covered_beats));
        }
    }
    candidates.sort_by(
        |(left_key, left_score, left_weight), (right_key, right_score, right_weight)| {
            right_weight
                .total_cmp(left_weight)
                .then_with(|| right_score.cmp(left_score))
                .then_with(|| left_key.fifths.abs().cmp(&right_key.fifths.abs()))
                .then_with(|| left_key.fifths.cmp(&right_key.fifths))
                .then_with(|| left_key.mode.cmp(&right_key.mode))
        },
    );
    let best_weight = candidates[0].2;
    candidates
        .into_iter()
        .take_while(|(_, _, weighted)| {
            !only_best_coverage || weighted.total_cmp(&best_weight).is_eq()
        })
        .map(
            |(key, covered_pitches, weighted_covered_beats)| KeyEstimate {
                key,
                covered_pitches,
                total_pitches,
                weighted_covered_beats,
                total_duration_beats,
                confidence: if total_duration_beats > 0.0 {
                    ((weighted_covered_beats / total_duration_beats * 100.0).round() as u8).min(100)
                } else {
                    0
                },
                rule_id: "duration-weighted-diatonic-pitch-coverage".to_string(),
                evidence: evidence.clone(),
            },
        )
        .collect()
}

/// Rank the equal-coverage candidates from [`estimate_keys`] by tonal profile.
///
/// This opt-in ranking preserves the candidate set and the legacy coverage
/// `confidence` value. The first candidate is a profile preference, not a
/// probability or a claim that a score has one unambiguous tonal center.
/// Microtonal input leaves the original order unchanged.
pub fn estimate_keys_with_tonal_profile(score: &Score) -> Vec<KeyEstimate> {
    let mut estimates = estimate_keys(score);
    rank_key_estimates_by_tonal_profile(
        score,
        &mut estimates,
        &KRUMHANSL_MAJOR,
        &KRUMHANSL_MINOR,
        "duration-weighted-diatonic-pitch-coverage+krumhansl-profile-ranking",
    );
    estimates
}

/// Rank all major/minor key spellings by the Aarden-Essen tonal profile.
///
/// This uses the profile behind music21 10.5.2's `analyze("key")`, while
/// preserving Acorde's own input model and key spellings. Unlike
/// [`estimate_keys_with_tonal_profile`], chromatic notes do not discard a key
/// before the profile is evaluated. `confidence` still means diatonic coverage,
/// not probability. Microtonal or uniform input falls back to the legacy set.
pub fn estimate_keys_with_aarden_essen_profile(score: &Score) -> Vec<KeyEstimate> {
    let mut estimates = estimate_key_candidates(score, false);
    if !rank_key_estimates_by_tonal_profile(
        score,
        &mut estimates,
        &AARDEN_ESSEN_MAJOR,
        &AARDEN_ESSEN_MINOR,
        "duration-weighted-aarden-essen-profile-ranking",
    ) {
        return estimate_keys(score);
    }
    estimates
}

// Major/minor weights are indexed from the tonic. The Aarden-Essen constants
// match music21 10.5.2's AardenEssen.getWeights implementation.
const KRUMHANSL_MAJOR: [f64; 12] = [
    6.35, 2.23, 3.48, 2.33, 4.38, 4.09, 2.52, 5.19, 2.39, 3.66, 2.29, 2.88,
];
const KRUMHANSL_MINOR: [f64; 12] = [
    6.33, 2.68, 3.52, 5.38, 2.60, 3.53, 2.54, 4.75, 3.98, 2.69, 3.34, 3.17,
];
const AARDEN_ESSEN_MAJOR: [f64; 12] = [
    17.7661, 0.145624, 14.9265, 0.160186, 19.8049, 11.3587, 0.291248, 22.062, 0.145624, 8.15494,
    0.232998, 4.95122,
];
const AARDEN_ESSEN_MINOR: [f64; 12] = [
    18.2648, 0.737619, 14.0499, 16.8599, 0.702494, 14.4362, 0.702494, 18.6161, 4.56621, 1.93186,
    7.37619, 1.75623,
];

fn rank_key_estimates_by_tonal_profile(
    score: &Score,
    estimates: &mut [KeyEstimate],
    major: &[f64; 12],
    minor: &[f64; 12],
    rule_id: &str,
) -> bool {
    if estimates.len() < 2 {
        return false;
    }

    let mut pitch_classes = [0.0_f64; 12];
    for part in &score.parts {
        for staff in &part.staves {
            for measure in &staff.measures {
                for voice in &measure.voices {
                    for note in voice {
                        if note.is_rest {
                            continue;
                        }
                        let beats = note.beats();
                        if !beats.is_finite() || beats <= 0.0 {
                            continue;
                        }
                        for pitch in &note.pitches {
                            if pitch.microtone_cents != 0 {
                                return false;
                            }
                            pitch_classes[pitch.to_midi().rem_euclid(12) as usize] += beats;
                        }
                    }
                }
            }
        }
    }
    let observed_mean = pitch_classes.iter().sum::<f64>() / 12.0;
    let observed_variance = pitch_classes
        .iter()
        .map(|value| (value - observed_mean).powi(2))
        .sum::<f64>();
    if observed_variance <= 0.0 {
        return false;
    }
    let correlation = |estimate: &KeyEstimate| {
        let weights = if estimate.key.mode == "minor" {
            minor
        } else {
            major
        };
        let tonic = (estimate.key.fifths * 7 + if estimate.key.mode == "minor" { 9 } else { 0 })
            .rem_euclid(12) as usize;
        let profile_mean = weights.iter().sum::<f64>() / 12.0;
        let profile_variance = weights
            .iter()
            .map(|value| (value - profile_mean).powi(2))
            .sum::<f64>();
        let covariance = pitch_classes
            .iter()
            .enumerate()
            .map(|(class, value)| {
                (value - observed_mean) * (weights[(class + 12 - tonic) % 12] - profile_mean)
            })
            .sum::<f64>();
        covariance / (observed_variance * profile_variance).sqrt()
    };
    estimates.sort_by(|left, right| correlation(right).total_cmp(&correlation(left)));
    for estimate in estimates {
        estimate.rule_id = rule_id.to_string();
    }
    true
}

/// Estimate a key for each physical measure from a centered pitch-class window.
///
/// `radius` counts measures on either side. Durations in all parts and staves
/// contribute equally; the first staff's written key signature breaks ties
/// between enharmonic spellings. The result has one position per measure, so
/// it cannot resolve a modulation within a measure. Empty or uniform windows
/// yield no estimate. The correlation is not a confidence probability.
pub fn estimate_local_keys_with_aarden_essen_profile(
    score: &Score,
    radius: usize,
) -> Vec<LocalKeyEstimate> {
    let measure_count = score
        .parts
        .iter()
        .flat_map(|part| &part.staves)
        .map(|staff| staff.measures.len())
        .max()
        .unwrap_or(0);
    let mut histograms = vec![[0.0_f64; 12]; measure_count];
    for part in &score.parts {
        for staff in &part.staves {
            for (index, measure) in staff.measures.iter().enumerate() {
                for voice in &measure.voices {
                    for note in voice {
                        if note.is_rest || note.is_unpitched {
                            continue;
                        }
                        let beats = note.beats();
                        if !beats.is_finite() || beats <= 0.0 {
                            continue;
                        }
                        for pitch in &note.pitches {
                            if pitch.microtone_cents == 0 {
                                histograms[index][pitch.to_midi().rem_euclid(12) as usize] += beats;
                            }
                        }
                    }
                }
            }
        }
    }
    let written_measures = score
        .parts
        .first()
        .and_then(|part| part.staves.first())
        .map(|staff| staff.measures.as_slice())
        .unwrap_or(&[]);
    let mut reference_fifths = 0_i8;
    let mut estimates = Vec::with_capacity(measure_count);
    for measure in 0..measure_count {
        if let Some(key) = written_measures
            .get(measure)
            .and_then(|written| written.key_sig.as_ref())
        {
            reference_fifths = key.fifths;
        }
        let start = measure.saturating_sub(radius);
        let end = measure
            .saturating_add(radius)
            .saturating_add(1)
            .min(measure_count);
        let mut observed = [0.0_f64; 12];
        for histogram in &histograms[start..end] {
            for (class, weight) in histogram.iter().enumerate() {
                observed[class] += weight;
            }
        }
        let observed_mean = observed.iter().sum::<f64>() / 12.0;
        let observed_variance = observed
            .iter()
            .map(|weight| (weight - observed_mean).powi(2))
            .sum::<f64>();
        if observed_variance <= 0.0 {
            continue;
        }
        let mut best: Option<(f64, i8, &str)> = None;
        for fifths in -7_i8..=7 {
            for mode in ["major", "minor"] {
                let weights = if mode == "minor" {
                    &AARDEN_ESSEN_MINOR
                } else {
                    &AARDEN_ESSEN_MAJOR
                };
                let tonic = (i32::from(fifths) * 7 + if mode == "minor" { 9 } else { 0 })
                    .rem_euclid(12) as usize;
                let profile_mean = weights.iter().sum::<f64>() / 12.0;
                let profile_variance = weights
                    .iter()
                    .map(|weight| (weight - profile_mean).powi(2))
                    .sum::<f64>();
                let covariance = observed
                    .iter()
                    .enumerate()
                    .map(|(class, weight)| {
                        (weight - observed_mean)
                            * (weights[(class + 12 - tonic) % 12] - profile_mean)
                    })
                    .sum::<f64>();
                let correlation = covariance / (observed_variance * profile_variance).sqrt();
                let better = best.is_none_or(|(best_correlation, best_fifths, best_mode)| {
                    correlation.total_cmp(&best_correlation).is_gt()
                        || (correlation.total_cmp(&best_correlation).is_eq()
                            && (
                                (i16::from(fifths) - i16::from(reference_fifths)).abs(),
                                i16::from(fifths).abs(),
                                fifths,
                                std::cmp::Reverse(mode),
                            ) < (
                                (i16::from(best_fifths) - i16::from(reference_fifths)).abs(),
                                i16::from(best_fifths).abs(),
                                best_fifths,
                                std::cmp::Reverse(best_mode),
                            ))
                });
                if better {
                    best = Some((correlation, fifths, mode));
                }
            }
        }
        if let Some((profile_correlation, fifths, mode)) = best {
            estimates.push(LocalKeyEstimate {
                measure,
                window_start: start,
                window_end_exclusive: end,
                key: KeySignature {
                    fifths,
                    mode: mode.to_string(),
                },
                profile_correlation,
                rule_id: "centered-aarden-essen-local-window".to_string(),
            });
        }
    }
    estimates
}

/// Analyze adjacent pitched notes in every voice without inferring missing events.
pub fn analyze_intervals(score: &Score) -> Vec<IntervalObservation> {
    analyze_intervals_in_region(score, None)
}

/// Analyze intervals whose endpoints touch an optional part/staff measure region.
pub fn analyze_intervals_in_region(
    score: &Score,
    region: Option<&AnalysisRegion>,
) -> Vec<IntervalObservation> {
    let mut observations = Vec::new();
    for (part_index, part) in score.parts.iter().enumerate() {
        for (staff_index, staff) in part.staves.iter().enumerate() {
            for (measure_index, measure) in staff.measures.iter().enumerate() {
                for (voice_index, voice) in measure.voices.iter().enumerate() {
                    for pair in voice.iter().enumerate().collect::<Vec<_>>().windows(2) {
                        let (from_index, from_note) = pair[0];
                        let (to_index, to_note) = pair[1];
                        if from_note.is_rest || to_note.is_rest {
                            continue;
                        }
                        let Some(from) = from_note.pitches.first() else {
                            continue;
                        };
                        let Some(to) = to_note.pitches.first() else {
                            continue;
                        };
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
                        if let Some(region) = region
                            && !analysis_region_contains(region, &from_addr)
                            && !analysis_region_contains(region, &to_addr)
                        {
                            continue;
                        }
                        observations.push(IntervalObservation {
                            from: from_addr.clone(),
                            to: to_addr.clone(),
                            semitones: (to.to_midi() - from.to_midi()).unsigned_abs() as u8,
                            cents: to.to_midi_cents() - from.to_midi_cents(),
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
    chord.display_text()
}

#[cfg(test)]
mod tests {
    use super::*;
    use acorde_core::{Duration, MeasureLength, Note, Pitch, Score, Step, TupletInfo};

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
        assert_eq!(result.chords[0].name, "C");
    }

    #[test]
    fn vertical_sonorities_do_not_turn_a_sequential_melody_into_a_chord() {
        let mut score = Score::new("melody", 120, 4, 4, 0, 1);
        score.parts[0].staves[0].measures[0].voices[0] = [Step::C, Step::E, Step::G]
            .into_iter()
            .map(|step| Note::new(Pitch::new(step, 4), Duration::Quarter))
            .collect();

        assert!(analyze_vertical_sonorities(&score).is_empty());
        // The legacy pass still has its original behavior for existing callers.
        assert_eq!(analyze_chords(&score).chords.len(), 1);
    }

    #[test]
    fn opt_in_arpeggiated_bar_labels_its_downbeat_with_later_note_evidence() {
        let mut score = Score::new("arpeggio", 120, 4, 4, 0, 1);
        let voice = &mut score.parts[0].staves[0].measures[0].voices[0];
        *voice = [(Step::C, 4), (Step::E, 4), (Step::G, 4), (Step::C, 5)]
            .into_iter()
            .map(|(step, octave)| Note::new(Pitch::new(step, octave), Duration::Eighth))
            .collect();
        voice.push(Note::rest(Duration::Half));

        assert!(analyze_vertical_sonorities(&score).is_empty());
        let report = analyze_arpeggiated_bars_report(&score);
        assert!(report.timeline_issues.is_empty());
        assert_eq!(report.sonorities.len(), 1);
        let inferred = &report.sonorities[0];
        assert_eq!((inferred.measure, inferred.beat), (0, 0.0));
        assert_eq!(inferred.label.roman_numeral.as_deref(), Some("I"));
        assert_eq!(inferred.label.rule_id, "bar-arpeggio-template");
        assert_eq!(inferred.label.evidence.len(), 4);

        // The exact-template rule does not silently turn a changed pitch into
        // a C-major harmony merely because three of the notes still fit.
        score.parts[0].staves[0].measures[0].voices[0][3].pitches[0] = Pitch::new(Step::F, 4);
        assert!(
            analyze_arpeggiated_bars_report(&score)
                .sonorities
                .is_empty()
        );
    }

    #[test]
    fn opt_in_forward_completion_requires_an_exact_tertian_window() {
        let mut score = Score::new("staggered chord", 120, 4, 4, 0, 1);
        let measure = &mut score.parts[0].staves[0].measures[0];
        measure.voices[0] = vec![
            Note::new(Pitch::new(Step::C, 4), Duration::Quarter),
            Note::rest(Duration::Half),
            Note::rest(Duration::Quarter),
        ];
        let mut upper = Note::new(Pitch::new(Step::E, 4), Duration::Eighth);
        upper.pitches.push(Pitch::new(Step::G, 4));
        measure.voices[1] = vec![
            Note::rest(Duration::Eighth),
            upper,
            Note::rest(Duration::Half),
            Note::rest(Duration::Quarter),
        ];

        let baseline = analyze_vertical_sonorities(&score);
        assert_eq!(baseline.len(), 1);
        assert_eq!(baseline[0].beat, 0.5);
        let inferred = analyze_arpeggiated_bars_report(&score);
        assert_eq!(inferred.sonorities.len(), 2);
        assert_eq!(inferred.sonorities[0].beat, 0.0);
        assert_eq!(
            inferred.sonorities[0].label.roman_numeral.as_deref(),
            Some("I")
        );
        assert_eq!(
            inferred.sonorities[0].label.rule_id,
            "forward-completed-sonority"
        );
        assert_eq!(inferred.sonorities[0].label.evidence.len(), 2);

        let mut shifted = score.clone();
        let upper = shifted.parts[0].staves[0].measures[0].voices[1][1].clone();
        shifted.parts[0].staves[0].measures[0].voices[0] = vec![
            Note::rest(Duration::Eighth),
            Note::new(Pitch::new(Step::C, 4), Duration::Quarter),
            Note::rest(Duration::Half),
            Note::rest(Duration::Eighth),
        ];
        shifted.parts[0].staves[0].measures[0].voices[1] = vec![
            Note::rest(Duration::Quarter),
            upper,
            Note::rest(Duration::Half),
            Note::rest(Duration::Eighth),
        ];
        let offbeat = analyze_arpeggiated_bars_report(&shifted);
        assert_eq!(offbeat.sonorities[0].beat, 0.5);
        assert_eq!(
            offbeat.sonorities[0].label.rule_id,
            "forward-completed-sonority"
        );

        // F-sharp approaches an otherwise complete C-major chord at beat 0.5.
        score.parts[0].staves[0].measures[0].voices[0] = vec![
            Note::new(Pitch::with_alter(Step::F, 4, 1), Duration::Eighth),
            Note::new(Pitch::new(Step::C, 4), Duration::Eighth),
            Note::rest(Duration::Half),
            Note::rest(Duration::Quarter),
        ];
        let changed = analyze_arpeggiated_bars_report(&score);
        assert!(
            changed
                .sonorities
                .iter()
                .all(|item| item.label.rule_id != "forward-completed-sonority")
        );
    }

    #[test]
    fn opt_in_vertical_cadence_uses_bass_and_outer_voice_resolution() {
        let mut score = Score::new("dominant tonic", 120, 4, 4, 0, 2);
        for (measure_index, bass_step, bass_octave, upper_pitches) in [
            (
                0,
                Step::G,
                3,
                vec![
                    Pitch::new(Step::B, 4),
                    Pitch::new(Step::D, 5),
                    Pitch::new(Step::G, 5),
                ],
            ),
            (
                1,
                Step::C,
                3,
                vec![
                    Pitch::new(Step::E, 4),
                    Pitch::new(Step::G, 4),
                    Pitch::new(Step::C, 5),
                ],
            ),
        ] {
            let measure = &mut score.parts[0].staves[0].measures[measure_index];
            measure.voices[0] = vec![Note::new(
                Pitch::new(bass_step, bass_octave),
                Duration::Whole,
            )];
            let mut upper = Note::new(upper_pitches[0].clone(), Duration::Whole);
            upper.pitches.extend(upper_pitches.into_iter().skip(1));
            measure.voices[1] = vec![upper];
        }

        let report = analyze_vertical_cadences_report(&score);
        assert!(report.timeline_issues.is_empty());
        assert_eq!(report.cadence_candidates.len(), 1);
        let candidate = &report.cadence_candidates[0];
        assert_eq!(candidate.kind, CadenceKind::Authentic);
        assert_eq!(candidate.from.measure, 0);
        assert_eq!(candidate.to.measure, 1);
        assert_eq!(candidate.rule_id, "vertical-dominant-tonic-outer-voices");

        let upper = &mut score.parts[0].staves[0].measures[1].voices[1][0];
        upper.pitches.retain(|pitch| pitch.step != Step::C);
        assert!(
            analyze_vertical_cadences_report(&score)
                .cadence_candidates
                .is_empty()
        );
    }

    #[test]
    fn local_key_windows_keep_physical_measure_addresses() {
        assert!(estimate_local_keys_with_aarden_essen_profile(&Score::default(), 4).is_empty());
        let mut score = Score::new("local keys", 120, 4, 4, 0, 3);
        for measure in &mut score.parts[0].staves[0].measures {
            let mut chord = Note::new(Pitch::new(Step::C, 4), Duration::Whole);
            chord.pitches.push(Pitch::new(Step::E, 4));
            chord.pitches.push(Pitch::new(Step::G, 4));
            measure.voices[0] = vec![chord];
        }
        let keys = estimate_local_keys_with_aarden_essen_profile(&score, 1);
        assert_eq!(keys.len(), 3);
        assert_eq!(keys[0].measure, 0);
        assert_eq!((keys[0].window_start, keys[0].window_end_exclusive), (0, 2));
        assert_eq!((keys[1].window_start, keys[1].window_end_exclusive), (0, 3));
        assert_eq!((keys[2].window_start, keys[2].window_end_exclusive), (1, 3));
        assert!(keys.iter().all(|key| key.profile_correlation.is_finite()));
    }

    #[test]
    fn vertical_sonorities_include_held_notes_across_parts() {
        let mut score = Score::new("vertical", 120, 4, 4, 0, 1);
        let part = score.parts[0].clone();
        score.parts.push(part.clone());
        score.parts.push(part);
        score.parts[0].staves[0].measures[0].voices[0] =
            vec![Note::new(Pitch::new(Step::C, 3), Duration::Whole)];
        score.parts[1].staves[0].measures[0].voices[0] = vec![
            Note::new(Pitch::new(Step::E, 4), Duration::Half),
            Note::new(Pitch::new(Step::F, 4), Duration::Half),
        ];
        score.parts[2].staves[0].measures[0].voices[0] =
            vec![Note::new(Pitch::new(Step::G, 4), Duration::Whole)];

        let sonorities = analyze_vertical_sonorities(&score);
        assert_eq!(sonorities.len(), 2);
        assert_eq!((sonorities[0].measure, sonorities[0].beat), (0, 0.0));
        assert_eq!((sonorities[1].measure, sonorities[1].beat), (0, 2.0));
        assert_eq!(sonorities[0].label.chord.root, "C");
        assert_eq!(sonorities[0].label.chord.kind, "major");
        assert_eq!(sonorities[1].label.chord.kind, "suspended-fourth");
        assert!(sonorities.iter().all(|item| item.label.evidence.len() == 3));
    }

    #[test]
    fn vertical_sonorities_preserve_authored_chord_at_its_beat() {
        let mut score = Score::new("authored", 120, 4, 4, 0, 1);
        let part = score.parts[0].clone();
        score.parts.push(part.clone());
        score.parts.push(part);
        let mut bass = Note::new(Pitch::new(Step::C, 3), Duration::Quarter);
        bass.chord_symbol = Some(ChordSymbol {
            root: "D".to_string(),
            kind: "minor".to_string(),
            bass: None,
            placement: None,
            extender: false,
            harmonic_degree: None,
            harmony_function: None,
            harmony_type: None,
            chord_ref: None,
            range_end: None,
            degrees: Vec::new(),
        });
        score.parts[0].staves[0].measures[0].voices[0] = vec![bass];
        score.parts[1].staves[0].measures[0].voices[0] =
            vec![Note::new(Pitch::new(Step::E, 4), Duration::Quarter)];
        score.parts[2].staves[0].measures[0].voices[0] =
            vec![Note::new(Pitch::new(Step::G, 4), Duration::Quarter)];

        let sonorities = analyze_vertical_sonorities(&score);
        assert_eq!(sonorities.len(), 1);
        assert_eq!(sonorities[0].label.rule_id, "authored-chord-symbol");
        assert_eq!(sonorities[0].label.chord.root, "D");
        assert_eq!(sonorities[0].label.confidence, 100);

        score.parts[0].staves[0].transpose_semitones = -2;
        let key = KeySignature {
            fifths: 0,
            mode: "major".to_string(),
        };
        let transposing = analyze_vertical_sonorities_in_key(&score, &key);
        assert_eq!(transposing[0].label.chord.root, "D");
        assert_eq!(transposing[0].label.roman_numeral, None);
    }

    #[test]
    fn vertical_sonorities_can_use_an_independently_estimated_minor_key() {
        let mut score = Score::new("minor", 120, 4, 4, -3, 1);
        let part = score.parts[0].clone();
        score.parts.push(part.clone());
        score.parts.push(part);
        for (part_index, (step, alter)) in [(Step::C, 0), (Step::E, -1), (Step::G, 0)]
            .into_iter()
            .enumerate()
        {
            score.parts[part_index].staves[0].measures[0].voices[0] = vec![Note::new(
                Pitch::with_alter(step, 4, alter),
                Duration::Quarter,
            )];
        }
        let c_minor = KeySignature {
            fifths: -3,
            mode: "minor".to_string(),
        };
        assert_eq!(
            analyze_vertical_sonorities(&score)[0]
                .label
                .roman_numeral
                .as_deref(),
            Some("vi")
        );
        assert_eq!(
            analyze_vertical_sonorities_in_key(&score, &c_minor)[0]
                .label
                .roman_numeral
                .as_deref(),
            Some("i")
        );
    }

    #[test]
    fn vertical_sonorities_report_unequal_part_bar_lengths() {
        let mut score = Score::new("unequal", 120, 4, 4, 0, 2);
        let part = score.parts[0].clone();
        score.parts.push(part.clone());
        score.parts.push(part);
        for (part_index, step) in [Step::C, Step::E, Step::G].into_iter().enumerate() {
            score.parts[part_index].staves[0].measures[0].voices[0] =
                vec![Note::new(Pitch::new(step, 4), Duration::Quarter)];
        }
        score.parts[2].staves[0].measures[0].actual_length = Some(MeasureLength {
            numerator: 3,
            denominator: 4,
        });
        let report = analyze_vertical_sonorities_report(&score);
        assert!(report.sonorities.iter().all(|item| item.measure != 0));
        assert_eq!(report.timeline_issues[0].part, 2);
        assert_eq!(
            report.timeline_issues[0].kind,
            VerticalTimelineIssueKind::DifferentLength
        );
        assert!(report.timeline_issues.iter().any(|issue| {
            issue.measure == 1 && issue.kind == VerticalTimelineIssueKind::DifferentStart
        }));
        assert_eq!(analyze_vertical_sonorities(&score), report.sonorities);
    }

    #[test]
    fn vertical_harmony_aligns_two_short_bars_with_one_long_bar() {
        let mut score = Score::new("mixed bar partitions", 120, 4, 4, 0, 2);
        let part = score.parts[0].clone();
        score.parts.push(part.clone());
        score.parts.push(part);

        let mut first_c = Note::new(Pitch::new(Step::C, 4), Duration::Whole);
        first_c.tie_start = true;
        let mut second_c = Note::new(Pitch::new(Step::C, 4), Duration::Whole);
        second_c.tie_end = true;
        score.parts[0].staves[0].measures[0].voices[0] = vec![first_c];
        score.parts[0].staves[0].measures[1].voices[0] = vec![second_c];

        score.parts[1].staves[0].measures.truncate(1);
        score.parts[1].staves[0].measures[0].actual_length = Some(MeasureLength {
            numerator: 2,
            denominator: 1,
        });
        score.parts[1].staves[0].measures[0].voices[0] =
            vec![Note::new(Pitch::new(Step::E, 4), Duration::Breve)];

        for measure in &mut score.parts[2].staves[0].measures {
            measure.voices[0] = vec![Note::new(Pitch::new(Step::G, 4), Duration::Whole)];
        }

        let key = KeySignature {
            fifths: 0,
            mode: "major".to_string(),
        };
        let report = analyze_vertical_sonorities_in_key_report(&score, &key);
        assert!(report.timeline_issues.is_empty());
        assert_eq!(report.sonorities.len(), 2);
        assert_eq!(
            (report.sonorities[1].measure, report.sonorities[1].beat),
            (1, 0.0)
        );
        assert_eq!(report.sonorities[1].label.chord.root, "C");
        assert_eq!(
            report.sonorities[1].label.roman_numeral.as_deref(),
            Some("I")
        );
        assert!(
            report.sonorities[1]
                .label
                .evidence
                .iter()
                .any(|address| address.part == 1 && address.measure == 0)
        );

        let without_key = analyze_vertical_sonorities_report(&score);
        assert_eq!(without_key.sonorities.len(), 2);
        assert_eq!(
            without_key.sonorities[1].label.roman_numeral.as_deref(),
            Some("I")
        );

        let mut transposing = score.clone();
        transposing.parts[2].staves[0].transpose_semitones = -2;
        for measure in &mut transposing.parts[2].staves[0].measures {
            measure.key_sig = Some(KeySignature {
                fifths: 2,
                mode: "major".to_string(),
            });
            measure.voices[0][0].pitches[0] = Pitch::new(Step::A, 4);
        }
        let converted = analyze_vertical_sonorities_report(&transposing);
        assert_eq!(converted.sonorities.len(), 2);
        assert_eq!(
            converted.sonorities[1].label.roman_numeral.as_deref(),
            Some("I")
        );

        score.parts[2].staves[0].measures[1].key_sig = Some(KeySignature {
            fifths: 1,
            mode: "major".to_string(),
        });
        let conflicting = analyze_vertical_sonorities_report(&score);
        assert_eq!(conflicting.sonorities.len(), 2);
        assert_eq!(
            conflicting.sonorities[0].label.roman_numeral.as_deref(),
            Some("I")
        );
        assert_eq!(conflicting.sonorities[1].label.roman_numeral, None);
    }

    #[test]
    fn vertical_harmony_uses_long_reference_bar_coordinates() {
        let mut score = Score::new("long reference bar", 120, 4, 4, 0, 2);
        let part = score.parts[0].clone();
        score.parts.push(part.clone());
        score.parts.push(part);
        score.parts[0].staves[0].measures.truncate(1);
        score.parts[0].staves[0].measures[0].actual_length = Some(MeasureLength {
            numerator: 2,
            denominator: 1,
        });
        score.parts[0].staves[0].measures[0].voices[0] =
            vec![Note::new(Pitch::new(Step::C, 4), Duration::Breve)];
        for measure_index in 0..2 {
            score.parts[1].staves[0].measures[measure_index].voices[0] =
                vec![Note::new(Pitch::new(Step::E, 4), Duration::Whole)];
            score.parts[2].staves[0].measures[measure_index].voices[0] =
                vec![Note::new(Pitch::new(Step::G, 4), Duration::Whole)];
        }

        let report = analyze_vertical_sonorities_report(&score);
        assert!(report.timeline_issues.is_empty());
        assert_eq!(report.sonorities.len(), 2);
        assert_eq!(
            (report.sonorities[1].measure, report.sonorities[1].beat),
            (0, 4.0)
        );
        assert_eq!(report.sonorities[1].label.chord.root, "C");
        assert_eq!(report.sonorities[1].label.address.measure, 1);
    }

    #[test]
    fn vertical_sonorities_do_not_relabel_a_fully_tied_barline() {
        let mut score = Score::new("ties", 120, 4, 4, 0, 2);
        let mut first = Note::new(Pitch::new(Step::C, 4), Duration::Whole);
        first
            .pitches
            .extend([Pitch::new(Step::E, 4), Pitch::new(Step::G, 4)]);
        first.set_pitch_ties(&[true, true, true], &[false, false, false]);
        let mut second = first.clone();
        second.set_pitch_ties(&[false, false, false], &[true, true, true]);
        score.parts[0].staves[0].measures[0].voices[0] = vec![first];
        score.parts[0].staves[0].measures[1].voices[0] = vec![second];
        let report = analyze_vertical_sonorities_report(&score);
        assert!(report.timeline_issues.is_empty());
        assert_eq!(report.sonorities.len(), 1);
        assert_eq!(report.sonorities[0].measure, 0);
        let opt_in = analyze_arpeggiated_bars_report(&score);
        assert_eq!(opt_in.sonorities.len(), 1);
        assert_eq!(opt_in.sonorities[0].measure, 0);
    }

    #[test]
    fn vertical_harmony_recognizes_tie_across_voices() {
        let mut score = Score::new("voice crossing tie", 120, 4, 4, 0, 2);
        let mut first = Note::new(Pitch::new(Step::C, 4), Duration::Whole);
        first
            .pitches
            .extend([Pitch::new(Step::E, 4), Pitch::new(Step::G, 4)]);
        first.set_pitch_ties(&[true, true, true], &[false, false, false]);
        let mut second = first.clone();
        second.set_pitch_ties(&[false, false, false], &[true, true, true]);
        score.parts[0].staves[0].measures[0].voices[0] = vec![first];
        score.parts[0].staves[0].measures[1].voices[1] = vec![second];

        let report = analyze_vertical_sonorities_report(&score);
        assert!(report.timeline_issues.is_empty());
        assert_eq!(report.sonorities.len(), 1);
        assert_eq!(report.sonorities[0].measure, 0);
    }

    #[test]
    fn cross_staff_tie_uses_explicit_source_location() {
        let mut score = Score::new("staff crossing tie", 120, 4, 4, 0, 2);
        let other_staff = score.parts[0].staves[0].clone();
        score.parts[0].staves.push(other_staff);
        let mut first = Note::new(Pitch::new(Step::C, 4), Duration::Quarter);
        first
            .pitches
            .extend([Pitch::new(Step::E, 4), Pitch::new(Step::G, 4)]);
        first.set_pitch_ties(&[true, true, true], &[false, false, false]);
        let mut second = first.clone();
        second.set_pitch_ties(&[false, false, false], &[true, true, true]);
        let source = acorde_core::TieSourceLocation {
            measures: -1,
            staves: -1,
            fractions: Some("3/4".to_string()),
            ..Default::default()
        };
        second.pitch_tie_sources = vec![Some(source); 3];
        score.parts[0].staves[0].measures[0].voices[0] = vec![
            Note::rest(Duration::Quarter),
            Note::rest(Duration::Quarter),
            Note::rest(Duration::Quarter),
            first,
        ];
        score.parts[0].staves[1].measures[1].voices[0] = vec![second];

        let report = analyze_vertical_sonorities_report(&score);
        assert!(report.timeline_issues.is_empty());
        assert_eq!(report.sonorities.len(), 1);
        assert_eq!(report.sonorities[0].beat, 3.0);

        for source in &mut score.parts[0].staves[1].measures[1].voices[0][0].pitch_tie_sources {
            source.as_mut().expect("source exists").staves = 0;
        }
        let invalid_source = analyze_vertical_sonorities_report(&score);
        assert_eq!(invalid_source.sonorities.len(), 2);
        assert_eq!(invalid_source.sonorities[1].measure, 1);
    }

    #[test]
    fn partial_or_dangling_ties_keep_new_harmonic_attacks() {
        let mut score = Score::new("partial ties", 120, 4, 4, 0, 2);
        let mut first = Note::new(Pitch::new(Step::C, 4), Duration::Whole);
        first
            .pitches
            .extend([Pitch::new(Step::E, 4), Pitch::new(Step::G, 4)]);
        first.set_pitch_ties(&[true, false, false], &[false, false, false]);
        let mut second = first.clone();
        second.set_pitch_ties(&[false, false, false], &[true, false, false]);
        score.parts[0].staves[0].measures[0].voices[0] = vec![first];
        score.parts[0].staves[0].measures[1].voices[0] = vec![second.clone()];
        let report = analyze_vertical_sonorities_report(&score);
        assert_eq!(report.sonorities.len(), 2);
        assert_eq!(report.sonorities[1].measure, 1);

        score.parts[0].staves[0].measures[0].voices[0][0]
            .set_pitch_ties(&[false, false, false], &[false, false, false]);
        second.set_pitch_ties(&[false, false, false], &[true, true, true]);
        score.parts[0].staves[0].measures[1].voices[0] = vec![second];
        let report = analyze_vertical_sonorities_report(&score);
        assert_eq!(report.sonorities.len(), 2);
    }

    #[test]
    fn tie_end_in_newly_used_voice_is_an_attack_without_a_predecessor() {
        let mut score = Score::new("new voice", 120, 4, 4, 0, 2);
        let mut note = Note::new(Pitch::new(Step::C, 4), Duration::Whole);
        note.pitches
            .extend([Pitch::new(Step::E, 4), Pitch::new(Step::G, 4)]);
        note.set_pitch_ties(&[false, false, false], &[true, true, true]);
        score.parts[0].staves[0].measures[1].voices[1] = vec![note];

        let report = analyze_vertical_sonorities_report(&score);
        assert!(report.timeline_issues.is_empty());
        assert_eq!(report.sonorities.len(), 1);
        assert_eq!(report.sonorities[0].measure, 1);
        assert_eq!(report.sonorities[0].label.address.voice, 1);
    }

    #[test]
    fn instrument_change_breaks_a_written_pitch_tie_for_harmony() {
        let mut score = Score::new("changed sounding pitch", 120, 4, 4, 0, 2);
        let mut first = Note::new(Pitch::new(Step::C, 4), Duration::Whole);
        first
            .pitches
            .extend([Pitch::new(Step::E, 4), Pitch::new(Step::G, 4)]);
        first.set_pitch_ties(&[true, true, true], &[false, false, false]);
        let mut second = first.clone();
        second.set_pitch_ties(&[false, false, false], &[true, true, true]);
        score.parts[0].staves[0].measures[0].voices[0] = vec![first];
        score.parts[0].staves[0].measures[1].voices[0] = vec![second];
        let mut clarinet = acorde_core::InstrumentDefinition::new("clarinet", "Clarinet");
        clarinet.transpose_semitones = -2;
        score.parts[0].staves[0].measures[1].instrument_change = Some(clarinet);

        let report = analyze_vertical_sonorities_report(&score);
        assert!(report.timeline_issues.is_empty());
        assert_eq!(report.sonorities.len(), 2);
        assert_eq!(report.sonorities[0].label.chord.root, "C");
        assert_eq!(report.sonorities[1].label.chord.root, "Bb");
    }

    #[test]
    fn new_attack_combines_with_tied_pitches_across_parts() {
        let mut score = Score::new("held harmony", 120, 4, 4, 0, 2);
        let part = score.parts[0].clone();
        score.parts.push(part.clone());
        score.parts.push(part);
        for (part_index, step) in [Step::C, Step::E, Step::G].into_iter().enumerate() {
            let mut first = Note::new(Pitch::new(step, 4), Duration::Whole);
            let mut second = first.clone();
            if part_index < 2 {
                first.tie_start = true;
                second.tie_end = true;
            }
            score.parts[part_index].staves[0].measures[0].voices[0] = vec![first];
            score.parts[part_index].staves[0].measures[1].voices[0] = vec![second];
        }
        let report = analyze_vertical_sonorities_report(&score);
        assert!(report.timeline_issues.is_empty());
        let second = report
            .sonorities
            .iter()
            .find(|sonority| sonority.measure == 1)
            .expect("the new G attack triggers a sonority");
        assert_eq!(second.label.evidence.len(), 3);
        assert_eq!(second.label.address.part, 2);
    }

    #[test]
    fn vertical_harmony_uses_concert_pitches_for_transposing_parts() {
        let mut score = Score::new("clarinet and piano", 120, 4, 4, 0, 1);
        let part = score.parts[0].clone();
        score.parts.push(part.clone());
        score.parts.push(part);
        score.parts[0].staves[0].transpose_semitones = -2;
        for (part_index, step) in [Step::D, Step::E, Step::G].into_iter().enumerate() {
            score.parts[part_index].staves[0].measures[0].voices[0] =
                vec![Note::new(Pitch::new(step, 4), Duration::Quarter)];
        }
        let key = KeySignature {
            fifths: 0,
            mode: "major".to_string(),
        };
        let in_key = analyze_vertical_sonorities_in_key_report(&score, &key);
        assert!(in_key.timeline_issues.is_empty());
        assert_eq!(in_key.sonorities.len(), 1);
        assert_eq!(in_key.sonorities[0].label.chord.root, "C");
        assert_eq!(
            in_key.sonorities[0].label.roman_numeral.as_deref(),
            Some("I")
        );

        let without_concert_key = analyze_vertical_sonorities_report(&score);
        assert_eq!(without_concert_key.sonorities[0].label.chord.root, "C");
        assert_eq!(without_concert_key.sonorities[0].label.roman_numeral, None);

        score.parts[0].staves[0].measures[0].key_sig = Some(KeySignature {
            fifths: 2,
            mode: "major".to_string(),
        });
        let reconciled = analyze_vertical_sonorities_report(&score);
        assert_eq!(
            reconciled.sonorities[0].label.roman_numeral.as_deref(),
            Some("I")
        );

        let mut silent = score.parts[1].clone();
        silent.staves[0].measures[0].voices[0] = vec![Note::rest(Duration::Whole)];
        silent.staves[0].measures[0].key_sig = Some(KeySignature {
            fifths: 1,
            mode: "major".to_string(),
        });
        score.parts.push(silent);
        let silent_conflict = analyze_vertical_sonorities_report(&score);
        assert_eq!(
            silent_conflict.sonorities[0].label.roman_numeral.as_deref(),
            Some("I")
        );

        score.parts[1].staves[0].measures[0].key_sig = Some(KeySignature {
            fifths: 1,
            mode: "major".to_string(),
        });
        let conflicting = analyze_vertical_sonorities_report(&score);
        assert_eq!(conflicting.sonorities[0].label.roman_numeral, None);
    }

    #[test]
    fn vertical_harmony_uses_measure_instrument_change() {
        let mut score = Score::new("changing instrument", 120, 4, 4, 0, 2);
        let part = score.parts[0].clone();
        score.parts.push(part.clone());
        score.parts.push(part);
        let mut clarinet = acorde_core::InstrumentDefinition::new("clarinet", "Clarinet");
        clarinet.transpose_semitones = -2;
        score.parts[0].instrument = Some(clarinet);
        score.parts[0].staves[0].measures[1].instrument_change =
            Some(acorde_core::InstrumentDefinition::new("piano", "Piano"));
        for (part_index, (part, step)) in score
            .parts
            .iter_mut()
            .zip([Step::D, Step::E, Step::G])
            .enumerate()
        {
            for (measure_index, measure) in part.staves[0].measures.iter_mut().enumerate() {
                let written = if measure_index == 1 && part_index == 0 {
                    Step::C
                } else {
                    step.clone()
                };
                measure.voices[0] = vec![Note::new(Pitch::new(written, 4), Duration::Quarter)];
            }
        }
        let key = KeySignature {
            fifths: 0,
            mode: "major".to_string(),
        };
        let report = analyze_vertical_sonorities_in_key_report(&score, &key);
        assert!(report.timeline_issues.is_empty());
        assert_eq!(report.sonorities.len(), 2);
        assert!(
            report
                .sonorities
                .iter()
                .all(|item| item.label.chord.root == "C")
        );

        score.parts[0].staves[0].measures[0].key_sig = Some(KeySignature {
            fifths: 2,
            mode: "major".to_string(),
        });
        let ambiguous = analyze_vertical_sonorities_report(&score);
        assert_eq!(
            ambiguous.sonorities[0].label.roman_numeral.as_deref(),
            Some("I")
        );
        assert_eq!(ambiguous.sonorities[1].label.roman_numeral, None);

        score.parts[0].staves[0].measures[1].key_sig = Some(key);
        let reconciled = analyze_vertical_sonorities_report(&score);
        assert_eq!(
            reconciled.sonorities[1].label.roman_numeral.as_deref(),
            Some("I")
        );
    }

    #[test]
    fn e_flat_instrument_keeps_concert_chord_spelling() {
        let mut score = Score::new("alto sax and piano", 120, 4, 4, 0, 1);
        let part = score.parts[0].clone();
        score.parts.push(part.clone());
        score.parts.push(part);
        score.parts[0].staves[0].transpose_semitones = -9;
        for (part_index, pitch) in [
            Pitch::new(Step::C, 5),
            Pitch::new(Step::G, 4),
            Pitch::with_alter(Step::B, 4, -1),
        ]
        .into_iter()
        .enumerate()
        {
            score.parts[part_index].staves[0].measures[0].voices[0] =
                vec![Note::new(pitch, Duration::Quarter)];
        }
        let report = analyze_vertical_sonorities_report(&score);
        assert!(report.timeline_issues.is_empty());
        assert_eq!(report.sonorities[0].label.chord.root, "Eb");
    }

    #[test]
    fn unrepresentable_concert_pitch_omits_the_measure() {
        let mut score = Score::new("out of range", 120, 4, 4, 0, 1);
        score.parts[0].staves[0].transpose_semitones = 12;
        score.parts[0].staves[0].measures[0].voices[0] =
            vec![Note::new(Pitch::new(Step::B, 127), Duration::Quarter)];

        let report = analyze_vertical_sonorities_report(&score);
        assert!(report.sonorities.is_empty());
        assert_eq!(
            report.timeline_issues[0].kind,
            VerticalTimelineIssueKind::UnrepresentableConcertPitch
        );
    }

    #[test]
    fn vertical_sonorities_report_invalid_and_overfull_voice_timing() {
        let mut score = Score::default();
        score.parts[0].staves[0].measures[0].voices[0] = (0..5)
            .map(|_| Note::new(Pitch::new(Step::C, 4), Duration::Quarter))
            .collect();
        let report = analyze_vertical_sonorities_report(&score);
        assert!(report.sonorities.is_empty());
        assert_eq!(
            report.timeline_issues[0].kind,
            VerticalTimelineIssueKind::OverfullVoice
        );

        let mut malformed = Note::new(Pitch::new(Step::C, 4), Duration::Quarter);
        malformed.tuplet = Some(TupletInfo {
            actual_notes: 0,
            normal_notes: 2,
        });
        score.parts[0].staves[0].measures[0].voices[0] = vec![malformed];
        let report = analyze_vertical_sonorities_report(&score);
        assert!(report.sonorities.is_empty());
        assert_eq!(
            report.timeline_issues[0].kind,
            VerticalTimelineIssueKind::InvalidNoteDuration
        );

        score.parts[0].staves[0].measures[0].voices[0].clear();
        score.parts[0].staves[0].measures[0].actual_length = Some(MeasureLength {
            numerator: 1,
            denominator: 0,
        });
        let report = analyze_vertical_sonorities_report(&score);
        assert_eq!(
            report.timeline_issues[0].kind,
            VerticalTimelineIssueKind::InvalidLength
        );
    }

    #[test]
    fn authored_chord_symbol_wins_with_source_address_provenance() {
        use acorde_core::ChordSymbol;

        let mut score = Score::default();
        let voice = &mut score.parts[0].staves[0].measures[0].voices[0];
        voice.clear();
        let mut note = Note::new(Pitch::new(Step::C, 4), Duration::Quarter);
        note.chord_symbol = Some(ChordSymbol {
            root: "F".to_owned(),
            kind: "dominant".to_owned(),
            bass: Some("A".to_owned()),
            placement: None,
            extender: false,
            harmonic_degree: Some("4".to_owned()),
            harmony_function: Some("D".to_owned()),
            harmony_type: None,
            chord_ref: None,
            range_end: None,
            degrees: Vec::new(),
        });
        voice.push(note);

        let result = analyze_score(&score);
        assert_eq!(result.chords.len(), 1);
        assert_eq!(result.chords[0].rule_id, "authored-chord-symbol");
        assert_eq!(result.chords[0].address.note, 0);
        assert_eq!(
            result.chords[0].evidence,
            vec![result.chords[0].address.clone()]
        );
        assert_eq!(chord_name(&result.chords[0].chord), "F7/A");
        assert_eq!(result.chords[0].name, "F7/A");
        let finding = analysis_provenance(&result, &result.chords[0].address)
            .into_iter()
            .find(|finding| finding.category == AnalysisCategory::Chords)
            .expect("authored chord provenance");
        assert_eq!(finding.label.as_deref(), Some("F7/A"));
    }

    #[test]
    fn chord_name_preserves_authored_degree_extensions() {
        use acorde_core::{ChordDegree, ChordSymbol};

        let chord = ChordSymbol {
            root: "C".to_owned(),
            kind: "dominant".to_owned(),
            bass: None,
            placement: None,
            extender: false,
            harmonic_degree: None,
            harmony_function: None,
            harmony_type: None,
            chord_ref: None,
            range_end: None,
            degrees: vec![
                ChordDegree {
                    value: 9,
                    alter: 1,
                    kind: "add".to_owned(),
                },
                ChordDegree {
                    value: 5,
                    alter: -1,
                    kind: "alter".to_owned(),
                },
                ChordDegree {
                    value: 3,
                    alter: 0,
                    kind: "subtract".to_owned(),
                },
            ],
        };

        assert_eq!(chord_name(&chord), "C7add#9b5no3");
    }

    #[test]
    fn provenance_returns_stable_explanations_for_a_note() {
        let mut score = Score::default();
        let voice = &mut score.parts[0].staves[0].measures[0].voices[0];
        voice.clear();
        voice.push(Note::new(Pitch::new(Step::C, 4), Duration::Quarter));
        voice.push(Note::new(Pitch::new(Step::E, 4), Duration::Quarter));
        voice.push(Note::new(Pitch::new(Step::G, 4), Duration::Quarter));
        let address = NoteAddr {
            part: 0,
            staff: 0,
            measure: 0,
            voice: 0,
            note: 0,
        };
        let findings = analysis_provenance(&analyze_score(&score), &address);
        let chord = findings
            .iter()
            .find(|finding| finding.category == AnalysisCategory::Chords)
            .expect("chord provenance");
        assert_eq!(chord.rule_id, "pitch-class-template");
        assert_eq!(chord.label.as_deref(), Some("C"));
        assert_eq!(chord.evidence[0], address);
        assert!(findings.windows(2).all(|pair| {
            (analysis_category_rank(pair[0].category), &pair[0].rule_id)
                <= (analysis_category_rank(pair[1].category), &pair[1].rule_id)
        }));
    }

    #[test]
    fn change_explanation_combines_diff_and_before_after_provenance() {
        let mut previous_score = Score::default();
        let voice = &mut previous_score.parts[0].staves[0].measures[0].voices[0];
        voice.clear();
        for step in [Step::C, Step::E, Step::G] {
            voice.push(Note::new(Pitch::new(step, 4), Duration::Quarter));
        }
        let previous = analyze_score(&previous_score);
        let mut current_score = previous_score.clone();
        current_score.parts[0].staves[0].measures[0].voices[0][1] =
            Note::new(Pitch::new(Step::F, 4), Duration::Quarter);
        let current = analyze_score(&current_score);
        let address = NoteAddr {
            part: 0,
            staff: 0,
            measure: 0,
            voice: 0,
            note: 1,
        };

        let explanation = explain_analysis_change(&previous, &current, &address);
        assert!(!explanation.diff.is_empty());
        assert!(!explanation.previous.is_empty());
        assert!(!explanation.current.is_empty());
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
        assert!(result.cache_key().starts_with("analysis-v14-fnv1a64-"));
        assert_eq!(result.cache_key(), analysis_cache_key(&Score::default()));
        let mut changed = result.clone();
        changed.schema_version = ANALYSIS_SCHEMA_VERSION + 1;
        assert_ne!(result.cache_key(), changed.cache_key());
    }

    #[test]
    fn analysis_cache_reuses_identical_score_and_misses_after_edit() {
        let score = Score::default();
        let mut cache = AnalysisCache::with_capacity(2).unwrap();
        let first = cache.analyze(&score);
        assert_eq!(cache.len(), 1);
        assert!(cache.get(&score).is_some());
        let second = cache.analyze(&score);
        assert_eq!(first, second);

        let mut changed = score.clone();
        changed.metadata.title = "changed".to_owned();
        assert!(cache.get(&changed).is_none());
        let changed_result = cache.analyze(&changed);
        assert_ne!(first.score_fingerprint, changed_result.score_fingerprint);
        assert_eq!(cache.len(), 2);
    }

    #[test]
    fn analysis_cache_eviction_is_bounded_and_deterministic() {
        let mut cache = AnalysisCache::with_capacity(1).unwrap();
        let first = Score::default();
        let mut second = first.clone();
        second.metadata.title = "second".to_owned();
        cache.analyze(&first);
        cache.analyze(&second);
        assert_eq!(cache.len(), 1);
        assert!(cache.get(&first).is_none());
        assert!(cache.get(&second).is_some());
    }

    #[test]
    fn analysis_cache_batch_preserves_order_and_reuses_duplicate_scores() {
        let first = Score::default();
        let mut second = first.clone();
        second.metadata.title = "second".to_owned();
        let mut cache = AnalysisCache::with_capacity(2).unwrap();
        let results = cache.analyze_batch(&[second.clone(), first.clone(), second.clone()]);

        assert_eq!(results.len(), 3);
        assert_eq!(results[0].score_fingerprint, results[2].score_fingerprint);
        assert_ne!(results[0].score_fingerprint, results[1].score_fingerprint);
        assert_eq!(cache.len(), 2);
        assert_eq!(cache.stats(), AnalysisCacheStats { hits: 1, misses: 2 });
        cache.reset_stats();
        assert_eq!(cache.stats(), AnalysisCacheStats::default());
        assert!(cache.get(&first).is_some());
    }

    #[test]
    fn analysis_cache_invalidates_one_score_without_affecting_others() {
        let first = Score::default();
        let mut second = first.clone();
        second.metadata.title = "second".to_owned();
        let mut cache = AnalysisCache::with_capacity(2).unwrap();
        cache.analyze_batch(&[first.clone(), second.clone()]);

        assert!(cache.invalidate(&first));
        assert!(!cache.invalidate(&first));
        assert!(cache.get(&first).is_none());
        assert!(cache.get(&second).is_some());
        assert_eq!(cache.len(), 1);
    }

    #[test]
    fn analysis_cache_analyze_after_edit_reclaims_previous_snapshot() {
        let previous = Score::default();
        let mut current = previous.clone();
        current.metadata.title = "edited".to_owned();
        let mut cache = AnalysisCache::with_capacity(2).unwrap();
        cache.analyze(&previous);

        let result = cache.analyze_after_edit(&previous, &current);
        assert!(result.matches_score(&current));
        assert!(cache.get(&previous).is_none());
        assert!(cache.get(&current).is_some());
        assert_eq!(cache.len(), 1);
    }

    #[test]
    fn analysis_cache_analyze_after_noop_edit_reuses_existing_result() {
        let score = Score::default();
        let mut cache = AnalysisCache::with_capacity(1).unwrap();
        cache.analyze(&score);
        cache.reset_stats();

        let result = cache.analyze_after_edit(&score, &score);
        assert!(result.matches_score(&score));
        assert_eq!(cache.stats(), AnalysisCacheStats { hits: 1, misses: 0 });
        assert_eq!(cache.len(), 1);
    }

    #[test]
    fn analysis_cache_edit_with_diff_returns_result_and_changed_categories() {
        let previous = Score::default();
        let mut current = previous.clone();
        let voice = &mut current.parts[0].staves[0].measures[0].voices[0];
        voice.clear();
        for step in [Step::C, Step::E, Step::G] {
            voice.push(Note::new(Pitch::new(step, 4), Duration::Quarter));
        }
        let previous_result = analyze_score(&previous);
        let mut cache = AnalysisCache::with_capacity(2).unwrap();
        cache.analyze(&previous);

        let edited = cache.analyze_after_edit_with_diff(&previous, &previous_result, &current);
        assert!(edited.analysis.matches_score(&current));
        assert_eq!(edited.diff.changed_categories[0], AnalysisCategory::Chords);
        assert!(
            edited
                .diff
                .changed_categories
                .contains(&AnalysisCategory::Intervals)
        );
        assert!(cache.get(&previous).is_none());
        assert!(cache.get(&current).is_some());

        cache.reset_stats();
        let no_op = cache.analyze_after_edit_with_diff(&current, &edited.analysis, &current);
        assert!(no_op.diff.is_empty());
        assert_eq!(cache.stats(), AnalysisCacheStats { hits: 1, misses: 0 });
    }

    #[test]
    fn selected_analysis_reuses_unchanged_categories_and_rebinds_identity() {
        let previous = Score::default();
        let previous_result = analyze_score(&previous);
        let mut current = previous.clone();
        current.metadata.title = "edited".to_owned();
        let mut cache = AnalysisCache::with_capacity(2).unwrap();
        let result = cache.analyze_selected_after_edit(&previous, &previous_result, &current, &[]);

        assert!(result.matches_score(&current));
        assert_eq!(result.chords, previous_result.chords);
        assert_ne!(result.score_fingerprint, previous_result.score_fingerprint);
        assert_eq!(cache.stats(), AnalysisCacheStats { hits: 0, misses: 1 });
    }

    #[test]
    fn hinted_analysis_uses_clean_hint_without_recomputing_categories() {
        let previous = Score::default();
        let previous_result = analyze_score(&previous);
        let mut current = previous.clone();
        current.metadata.title = "edited".to_owned();
        let hint = ChangeHint {
            scope: acorde_core::ChangeScope::Global,
            layout_dirty: false,
            playback_dirty: false,
        };
        let mut cache = AnalysisCache::with_capacity(2).unwrap();
        let edited =
            cache.analyze_after_edit_with_hint(&previous, &previous_result, &current, &hint);

        assert!(edited.diff.is_empty());
        assert!(edited.analysis.matches_score(&current));
        assert_eq!(edited.analysis.chords, previous_result.chords);
    }

    #[test]
    fn analysis_cache_rejects_zero_capacity() {
        assert!(matches!(
            AnalysisCache::with_capacity(0),
            Err(AnalysisCacheError::ZeroCapacity)
        ));
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
    fn analysis_diff_is_empty_for_identical_results() {
        let result = analyze_score(&Score::default());
        let diff = diff_analysis(&result, &result);
        assert!(diff.is_empty());
        assert_eq!(diff.previous_score_fingerprint, result.score_fingerprint);
        assert_eq!(diff.current_score_fingerprint, result.score_fingerprint);
    }

    #[test]
    fn analysis_diff_keeps_score_identity_separate_from_category_changes() {
        let mut first_score = Score::default();
        first_score.metadata.title = "first".to_owned();
        let mut second_score = first_score.clone();
        second_score.metadata.title = "second".to_owned();
        let diff = diff_analysis(&analyze_score(&first_score), &analyze_score(&second_score));

        assert!(diff.is_empty());
        assert!(diff.changed_categories.is_empty());
        assert_ne!(
            diff.previous_score_fingerprint,
            diff.current_score_fingerprint
        );
    }

    #[test]
    fn analysis_diff_reports_changed_categories_in_stable_order() {
        let previous = analyze_score(&Score::default());
        let mut changed_score = Score::default();
        let voice = &mut changed_score.parts[0].staves[0].measures[0].voices[0];
        voice.clear();
        for step in [Step::C, Step::E, Step::G] {
            voice.push(Note::new(Pitch::new(step, 4), Duration::Quarter));
        }
        let current = analyze_score(&changed_score);
        let diff = diff_analysis(&previous, &current);

        assert!(!diff.is_empty());
        assert_eq!(diff.changed_categories[0], AnalysisCategory::Chords);
        assert!(
            diff.changed_categories
                .contains(&AnalysisCategory::Intervals)
        );
    }

    #[test]
    fn affected_categories_are_conservative_and_stable() {
        let clean = ChangeHint {
            scope: acorde_core::ChangeScope::Global,
            layout_dirty: false,
            playback_dirty: false,
        };
        assert!(affected_categories_for_change_hint(&clean).is_empty());

        let dirty = ChangeHint {
            scope: acorde_core::ChangeScope::Measures {
                part: 0,
                staff: 0,
                start: 1,
                end: 2,
            },
            layout_dirty: false,
            playback_dirty: true,
        };
        assert_eq!(
            affected_categories_for_change_hint(&dirty),
            vec![
                AnalysisCategory::Chords,
                AnalysisCategory::Intervals,
                AnalysisCategory::KeyEstimates,
                AnalysisCategory::CadenceCandidates,
                AnalysisCategory::VoiceLeading,
                AnalysisCategory::SatbDiagnostics,
                AnalysisCategory::Motifs,
                AnalysisCategory::PhraseBoundaries,
            ]
        );
    }

    #[test]
    fn refresh_plan_separates_local_and_global_dependencies() {
        let hint = ChangeHint {
            scope: acorde_core::ChangeScope::Measures {
                part: 1,
                staff: 2,
                start: 3,
                end: 4,
            },
            layout_dirty: true,
            playback_dirty: true,
        };
        let plan = analysis_refresh_plan(&hint);
        assert_eq!(
            plan.region,
            Some(AnalysisRegion {
                part: 1,
                staff: 2,
                start_measure: 3,
                end_measure: 4,
            })
        );
        assert!(plan.local_categories.contains(&AnalysisCategory::Chords));
        assert!(
            plan.local_categories
                .contains(&AnalysisCategory::CadenceCandidates)
        );
        assert_eq!(
            plan.global_categories,
            vec![AnalysisCategory::KeyEstimates, AnalysisCategory::Motifs]
        );
        assert_eq!(plan.context_before, 1);
        assert_eq!(plan.context_after, 1);
    }

    #[test]
    fn chord_region_keeps_outside_results_and_refreshes_inside() {
        let mut score = Score::default();
        for measure in score.parts[0].staves[0].measures.iter_mut().take(2) {
            let voice = &mut measure.voices[0];
            voice.clear();
            for step in [Step::C, Step::E, Step::G] {
                voice.push(Note::new(Pitch::new(step, 4), Duration::Quarter));
            }
        }
        let region = AnalysisRegion {
            part: 0,
            staff: 0,
            start_measure: 0,
            end_measure: 1,
        };
        let chords = analyze_chords_in_region(&score, Some(&region));
        assert_eq!(chords.len(), 1);
        assert_eq!(chords[0].address.measure, 0);
    }

    #[test]
    fn local_analysis_refresh_matches_full_pass_after_a_middle_measure_edit() {
        let mut score = Score::default();
        for measure in &mut score.parts[0].staves[0].measures {
            measure.voices[0] = [Step::C, Step::E, Step::G]
                .into_iter()
                .map(|step| Note::new(Pitch::new(step, 4), Duration::Quarter))
                .collect();
            measure.voices[1] = [Step::G, Step::A, Step::B]
                .into_iter()
                .map(|step| Note::new(Pitch::new(step, 3), Duration::Quarter))
                .collect();
        }
        let before = analyze_score(&score);
        score.parts[0].staves[0].measures[1].voices[0][1] =
            Note::new(Pitch::new(Step::F, 4), Duration::Quarter);
        score.parts[0].staves[0].measures[1].voices[1][1] =
            Note::new(Pitch::new(Step::F, 3), Duration::Quarter);
        let region = AnalysisRegion {
            part: 0,
            staff: 0,
            start_measure: 1,
            end_measure: 2,
        };
        let refreshed = analyze_selected_categories_in_region(
            &score,
            &before,
            &[
                AnalysisCategory::Chords,
                AnalysisCategory::Intervals,
                AnalysisCategory::VoiceLeading,
            ],
            Some(&region),
        );
        let full = analyze_score(&score);
        assert_eq!(refreshed.chords, full.chords);
        assert_eq!(refreshed.intervals, full.intervals);
        assert_eq!(refreshed.voice_leading, full.voice_leading);
        assert_eq!(refreshed.key_estimates, before.key_estimates);
    }

    #[test]
    fn interval_region_includes_boundary_observations() {
        let mut score = Score::default();
        for measure in score.parts[0].staves[0].measures.iter_mut().take(2) {
            let voice = &mut measure.voices[0];
            voice.clear();
            voice.push(Note::new(Pitch::new(Step::C, 4), Duration::Quarter));
            voice.push(Note::new(Pitch::new(Step::D, 4), Duration::Quarter));
        }
        let region = AnalysisRegion {
            part: 0,
            staff: 0,
            start_measure: 0,
            end_measure: 1,
        };
        let intervals = analyze_intervals_in_region(&score, Some(&region));
        assert_eq!(intervals.len(), 1);
        assert_eq!(intervals[0].from.measure, 0);
    }

    #[test]
    fn voice_leading_region_limits_measure_traversal() {
        let mut score = Score::default();
        for measure in score.parts[0].staves[0].measures.iter_mut().take(2) {
            measure.voices[0].clear();
            measure.voices[1].clear();
            measure.voices[0].push(Note::new(Pitch::new(Step::C, 4), Duration::Quarter));
            measure.voices[1].push(Note::new(Pitch::new(Step::G, 3), Duration::Quarter));
            measure.voices[0].push(Note::new(Pitch::new(Step::D, 4), Duration::Quarter));
            measure.voices[1].push(Note::new(Pitch::new(Step::A, 3), Duration::Quarter));
        }
        let region = AnalysisRegion {
            part: 0,
            staff: 0,
            start_measure: 1,
            end_measure: 2,
        };
        let observations = analyze_voice_leading_in_region(&score, Some(&region));
        assert_eq!(observations.len(), 1);
        assert_eq!(observations[0].upper.measure, 1);
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
    fn interval_analysis_treats_rests_as_melodic_boundaries() {
        let mut score = Score::default();
        let voice = &mut score.parts[0].staves[0].measures[0].voices[0];
        voice.clear();
        voice.push(Note::new(Pitch::new(Step::C, 4), Duration::Quarter));
        voice.push(Note::rest(Duration::Quarter));
        voice.push(Note::new(Pitch::new(Step::G, 4), Duration::Quarter));

        assert!(analyze_intervals(&score).is_empty());
    }

    #[test]
    fn interval_observation_preserves_exact_microtonal_distance() {
        let mut score = Score::default();
        let voice = &mut score.parts[0].staves[0].measures[0].voices[0];
        voice.clear();
        voice.push(Note::new(
            Pitch::try_with_microtone(Step::C, 4, 0, 25).expect("valid microtone"),
            Duration::Quarter,
        ));
        voice.push(Note::new(Pitch::new(Step::D, 4), Duration::Quarter));

        let intervals = analyze_intervals(&score);
        assert_eq!(intervals.len(), 1);
        assert_eq!(intervals[0].semitones, 2);
        assert_eq!(intervals[0].cents, 175);
    }

    #[test]
    fn key_estimates_report_duration_weighted_coverage() {
        let mut score = Score::default();
        let voice = &mut score.parts[0].staves[0].measures[0].voices[0];
        voice.clear();
        voice.push(Note::new(Pitch::new(Step::C, 4), Duration::Whole));
        voice.push(Note::new(Pitch::new(Step::D, 4), Duration::Eighth));

        let estimates = estimate_keys(&score);
        assert!(!estimates.is_empty());
        assert!(estimates.iter().all(|estimate| {
            (estimate.total_duration_beats - 4.5).abs() < f64::EPSILON
                && estimate.weighted_covered_beats <= estimate.total_duration_beats
                && estimate.rule_id == "duration-weighted-diatonic-pitch-coverage"
        }));
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
    fn tonal_profile_ranks_minor_center_without_discarding_legacy_candidates() {
        let mut score = Score::default();
        let voice = &mut score.parts[0].staves[0].measures[0].voices[0];
        voice.clear();
        for (step, duration) in [
            (Step::A, Duration::Whole),
            (Step::C, Duration::Quarter),
            (Step::E, Duration::Quarter),
            (Step::A, Duration::Whole),
        ] {
            voice.push(Note::new(Pitch::new(step, 4), duration));
        }

        let estimates = estimate_keys(&score);
        let original_keys: Vec<_> = estimates
            .iter()
            .map(|estimate| estimate.key.display_name())
            .collect();
        assert!(original_keys.contains(&"C major".to_string()));
        assert!(original_keys.contains(&"A minor".to_string()));
        let ranked = estimate_keys_with_tonal_profile(&score);
        assert_eq!(ranked[0].key.display_name(), "A minor");
        assert_eq!(ranked.len(), original_keys.len());
        assert!(
            ranked
                .iter()
                .all(|estimate| estimate.rule_id.ends_with("krumhansl-profile-ranking"))
        );
    }

    #[test]
    fn tonal_profile_does_not_round_microtones_into_key_evidence() {
        let mut score = Score::default();
        let voice = &mut score.parts[0].staves[0].measures[0].voices[0];
        voice.clear();
        voice.push(Note::new(Pitch::new(Step::A, 4), Duration::Quarter));
        voice.push(Note::new(
            Pitch::with_microtone(Step::C, 5, 0, 25),
            Duration::Quarter,
        ));
        let original = estimate_keys(&score);
        assert_eq!(estimate_keys_with_tonal_profile(&score), original);
    }

    #[test]
    fn aarden_essen_profile_keeps_candidates_discarded_by_coverage() {
        let mut score = Score::default();
        let voice = &mut score.parts[0].staves[0].measures[0].voices[0];
        voice.clear();
        for step in [
            Step::A,
            Step::B,
            Step::C,
            Step::D,
            Step::E,
            Step::F,
            Step::G,
        ] {
            voice.push(Note::new(Pitch::new(step, 4), Duration::Quarter));
        }
        let coverage = estimate_keys(&score);
        let full = estimate_keys_with_aarden_essen_profile(&score);
        assert!(coverage.len() < full.len());
        assert_eq!(full.len(), 30);
        assert!(
            full.iter()
                .any(|estimate| estimate.key.display_name() == "A major")
        );
        assert!(full.iter().all(|estimate| {
            estimate.rule_id == "duration-weighted-aarden-essen-profile-ranking"
        }));
    }

    #[test]
    fn aarden_essen_profile_falls_back_on_microtones() {
        let mut score = Score::default();
        let voice = &mut score.parts[0].staves[0].measures[0].voices[0];
        voice.clear();
        voice.push(Note::new(Pitch::new(Step::A, 4), Duration::Quarter));
        voice.push(Note::new(
            Pitch::with_microtone(Step::C, 5, 0, 25),
            Duration::Quarter,
        ));
        assert_eq!(
            estimate_keys_with_aarden_essen_profile(&score),
            estimate_keys(&score)
        );
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
        assert_eq!(observations[0].upper_motion_cents, 200);
        assert_eq!(observations[0].lower_motion_cents, 200);
        assert_eq!(observations[0].evidence.len(), 2);
        let diagnostics = analyze_satb(&score);
        assert_eq!(diagnostics.len(), 1);
        assert_eq!(diagnostics[0].kind, SatbDiagnosticKind::ParallelPerfect);
        assert_eq!(diagnostics[0].severity, SatbSeverity::Warning);
    }

    #[test]
    fn located_quartets_include_cross_bar_nonadjacent_voices() {
        let mut score = Score::default();
        for (measure_index, pitches) in [
            [(Step::G, 4), (Step::E, 4), (Step::C, 4)],
            [(Step::A, 4), (Step::F, 4), (Step::D, 4)],
        ]
        .into_iter()
        .enumerate()
        {
            let measure = &mut score.parts[0].staves[0].measures[measure_index];
            for (voice_index, (step, octave)) in pitches.into_iter().enumerate() {
                measure.voices[voice_index] =
                    vec![Note::new(Pitch::new(step, octave), Duration::Whole)];
            }
        }
        let quartets = analyze_voice_leading_quartets(&score);
        assert_eq!(quartets.len(), 3);
        let outer = quartets
            .iter()
            .find(|quartet| quartet.second_before.address.voice == 2)
            .unwrap();
        assert_eq!(outer.first_before.address.measure, 0);
        assert_eq!(outer.first_after.address.measure, 1);
        assert_eq!(outer.first_after.onset_beats, 4.0);
        assert_eq!(outer.second_before.duration_beats, 4.0);
        assert!(outer.similar_motion);
        assert!(outer.parallel_fifth);
        assert_eq!(outer.evidence.len(), 4);
    }

    #[test]
    fn located_quartets_separate_perfect_spelling_and_motion() {
        let mut score = Score::default();
        let measure = &mut score.parts[0].staves[0].measures[0];
        measure.voices[0] = vec![
            Note::new(Pitch::new(Step::G, 4), Duration::Quarter),
            Note::new(Pitch::new(Step::A, 4), Duration::Quarter),
        ];
        measure.voices[1] = vec![
            Note::new(Pitch::new(Step::C, 4), Duration::Quarter),
            Note::new(Pitch::new(Step::D, 3), Duration::Quarter),
        ];
        let quartet = &analyze_voice_leading_quartets(&score)[0];
        assert!(quartet.contrary_motion);
        assert!(!quartet.similar_motion);
        assert!(quartet.parallel_fifth);
        assert!(!quartet.parallel_octave);

        score.parts[0].staves[0].measures[0].voices[1][0].pitches[0] =
            Pitch::with_alter(Step::C, 4, 1);
        assert!(!analyze_voice_leading_quartets(&score)[0].parallel_fifth);
    }

    #[test]
    fn located_quartets_align_distinct_parts_and_reject_overfull_bars() {
        let mut score = Score::default();
        score.parts.push(score.parts[0].clone());
        score.parts[0].staves[0].measures[0].voices[0] = vec![
            Note::new(Pitch::new(Step::C, 5), Duration::Half),
            Note::new(Pitch::new(Step::D, 5), Duration::Half),
        ];
        score.parts[1].staves[0].measures[0].voices[0] = vec![
            Note::new(Pitch::new(Step::C, 4), Duration::Half),
            Note::new(Pitch::new(Step::D, 4), Duration::Half),
        ];
        let quartets = analyze_voice_leading_quartets(&score);
        assert_eq!(quartets.len(), 1);
        assert_eq!(quartets[0].first_before.address.part, 0);
        assert_eq!(quartets[0].second_before.address.part, 1);
        assert!(quartets[0].parallel_octave);

        score.parts[1].staves[0].measures[0].voices[0]
            .push(Note::new(Pitch::new(Step::E, 4), Duration::Quarter));
        assert!(analyze_voice_leading_quartets(&score).is_empty());
    }

    #[test]
    fn located_quartets_do_not_bridge_rests_or_call_unisons_octaves() {
        let mut score = Score::default();
        {
            let measure = &mut score.parts[0].staves[0].measures[0];
            for voice in measure.voices.iter_mut().take(2) {
                *voice = vec![
                    Note::new(Pitch::new(Step::C, 4), Duration::Quarter),
                    Note::rest(Duration::Quarter),
                    Note::new(Pitch::new(Step::D, 4), Duration::Quarter),
                ];
            }
        }
        assert!(analyze_voice_leading_quartets(&score).is_empty());
        for voice in score.parts[0].staves[0].measures[0]
            .voices
            .iter_mut()
            .take(2)
        {
            voice.remove(1);
        }
        let quartet = &analyze_voice_leading_quartets(&score)[0];
        assert!(quartet.similar_motion);
        assert!(!quartet.parallel_octave);
    }

    #[test]
    fn keyless_quartet_resolution_checks_each_dissonance_family() {
        let pitch = |step, octave| Pitch::new(step, octave);
        let flat = |step, octave| Pitch::with_alter(step, octave, -1);
        let resolves = |a: Pitch, b: Pitch, c: Pitch, d: Pitch| {
            let first_motion = b.to_midi_cents() - a.to_midi_cents();
            let second_motion = d.to_midi_cents() - c.to_midi_cents();
            keyless_quartet_resolution(&a, &b, &c, &d, first_motion, second_motion)
        };
        // Perfect fourth: the first voice may stay or descend.
        assert!(!resolves(
            pitch(Step::F, 5),
            pitch(Step::G, 5),
            pitch(Step::C, 4),
            pitch(Step::C, 4)
        ));
        assert!(resolves(
            pitch(Step::F, 5),
            pitch(Step::E, 5),
            pitch(Step::C, 4),
            pitch(Step::C, 4)
        ));
        // Augmented fourth opens to a sixth; diminished fifth closes to a third.
        assert!(resolves(
            pitch(Step::E, 5),
            pitch(Step::F, 5),
            flat(Step::B, 4),
            pitch(Step::A, 4)
        ));
        assert!(resolves(
            flat(Step::B, 4),
            pitch(Step::A, 4),
            pitch(Step::E, 4),
            pitch(Step::F, 4)
        ));
        // Minor seventh must arrive at a generic third.
        assert!(resolves(
            flat(Step::B, 4),
            pitch(Step::A, 4),
            pitch(Step::C, 4),
            pitch(Step::F, 4)
        ));
        assert!(!resolves(
            flat(Step::B, 4),
            pitch(Step::A, 4),
            pitch(Step::C, 4),
            pitch(Step::G, 4)
        ));
    }

    #[test]
    fn voice_leading_does_not_bridge_rests() {
        let mut score = Score::default();
        let measure = &mut score.parts[0].staves[0].measures[0];
        measure.voices[0].clear();
        measure.voices[1].clear();
        let (upper_voices, lower_voices) = measure.voices.split_at_mut(1);
        for voice in [&mut upper_voices[0], &mut lower_voices[0]] {
            voice.push(Note::new(Pitch::new(Step::C, 4), Duration::Quarter));
            voice.push(Note::rest(Duration::Quarter));
            voice.push(Note::new(Pitch::new(Step::D, 4), Duration::Quarter));
        }
        assert!(analyze_voice_leading(&score).is_empty());
    }

    #[test]
    fn voice_leading_uses_written_onsets_instead_of_matching_note_indices() {
        let mut score = Score::default();
        let measure = &mut score.parts[0].staves[0].measures[0];
        measure.voices[0] = vec![
            Note::new(Pitch::new(Step::C, 4), Duration::Quarter),
            Note::new(Pitch::new(Step::D, 4), Duration::Quarter),
        ];
        measure.voices[1] = vec![
            Note::new(Pitch::new(Step::C, 3), Duration::Half),
            Note::new(Pitch::new(Step::D, 3), Duration::Quarter),
        ];
        let observations = analyze_voice_leading(&score);
        assert_eq!(observations.len(), 1);
        assert_eq!(observations[0].upper_motion_cents, 200);
        assert_eq!(observations[0].lower_motion_cents, 0);
        assert!(!observations[0].parallel_perfect);
        assert_eq!(observations[0].lower.note, 0);
    }

    #[test]
    fn satb_does_not_compare_notes_at_different_written_beats() {
        let mut score = Score::default();
        let measure = &mut score.parts[0].staves[0].measures[0];
        measure.voices[0] = vec![
            Note::new(Pitch::new(Step::C, 4), Duration::Quarter),
            Note::new(Pitch::new(Step::D, 4), Duration::Quarter),
        ];
        measure.voices[1] = vec![
            Note::new(Pitch::new(Step::C, 3), Duration::Half),
            Note::new(Pitch::new(Step::D, 5), Duration::Quarter),
        ];
        assert!(analyze_satb(&score).is_empty());
    }

    #[test]
    fn written_timeline_uses_actual_length_for_a_lone_whole_rest() {
        let mut score = Score::new("pickup", 120, 3, 4, 0, 1);
        let measure = &mut score.parts[0].staves[0].measures[0];
        measure.voices[0] = vec![Note::rest(Duration::Whole)];
        let events = written_measure_events(&score, 0);
        assert_eq!(events.len(), 1);
        assert_eq!(events[0].start, 0.0);
        assert_eq!(events[0].end, 3.0);
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
    fn satb_region_replaces_local_findings_and_preserves_outside_findings() {
        let mut previous_score = Score::default();
        for measure in &mut previous_score.parts[0].staves[0].measures {
            measure.voices[0].clear();
            measure.voices[1].clear();
            measure.voices[0].push(Note::new(Pitch::new(Step::C, 3), Duration::Quarter));
            measure.voices[1].push(Note::new(Pitch::new(Step::C, 4), Duration::Quarter));
        }
        let previous = analyze_score(&previous_score);

        let mut edited_score = previous_score.clone();
        let measure = &mut edited_score.parts[0].staves[0].measures[0];
        measure.voices[0][0] = Note::new(Pitch::new(Step::C, 4), Duration::Quarter);
        let region = AnalysisRegion {
            part: 0,
            staff: 0,
            start_measure: 0,
            end_measure: 1,
        };
        let refreshed = analyze_selected_categories_in_region(
            &edited_score,
            &previous,
            &[AnalysisCategory::SatbDiagnostics],
            Some(&region),
        );

        assert_eq!(refreshed.satb_diagnostics.len(), 3);
        assert!(
            refreshed
                .satb_diagnostics
                .iter()
                .all(|diagnostic| diagnostic.upper.measure >= 1)
        );
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
