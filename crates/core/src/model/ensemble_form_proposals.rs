//! Bounded, source-bound proposals that vary entrances and textures by section.

use serde::{Deserialize, Serialize};

use super::bass_line::BassLinePattern;
use super::chord_proposals::fingerprint;
use super::chordal_pad::ChordalPadPattern;
use super::ensemble_form::{
    EnsembleFormDiagnostic, EnsembleFormMeasure, EnsembleFormRequest, EnsembleFormSection,
    FormBass, FormHarmony, ResolvedEnsembleForm, arrange_ensemble_form, replay_ensemble_form,
};
use super::ensemble_roles::EnsembleRoleOptions;
use super::notation::Dynamic;
use super::playback::{PlaybackOptions, to_playback_events};
use super::score::Score;

pub const FORM_PROPOSAL_CONTRACT_VERSION: u16 = 1;
const PROFILE_IDS: [&str; 5] = [
    "terraced-arc",
    "section-dialogue",
    "bass-led-entry",
    "active-contrast",
    "thin-middle",
];

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
#[serde(deny_unknown_fields)]
pub struct FormProposalBoundary {
    pub label: String,
    pub start_measure: usize,
    pub end_measure_exclusive: usize,
}

#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
#[serde(deny_unknown_fields)]
pub struct EnsembleFormProposalRequest {
    /// Instrument ranges and a selected chord plan. Section treatment is
    /// generated here; preexisting role sections and pattern sections conflict.
    pub ensemble: EnsembleRoleOptions,
    /// Three to 64 adjacent written sections covering the source exactly.
    pub sections: Vec<FormProposalBoundary>,
    /// Empty derives the end of each supplied section.
    #[serde(default)]
    pub phrase_end_measures: Vec<usize>,
    /// Exact candidate count, three to five.
    pub count: u8,
}

#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
#[serde(deny_unknown_fields)]
pub struct ResolvedFormProposal {
    pub contract_version: u16,
    pub source_fingerprint: String,
    pub profile_id: String,
    pub form: ResolvedEnsembleForm,
    /// Sections where a collided countermelody was omitted after realization.
    pub omitted_countermelody_sections: Vec<usize>,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct EnsembleFormProposalCandidate {
    pub id: String,
    pub score: Score,
    pub resolved: ResolvedFormProposal,
    pub measures: Vec<EnsembleFormMeasure>,
    pub difference_codes: Vec<String>,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct EnsembleFormProposalResult {
    pub contract_version: u16,
    pub source_fingerprint: String,
    pub candidates: Vec<EnsembleFormProposalCandidate>,
}

#[derive(Debug, thiserror::Error)]
pub enum FormProposalError {
    #[error("invalid form proposal request")]
    InvalidRequest,
    #[error("invalid source score")]
    InvalidSource,
    #[error("form realization failed for {profile}: {diagnostic:?}")]
    Realization {
        profile: &'static str,
        diagnostic: Option<EnsembleFormDiagnostic>,
    },
    #[error("only {found} distinct form proposals were available")]
    InsufficientDistinct { found: usize },
    #[error("unsupported form proposal contract")]
    UnsupportedContract,
    #[error("form proposal source mismatch")]
    SourceMismatch,
    #[error("form proposal replay failed: {diagnostics:?}")]
    ReplayFailed {
        diagnostics: Vec<EnsembleFormDiagnostic>,
    },
}

fn valid_request(score: &Score, request: &EnsembleFormProposalRequest) -> bool {
    let count = score
        .parts
        .first()
        .and_then(|part| part.staves.first())
        .map_or(0, |staff| staff.measures.len());
    if !(3..=5).contains(&request.count)
        || !(3..=64).contains(&request.sections.len())
        || request.ensemble.harmony.is_none()
        || request.ensemble.bass.is_none()
        || request
            .sections
            .first()
            .is_none_or(|section| section.start_measure != 0)
        || request
            .sections
            .last()
            .is_none_or(|section| section.end_measure_exclusive != count)
        || request.sections.iter().any(|section| {
            section.label.trim().is_empty()
                || section.label.len() > 80
                || section.start_measure >= section.end_measure_exclusive
        })
        || request
            .sections
            .windows(2)
            .any(|pair| pair[0].end_measure_exclusive != pair[1].start_measure)
    {
        return false;
    }
    let phrase_ends = &request.phrase_end_measures;
    phrase_ends.is_empty()
        || phrase_ends.iter().all(|measure| *measure < count)
            && phrase_ends.windows(2).all(|pair| pair[0] < pair[1])
}

fn section(
    profile: usize,
    ordinal: usize,
    count: usize,
    has_countermelody: bool,
    boundary: &FormProposalBoundary,
) -> EnsembleFormSection {
    let (harmony, bass, counter) = if ordinal + 1 == count {
        (Some((ChordalPadPattern::Held, Dynamic::Pp)), None, None)
    } else {
        match (profile, ordinal % 3) {
            (0, 0) => (Some((ChordalPadPattern::Held, Dynamic::P)), None, None),
            (0, 1) => (
                Some((ChordalPadPattern::OffbeatPulse, Dynamic::Mp)),
                Some((BassLinePattern::Approach, Dynamic::Mp)),
                Some(Dynamic::P),
            ),
            (0, _) => (
                Some((ChordalPadPattern::ArpeggioUpDown, Dynamic::Mf)),
                Some((BassLinePattern::RootFifth, Dynamic::Mf)),
                Some(Dynamic::Mp),
            ),
            (1, 0) => (
                Some((ChordalPadPattern::OffbeatPulse, Dynamic::Mp)),
                Some((BassLinePattern::RootPulse, Dynamic::Mp)),
                None,
            ),
            (1, 1) => (
                Some((ChordalPadPattern::Held, Dynamic::P)),
                Some((BassLinePattern::RootFifth, Dynamic::Mp)),
                Some(Dynamic::P),
            ),
            (1, _) => (
                Some((ChordalPadPattern::MeterPulse, Dynamic::Mf)),
                Some((BassLinePattern::Approach, Dynamic::Mf)),
                Some(Dynamic::Mp),
            ),
            (2, 0) => (None, Some((BassLinePattern::RootPulse, Dynamic::P)), None),
            (2, 1) => (
                Some((ChordalPadPattern::Held, Dynamic::Mp)),
                Some((BassLinePattern::RootFifth, Dynamic::Mp)),
                Some(Dynamic::P),
            ),
            (2, _) => (
                Some((ChordalPadPattern::OffbeatPulse, Dynamic::Mf)),
                Some((BassLinePattern::Approach, Dynamic::Mf)),
                Some(Dynamic::Mp),
            ),
            (3, 0) => (
                Some((ChordalPadPattern::ArpeggioUpDown, Dynamic::Mf)),
                Some((BassLinePattern::RootFifth, Dynamic::Mf)),
                Some(Dynamic::P),
            ),
            (3, 1) => (
                Some((ChordalPadPattern::Held, Dynamic::P)),
                Some((BassLinePattern::RootPulse, Dynamic::P)),
                None,
            ),
            (3, _) => (
                Some((ChordalPadPattern::OffbeatPulse, Dynamic::Mp)),
                Some((BassLinePattern::Approach, Dynamic::Mp)),
                Some(Dynamic::P),
            ),
            (4, 0) => (
                Some((ChordalPadPattern::Held, Dynamic::Pp)),
                Some((BassLinePattern::RootPulse, Dynamic::P)),
                Some(Dynamic::P),
            ),
            (4, 1) => (
                None,
                Some((BassLinePattern::RootFifth, Dynamic::Mp)),
                Some(Dynamic::P),
            ),
            (4, _) => (
                Some((ChordalPadPattern::MeterPulse, Dynamic::Mf)),
                Some((BassLinePattern::Approach, Dynamic::Mf)),
                None,
            ),
            _ => (Some((ChordalPadPattern::Held, Dynamic::P)), None, None),
        }
    };
    EnsembleFormSection {
        label: boundary.label.clone(),
        start_measure: boundary.start_measure,
        end_measure_exclusive: boundary.end_measure_exclusive,
        harmony: harmony.map(|(pattern, dynamic)| FormHarmony { pattern, dynamic }),
        bass: bass.map(|(pattern, dynamic)| FormBass {
            pattern,
            dynamic,
            rhythm: None,
        }),
        countermelody: counter.filter(|_| has_countermelody),
    }
}

pub(crate) fn signature(score: &Score, source_parts: usize) -> Vec<(usize, u64, u64, u8, u8)> {
    to_playback_events(score, &PlaybackOptions::default())
        .into_iter()
        .filter_map(|event| {
            event.source.and_then(|address| {
                (address.part >= source_parts).then_some((
                    address.part,
                    (event.time_beats * 480.0).round() as u64,
                    (event.duration_beats * 480.0).round() as u64,
                    event.pitch_midi,
                    event.velocity,
                ))
            })
        })
        .collect()
}

fn difference_codes(
    reference: &EnsembleFormProposalCandidate,
    candidate: &EnsembleFormProposalCandidate,
) -> Vec<String> {
    let left = &reference.resolved.form.request.sections;
    let right = &candidate.resolved.form.request.sections;
    let mut codes = Vec::new();
    if left.iter().zip(right).any(|(a, b)| {
        a.harmony.is_some() != b.harmony.is_some()
            || a.bass.is_some() != b.bass.is_some()
            || a.countermelody.is_some() != b.countermelody.is_some()
    }) {
        codes.push("section-activity-changed".into());
    }
    if left.iter().zip(right).any(|(a, b)| {
        a.harmony.as_ref().map(|item| item.pattern) != b.harmony.as_ref().map(|item| item.pattern)
            || a.bass.as_ref().map(|item| item.pattern) != b.bass.as_ref().map(|item| item.pattern)
    }) {
        codes.push("section-texture-changed".into());
    }
    if left.iter().zip(right).any(|(a, b)| {
        a.harmony.as_ref().map(|item| item.dynamic) != b.harmony.as_ref().map(|item| item.dynamic)
            || a.bass.as_ref().map(|item| item.dynamic) != b.bass.as_ref().map(|item| item.dynamic)
            || a.countermelody != b.countermelody
    }) {
        codes.push("section-dynamics-changed".into());
    }
    if reference
        .measures
        .iter()
        .zip(&candidate.measures)
        .any(|(a, b)| {
            a.harmony_attacks != b.harmony_attacks
                || a.bass_attacks != b.bass_attacks
                || a.countermelody_attacks != b.countermelody_attacks
        })
    {
        codes.push("section-attack-shape-changed".into());
    }
    codes
}

/// Select three to five complete form variants from an explicit chord and
/// instrument template. Countermelody is omitted only in sections where the
/// combined roles collide; every omission is part of the replay contract.
pub fn suggest_ensemble_form_proposals(
    source: &Score,
    request: &EnsembleFormProposalRequest,
) -> Result<EnsembleFormProposalResult, FormProposalError> {
    if !valid_request(source, request) {
        return Err(FormProposalError::InvalidRequest);
    }
    let Some(source_fingerprint) = fingerprint(source) else {
        return Err(FormProposalError::InvalidSource);
    };
    let phrase_end_measures = if request.phrase_end_measures.is_empty() {
        request
            .sections
            .iter()
            .map(|section| section.end_measure_exclusive - 1)
            .collect::<Vec<_>>()
    } else {
        request.phrase_end_measures.clone()
    };
    let mut candidates = Vec::<EnsembleFormProposalCandidate>::new();
    let mut signatures = Vec::new();
    let mut first_failure = None;
    for (profile, profile_id) in PROFILE_IDS.iter().enumerate() {
        if candidates.len() == usize::from(request.count) {
            break;
        }
        let mut form = EnsembleFormRequest {
            ensemble: request.ensemble.clone(),
            sections: request
                .sections
                .iter()
                .enumerate()
                .map(|(ordinal, boundary)| {
                    section(
                        profile,
                        ordinal,
                        request.sections.len(),
                        request.ensemble.countermelody.is_some(),
                        boundary,
                    )
                })
                .collect(),
            phrase_end_measures: phrase_end_measures.clone(),
        };
        let mut omitted = Vec::new();
        let built = loop {
            let built = arrange_ensemble_form(source, &form);
            if built.score.is_some() {
                break built;
            }
            let Some(problem) = built.diagnostics.first() else {
                break built;
            };
            if problem.code != "harmony-countermelody-collision" {
                break built;
            }
            let Some(index) = problem.section_index else {
                break built;
            };
            let Some(treatment) = form.sections.get_mut(index) else {
                break built;
            };
            if treatment.countermelody.take().is_none() {
                break built;
            }
            omitted.push(index);
        };
        let (Some(score), Some(resolved)) = (built.score, built.resolved) else {
            first_failure.get_or_insert(FormProposalError::Realization {
                profile: profile_id,
                diagnostic: built.diagnostics.first().cloned(),
            });
            continue;
        };
        omitted.sort_unstable();
        let event_signature = signature(&score, source.parts.len());
        if signatures.contains(&event_signature) {
            continue;
        }
        signatures.push(event_signature);
        candidates.push(EnsembleFormProposalCandidate {
            id: (*profile_id).into(),
            score,
            resolved: ResolvedFormProposal {
                contract_version: FORM_PROPOSAL_CONTRACT_VERSION,
                source_fingerprint: source_fingerprint.clone(),
                profile_id: (*profile_id).into(),
                form: resolved,
                omitted_countermelody_sections: omitted,
            },
            measures: built.measures,
            difference_codes: Vec::new(),
        });
    }
    if candidates.len() < usize::from(request.count) {
        return Err(
            first_failure.unwrap_or(FormProposalError::InsufficientDistinct {
                found: candidates.len(),
            }),
        );
    }
    if let Some(reference) = candidates.first().cloned() {
        describe_differences(&mut candidates, &reference);
    }
    Ok(EnsembleFormProposalResult {
        contract_version: FORM_PROPOSAL_CONTRACT_VERSION,
        source_fingerprint,
        candidates,
    })
}

fn describe_differences(
    candidates: &mut [EnsembleFormProposalCandidate],
    reference: &EnsembleFormProposalCandidate,
) {
    if let Some(first) = candidates.first_mut() {
        first.difference_codes.push("reference-form".into());
    }
    for candidate in candidates.iter_mut().skip(1) {
        candidate.difference_codes = difference_codes(reference, candidate);
    }
}

/// Rebuild one selected form proposal against its original source.
pub fn replay_ensemble_form_proposal(
    source: &Score,
    resolved: &ResolvedFormProposal,
) -> Result<Score, FormProposalError> {
    if resolved.contract_version != FORM_PROPOSAL_CONTRACT_VERSION
        || resolved.source_fingerprint != resolved.form.source_fingerprint
    {
        return Err(FormProposalError::UnsupportedContract);
    }
    let profile = PROFILE_IDS
        .iter()
        .position(|id| *id == resolved.profile_id)
        .ok_or(FormProposalError::UnsupportedContract)?;
    let sections = &resolved.form.request.sections;
    let omissions = &resolved.omitted_countermelody_sections;
    if !(3..=64).contains(&sections.len())
        || omissions.iter().any(|index| *index >= sections.len())
        || omissions.windows(2).any(|pair| pair[0] >= pair[1])
    {
        return Err(FormProposalError::UnsupportedContract);
    }
    for (index, actual) in sections.iter().enumerate() {
        let boundary = FormProposalBoundary {
            label: actual.label.clone(),
            start_measure: actual.start_measure,
            end_measure_exclusive: actual.end_measure_exclusive,
        };
        let mut expected = section(
            profile,
            index,
            sections.len(),
            resolved.form.request.ensemble.countermelody.is_some(),
            &boundary,
        );
        if omissions.binary_search(&index).is_ok() {
            if expected.countermelody.is_none() {
                return Err(FormProposalError::UnsupportedContract);
            }
            expected.countermelody = None;
        }
        if *actual != expected {
            return Err(FormProposalError::UnsupportedContract);
        }
    }
    if fingerprint(source).as_deref() != Some(resolved.source_fingerprint.as_str()) {
        return Err(FormProposalError::SourceMismatch);
    }
    let result = replay_ensemble_form(source, &resolved.form);
    result.score.ok_or(FormProposalError::ReplayFailed {
        diagnostics: result.diagnostics,
    })
}
