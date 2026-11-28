//! One entry point for the arrangement proposal engines.
//!
//! This module is behind the `unstable-arrangement` cargo feature. Its types may
//! change in a minor release.
//!
//! [`suggest_arrangements`] takes one [`ArrangementRequest`] that names an engine,
//! runs it, and returns candidates in one shape: a score, a tagged resolved contract
//! and difference codes. [`replay_arrangement`] rebuilds a candidate from that
//! contract. Each engine keeps its own ranking, limits and replay checks; this
//! module only routes and normalizes. With an assessment request it also attaches
//! [`assess_arrangement`] counts and can set aside candidates over given limits.

use serde::{Deserialize, Serialize};

use super::arrangement_assessment::{
    ArrangementAssessment, ArrangementAssessmentError, ArrangementAssessmentRequest,
    assess_arrangement,
};
use super::arrangement_proposals::{
    ArrangementProposalRequest, ResolvedArrangementProposal, replay_arrangement_proposal,
    suggest_arrangement_proposals,
};
use super::cross_plan_form_proposals::{
    CrossPlanFormRequest, ResolvedCrossPlanForm, replay_cross_plan_form, suggest_cross_plan_forms,
};
use super::developed_form_proposals::{
    ResolvedDevelopedFormProposal, replay_developed_form_proposal, suggest_developed_form_proposals,
};
use super::ensemble_form_proposals::{
    EnsembleFormProposalRequest, ResolvedFormProposal, replay_ensemble_form_proposal,
    suggest_ensemble_form_proposals,
};
use super::harmony_groove_proposals::{
    HarmonyGrooveRequest, ResolvedHarmonyGrooveArrangement, replay_harmony_groove_arrangement,
    suggest_harmony_groove_arrangements,
};
use super::integrated_arrangement_proposals::{
    IntegratedArrangementRequest, ResolvedIntegratedArrangement, replay_integrated_arrangement,
    suggest_integrated_arrangements,
};
use super::score::Score;

/// Version of the unified request and result contract.
pub const ARRANGEMENT_SUGGESTIONS_CONTRACT_VERSION: u16 = 1;

/// The engine to run and its own request.
///
/// In JSON: `{"engine": "roles", "request": { ... }}`.
#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
#[serde(tag = "engine", content = "request", rename_all = "kebab-case")]
#[non_exhaustive]
pub enum ArrangementEngineRequest {
    /// Harmony plan plus pad, bass and countermelody roles
    /// ([`suggest_arrangement_proposals`]).
    Roles(ArrangementProposalRequest),
    /// Phrase harmony, rhythm section and melody techniques
    /// ([`suggest_integrated_arrangements`]).
    Integrated(IntegratedArrangementRequest),
    /// Plain, color and rhythm harmony/comping alternatives
    /// ([`suggest_harmony_groove_arrangements`]).
    HarmonyGroove(HarmonyGrooveRequest),
    /// Section entrances and textures ([`suggest_ensemble_form_proposals`]).
    EnsembleForm(EnsembleFormProposalRequest),
    /// Sectional forms with countermelody treatments
    /// ([`suggest_developed_form_proposals`]).
    DevelopedForm(EnsembleFormProposalRequest),
    /// Sectional forms across distinct chord plans ([`suggest_cross_plan_forms`]).
    CrossPlanForm(CrossPlanFormRequest),
}

impl ArrangementEngineRequest {
    /// Stable engine name, as used in JSON.
    pub fn engine(&self) -> &'static str {
        match self {
            Self::Roles(_) => "roles",
            Self::Integrated(_) => "integrated",
            Self::HarmonyGroove(_) => "harmony-groove",
            Self::EnsembleForm(_) => "ensemble-form",
            Self::DevelopedForm(_) => "developed-form",
            Self::CrossPlanForm(_) => "cross-plan-form",
        }
    }
}

/// Upper limits on assessment counts. A candidate over any limit is set aside.
#[derive(Debug, Clone, Default, PartialEq, Eq, Serialize, Deserialize)]
pub struct ArrangementAssessmentLimits {
    #[serde(default)]
    pub max_melody_masked_ticks: Option<u64>,
    #[serde(default)]
    pub max_melody_semitone_clash_ticks: Option<u64>,
    /// Outer parallel fifths plus outer parallel octaves.
    #[serde(default)]
    pub max_outer_parallels: Option<usize>,
    #[serde(default)]
    pub max_out_of_range_notes: Option<usize>,
    #[serde(default)]
    pub max_bass_leaps: Option<usize>,
}

impl ArrangementAssessmentLimits {
    /// Names of the limits `assessment` exceeds, in field order.
    pub fn exceeded(&self, assessment: &ArrangementAssessment) -> Vec<String> {
        let mut exceeded = Vec::new();
        let mut check = |name: &str, limit: Option<u64>, value: u64| {
            if limit.is_some_and(|limit| value > limit) {
                exceeded.push(name.to_owned());
            }
        };
        check(
            "max-melody-masked-ticks",
            self.max_melody_masked_ticks,
            assessment.melody_masked_ticks,
        );
        check(
            "max-melody-semitone-clash-ticks",
            self.max_melody_semitone_clash_ticks,
            assessment.melody_semitone_clash_ticks,
        );
        check(
            "max-outer-parallels",
            self.max_outer_parallels.map(|v| v as u64),
            (assessment.outer_parallel_fifths + assessment.outer_parallel_octaves) as u64,
        );
        check(
            "max-out-of-range-notes",
            self.max_out_of_range_notes.map(|v| v as u64),
            assessment.out_of_range_notes as u64,
        );
        check(
            "max-bass-leaps",
            self.max_bass_leaps.map(|v| v as u64),
            assessment.bass_leaps as u64,
        );
        exceeded
    }
}

/// Assess every candidate and optionally set aside those over limits.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct ArrangementAssessmentOptions {
    pub request: ArrangementAssessmentRequest,
    #[serde(default)]
    pub limits: ArrangementAssessmentLimits,
}

/// One request for [`suggest_arrangements`].
#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
pub struct ArrangementRequest {
    #[serde(flatten)]
    pub engine: ArrangementEngineRequest,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub assessment: Option<ArrangementAssessmentOptions>,
}

impl ArrangementRequest {
    /// A request without assessment.
    pub fn new(engine: ArrangementEngineRequest) -> Self {
        Self {
            engine,
            assessment: None,
        }
    }
}

/// The engine-specific contract that [`replay_arrangement`] rebuilds a candidate from.
///
/// In JSON: `{"engine": "roles", "resolved": { ... }}`.
#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
#[serde(tag = "engine", content = "resolved", rename_all = "kebab-case")]
#[non_exhaustive]
pub enum ResolvedArrangement {
    Roles(Box<ResolvedArrangementProposal>),
    Integrated(Box<ResolvedIntegratedArrangement>),
    HarmonyGroove(Box<ResolvedHarmonyGrooveArrangement>),
    EnsembleForm(Box<ResolvedFormProposal>),
    DevelopedForm(Box<ResolvedDevelopedFormProposal>),
    CrossPlanForm(Box<ResolvedCrossPlanForm>),
}

/// One complete arranged score.
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct ArrangementCandidate {
    /// Engine-provided ID, or `<engine>-<index>` when the engine has none.
    pub id: String,
    pub score: Score,
    pub resolved: ResolvedArrangement,
    /// The engine's difference codes relative to its first candidate.
    pub difference_codes: Vec<String>,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub assessment: Option<ArrangementAssessment>,
}

/// A candidate set aside by assessment limits.
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct SetAsideArrangement {
    pub candidate: ArrangementCandidate,
    /// Names of the exceeded limits.
    pub exceeded: Vec<String>,
}

/// Result of [`suggest_arrangements`].
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct ArrangementSuggestions {
    pub contract_version: u16,
    pub engine: String,
    /// Candidates in the engine's order, without those set aside.
    pub candidates: Vec<ArrangementCandidate>,
    pub set_aside: Vec<SetAsideArrangement>,
    /// Number of rejected attempts or diagnostics the engine reported.
    pub engine_rejections: usize,
}

/// Why [`suggest_arrangements`] or [`replay_arrangement`] failed. No partial result
/// is returned.
#[derive(Debug, thiserror::Error)]
#[non_exhaustive]
pub enum ArrangementSuggestionError {
    /// The engine rejected the request or could not replay the contract.
    #[error("{engine} engine: {message}")]
    Engine {
        engine: &'static str,
        message: String,
    },
    #[error("assessment: {0}")]
    Assessment(#[from] ArrangementAssessmentError),
}

fn engine_error(engine: &'static str, error: impl std::fmt::Display) -> ArrangementSuggestionError {
    ArrangementSuggestionError::Engine {
        engine,
        message: error.to_string(),
    }
}

fn diagnostics_error(
    engine: &'static str,
    diagnostics: &[super::arrangement_proposals::ArrangementProposalDiagnostic],
) -> ArrangementSuggestionError {
    let codes: Vec<String> = diagnostics
        .iter()
        .map(|d| match d.measure_index {
            Some(measure) => format!("{} (measure {measure})", d.code),
            None => d.code.clone(),
        })
        .collect();
    ArrangementSuggestionError::Engine {
        engine,
        message: codes.join(", "),
    }
}

/// Run the requested engine and return its candidates in one shape.
pub fn suggest_arrangements(
    score: &Score,
    request: &ArrangementRequest,
) -> Result<ArrangementSuggestions, ArrangementSuggestionError> {
    let engine = request.engine.engine();
    let (candidates, engine_rejections): (Vec<ArrangementCandidate>, usize) = match &request.engine
    {
        ArrangementEngineRequest::Roles(inner) => {
            let result = suggest_arrangement_proposals(score, inner);
            if result.candidates.is_empty() {
                return Err(diagnostics_error(engine, &result.diagnostics));
            }
            let rejections = result.diagnostics.len();
            let candidates = result
                .candidates
                .into_iter()
                .map(|c| ArrangementCandidate {
                    id: c.id,
                    score: c.score,
                    resolved: ResolvedArrangement::Roles(Box::new(c.resolved)),
                    difference_codes: c.difference_codes,
                    assessment: None,
                })
                .collect();
            (candidates, rejections)
        }
        ArrangementEngineRequest::Integrated(inner) => {
            let result = suggest_integrated_arrangements(score, inner)
                .map_err(|e| engine_error(engine, e))?;
            let rejections = result.rejections.len();
            let candidates = result
                .candidates
                .into_iter()
                .enumerate()
                .map(|(index, c)| ArrangementCandidate {
                    id: format!("{engine}-{index}"),
                    score: c.score,
                    resolved: ResolvedArrangement::Integrated(Box::new(c.resolved)),
                    difference_codes: c.difference_codes,
                    assessment: None,
                })
                .collect();
            (candidates, rejections)
        }
        ArrangementEngineRequest::HarmonyGroove(inner) => {
            let result = suggest_harmony_groove_arrangements(score, inner)
                .map_err(|e| engine_error(engine, e))?;
            let rejections = result.missing_profiles.len();
            let candidates = result
                .candidates
                .into_iter()
                .enumerate()
                .map(|(index, c)| ArrangementCandidate {
                    id: format!("{engine}-{index}"),
                    score: c.arrangement.score,
                    difference_codes: vec![format!("profile-{}", profile_name(&c.resolved))],
                    resolved: ResolvedArrangement::HarmonyGroove(Box::new(c.resolved)),
                    assessment: None,
                })
                .collect();
            (candidates, rejections)
        }
        ArrangementEngineRequest::EnsembleForm(inner) => {
            let result = suggest_ensemble_form_proposals(score, inner)
                .map_err(|e| engine_error(engine, e))?;
            let candidates = result
                .candidates
                .into_iter()
                .map(|c| ArrangementCandidate {
                    id: c.id,
                    score: c.score,
                    resolved: ResolvedArrangement::EnsembleForm(Box::new(c.resolved)),
                    difference_codes: c.difference_codes,
                    assessment: None,
                })
                .collect();
            (candidates, 0)
        }
        ArrangementEngineRequest::DevelopedForm(inner) => {
            let result = suggest_developed_form_proposals(score, inner)
                .map_err(|e| engine_error(engine, e))?;
            let rejections = result.rejections.len();
            let candidates = result
                .candidates
                .into_iter()
                .map(|c| ArrangementCandidate {
                    id: c.id,
                    score: c.score,
                    resolved: ResolvedArrangement::DevelopedForm(Box::new(c.resolved)),
                    difference_codes: c.difference_codes,
                    assessment: None,
                })
                .collect();
            (candidates, rejections)
        }
        ArrangementEngineRequest::CrossPlanForm(inner) => {
            let result =
                suggest_cross_plan_forms(score, inner).map_err(|e| engine_error(engine, e))?;
            let rejections = result.rejections.len();
            let candidates = result
                .candidates
                .into_iter()
                .map(|c| ArrangementCandidate {
                    id: c.id,
                    score: c.score,
                    resolved: ResolvedArrangement::CrossPlanForm(Box::new(c.resolved)),
                    difference_codes: c.difference_codes,
                    assessment: None,
                })
                .collect();
            (candidates, rejections)
        }
    };

    let mut kept = Vec::new();
    let mut set_aside = Vec::new();
    for mut candidate in candidates {
        if let Some(options) = &request.assessment {
            let assessment = assess_arrangement(&candidate.score, &options.request)?;
            let exceeded = options.limits.exceeded(&assessment);
            candidate.assessment = Some(assessment);
            if !exceeded.is_empty() {
                set_aside.push(SetAsideArrangement {
                    candidate,
                    exceeded,
                });
                continue;
            }
        }
        kept.push(candidate);
    }
    Ok(ArrangementSuggestions {
        contract_version: ARRANGEMENT_SUGGESTIONS_CONTRACT_VERSION,
        engine: engine.to_owned(),
        candidates: kept,
        set_aside,
        engine_rejections,
    })
}

fn profile_name(resolved: &ResolvedHarmonyGrooveArrangement) -> String {
    serde_json::to_value(resolved.profile)
        .ok()
        .and_then(|value| value.as_str().map(str::to_owned))
        .unwrap_or_default()
}

/// Rebuild a candidate's score from its resolved contract. The engine checks that
/// `score` is the source the contract was made from.
pub fn replay_arrangement(
    score: &Score,
    resolved: &ResolvedArrangement,
) -> Result<Score, ArrangementSuggestionError> {
    match resolved {
        ResolvedArrangement::Roles(inner) => {
            let result = replay_arrangement_proposal(score, inner);
            match result.score {
                Some(score) if result.diagnostics.is_empty() => Ok(score),
                _ => Err(diagnostics_error("roles", &result.diagnostics)),
            }
        }
        ResolvedArrangement::Integrated(inner) => {
            replay_integrated_arrangement(score, inner).map_err(|e| engine_error("integrated", e))
        }
        ResolvedArrangement::HarmonyGroove(inner) => {
            replay_harmony_groove_arrangement(score, inner)
                .map(|candidate| candidate.arrangement.score)
                .map_err(|e| engine_error("harmony-groove", e))
        }
        ResolvedArrangement::EnsembleForm(inner) => replay_ensemble_form_proposal(score, inner)
            .map_err(|e| engine_error("ensemble-form", e)),
        ResolvedArrangement::DevelopedForm(inner) => replay_developed_form_proposal(score, inner)
            .map_err(|e| engine_error("developed-form", e)),
        ResolvedArrangement::CrossPlanForm(inner) => {
            replay_cross_plan_form(score, inner).map_err(|e| engine_error("cross-plan-form", e))
        }
    }
}
