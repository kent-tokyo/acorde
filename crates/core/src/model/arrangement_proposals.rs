//! Deterministic, bounded selection of complete multi-role arrangements.

use serde::{Deserialize, Serialize};

use super::arrange::PositionedAccordionChord;
use super::bass_line::BassLinePattern;
use super::chord_plan::{
    CHORD_PLAN_CONTRACT_VERSION, ChordPlan, ChordPlanEntry, chord_plan_to_preset_selection,
};
use super::chord_proposals::{
    ChordProposalCandidate, ChordProposalRequest, fingerprint, suggest_chord_plans_with_locks,
};
use super::chordal_pad::{ChordalPadPattern, ChordalPadVoicing, emit_rest, tick};
use super::ensemble_roles::{
    EnsembleRole, EnsembleRoleOptions, EnsembleRolePart, EnsembleRoleResult, arrange_ensemble_roles,
};
use super::notation::{Articulation, Dynamic};
use super::playback::to_playback_events;
use super::score::{Measure, Part, Score};

pub const ARRANGEMENT_PROPOSAL_CONTRACT_VERSION: u16 = 1;
const MAX_LOCKS: usize = 512;

#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
pub struct ArrangementProposalRequest {
    /// The source plan, harmonic locks and change budget live here.
    pub harmony: ChordProposalRequest,
    /// Indices into `harmony.source_plan.entries`.
    #[serde(default)]
    pub locked_chord_entries: Vec<usize>,
    /// Instrument ranges and section activity. The selected chord plan replaces
    /// each supplied role's `chords`; harmony and bass options are required.
    pub ensemble_template: EnsembleRoleOptions,
    /// Exact number of distinct complete scores requested, from three to five.
    pub count: u8,
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct ArrangementProposalDiagnostic {
    pub code: String,
    pub measure_index: Option<usize>,
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct ArrangementProposalFeatures {
    pub harmony_attack_count: usize,
    pub bass_attack_count: usize,
    pub countermelody_attack_count: usize,
    pub peak_generated_attacks_per_measure: usize,
    pub generated_attacks_by_measure: Vec<usize>,
    pub generated_part_count: usize,
    pub changed_harmony_spans: usize,
}

#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
pub struct ResolvedArrangementProposal {
    pub contract_version: u16,
    pub source_fingerprint: String,
    pub id: String,
    pub harmony_candidate_id: String,
    pub profile_id: String,
    pub plan: ChordPlan,
    pub ensemble_options: EnsembleRoleOptions,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct ArrangementProposalCandidate {
    pub id: String,
    pub score: Score,
    /// The selected harmony with source attribution and changed-region origin.
    pub plan: ChordPlan,
    pub resolved: ResolvedArrangementProposal,
    pub features: ArrangementProposalFeatures,
    /// Stable descriptions of the selected harmony and accompaniment choices.
    pub reason_codes: Vec<String>,
    /// Observable differences from the first returned candidate.
    pub difference_codes: Vec<String>,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct ArrangementProposalResult {
    pub contract_version: u16,
    pub source_fingerprint: String,
    pub candidates: Vec<ArrangementProposalCandidate>,
    pub diagnostics: Vec<ArrangementProposalDiagnostic>,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct ArrangementProposalReplayResult {
    pub score: Option<Score>,
    pub diagnostics: Vec<ArrangementProposalDiagnostic>,
}

#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
pub struct ArrangementRegionRequest {
    pub original: ResolvedArrangementProposal,
    pub replacement: ResolvedArrangementProposal,
    /// Half-open physical-measure range to replace.
    pub start_measure: usize,
    pub end_measure_exclusive: usize,
    /// Keep these generated roles unchanged inside the selected region.
    #[serde(default)]
    pub locked_roles: Vec<EnsembleRole>,
    /// Keep all generated roles unchanged in these physical measures.
    #[serde(default)]
    pub locked_measures: Vec<usize>,
    /// Indices into `original.plan.entries`. Each lock covers that chord
    /// through the following original entry, including mid-measure changes.
    #[serde(default)]
    pub locked_chord_entries: Vec<usize>,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct ArrangementRegionResult {
    pub score: Option<Score>,
    pub resolved_request: ArrangementRegionRequest,
    pub role_parts: Vec<EnsembleRolePart>,
    pub changed_roles: Vec<EnsembleRole>,
    pub changed_measures: Vec<usize>,
    pub diagnostics: Vec<ArrangementProposalDiagnostic>,
}

#[derive(Clone, Copy)]
struct Profile {
    id: &'static str,
    pad: ChordalPadPattern,
    voicing: ChordalPadVoicing,
    bass: BassLinePattern,
    counter: bool,
}

const PROFILES: [Profile; 7] = [
    Profile {
        id: "sustained-root",
        pad: ChordalPadPattern::Held,
        voicing: ChordalPadVoicing::Smooth,
        bass: BassLinePattern::RootPulse,
        counter: false,
    },
    Profile {
        id: "steady-fifth",
        pad: ChordalPadPattern::QuarterPulse,
        voicing: ChordalPadVoicing::Smooth,
        bass: BassLinePattern::RootFifth,
        counter: false,
    },
    Profile {
        id: "offbeat-approach",
        pad: ChordalPadPattern::OffbeatPulse,
        voicing: ChordalPadVoicing::Open,
        bass: BassLinePattern::Approach,
        counter: false,
    },
    Profile {
        id: "arpeggio-answer",
        pad: ChordalPadPattern::ArpeggioUpDown,
        voicing: ChordalPadVoicing::Smooth,
        bass: BassLinePattern::RootFifth,
        counter: true,
    },
    Profile {
        id: "meter-answer",
        pad: ChordalPadPattern::MeterPulse,
        voicing: ChordalPadVoicing::GlobalSmooth,
        bass: BassLinePattern::Approach,
        counter: true,
    },
    // Narrow harmony ranges can make open voicings impossible, while an
    // approaching bass can collide with low harmony. Keep both rhythmic
    // textures available with a close voicing and a bounded root/fifth bass.
    Profile {
        id: "offbeat-fifth",
        pad: ChordalPadPattern::OffbeatPulse,
        voicing: ChordalPadVoicing::Smooth,
        bass: BassLinePattern::RootFifth,
        counter: false,
    },
    Profile {
        id: "meter-fifth-answer",
        pad: ChordalPadPattern::MeterPulse,
        voicing: ChordalPadVoicing::GlobalSmooth,
        bass: BassLinePattern::RootFifth,
        counter: true,
    },
];

fn diagnostic(code: &str, measure_index: Option<usize>) -> ArrangementProposalDiagnostic {
    ArrangementProposalDiagnostic {
        code: code.into(),
        measure_index,
    }
}

fn role_index(result: &EnsembleRoleResult, role: EnsembleRole) -> Option<usize> {
    result
        .role_parts
        .iter()
        .find(|part| part.role == role)
        .map(|part| part.part_index)
}

fn attack_count(score: &Score, part: Option<usize>) -> usize {
    part.and_then(|index| score.parts.get(index))
        .map_or(0, |part| {
            part.staves
                .iter()
                .flat_map(|staff| &staff.measures)
                .flat_map(|measure| &measure.voices)
                .flatten()
                .filter(|note| !note.is_rest && !note.tie_end)
                .count()
        })
}

fn features(
    score: &Score,
    arranged: &EnsembleRoleResult,
    changed_harmony_spans: usize,
) -> ArrangementProposalFeatures {
    let mut peak = 0;
    let measures = score.parts[0].staves[0].measures.len();
    let mut attacks_by_measure = Vec::with_capacity(measures);
    for measure in 0..measures {
        let attacks = arranged
            .role_parts
            .iter()
            .map(|part| {
                score.parts[part.part_index].staves[0].measures[measure].voices[0]
                    .iter()
                    .filter(|note| !note.is_rest && !note.tie_end)
                    .count()
            })
            .sum();
        peak = peak.max(attacks);
        attacks_by_measure.push(attacks);
    }
    ArrangementProposalFeatures {
        harmony_attack_count: attack_count(score, role_index(arranged, EnsembleRole::Harmony)),
        bass_attack_count: attack_count(score, role_index(arranged, EnsembleRole::Bass)),
        countermelody_attack_count: attack_count(
            score,
            role_index(arranged, EnsembleRole::Countermelody),
        ),
        peak_generated_attacks_per_measure: peak,
        generated_attacks_by_measure: attacks_by_measure,
        generated_part_count: arranged.role_parts.len(),
        changed_harmony_spans,
    }
}

/// A note-ID-independent written event signature. The role, instrument,
/// measure, onset, duration, attack status, articulation and all chord pitches
/// take part in deduplication.
#[derive(Debug, Clone, PartialEq)]
struct WrittenEventSignature {
    role: u8,
    program: u8,
    measure: usize,
    onset_tick: u32,
    duration_tick: u32,
    pitch: i16,
    attack: bool,
    articulations: Vec<Articulation>,
    dynamic: Option<Dynamic>,
}

fn event_signature(score: &Score, arranged: &EnsembleRoleResult) -> Vec<WrittenEventSignature> {
    let mut signature = Vec::new();
    for role_part in &arranged.role_parts {
        let part = &score.parts[role_part.part_index];
        let role = match role_part.role {
            EnsembleRole::Harmony => 0,
            EnsembleRole::Bass => 1,
            EnsembleRole::Countermelody => 2,
        };
        for (measure, bar) in part.staves[0].measures.iter().enumerate() {
            let mut onset = 0.0_f64;
            for note in &bar.voices[0] {
                let duration = note.beats();
                if !note.is_rest {
                    for pitch in &note.pitches {
                        signature.push(WrittenEventSignature {
                            role,
                            program: part.midi_program,
                            measure,
                            onset_tick: (onset * 32.0).round() as u32,
                            duration_tick: (duration * 32.0).round() as u32,
                            pitch: pitch.to_midi(),
                            attack: !note.tie_end,
                            articulations: note.articulations.clone(),
                            dynamic: note.dynamic,
                        });
                    }
                }
                onset += duration;
            }
        }
    }
    signature
}

fn options_for(
    template: &EnsembleRoleOptions,
    chord: &ChordProposalCandidate,
    profile: Profile,
) -> Option<EnsembleRoleOptions> {
    let selection =
        chord_plan_to_preset_selection(&chord.plan, chord.resolved.bass_mode).selection?;
    let mut options = template.clone();
    let harmony = options.harmony.as_mut()?;
    harmony.chords = selection.chords.clone();
    harmony.pattern = profile.pad;
    harmony.voicing = profile.voicing;
    let bass = options.bass.as_mut()?;
    bass.chords = selection.chords.clone();
    bass.pattern = profile.bass;
    if profile.counter {
        if let Some(counter) = options.countermelody.as_mut() {
            counter.chords = selection.chords;
        }
    } else {
        options.countermelody = None;
        options.countermelody_dynamic = None;
        options
            .sections
            .retain(|section| section.role != EnsembleRole::Countermelody);
    }
    Some(options)
}

fn resolved_plan_matches_options(resolved: &ResolvedArrangementProposal) -> bool {
    if resolved.plan.contract_version != CHORD_PLAN_CONTRACT_VERSION
        || resolved.plan.entries.is_empty()
        || resolved.plan.entries.len() > MAX_LOCKS
        || resolved.plan.entries[0].position.measure_index != 0
        || resolved.plan.entries[0].position.offset.numerator != 0
        || resolved.plan.entries.iter().any(|entry| {
            entry.position.offset.denominator == 0
                || !entry.offset_beats.is_finite()
                || (entry.offset_beats - entry.position.offset.beats()).abs() > 1e-9
                || !entry.position.offset.on_preset_grid()
        })
        || resolved
            .plan
            .entries
            .windows(2)
            .any(|pair| !position_before(&pair[0], &pair[1]))
    {
        return false;
    }
    let chords = resolved
        .plan
        .entries
        .iter()
        .map(|entry| PositionedAccordionChord {
            measure_index: entry.position.measure_index,
            offset_beats: entry.position.offset.beats(),
            chord: entry.chord.clone(),
        })
        .collect::<Vec<_>>();
    resolved
        .ensemble_options
        .harmony
        .as_ref()
        .is_some_and(|options| options.chords == chords)
        && resolved
            .ensemble_options
            .bass
            .as_ref()
            .is_some_and(|options| options.chords == chords)
        && resolved
            .ensemble_options
            .countermelody
            .as_ref()
            .is_none_or(|options| options.chords == chords)
}

/// Return exactly 3–5 distinct complete arrangements or no candidate set.
/// The pool is deterministic; no random seed or external model is used.
pub fn suggest_arrangement_proposals(
    score: &Score,
    request: &ArrangementProposalRequest,
) -> ArrangementProposalResult {
    let source_fingerprint = fingerprint(score).unwrap_or_default();
    let mut result = ArrangementProposalResult {
        contract_version: ARRANGEMENT_PROPOSAL_CONTRACT_VERSION,
        source_fingerprint: source_fingerprint.clone(),
        candidates: Vec::new(),
        diagnostics: Vec::new(),
    };
    if !(3..=5).contains(&request.count)
        || request.locked_chord_entries.len() > MAX_LOCKS
        || request.ensemble_template.harmony.is_none()
        || request.ensemble_template.bass.is_none()
    {
        result
            .diagnostics
            .push(diagnostic("invalid-proposal-request", None));
        return result;
    }
    let melody_part_index = request
        .ensemble_template
        .harmony
        .as_ref()
        .and_then(|options| options.melody_part_index)
        .unwrap_or(0);
    let Some(melody_part) = score.parts.get(melody_part_index) else {
        result
            .diagnostics
            .push(diagnostic("unsupported-melody-part", None));
        return result;
    };
    let Some(melody_staff) = melody_part.staves.first() else {
        result
            .diagnostics
            .push(diagnostic("unsupported-melody-part", None));
        return result;
    };
    let mut melody_score = score.clone();
    let mut selected_part = melody_part.clone();
    selected_part.staves = vec![melody_staff.clone()];
    melody_score.parts = vec![selected_part];
    let proposed = suggest_chord_plans_with_locks(
        &melody_score,
        &request.harmony,
        &request.locked_chord_entries,
    );
    if proposed.candidates.is_empty() {
        result.diagnostics.extend(
            proposed
                .diagnostics
                .iter()
                .map(|item| diagnostic(&item.code, item.measure_index)),
        );
        return result;
    }
    let mut unique = Vec::new();
    // First sample distinct textures and harmonic plans, then try every
    // remaining pairing to fill the requested count after failed realizations.
    let plan_count = proposed.candidates.len();
    // When a countermelody is requested, try both answer textures before
    // filling a three-proposal set with accompaniment-only textures.
    let profile_order = if request.ensemble_template.countermelody.is_some() {
        [0, 3, 4, 1, 2, 5, 6]
    } else {
        [0, 1, 2, 3, 4, 5, 6]
    };
    let order = profile_order
        .into_iter()
        .map(|profile| (profile, profile % plan_count))
        .chain(
            profile_order
                .into_iter()
                .flat_map(|profile| (0..plan_count).map(move |plan| (profile, plan))),
        );
    for (profile_index, plan_index) in order {
        if result.candidates.len() == usize::from(request.count) {
            break;
        }
        let profile = PROFILES[profile_index];
        let plan = &proposed.candidates[plan_index];
        let Some(options) = options_for(&request.ensemble_template, plan, profile) else {
            continue;
        };
        let mut arranged = arrange_ensemble_roles(score, options);
        let Some(output) = arranged.score.take() else {
            continue;
        };
        let signature = event_signature(&output, &arranged);
        if unique.contains(&signature) {
            continue;
        }
        unique.push(signature);
        let id = format!("{}-{}", plan.id, profile.id);
        let mut reason_codes = plan.reason_codes.clone();
        reason_codes.push(format!("texture-{}", profile.id));
        if arranged
            .role_parts
            .iter()
            .any(|part| part.role == EnsembleRole::Countermelody)
        {
            reason_codes.push("motif-derived-countermelody".into());
        }
        let resolved = ResolvedArrangementProposal {
            contract_version: ARRANGEMENT_PROPOSAL_CONTRACT_VERSION,
            source_fingerprint: source_fingerprint.clone(),
            id: id.clone(),
            harmony_candidate_id: plan.id.clone(),
            profile_id: profile.id.into(),
            plan: plan.plan.clone(),
            ensemble_options: arranged.resolved_options.clone(),
        };
        let features = features(&output, &arranged, plan.changed_spans.len());
        result.candidates.push(ArrangementProposalCandidate {
            id,
            score: output,
            plan: plan.plan.clone(),
            resolved,
            features,
            reason_codes,
            difference_codes: Vec::new(),
        });
    }
    if result.candidates.len() < usize::from(request.count) {
        result.candidates.clear();
        result
            .diagnostics
            .push(diagnostic("insufficient-distinct-arrangements", None));
    } else if let Some(reference) = result.candidates.first() {
        let reference_plan = reference
            .resolved
            .ensemble_options
            .harmony
            .as_ref()
            .map(|options| &options.chords)
            .cloned();
        let reference_features = reference.features.clone();
        let reference_signature = unique[0].clone();
        for (index, candidate) in result.candidates.iter_mut().enumerate() {
            if index == 0 {
                candidate
                    .difference_codes
                    .push("reference-arrangement".into());
                continue;
            }
            if candidate
                .resolved
                .ensemble_options
                .harmony
                .as_ref()
                .map(|options| &options.chords)
                .cloned()
                != reference_plan
            {
                candidate.difference_codes.push("harmony-changed".into());
            }
            let signature = &unique[index];
            for (role, code) in [
                (0, "harmony-notes-changed"),
                (1, "bass-line-changed"),
                (2, "countermelody-changed"),
            ] {
                if signature
                    .iter()
                    .filter(|event| event.role == role)
                    .ne(reference_signature
                        .iter()
                        .filter(|event| event.role == role))
                {
                    candidate.difference_codes.push(code.into());
                }
            }
            if candidate.features.peak_generated_attacks_per_measure
                != reference_features.peak_generated_attacks_per_measure
                || candidate.features.generated_attacks_by_measure
                    != reference_features.generated_attacks_by_measure
                || candidate.features.harmony_attack_count
                    != reference_features.harmony_attack_count
                || candidate.features.bass_attack_count != reference_features.bass_attack_count
            {
                candidate.difference_codes.push("density-changed".into());
            }
            if candidate.features.generated_part_count != reference_features.generated_part_count {
                candidate
                    .difference_codes
                    .push("orchestration-changed".into());
            }
        }
    }
    result
}

/// Replay the selected resolved harmony, texture, instrument range and activity.
pub fn replay_arrangement_proposal(
    score: &Score,
    resolved: &ResolvedArrangementProposal,
) -> ArrangementProposalReplayResult {
    let mut result = ArrangementProposalReplayResult {
        score: None,
        diagnostics: Vec::new(),
    };
    if resolved.contract_version != ARRANGEMENT_PROPOSAL_CONTRACT_VERSION {
        result
            .diagnostics
            .push(diagnostic("unsupported-version", None));
        return result;
    }
    if fingerprint(score).as_deref() != Some(resolved.source_fingerprint.as_str()) {
        result.diagnostics.push(diagnostic("input-mismatch", None));
        return result;
    }
    if !resolved_plan_matches_options(resolved) {
        result
            .diagnostics
            .push(diagnostic("inconsistent-resolved-plan", None));
        return result;
    }
    let arranged = arrange_ensemble_roles(score, resolved.ensemble_options.clone());
    if arranged.score.is_none() {
        result.diagnostics.extend(
            arranged
                .diagnostics
                .iter()
                .map(|item| diagnostic(&item.code, item.measure_index)),
        );
    }
    result.score = arranged.score;
    result
}

fn boundary_has_tie(score: &Score, part_index: usize, boundary: usize) -> bool {
    let Some(staff) = score
        .parts
        .get(part_index)
        .and_then(|part| part.staves.first())
    else {
        return true;
    };
    let previous = boundary
        .checked_sub(1)
        .and_then(|index| staff.measures.get(index))
        .and_then(|bar| bar.voices.first())
        .and_then(|voice| voice.last());
    let next = staff
        .measures
        .get(boundary)
        .and_then(|bar| bar.voices.first())
        .and_then(|voice| voice.first());
    previous.is_some_and(|note| note.tie_start) || next.is_some_and(|note| note.tie_end)
}

fn silent_measure(score: &Score, part: &Part, measure: usize) -> Option<Measure> {
    let mut bar = part.staves.first()?.measures.get(measure)?.clone();
    let mut rests = Vec::new();
    if !emit_rest(tick(score.playback_measure_beats(measure))?, &mut rests) {
        return None;
    }
    for voice in &mut bar.voices {
        if !voice.is_empty() {
            *voice = rests.clone();
        }
    }
    Some(bar)
}

fn region_fail(
    mut result: ArrangementRegionResult,
    code: &str,
    measure_index: Option<usize>,
) -> ArrangementRegionResult {
    result.score = None;
    result.role_parts.clear();
    result.changed_roles.clear();
    result.changed_measures.clear();
    result.diagnostics.push(diagnostic(code, measure_index));
    result
}

fn position_before(left: &ChordPlanEntry, right: &ChordPlanEntry) -> bool {
    left.position.measure_index < right.position.measure_index
        || left.position.measure_index == right.position.measure_index
            && u128::from(left.position.offset.numerator)
                * u128::from(right.position.offset.denominator)
                < u128::from(right.position.offset.numerator)
                    * u128::from(left.position.offset.denominator)
}

fn locked_harmony_retained(
    original: &ChordPlan,
    replacement: &ChordPlan,
    locked_entries: &[usize],
) -> bool {
    for &index in locked_entries {
        let Some(locked) = original.entries.get(index) else {
            return false;
        };
        let end = original.entries.get(index + 1);
        let Some(at_start) = replacement
            .entries
            .iter()
            .rev()
            .find(|entry| !position_before(locked, entry))
        else {
            return false;
        };
        if at_start.chord != locked.chord
            || replacement.entries.iter().any(|entry| {
                position_before(locked, entry)
                    && end.is_none_or(|end| position_before(entry, end))
                    && entry.chord != locked.chord
            })
        {
            return false;
        }
    }
    true
}

/// Replace only selected generated measures. Source parts, locked measures,
/// locked roles and all generated measures outside the region are copied from
/// the original proposal. Cross-boundary ties are rejected atomically.
pub fn regenerate_arrangement_region(
    source: &Score,
    mut request: ArrangementRegionRequest,
) -> ArrangementRegionResult {
    request.locked_roles.sort();
    request.locked_roles.dedup();
    request.locked_measures.sort();
    request.locked_measures.dedup();
    request.locked_chord_entries.sort();
    request.locked_chord_entries.dedup();
    let mut result = ArrangementRegionResult {
        score: None,
        resolved_request: request,
        role_parts: Vec::new(),
        changed_roles: Vec::new(),
        changed_measures: Vec::new(),
        diagnostics: Vec::new(),
    };
    let request = result.resolved_request.clone();
    let Some(count) = source
        .parts
        .first()
        .and_then(|part| part.staves.first())
        .map(|staff| staff.measures.len())
    else {
        return region_fail(result, "unsupported-source", None);
    };
    if request.start_measure >= request.end_measure_exclusive
        || request.end_measure_exclusive > count
        || request.locked_roles.len() > 3
        || request.locked_measures.len() > MAX_LOCKS
        || request.locked_chord_entries.len() > MAX_LOCKS
        || request.locked_measures.iter().any(|index| *index >= count)
    {
        return region_fail(result, "invalid-region", Some(request.start_measure));
    }
    if request.original.contract_version != ARRANGEMENT_PROPOSAL_CONTRACT_VERSION
        || request.replacement.contract_version != ARRANGEMENT_PROPOSAL_CONTRACT_VERSION
    {
        return region_fail(result, "unsupported-version", None);
    }
    let fingerprint = fingerprint(source);
    if fingerprint.as_deref() != Some(request.original.source_fingerprint.as_str())
        || fingerprint.as_deref() != Some(request.replacement.source_fingerprint.as_str())
    {
        return region_fail(result, "input-mismatch", None);
    }
    if !resolved_plan_matches_options(&request.original)
        || !resolved_plan_matches_options(&request.replacement)
    {
        return region_fail(result, "inconsistent-resolved-plan", None);
    }
    let original_levels = &request.original.ensemble_options;
    let replacement_levels = &request.replacement.ensemble_options;
    if original_levels.harmony_dynamic != replacement_levels.harmony_dynamic
        || original_levels.bass_dynamic != replacement_levels.bass_dynamic
        || original_levels.countermelody.is_some()
            && replacement_levels.countermelody.is_some()
            && original_levels.countermelody_dynamic != replacement_levels.countermelody_dynamic
    {
        return region_fail(result, "incompatible-dynamics", None);
    }
    if request
        .locked_chord_entries
        .iter()
        .any(|index| *index >= request.original.plan.entries.len())
    {
        return region_fail(result, "invalid-chord-lock", None);
    }
    if !locked_harmony_retained(
        &request.original.plan,
        &request.replacement.plan,
        &request.locked_chord_entries,
    ) {
        return region_fail(result, "locked-harmony-changed", None);
    }
    let mut original = arrange_ensemble_roles(source, request.original.ensemble_options.clone());
    let mut replacement =
        arrange_ensemble_roles(source, request.replacement.ensemble_options.clone());
    let (Some(mut output), Some(next)) = (original.score.take(), replacement.score.take()) else {
        return region_fail(result, "proposal-replay-failed", None);
    };
    result.role_parts = original.role_parts.clone();
    let outside_events = |score: &Score| {
        to_playback_events(score, &Default::default())
            .into_iter()
            .filter(|event| {
                event.source.as_ref().is_some_and(|address| {
                    address.measure < request.start_measure
                        || address.measure >= request.end_measure_exclusive
                        || request.locked_measures.contains(&address.measure)
                })
            })
            .collect::<Vec<_>>()
    };
    let original_outside_playback = outside_events(&output);
    let original_events = event_signature(&output, &original);
    let replacement_events = event_signature(&next, &replacement);
    for role in [
        EnsembleRole::Harmony,
        EnsembleRole::Bass,
        EnsembleRole::Countermelody,
    ] {
        if request.locked_roles.contains(&role) {
            continue;
        }
        let role_code = match role {
            EnsembleRole::Harmony => 0,
            EnsembleRole::Bass => 1,
            EnsembleRole::Countermelody => 2,
        };
        let original_part = original
            .role_parts
            .iter()
            .find(|part| part.role == role)
            .map(|part| part.part_index);
        let replacement_part = replacement
            .role_parts
            .iter()
            .find(|part| part.role == role)
            .map(|part| part.part_index);
        if !((request.start_measure..request.end_measure_exclusive).any(|measure| {
            !request.locked_measures.contains(&measure)
                && original_events
                    .iter()
                    .filter(|event| event.role == role_code && event.measure == measure)
                    .ne(replacement_events
                        .iter()
                        .filter(|event| event.role == role_code && event.measure == measure))
        })) {
            continue;
        }
        let output_part = match (original_part, replacement_part) {
            (Some(left), Some(right)) => {
                let current = &output.parts[left];
                let replacement = &next.parts[right];
                if current.midi_program != replacement.midi_program
                    || current.midi_channel != replacement.midi_channel
                    || current.staves.len() != 1
                    || replacement.staves.len() != 1
                {
                    return region_fail(result, "incompatible-instrument", None);
                }
                left
            }
            (Some(left), None) => left,
            (None, Some(right)) => {
                let mut added = next.parts[right].clone();
                if added.staves.len() != 1 {
                    return region_fail(result, "incompatible-instrument", None);
                }
                for measure in 0..count {
                    let Some(silence) = silent_measure(source, &added, measure) else {
                        return region_fail(result, "unsupported-rhythm", Some(measure));
                    };
                    added.staves[0].measures[measure] = silence;
                }
                let index = output.parts.len();
                output.parts.push(added);
                result.role_parts.push(EnsembleRolePart {
                    role,
                    part_index: index,
                });
                index
            }
            (None, None) => continue,
        };
        for measure in request.start_measure..request.end_measure_exclusive {
            if request.locked_measures.contains(&measure) {
                continue;
            }
            if original_events
                .iter()
                .filter(|event| event.role == role_code && event.measure == measure)
                .eq(replacement_events
                    .iter()
                    .filter(|event| event.role == role_code && event.measure == measure))
            {
                continue;
            }
            if (measure == request.start_measure && measure > 0
                || measure > request.start_measure
                    && request.locked_measures.contains(&(measure - 1)))
                && (boundary_has_tie(&output, output_part, measure)
                    || replacement_part.is_some_and(|part| boundary_has_tie(&next, part, measure)))
            {
                return region_fail(result, "cross-boundary-tie", Some(measure));
            }
            if (measure + 1 == request.end_measure_exclusive && measure + 1 < count
                || measure + 1 < count && request.locked_measures.contains(&(measure + 1)))
                && (boundary_has_tie(&output, output_part, measure + 1)
                    || replacement_part
                        .is_some_and(|part| boundary_has_tie(&next, part, measure + 1)))
            {
                return region_fail(result, "cross-boundary-tie", Some(measure + 1));
            }
            let next_measure = if let Some(part) = replacement_part {
                next.parts[part].staves[0].measures[measure].clone()
            } else {
                let Some(silence) = silent_measure(source, &output.parts[output_part], measure)
                else {
                    return region_fail(result, "unsupported-rhythm", Some(measure));
                };
                silence
            };
            output.parts[output_part].staves[0].measures[measure] = next_measure;
            if !result.changed_roles.contains(&role) {
                result.changed_roles.push(role);
            }
            if !result.changed_measures.contains(&measure) {
                result.changed_measures.push(measure);
            }
        }
        let level = match role {
            EnsembleRole::Harmony => original_levels.harmony_dynamic,
            EnsembleRole::Bass => original_levels.bass_dynamic,
            EnsembleRole::Countermelody => original_part
                .map_or(replacement_levels.countermelody_dynamic, |_| {
                    original_levels.countermelody_dynamic
                }),
        };
        if result.changed_roles.contains(&role)
            && let Some(level) = level
        {
            let first = output.parts[output_part].staves[0]
                .measures
                .iter()
                .enumerate()
                .find_map(|(measure, bar)| {
                    bar.voices[0]
                        .iter()
                        .position(|note| !note.is_rest && !note.tie_end)
                        .map(|note| (measure, note))
                });
            if let Some((measure, note)) = first
                && (request.start_measure..request.end_measure_exclusive).contains(&measure)
                && !request.locked_measures.contains(&measure)
            {
                output.parts[output_part].staves[0].measures[measure].voices[0][note].dynamic =
                    Some(level);
            }
        }
    }
    result.changed_measures.sort();
    if original_outside_playback != outside_events(&output) {
        return region_fail(result, "outside-playback-changed", None);
    }
    if !super::validate::validate(&output).is_valid() {
        return region_fail(result, "invalid-regenerated-score", None);
    }
    result.score = Some(output);
    result
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::{Duration, Note, Pitch};

    #[test]
    fn event_identity_distinguishes_reattack_and_articulation() {
        let mut source = Score::new("event identity", 120, 4, 4, 0, 1);
        source.parts[0].staves[0].measures[0].voices[0] =
            vec![Note::new(Pitch::from_midi(72, false), Duration::Whole)];
        let options: EnsembleRoleOptions = serde_json::from_value(serde_json::json!({
            "harmony":{
                "chords":[{"measure_index":0,"offset_beats":0.0,
                    "chord":{"root":"C","kind":"major"}}],
                "pattern":"held","midi_program":0,
                "low_midi":48,"center_midi":60,"high_midi":72
            }
        }))
        .unwrap();
        let arranged = arrange_ensemble_roles(&source, options);
        let original = arranged.score.as_ref().unwrap();
        let signature = event_signature(original, &arranged);
        let mut tied = original.clone();
        tied.parts[1].staves[0].measures[0].voices[0][0].tie_end = true;
        assert_ne!(signature, event_signature(&tied, &arranged));
        let mut accented = original.clone();
        accented.parts[1].staves[0].measures[0].voices[0][0]
            .articulations
            .push(Articulation::Accent);
        assert_ne!(signature, event_signature(&accented, &arranged));
    }

    #[test]
    fn harmonic_lock_covers_midbar_insertions_until_the_next_original_change() {
        let entry = |numerator: u32, root: &str| {
            serde_json::json!({
                "position":{"measure_index":0,
                    "offset":{"numerator":numerator,"denominator":1}},
                "offset_beats":numerator as f64,
                "chord":{"root":root,"kind":"major"},
                "origin":"host-authored","sources":[]
            })
        };
        let original: ChordPlan = serde_json::from_value(serde_json::json!({
            "contract_version":1,
            "entries":[entry(0,"C"),entry(2,"F")]
        }))
        .unwrap();
        let inserted: ChordPlan = serde_json::from_value(serde_json::json!({
            "contract_version":1,
            "entries":[entry(0,"C"),entry(1,"G"),entry(2,"F")]
        }))
        .unwrap();
        assert!(!locked_harmony_retained(&original, &inserted, &[0]));
        let repeated: ChordPlan = serde_json::from_value(serde_json::json!({
            "contract_version":1,
            "entries":[entry(0,"C"),entry(1,"C"),entry(2,"F")]
        }))
        .unwrap();
        assert!(locked_harmony_retained(&original, &repeated, &[0]));
    }
}
