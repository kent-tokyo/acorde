//! Select complete sectional forms with distinct rhythmic countermelody treatments.

use std::collections::{BTreeMap, BTreeSet};

use serde::{Deserialize, Serialize};

use super::chord_plan::ChordPlan;
use super::chord_proposals::fingerprint;
use super::countermelody_development::CountermelodyDevelopmentKind;
use super::ensemble_form::{EnsembleFormDiagnostic, EnsembleFormMeasure, EnsembleFormSection};
use super::ensemble_form_development::{
    EnsembleFormDevelopmentError, EnsembleFormDevelopmentRequest, ResolvedEnsembleFormDevelopment,
    arrange_ensemble_form_development, replay_ensemble_form_development,
};
use super::ensemble_form_proposals::{
    EnsembleFormProposalRequest, FormProposalError, ResolvedFormProposal,
    replay_ensemble_form_proposal, signature, suggest_ensemble_form_proposals,
};
use super::ensemble_roles::{EnsembleRole, EnsembleRolePart};
use super::playback::{PlaybackOptions, to_playback_events};
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
    /// Generated parts and their role identity, used by downstream selection.
    #[serde(default)]
    pub role_parts: Vec<EnsembleRolePart>,
    pub difference_codes: Vec<String>,
    /// Difference from the first selected form, on six observed axes. Each
    /// axis is a Jaccard distance in 0..=1000 (0 means identical).
    #[serde(default)]
    pub axis_distance_from_first: DevelopedFormAxisDistance,
    /// Sum of the six axis distances to the closest previously selected form.
    #[serde(default)]
    pub nearest_selected_distance: u32,
}

/// Observed distance on six independent summaries of generated playback.
/// `harmony` compares pitch classes per written bar, `rhythm` attack times and
/// lengths, `bass` exact bass events, `register` four-semitone pitch bands,
/// `roles` sounding role activity by bar, and `form` each section's role,
/// pattern and dynamic. These are selection proxies, not listening scores.
#[derive(Debug, Clone, Copy, Default, PartialEq, Eq, Serialize, Deserialize)]
pub struct DevelopedFormAxisDistance {
    pub harmony: u16,
    pub rhythm: u16,
    pub bass: u16,
    pub register: u16,
    pub roles: u16,
    pub form: u16,
}

impl DevelopedFormAxisDistance {
    pub fn total(self) -> u32 {
        u32::from(self.harmony)
            + u32::from(self.rhythm)
            + u32::from(self.bass)
            + u32::from(self.register)
            + u32::from(self.roles)
            + u32::from(self.form)
    }
}

#[derive(Default)]
pub(super) struct ObservedAxes {
    harmony: BTreeSet<(usize, i32)>,
    harmony_plan: BTreeSet<(usize, u64, String)>,
    rhythm: BTreeSet<(EnsembleRole, u64, u64)>,
    bass: BTreeSet<(u64, u64, i32)>,
    register: BTreeSet<(EnsembleRole, i32)>,
    roles: BTreeSet<(usize, EnsembleRole)>,
    form: BTreeSet<(usize, EnsembleRole, u8, u8)>,
}

fn ticks(beats: f64) -> u64 {
    (beats * 480.0).round().max(0.0) as u64
}

impl ObservedAxes {
    pub(super) fn collect(
        score: &Score,
        role_parts: &[EnsembleRolePart],
        measures: &[EnsembleFormMeasure],
        sections: &[EnsembleFormSection],
    ) -> Self {
        let mut axes = Self::default();
        let by_part: BTreeMap<_, _> = role_parts
            .iter()
            .map(|part| (part.part_index, part.role))
            .collect();
        for event in to_playback_events(score, &PlaybackOptions::default()) {
            let Some(role) = by_part.get(&event.part_index).copied() else {
                continue;
            };
            let onset = ticks(event.time_beats);
            let duration = ticks(event.duration_beats);
            axes.rhythm.insert((role, onset, duration));
            axes.register
                .insert((role, event.pitch_midi_cents.div_euclid(400)));
            match role {
                EnsembleRole::Harmony => {
                    if let Some(source) = event.source {
                        axes.harmony
                            .insert((source.measure, event.pitch_midi_cents.rem_euclid(1200)));
                    }
                }
                EnsembleRole::Bass => {
                    axes.bass.insert((onset, duration, event.pitch_midi_cents));
                }
                EnsembleRole::Countermelody => {}
            }
        }
        for (measure, report) in measures.iter().enumerate() {
            for (role, attacks) in [
                (EnsembleRole::Harmony, report.harmony_attacks),
                (EnsembleRole::Bass, report.bass_attacks),
                (EnsembleRole::Countermelody, report.countermelody_attacks),
            ] {
                if attacks > 0 {
                    axes.roles.insert((measure, role));
                }
            }
        }
        for (index, section) in sections.iter().enumerate() {
            if let Some(harmony) = &section.harmony {
                axes.form.insert((
                    index,
                    EnsembleRole::Harmony,
                    harmony.pattern as u8,
                    harmony.dynamic.to_velocity(),
                ));
            }
            if let Some(bass) = &section.bass {
                axes.form.insert((
                    index,
                    EnsembleRole::Bass,
                    bass.pattern as u8,
                    bass.dynamic.to_velocity(),
                ));
            }
            if let Some(level) = section.countermelody {
                axes.form
                    .insert((index, EnsembleRole::Countermelody, 0, level.to_velocity()));
            }
        }
        axes
    }

    pub(super) fn with_plan(mut self, plan: &ChordPlan) -> Self {
        self.harmony_plan = plan
            .entries
            .iter()
            .map(|entry| {
                (
                    entry.position.measure_index,
                    ticks(entry.position.offset.beats()),
                    serde_json::to_string(&entry.chord).unwrap_or_default(),
                )
            })
            .collect();
        self
    }

    pub(super) fn distance(&self, other: &Self) -> DevelopedFormAxisDistance {
        DevelopedFormAxisDistance {
            harmony: jaccard(&self.harmony, &other.harmony)
                .max(jaccard(&self.harmony_plan, &other.harmony_plan)),
            rhythm: jaccard(&self.rhythm, &other.rhythm),
            bass: jaccard(&self.bass, &other.bass),
            register: jaccard(&self.register, &other.register),
            roles: jaccard(&self.roles, &other.roles),
            form: jaccard(&self.form, &other.form),
        }
    }
}

fn jaccard<T: Ord>(left: &BTreeSet<T>, right: &BTreeSet<T>) -> u16 {
    let union = left.union(right).count();
    if union == 0 {
        return 0;
    }
    let common = left.intersection(right).count();
    (((union - common) * 1000 + union / 2) / union) as u16
}

pub(super) fn observed_difference_codes(distance: DevelopedFormAxisDistance) -> Vec<String> {
    [
        (distance.harmony, "harmony-pitch-classes-changed"),
        (distance.rhythm, "generated-rhythm-changed"),
        (distance.bass, "bass-events-changed"),
        (distance.register, "generated-register-changed"),
        (distance.roles, "role-activity-changed"),
        (distance.form, "section-form-changed"),
    ]
    .into_iter()
    .filter_map(|(value, code)| (value > 0).then_some(code.into()))
    .collect()
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
/// distinct playback sequences. After retaining the first form as a stable
/// reference, selection keeps a new countermelody treatment when available,
/// then maximizes the minimum observed six-axis distance to chosen forms.
/// A new form profile breaks remaining distance ties.
/// Failed realizations are skipped; an insufficient set returns no partial set.
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
            let axes = ObservedAxes::collect(
                &developed.score,
                &developed.role_parts,
                &developed.measures,
                &form.resolved.form.request.sections,
            );
            pool.push((
                DevelopedFormProposalCandidate {
                    id: format!("{}-{}", form.id, kind_name(kind)),
                    score: developed.score,
                    resolved: ResolvedDevelopedFormProposal {
                        contract_version: DEVELOPED_FORM_PROPOSAL_CONTRACT_VERSION,
                        source_fingerprint: source_fingerprint.clone(),
                        form_proposal: form.resolved.clone(),
                        development: developed.resolved,
                    },
                    measures: developed.measures,
                    role_parts: developed.role_parts,
                    difference_codes: Vec::new(),
                    axis_distance_from_first: DevelopedFormAxisDistance::default(),
                    nearest_selected_distance: 0,
                },
                axes,
            ));
        }
    }
    let wanted = usize::from(request.count);
    if pool.len() < wanted {
        return Err(DevelopedFormProposalError::InsufficientDistinct {
            found: pool.len(),
            rejections,
        });
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
                let nearest = selected
                    .iter()
                    .map(|(_, chosen)| axes.distance(chosen).total())
                    .min()
                    .unwrap_or_default();
                let new_form = !selected.iter().any(|(chosen, _)| {
                    chosen.resolved.form_proposal.profile_id
                        == candidate.resolved.form_proposal.profile_id
                });
                let new_kind = !selected.iter().any(|(chosen, _)| {
                    chosen.resolved.development.countermelody_kind
                        == candidate.resolved.development.countermelody_kind
                });
                (
                    u8::from(new_kind),
                    nearest,
                    u8::from(new_form),
                    std::cmp::Reverse(*index),
                )
            })
            .map(|(index, _)| index)
            .ok_or_else(|| DevelopedFormProposalError::InsufficientDistinct {
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
        if selected[0].0.resolved.form_proposal.profile_id
            != candidate.resolved.form_proposal.profile_id
        {
            candidate
                .difference_codes
                .push("form-profile-changed".into());
        }
        if selected[0].0.resolved.development.countermelody_kind
            != candidate.resolved.development.countermelody_kind
        {
            candidate
                .difference_codes
                .push("countermelody-treatment-changed".into());
        }
        selected.push((candidate, axes));
    }
    let candidates = selected
        .into_iter()
        .map(|(candidate, _)| candidate)
        .collect();
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
