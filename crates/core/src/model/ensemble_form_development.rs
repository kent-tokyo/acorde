//! Coordinate source-rhythm countermelody development with an explicit form.

use serde::{Deserialize, Serialize};

use super::chord_proposals::fingerprint;
use super::countermelody_development::CountermelodyDevelopmentKind;
use super::ensemble_form::{
    ENSEMBLE_FORM_ENGINE_VERSION, EnsembleFormDiagnostic, EnsembleFormMeasure, EnsembleFormRequest,
    ResolvedEnsembleForm, arrange_ensemble_form_with_development,
};
use super::ensemble_roles::EnsembleRolePart;
use super::score::{NoteAddr, Score};

pub const ENSEMBLE_FORM_DEVELOPMENT_CONTRACT_VERSION: u16 = 1;

#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
#[serde(deny_unknown_fields)]
pub struct EnsembleFormDevelopmentRequest {
    pub form: EnsembleFormRequest,
    pub countermelody_kind: CountermelodyDevelopmentKind,
}

#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
#[serde(deny_unknown_fields)]
pub struct ResolvedEnsembleFormDevelopment {
    pub contract_version: u16,
    pub engine_version: String,
    pub source_fingerprint: String,
    pub form: ResolvedEnsembleForm,
    pub countermelody_kind: CountermelodyDevelopmentKind,
    /// Optional phrase-entry leap constraint. Absent in the original contract.
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub phrase_entry_max_leap_semitones: Option<u8>,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct EnsembleFormDevelopmentResult {
    pub score: Score,
    pub resolved: ResolvedEnsembleFormDevelopment,
    pub role_parts: Vec<EnsembleRolePart>,
    pub measures: Vec<EnsembleFormMeasure>,
    pub dynamic_marks: Vec<NoteAddr>,
}

#[derive(Debug, thiserror::Error)]
pub enum EnsembleFormDevelopmentError {
    #[error("phrase-entry leap limit must be at most 24 semitones")]
    InvalidPhraseJoinLimit,
    #[error("form must contain an active countermelody section")]
    MissingCountermelody,
    #[error("form realization failed: {0:?}")]
    Form(EnsembleFormDiagnostic),
    #[error("form realization returned no score or resolved contract")]
    InvalidOutput,
    #[error("unsupported form development contract")]
    UnsupportedContract,
    #[error("form development source mismatch")]
    SourceMismatch,
}

/// Compose every generated role under the form's activity, density, collision
/// and dynamic rules, with the chosen rhythmic countermelody treatment.
pub fn arrange_ensemble_form_development(
    source: &Score,
    request: &EnsembleFormDevelopmentRequest,
) -> Result<EnsembleFormDevelopmentResult, EnsembleFormDevelopmentError> {
    arrange_ensemble_form_development_internal(source, request, None)
}

/// Keep each countermelody phrase entry within the requested semitone distance
/// of the previous phrase's final sounding note. A silent role section resets
/// this link; an impossible entry fails atomically.
pub fn arrange_ensemble_form_development_with_phrase_joins(
    source: &Score,
    request: &EnsembleFormDevelopmentRequest,
    max_leap_semitones: u8,
) -> Result<EnsembleFormDevelopmentResult, EnsembleFormDevelopmentError> {
    arrange_ensemble_form_development_internal(source, request, Some(max_leap_semitones))
}

fn arrange_ensemble_form_development_internal(
    source: &Score,
    request: &EnsembleFormDevelopmentRequest,
    phrase_entry_max_leap_semitones: Option<u8>,
) -> Result<EnsembleFormDevelopmentResult, EnsembleFormDevelopmentError> {
    if phrase_entry_max_leap_semitones.is_some_and(|limit| limit > 24) {
        return Err(EnsembleFormDevelopmentError::InvalidPhraseJoinLimit);
    }
    if request.form.ensemble.countermelody.is_none()
        || !request
            .form
            .sections
            .iter()
            .any(|section| section.countermelody.is_some())
    {
        return Err(EnsembleFormDevelopmentError::MissingCountermelody);
    }
    let result = arrange_ensemble_form_with_development(
        source,
        &request.form,
        request.countermelody_kind,
        phrase_entry_max_leap_semitones,
    );
    if let Some(problem) = result.diagnostics.first() {
        return Err(EnsembleFormDevelopmentError::Form(problem.clone()));
    }
    let score = result
        .score
        .ok_or(EnsembleFormDevelopmentError::InvalidOutput)?;
    let form = result
        .resolved
        .ok_or(EnsembleFormDevelopmentError::InvalidOutput)?;
    Ok(EnsembleFormDevelopmentResult {
        score,
        resolved: ResolvedEnsembleFormDevelopment {
            contract_version: ENSEMBLE_FORM_DEVELOPMENT_CONTRACT_VERSION,
            engine_version: env!("CARGO_PKG_VERSION").into(),
            source_fingerprint: form.source_fingerprint.clone(),
            form,
            countermelody_kind: request.countermelody_kind,
            phrase_entry_max_leap_semitones,
        },
        role_parts: result.role_parts,
        measures: result.measures,
        dynamic_marks: result.dynamic_marks,
    })
}

/// Replay only with the same source, form contract and generator version.
pub fn replay_ensemble_form_development(
    source: &Score,
    resolved: &ResolvedEnsembleFormDevelopment,
) -> Result<Score, EnsembleFormDevelopmentError> {
    if resolved.contract_version != ENSEMBLE_FORM_DEVELOPMENT_CONTRACT_VERSION
        || resolved.engine_version != env!("CARGO_PKG_VERSION")
        || resolved.form.engine_version != ENSEMBLE_FORM_ENGINE_VERSION
        || resolved.source_fingerprint != resolved.form.source_fingerprint
    {
        return Err(EnsembleFormDevelopmentError::UnsupportedContract);
    }
    if fingerprint(source).as_deref() != Some(resolved.source_fingerprint.as_str()) {
        return Err(EnsembleFormDevelopmentError::SourceMismatch);
    }
    Ok(arrange_ensemble_form_development_internal(
        source,
        &EnsembleFormDevelopmentRequest {
            form: resolved.form.request.clone(),
            countermelody_kind: resolved.countermelody_kind,
        },
        resolved.phrase_entry_max_leap_semitones,
    )?
    .score)
}
