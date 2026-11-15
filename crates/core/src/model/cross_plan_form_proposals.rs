//! Rank complete sectioned arrangements across distinct chord plans.

use serde::{Deserialize, Serialize};

use super::arrangement_proposals::{
    ArrangementProposalDiagnostic, ArrangementProposalRequest, ResolvedArrangementProposal,
    suggest_arrangement_proposals,
};
use super::chord_plan::{ChordPlan, chord_plan_to_preset_selection};
use super::chord_proposals::fingerprint;
use super::developed_form_proposals::{
    DevelopedFormAxisDistance, DevelopedFormProposalError, ObservedAxes,
    ResolvedDevelopedFormProposal, observed_difference_codes, replay_developed_form_proposal,
    suggest_developed_form_proposals,
};
use super::ensemble_form_proposals::{
    EnsembleFormProposalRequest, FormProposalBoundary, signature,
};
use super::score::Score;

pub const CROSS_PLAN_FORM_CONTRACT_VERSION: u16 = 1;
const MAX_LOCKS: usize = 512;

fn harmonic_signature(plan: &ChordPlan) -> Vec<(usize, u32, u32, String)> {
    plan.entries
        .iter()
        .map(|entry| {
            (
                entry.position.measure_index,
                entry.position.offset.numerator,
                entry.position.offset.denominator,
                serde_json::to_string(&entry.chord).unwrap_or_default(),
            )
        })
        .collect()
}

#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
#[serde(deny_unknown_fields)]
pub struct CrossPlanFormRequest {
    /// Harmony search, source locks, role ranges and final candidate count.
    pub arrangement: ArrangementProposalRequest,
    /// Three or more adjacent form sections covering the source score.
    pub sections: Vec<FormProposalBoundary>,
    /// Empty derives every section end, as in `EnsembleFormProposalRequest`.
    #[serde(default)]
    pub phrase_end_measures: Vec<usize>,
}

#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
#[serde(deny_unknown_fields)]
pub struct ResolvedCrossPlanForm {
    pub contract_version: u16,
    pub source_fingerprint: String,
    pub request: CrossPlanFormRequest,
    /// Base search count that admitted the selected harmonic plan.
    pub base_count: u8,
    pub arrangement: ResolvedArrangementProposal,
    pub developed: ResolvedDevelopedFormProposal,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct CrossPlanFormCandidate {
    pub id: String,
    pub score: Score,
    pub plan: ChordPlan,
    /// Number of original source-plan spans changed by harmony search.
    pub changed_harmony_spans: usize,
    pub resolved: ResolvedCrossPlanForm,
    pub axis_distance_from_first: DevelopedFormAxisDistance,
    pub nearest_selected_distance: u32,
    pub difference_codes: Vec<String>,
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct CrossPlanRejection {
    pub harmony_candidate_id: String,
    pub code: String,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct CrossPlanFormResult {
    pub contract_version: u16,
    pub source_fingerprint: String,
    pub candidates: Vec<CrossPlanFormCandidate>,
    pub rejections: Vec<CrossPlanRejection>,
}

#[derive(Debug, thiserror::Error)]
pub enum CrossPlanFormError {
    #[error("invalid cross-plan form request")]
    InvalidRequest,
    #[error("base harmony/arrangement search failed: {0:?}")]
    Base(Vec<ArrangementProposalDiagnostic>),
    #[error("fewer than two distinct harmonic plans survived form realization")]
    InsufficientHarmonicPlans,
    #[error("only {found} distinct cross-plan forms were available")]
    InsufficientDistinct {
        found: usize,
        rejections: Vec<CrossPlanRejection>,
    },
    #[error("unsupported cross-plan form contract")]
    UnsupportedContract,
    #[error("cross-plan form source mismatch")]
    SourceMismatch,
    #[error("developed form replay failed: {0}")]
    Replay(#[from] DevelopedFormProposalError),
}

fn valid_request(request: &CrossPlanFormRequest) -> bool {
    (3..=5).contains(&request.arrangement.count)
        && request.arrangement.locked_chord_entries.len() <= MAX_LOCKS
        && request.arrangement.ensemble_template.harmony.is_some()
        && request.arrangement.ensemble_template.bass.is_some()
        && request
            .arrangement
            .ensemble_template
            .countermelody
            .is_some()
        && (3..=64).contains(&request.sections.len())
}

fn base_search(
    source: &Score,
    request: &ArrangementProposalRequest,
) -> Result<
    (
        u8,
        Vec<super::arrangement_proposals::ArrangementProposalCandidate>,
    ),
    CrossPlanFormError,
> {
    let mut last = Vec::new();
    for count in (3..=5).rev() {
        let mut attempt = request.clone();
        attempt.count = count;
        let result = suggest_arrangement_proposals(source, &attempt);
        if !result.candidates.is_empty() {
            return Ok((count, result.candidates));
        }
        last = result.diagnostics;
    }
    Err(CrossPlanFormError::Base(last))
}

fn form_request(
    request: &CrossPlanFormRequest,
    plan: &ChordPlan,
) -> Option<EnsembleFormProposalRequest> {
    let chords = chord_plan_to_preset_selection(plan, request.arrangement.harmony.bass_mode)
        .selection?
        .chords;
    let mut ensemble = request.arrangement.ensemble_template.clone();
    ensemble.harmony.as_mut()?.chords = chords.clone();
    ensemble.bass.as_mut()?.chords = chords.clone();
    ensemble.countermelody.as_mut()?.chords = chords;
    Some(EnsembleFormProposalRequest {
        ensemble,
        sections: request.sections.clone(),
        phrase_end_measures: request.phrase_end_measures.clone(),
        count: 5,
    })
}

/// Select three to five complete scores from up to four chord plans, five
/// section profiles and five rhythmic countermelody treatments. The existing
/// harmony search enforces source locks and changed-region budgets before any
/// form is realized. At least two distinct harmonic plans must survive.
pub fn suggest_cross_plan_forms(
    source: &Score,
    request: &CrossPlanFormRequest,
) -> Result<CrossPlanFormResult, CrossPlanFormError> {
    if !valid_request(request) {
        return Err(CrossPlanFormError::InvalidRequest);
    }
    let source_fingerprint = fingerprint(source).ok_or(CrossPlanFormError::InvalidRequest)?;
    let (base_count, base) = base_search(source, &request.arrangement)?;
    // Keep the first complete arrangement contract for each harmonic plan.
    // It binds that plan to the exact source and all requested chord locks.
    let mut base_by_plan = Vec::new();
    let mut base_signatures = Vec::new();
    for candidate in base {
        let plan_signature = harmonic_signature(&candidate.plan);
        if !base_signatures.contains(&plan_signature) {
            base_signatures.push(plan_signature);
            base_by_plan.push(candidate);
        }
    }
    let mut pool = Vec::new();
    let mut signatures = Vec::new();
    let mut rejections = Vec::new();
    for harmonic in base_by_plan {
        let Some(mut form_request) = form_request(request, &harmonic.plan) else {
            rejections.push(CrossPlanRejection {
                harmony_candidate_id: harmonic.resolved.harmony_candidate_id.clone(),
                code: "unusable-chord-plan".into(),
            });
            continue;
        };
        let mut developed = None;
        let mut last_error = None;
        for count in (3..=5).rev() {
            form_request.count = count;
            match suggest_developed_form_proposals(source, &form_request) {
                Ok(result) => {
                    developed = Some(result);
                    break;
                }
                Err(error) => last_error = Some(error),
            }
        }
        let Some(developed) = developed else {
            rejections.push(CrossPlanRejection {
                harmony_candidate_id: harmonic.resolved.harmony_candidate_id.clone(),
                code: last_error.map_or_else(
                    || "form-search-failed".into(),
                    |error| format!("form-search-failed:{error}"),
                ),
            });
            continue;
        };
        for candidate in developed.candidates {
            let event_signature = signature(&candidate.score, source.parts.len());
            if signatures.contains(&event_signature) {
                continue;
            }
            signatures.push(event_signature);
            let axes = ObservedAxes::collect(
                &candidate.score,
                &candidate.role_parts,
                &candidate.measures,
                &candidate.resolved.development.form.request.sections,
            )
            .with_plan(&harmonic.plan);
            pool.push((
                CrossPlanFormCandidate {
                    id: format!(
                        "{}::{}",
                        harmonic.resolved.harmony_candidate_id, candidate.id
                    ),
                    score: candidate.score,
                    plan: harmonic.plan.clone(),
                    changed_harmony_spans: harmonic.features.changed_harmony_spans,
                    resolved: ResolvedCrossPlanForm {
                        contract_version: CROSS_PLAN_FORM_CONTRACT_VERSION,
                        source_fingerprint: source_fingerprint.clone(),
                        request: request.clone(),
                        base_count,
                        arrangement: harmonic.resolved.clone(),
                        developed: candidate.resolved,
                    },
                    axis_distance_from_first: DevelopedFormAxisDistance::default(),
                    nearest_selected_distance: 0,
                    difference_codes: Vec::new(),
                },
                axes,
            ));
        }
    }
    let wanted = usize::from(request.arrangement.count);
    if pool.len() < wanted {
        return Err(CrossPlanFormError::InsufficientDistinct {
            found: pool.len(),
            rejections,
        });
    }
    if pool.iter().all(|(candidate, _)| {
        harmonic_signature(&candidate.plan) == harmonic_signature(&pool[0].0.plan)
    }) {
        return Err(CrossPlanFormError::InsufficientHarmonicPlans);
    }
    let mut selected = Vec::with_capacity(wanted);
    let (mut reference, reference_axes) = pool.remove(0);
    reference.difference_codes.push("reference-form".into());
    selected.push((reference, reference_axes));
    while selected.len() < wanted {
        let index = pool
            .iter()
            .enumerate()
            .max_by_key(|(index, (candidate, axes))| {
                let new_plan = !selected.iter().any(|(chosen, _)| {
                    harmonic_signature(&chosen.plan) == harmonic_signature(&candidate.plan)
                });
                let nearest = selected
                    .iter()
                    .map(|(_, chosen)| axes.distance(chosen).total())
                    .min()
                    .unwrap_or_default();
                let new_kind = !selected.iter().any(|(chosen, _)| {
                    chosen.resolved.developed.development.countermelody_kind
                        == candidate.resolved.developed.development.countermelody_kind
                });
                (
                    u8::from(new_plan),
                    nearest,
                    u8::from(new_kind),
                    std::cmp::Reverse(*index),
                )
            })
            .map(|(index, _)| index)
            .ok_or_else(|| CrossPlanFormError::InsufficientDistinct {
                found: selected.len(),
                rejections: rejections.clone(),
            })?;
        let (mut candidate, axes) = pool.remove(index);
        let distance = axes.distance(&selected[0].1);
        candidate.axis_distance_from_first = distance;
        candidate.nearest_selected_distance = selected
            .iter()
            .map(|(_, chosen)| axes.distance(chosen).total())
            .min()
            .unwrap_or_default();
        candidate.difference_codes = observed_difference_codes(distance);
        if harmonic_signature(&candidate.plan) != harmonic_signature(&selected[0].0.plan) {
            candidate
                .difference_codes
                .push("harmony-plan-changed".into());
        }
        selected.push((candidate, axes));
    }
    Ok(CrossPlanFormResult {
        contract_version: CROSS_PLAN_FORM_CONTRACT_VERSION,
        source_fingerprint,
        candidates: selected
            .into_iter()
            .map(|(candidate, _)| candidate)
            .collect(),
        rejections,
    })
}

/// Recheck source locks, selected harmony and sectioned score against the
/// original score. No part of a failed replay is returned.
pub fn replay_cross_plan_form(
    source: &Score,
    resolved: &ResolvedCrossPlanForm,
) -> Result<Score, CrossPlanFormError> {
    if resolved.contract_version != CROSS_PLAN_FORM_CONTRACT_VERSION
        || !valid_request(&resolved.request)
        || !(3..=5).contains(&resolved.base_count)
        || resolved.arrangement.source_fingerprint != resolved.source_fingerprint
        || resolved.developed.source_fingerprint != resolved.source_fingerprint
    {
        return Err(CrossPlanFormError::UnsupportedContract);
    }
    if fingerprint(source).as_deref() != Some(resolved.source_fingerprint.as_str()) {
        return Err(CrossPlanFormError::SourceMismatch);
    }
    let mut base_request = resolved.request.arrangement.clone();
    base_request.count = resolved.base_count;
    let base = suggest_arrangement_proposals(source, &base_request);
    if !base
        .candidates
        .iter()
        .any(|candidate| candidate.resolved == resolved.arrangement)
    {
        return Err(CrossPlanFormError::UnsupportedContract);
    }
    let expected = form_request(&resolved.request, &resolved.arrangement.plan)
        .ok_or(CrossPlanFormError::UnsupportedContract)?;
    let actual = &resolved.developed.development.form.request;
    let phrase_ends = if expected.phrase_end_measures.is_empty() {
        expected
            .sections
            .iter()
            .map(|section| section.end_measure_exclusive - 1)
            .collect::<Vec<_>>()
    } else {
        expected.phrase_end_measures.clone()
    };
    if actual.ensemble != expected.ensemble
        || actual.phrase_end_measures != phrase_ends
        || actual.sections.len() != expected.sections.len()
        || actual
            .sections
            .iter()
            .zip(&expected.sections)
            .any(|(section, boundary)| {
                section.label != boundary.label
                    || section.start_measure != boundary.start_measure
                    || section.end_measure_exclusive != boundary.end_measure_exclusive
            })
    {
        return Err(CrossPlanFormError::UnsupportedContract);
    }
    Ok(replay_developed_form_proposal(source, &resolved.developed)?)
}
