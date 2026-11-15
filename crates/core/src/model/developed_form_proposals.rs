//! Select complete sectional forms with distinct rhythmic countermelody treatments.

use serde::{Deserialize, Serialize};

use super::chord_proposals::fingerprint;
use super::countermelody_development::CountermelodyDevelopmentKind;
use super::ensemble_form::{EnsembleFormDiagnostic, EnsembleFormMeasure};
use super::ensemble_form_development::{
    EnsembleFormDevelopmentError, EnsembleFormDevelopmentRequest, ResolvedEnsembleFormDevelopment,
    arrange_ensemble_form_development, replay_ensemble_form_development,
};
use super::ensemble_form_proposals::{
    EnsembleFormProposalRequest, FormProposalError, ResolvedFormProposal,
    replay_ensemble_form_proposal, signature, suggest_ensemble_form_proposals,
};
use super::score::Score;

pub const DEVELOPED_FORM_PROPOSAL_CONTRACT_VERSION: u16 = 1;
const KINDS: [CountermelodyDevelopmentKind; 5] = [
    CountermelodyDevelopmentKind::Imitation,
    CountermelodyDevelopmentKind::Inversion,
    CountermelodyDevelopmentKind::Augmentation,
    CountermelodyDevelopmentKind::Diminution,
    CountermelodyDevelopmentKind::Answer,
];

#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
#[serde(deny_unknown_fields)]
pub struct ResolvedDevelopedFormProposal {
    pub contract_version: u16,
    pub source_fingerprint: String,
    pub form_proposal: ResolvedFormProposal,
    pub development: ResolvedEnsembleFormDevelopment,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct DevelopedFormProposalCandidate {
    pub id: String,
    pub score: Score,
    pub resolved: ResolvedDevelopedFormProposal,
    pub measures: Vec<EnsembleFormMeasure>,
    pub difference_codes: Vec<String>,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct DevelopedFormRejection {
    pub profile_id: String,
    pub countermelody_kind: CountermelodyDevelopmentKind,
    pub code: String,
    pub diagnostic: Option<EnsembleFormDiagnostic>,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct DevelopedFormProposalResult {
    pub contract_version: u16,
    pub source_fingerprint: String,
    pub candidates: Vec<DevelopedFormProposalCandidate>,
    /// Failed form/treatment attempts. A form without an active counter role
    /// is reported once per requested treatment.
    pub rejections: Vec<DevelopedFormRejection>,
}

#[derive(Debug, thiserror::Error)]
pub enum DevelopedFormProposalError {
    #[error("developed form proposal count must be three to five")]
    InvalidCount,
    #[error("countermelody options are required for developed form proposals")]
    MissingCountermelody,
    #[error("form proposals failed: {0}")]
    Form(#[from] FormProposalError),
    #[error("only {found} distinct developed forms were available")]
    InsufficientDistinct {
        found: usize,
        rejections: Vec<DevelopedFormRejection>,
    },
    #[error("unsupported developed form proposal contract")]
    UnsupportedContract,
    #[error("developed form proposal source mismatch")]
    SourceMismatch,
    #[error("developed form replay failed: {0}")]
    Development(#[from] EnsembleFormDevelopmentError),
}

/// Evaluate at most 25 form/treatment combinations, then choose three to five
/// distinct playback sequences. Selection prefers a new form profile and a
/// new rhythm treatment when both are available. Failed realizations are
/// skipped; an insufficient set returns no partial proposal collection.
pub fn suggest_developed_form_proposals(
    source: &Score,
    request: &EnsembleFormProposalRequest,
) -> Result<DevelopedFormProposalResult, DevelopedFormProposalError> {
    if !(3..=5).contains(&request.count) {
        return Err(DevelopedFormProposalError::InvalidCount);
    }
    if request.ensemble.countermelody.is_none() {
        return Err(DevelopedFormProposalError::MissingCountermelody);
    }
    let mut form_request = request.clone();
    let mut first_error = None;
    let mut forms = None;
    for form_count in (3..=5).rev() {
        form_request.count = form_count;
        match suggest_ensemble_form_proposals(source, &form_request) {
            Ok(proposed) => {
                forms = Some(proposed);
                break;
            }
            Err(error) => {
                first_error.get_or_insert(error);
            }
        }
    }
    let forms = forms.ok_or_else(|| {
        DevelopedFormProposalError::Form(
            first_error.unwrap_or(FormProposalError::InsufficientDistinct { found: 0 }),
        )
    })?;
    let source_fingerprint = forms.source_fingerprint;
    let mut pool = Vec::new();
    let mut signatures = Vec::new();
    let mut rejections = Vec::new();
    for form in forms.candidates {
        if !form
            .resolved
            .form
            .request
            .sections
            .iter()
            .any(|section| section.countermelody.is_some())
        {
            rejections.extend(KINDS.into_iter().map(|kind| DevelopedFormRejection {
                profile_id: form.id.clone(),
                countermelody_kind: kind,
                code: "no-active-countermelody-section".into(),
                diagnostic: None,
            }));
            continue;
        }
        for kind in KINDS {
            let result = arrange_ensemble_form_development(
                source,
                &EnsembleFormDevelopmentRequest {
                    form: form.resolved.form.request.clone(),
                    countermelody_kind: kind,
                },
            );
            let developed = match result {
                Ok(developed) => developed,
                Err(error) => {
                    let diagnostic = match &error {
                        EnsembleFormDevelopmentError::Form(problem) => Some(problem.clone()),
                        _ => None,
                    };
                    rejections.push(DevelopedFormRejection {
                        profile_id: form.id.clone(),
                        countermelody_kind: kind,
                        code: diagnostic.as_ref().map_or_else(
                            || match error {
                                EnsembleFormDevelopmentError::MissingCountermelody => {
                                    "missing-countermelody".into()
                                }
                                EnsembleFormDevelopmentError::InvalidOutput => {
                                    "invalid-output".into()
                                }
                                _ => "development-failed".into(),
                            },
                            |problem| problem.code.clone(),
                        ),
                        diagnostic,
                    });
                    continue;
                }
            };
            let event_signature = signature(&developed.score, source.parts.len());
            if signatures.contains(&event_signature) {
                continue;
            }
            signatures.push(event_signature);
            pool.push(DevelopedFormProposalCandidate {
                id: format!("{}-{}", form.id, kind_name(kind)),
                score: developed.score,
                resolved: ResolvedDevelopedFormProposal {
                    contract_version: DEVELOPED_FORM_PROPOSAL_CONTRACT_VERSION,
                    source_fingerprint: source_fingerprint.clone(),
                    form_proposal: form.resolved.clone(),
                    development: developed.resolved,
                },
                measures: developed.measures,
                difference_codes: Vec::new(),
            });
        }
    }
    let wanted = usize::from(request.count);
    if pool.len() < wanted {
        return Err(DevelopedFormProposalError::InsufficientDistinct {
            found: pool.len(),
            rejections,
        });
    }
    let mut candidates: Vec<DevelopedFormProposalCandidate> = Vec::with_capacity(wanted);
    while candidates.len() < wanted {
        let index = pool
            .iter()
            .enumerate()
            .max_by_key(|(index, candidate)| {
                let new_form = !candidates.iter().any(|chosen| {
                    chosen.resolved.form_proposal.profile_id
                        == candidate.resolved.form_proposal.profile_id
                });
                let new_kind = !candidates.iter().any(|chosen| {
                    chosen.resolved.development.countermelody_kind
                        == candidate.resolved.development.countermelody_kind
                });
                (
                    u8::from(new_form) + u8::from(new_kind),
                    u8::from(new_form),
                    std::cmp::Reverse(*index),
                )
            })
            .map(|(index, _)| index)
            .ok_or_else(|| DevelopedFormProposalError::InsufficientDistinct {
                found: candidates.len(),
                rejections: rejections.clone(),
            })?;
        let mut candidate = pool.remove(index);
        if let Some(reference) = candidates.first() {
            if reference.resolved.form_proposal.profile_id
                != candidate.resolved.form_proposal.profile_id
            {
                candidate
                    .difference_codes
                    .push("form-profile-changed".into());
            }
            if reference.resolved.development.countermelody_kind
                != candidate.resolved.development.countermelody_kind
            {
                candidate
                    .difference_codes
                    .push("countermelody-treatment-changed".into());
            }
        } else {
            candidate.difference_codes.push("reference-form".into());
        }
        candidates.push(candidate);
    }
    Ok(DevelopedFormProposalResult {
        contract_version: DEVELOPED_FORM_PROPOSAL_CONTRACT_VERSION,
        source_fingerprint,
        candidates,
        rejections,
    })
}

fn kind_name(kind: CountermelodyDevelopmentKind) -> &'static str {
    match kind {
        CountermelodyDevelopmentKind::Imitation => "imitation",
        CountermelodyDevelopmentKind::Inversion => "inversion",
        CountermelodyDevelopmentKind::Augmentation => "augmentation",
        CountermelodyDevelopmentKind::Diminution => "diminution",
        CountermelodyDevelopmentKind::Answer => "answer",
    }
}

/// Recheck the form profile and rhythmic treatment against the original source.
pub fn replay_developed_form_proposal(
    source: &Score,
    resolved: &ResolvedDevelopedFormProposal,
) -> Result<Score, DevelopedFormProposalError> {
    if resolved.contract_version != DEVELOPED_FORM_PROPOSAL_CONTRACT_VERSION
        || resolved.source_fingerprint != resolved.form_proposal.source_fingerprint
        || resolved.source_fingerprint != resolved.development.source_fingerprint
        || resolved.form_proposal.form != resolved.development.form
    {
        return Err(DevelopedFormProposalError::UnsupportedContract);
    }
    if fingerprint(source).as_deref() != Some(resolved.source_fingerprint.as_str()) {
        return Err(DevelopedFormProposalError::SourceMismatch);
    }
    replay_ensemble_form_proposal(source, &resolved.form_proposal)?;
    Ok(replay_ensemble_form_development(
        source,
        &resolved.development,
    )?)
}
