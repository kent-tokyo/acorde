//! Phrase-authored harmonic rhythm before bounded chord search.

use serde::{Deserialize, Serialize};

use super::chord_plan::{
    BeatFraction, CHORD_PLAN_CONTRACT_VERSION, ChordPlan, ChordPlanOrigin, ChordPlanPosition,
    chord_plan_to_preset_selection,
};
use super::chord_proposals::{
    ChordCadenceKind, ChordCadenceTarget, ChordColorKind, ChordColorTarget, ChordKeyRegion,
    ChordProposalDiagnostic, ChordProposalRequest, ChordProposalResult, changed_spans, key_at,
    suggest_chord_plans_at_positions_with_context, suggest_chord_plans_with_locks_and_context,
    tonic,
};
use super::chordal_pad::default_meter_groups;
use super::score::Score;

const MAX_PHRASES: usize = 64;
const MAX_POSITIONS: usize = 512;

mod targets;
pub use targets::{
    ModulationApproach, PHRASE_HARMONY_TARGET_CONTRACT_VERSION, PhraseHarmonyTargetCandidate,
    PhraseHarmonyTargetError, PhraseHarmonyTargetRejection, PhraseHarmonyTargetRequest,
    PhraseHarmonyTargetResult, PhraseModulationTarget, ResolvedModulationChoice,
    ResolvedPhraseHarmonyTarget, SecondaryDominantTarget, TargetResolutionEvidence,
    replay_phrase_harmony_target, suggest_phrase_harmony_targets,
};

#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "kebab-case")]
pub enum PhraseHarmonicPace {
    EverySecondBar,
    EveryBar,
    TwoGroupsPerBar,
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
#[serde(deny_unknown_fields)]
pub struct PhraseHarmonySection {
    pub start_measure: usize,
    pub end_measure_exclusive: usize,
    pub pace: PhraseHarmonicPace,
    /// Optional denominator-unit beat grouping for every bar in this section.
    /// The total must equal each meter numerator. Omit for Acorde's defaults.
    #[serde(default)]
    pub meter_groups: Vec<u8>,
}

#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
#[serde(deny_unknown_fields)]
pub struct PhraseHarmonyRequest {
    pub phrases: Vec<PhraseHarmonySection>,
    pub proposal: ChordProposalRequest,
    /// Indices in proposal.source_plan; the source chord and its region remain
    /// exact when the phrase-generated boundaries match the source plan.
    #[serde(default)]
    pub locked_chord_entries: Vec<usize>,
    /// Optional source-plan index for each generated position, in order. This
    /// explicitly authorizes moving or duplicating an unlocked source chord.
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub source_reanchor: Option<Vec<usize>>,
    /// Optional key overrides used only during chord search, beginning and
    /// ending at generated bar-start chord boundaries.
    #[serde(default, skip_serializing_if = "Vec::is_empty")]
    pub local_keys: Vec<ChordKeyRegion>,
    /// Root-motion targets at generated harmonic positions.
    #[serde(default, skip_serializing_if = "Vec::is_empty")]
    pub cadences: Vec<ChordCadenceTarget>,
    /// Required parallel-mode borrowing or chromatic passing at selected
    /// phrase-generated harmonic positions.
    #[serde(default, skip_serializing_if = "Vec::is_empty")]
    pub colors: Vec<ChordColorTarget>,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct PhraseHarmonyResult {
    pub positions: Vec<ChordPlanPosition>,
    pub proposals: ChordProposalResult,
    pub diagnostics: Vec<ChordProposalDiagnostic>,
    pub contract_version: u16,
    /// The exact provisional plan used for source-aware progression search.
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub reanchored_source_plan: Option<ChordPlan>,
    /// Explicitly omitted source entries when a slower pace merges regions.
    #[serde(default, skip_serializing_if = "Vec::is_empty")]
    pub omitted_source_entries: Vec<usize>,
    #[serde(default, skip_serializing_if = "Vec::is_empty")]
    pub applied_local_keys: Vec<ChordKeyRegion>,
    #[serde(default, skip_serializing_if = "Vec::is_empty")]
    pub applied_cadences: Vec<ChordCadenceTarget>,
    #[serde(default, skip_serializing_if = "Vec::is_empty")]
    pub applied_colors: Vec<ChordColorTarget>,
}

fn failure(code: &str, measure: Option<usize>) -> PhraseHarmonyResult {
    PhraseHarmonyResult {
        positions: Vec::new(),
        proposals: ChordProposalResult {
            contract_version: super::chord_proposals::CHORD_PROPOSAL_CONTRACT_VERSION,
            input_fingerprint: String::new(),
            candidates: Vec::new(),
            diagnostics: Vec::new(),
        },
        diagnostics: vec![ChordProposalDiagnostic {
            code: code.into(),
            measure_index: measure,
        }],
        contract_version: 1,
        reanchored_source_plan: None,
        omitted_source_entries: Vec::new(),
        applied_local_keys: Vec::new(),
        applied_cadences: Vec::new(),
        applied_colors: Vec::new(),
    }
}

fn pushed(
    positions: &mut Vec<ChordPlanPosition>,
    measure_index: usize,
    offset: BeatFraction,
) -> bool {
    if positions.len() >= MAX_POSITIONS {
        return false;
    }
    positions.push(ChordPlanPosition {
        measure_index,
        offset,
    });
    true
}

fn same_position(left: &ChordPlanPosition, right: &ChordPlanPosition) -> bool {
    left.measure_index == right.measure_index
        && u128::from(left.offset.numerator) * u128::from(right.offset.denominator)
            == u128::from(right.offset.numerator) * u128::from(left.offset.denominator)
}

fn before(left: &ChordPlanPosition, right: &ChordPlanPosition) -> bool {
    left.measure_index < right.measure_index
        || left.measure_index == right.measure_index
            && u128::from(left.offset.numerator) * u128::from(right.offset.denominator)
                < u128::from(right.offset.numerator) * u128::from(left.offset.denominator)
}

fn changed_original_regions(
    source: &ChordPlan,
    changed: &[super::chord_proposals::ChordChangedSpan],
    measure_count: usize,
) -> usize {
    source
        .entries
        .iter()
        .enumerate()
        .filter(|(index, entry)| {
            let end = source.entries.get(index + 1).map_or_else(
                || ChordPlanPosition {
                    measure_index: measure_count,
                    offset: BeatFraction::ZERO,
                },
                |next| next.position.clone(),
            );
            changed
                .iter()
                .any(|span| before(&entry.position, &span.end) && before(&span.start, &end))
        })
        .count()
}

fn matches_cadences(
    score: &Score,
    plan: &ChordPlan,
    positions: &[ChordPlanPosition],
    cadences: &[ChordCadenceTarget],
    local_keys: &[ChordKeyRegion],
) -> bool {
    cadences.iter().all(|target| {
        let arrival = &positions[target.arrival_position_index];
        let Some(home) = tonic(key_at(score, arrival.measure_index, local_keys)) else {
            return false;
        };
        let root_at = |position: &ChordPlanPosition| {
            plan.entries
                .iter()
                .find(|entry| same_position(&entry.position, position))
                .and_then(|entry| super::arrange::accordion_pitch_class(&entry.chord.root))
        };
        let arrival_root = root_at(arrival);
        match target.kind {
            ChordCadenceKind::DominantArrival => arrival_root == Some((home + 7) % 12),
            ChordCadenceKind::DominantTonic => {
                arrival_root == Some(home)
                    && root_at(&positions[target.arrival_position_index - 1])
                        == Some((home + 7) % 12)
            }
            ChordCadenceKind::SubdominantTonic => {
                arrival_root == Some(home)
                    && root_at(&positions[target.arrival_position_index - 1])
                        == Some((home + 5) % 12)
            }
        }
    })
}

fn matches_colors(
    score: &Score,
    plan: &ChordPlan,
    positions: &[ChordPlanPosition],
    colors: &[ChordColorTarget],
    local_keys: &[ChordKeyRegion],
) -> bool {
    let root_at = |index: usize| {
        plan.entries
            .iter()
            .find(|entry| same_position(&entry.position, &positions[index]))
            .and_then(|entry| super::arrange::accordion_pitch_class(&entry.chord.root))
    };
    colors.iter().all(|target| {
        let index = target.position_index;
        let Some(entry) = plan
            .entries
            .iter()
            .find(|entry| same_position(&entry.position, &positions[index]))
        else {
            return false;
        };
        match target.kind {
            ChordColorKind::BorrowedFromParallel => {
                let key = key_at(score, positions[index].measure_index, local_keys);
                let Some(home) = tonic(key) else { return false };
                let Some(root) = root_at(index) else {
                    return false;
                };
                let degree = (root + 12 - home) % 12;
                if key.mode == "minor" {
                    matches!(
                        (degree, entry.chord.kind.as_str()),
                        (0, "major") | (2, "minor") | (5, "major") | (9, "minor")
                    )
                } else {
                    matches!(
                        (degree, entry.chord.kind.as_str()),
                        (3, "major") | (5, "minor") | (8, "major") | (10, "major")
                    )
                }
            }
            ChordColorKind::ChromaticPassing => {
                let (Some(before), Some(here), Some(after)) =
                    (root_at(index - 1), root_at(index), root_at(index + 1))
                else {
                    return false;
                };
                entry.chord.kind == "diminished"
                    && matches!((here + 12 - before) % 12, 1 | 11)
                    && (here + 12 - before) % 12 == (after + 12 - here) % 12
            }
        }
    })
}

struct ReanchoredSource {
    plan: ChordPlan,
    locked_targets: Vec<usize>,
    omitted: Vec<usize>,
}

type ReanchorError = (&'static str, Option<usize>);

fn reanchor_source(
    score: &Score,
    source: &ChordPlan,
    positions: &[ChordPlanPosition],
    mapping: &[usize],
    locks: &[usize],
    bass_mode: super::arrange::AccordionBassMode,
) -> Result<ReanchoredSource, ReanchorError> {
    if source.contract_version != CHORD_PLAN_CONTRACT_VERSION
        || source.entries.is_empty()
        || source.entries.len() > MAX_POSITIONS
        || !same_position(&source.entries[0].position, &positions[0])
        || source
            .entries
            .windows(2)
            .any(|pair| !before(&pair[0].position, &pair[1].position))
        || source.entries.iter().fold(0usize, |total, entry| {
            total.saturating_add(entry.sources.len())
        }) > 4096
        || source.entries.iter().any(|entry| {
            entry.position.measure_index >= score.parts[0].staves[0].measures.len()
                || !entry.position.offset.on_preset_grid()
                || entry.position.offset.beats()
                    >= score.playback_measure_beats(entry.position.measure_index)
                || entry.chord.root.len() > 32
                || entry.chord.kind.len() > 64
                || entry
                    .chord
                    .bass
                    .as_ref()
                    .is_some_and(|bass| bass.len() > 32)
                || entry.chord.degrees.len() > 64
                || entry.sources.iter().any(|source| {
                    source
                        .reference
                        .as_ref()
                        .is_some_and(|reference| reference.len() > 2048)
                })
        })
        || chord_plan_to_preset_selection(source, bass_mode)
            .selection
            .is_none()
    {
        return Err(("invalid-reanchor-source-plan", None));
    }
    if mapping.len() != positions.len()
        || mapping.first() != Some(&0)
        || mapping.iter().any(|&index| index >= source.entries.len())
        || mapping.windows(2).any(|pair| pair[0] > pair[1])
    {
        return Err(("invalid-source-reanchor", None));
    }
    let mut locked_targets = Vec::new();
    for &index in locks {
        let Some(entry) = source.entries.get(index) else {
            return Err(("invalid-chord-lock", None));
        };
        let targets = mapping
            .iter()
            .enumerate()
            .filter(|(_, mapped)| **mapped == index)
            .map(|(target, _)| target)
            .collect::<Vec<_>>();
        let Some(&first) = targets.first() else {
            return Err((
                "locked-source-region-moved",
                Some(entry.position.measure_index),
            ));
        };
        if !same_position(&positions[first], &entry.position)
            || targets.iter().any(|&target| {
                before(&positions[target], &entry.position)
                    || source
                        .entries
                        .get(index + 1)
                        .is_some_and(|next| !before(&positions[target], &next.position))
            })
            || source.entries.get(index + 1).is_some_and(|next| {
                mapping
                    .iter()
                    .enumerate()
                    .find(|(_, mapped)| **mapped > index)
                    .is_none_or(|(target, _)| !same_position(&positions[target], &next.position))
            })
        {
            return Err((
                "locked-source-region-moved",
                Some(entry.position.measure_index),
            ));
        }
        locked_targets.extend(targets);
    }
    locked_targets.sort_unstable();
    locked_targets.dedup();
    let omitted = (0..source.entries.len())
        .filter(|index| !mapping.contains(index))
        .collect::<Vec<_>>();
    let entries = positions
        .iter()
        .zip(mapping)
        .map(|(position, &index)| {
            let mut entry = source.entries[index].clone();
            if !same_position(position, &entry.position) {
                entry.origin = ChordPlanOrigin::GeneratedReharmonization;
            }
            entry.position = position.clone();
            entry.offset_beats = position.offset.beats();
            entry
        })
        .collect();
    Ok(ReanchoredSource {
        plan: ChordPlan {
            contract_version: CHORD_PLAN_CONTRACT_VERSION,
            entries,
        },
        locked_targets,
        omitted,
    })
}

/// Choose chord-change locations from a complete written phrase schedule,
/// then run the existing deterministic progression search. Source harmony
/// uses identical boundaries or an explicit reanchoring map; locked regions
/// retain their original boundaries.
pub fn suggest_phrase_harmony(
    score: &Score,
    request: &PhraseHarmonyRequest,
) -> PhraseHarmonyResult {
    let Some(staff) = score.parts.first().and_then(|part| part.staves.first()) else {
        return failure("unsupported-phrase-source", None);
    };
    let count = staff.measures.len();
    if count == 0
        || count > 512
        || request.phrases.is_empty()
        || request.phrases.len() > MAX_PHRASES
        || request.locked_chord_entries.len() > MAX_POSITIONS
    {
        return failure("phrase-harmony-limit", None);
    }
    let mut positions = Vec::new();
    let mut cursor = 0usize;
    for phrase in &request.phrases {
        if phrase.start_measure != cursor
            || phrase.start_measure >= phrase.end_measure_exclusive
            || phrase.end_measure_exclusive > count
        {
            return failure("invalid-harmonic-phrase", Some(phrase.start_measure));
        }
        if phrase.meter_groups.len() > 32 || phrase.meter_groups.contains(&0) {
            return failure("invalid-harmonic-groups", Some(phrase.start_measure));
        }
        for measure in phrase.start_measure..phrase.end_measure_exclusive {
            let meter = staff.meter_at(measure, &score.settings.time_signature);
            let groups = if phrase.meter_groups.is_empty() {
                default_meter_groups(meter.numerator, meter.denominator)
            } else {
                phrase.meter_groups.clone()
            };
            if groups.iter().map(|item| usize::from(*item)).sum::<usize>()
                != usize::from(meter.numerator)
            {
                return failure("invalid-harmonic-groups", Some(measure));
            }
            if (phrase.pace != PhraseHarmonicPace::EverySecondBar
                || (measure - phrase.start_measure).is_multiple_of(2))
                && !pushed(&mut positions, measure, BeatFraction::ZERO)
            {
                return failure("phrase-harmony-limit", Some(measure));
            }
            if phrase.pace == PhraseHarmonicPace::TwoGroupsPerBar {
                if groups.len() < 2 {
                    return failure("unsupported-harmonic-groups", Some(measure));
                }
                let group_boundary = groups
                    .iter()
                    .take(groups.len().div_ceil(2))
                    .map(|group| u32::from(*group))
                    .sum::<u32>();
                let offset = BeatFraction {
                    numerator: group_boundary.saturating_mul(4),
                    denominator: u32::from(meter.denominator),
                };
                if !offset.on_preset_grid()
                    || offset.beats() >= score.playback_measure_beats(measure)
                {
                    return failure("unsupported-harmonic-groups", Some(measure));
                }
                if !pushed(&mut positions, measure, offset) {
                    return failure("phrase-harmony-limit", Some(measure));
                }
            }
        }
        cursor = phrase.end_measure_exclusive;
    }
    if cursor != count {
        return failure("incomplete-harmonic-phrases", Some(cursor));
    }
    if request.local_keys.len() > MAX_PHRASES {
        return failure("local-key-limit", None);
    }
    let mut prior_key_end = 0;
    for region in &request.local_keys {
        let bar_boundary = |measure| {
            positions.iter().any(|position| {
                position.measure_index == measure && position.offset == BeatFraction::ZERO
            })
        };
        if region.start_measure < prior_key_end
            || region.start_measure >= region.end_measure_exclusive
            || region.end_measure_exclusive > count
            || !bar_boundary(region.start_measure)
            || (region.end_measure_exclusive < count && !bar_boundary(region.end_measure_exclusive))
            || !(-7..=7).contains(&region.key.fifths)
            || !matches!(region.key.mode.as_str(), "major" | "minor")
        {
            return failure("invalid-local-key-region", Some(region.start_measure));
        }
        prior_key_end = region.end_measure_exclusive;
    }
    if request.cadences.len() > MAX_PHRASES {
        return failure("cadence-limit", None);
    }
    let mut prior_arrival = None;
    for target in &request.cadences {
        if target.arrival_position_index == 0
            || target.arrival_position_index >= positions.len()
            || prior_arrival.is_some_and(|prior| target.arrival_position_index < prior + 2)
        {
            return failure("invalid-cadence-target", None);
        }
        prior_arrival = Some(target.arrival_position_index);
    }
    if request.colors.len() > MAX_PHRASES {
        return failure("color-target-limit", None);
    }
    let mut prior_color = None;
    for target in &request.colors {
        let index = target.position_index;
        if index >= positions.len()
            || prior_color.is_some_and(|prior| index <= prior)
            || (target.kind == ChordColorKind::ChromaticPassing
                && (index == 0
                    || index + 1 >= positions.len()
                    || request.proposal.bass_mode != super::arrange::AccordionBassMode::FreeBass
                    || (positions
                        .get(index + 1)
                        .map(|end| {
                            (positions[index].measure_index..end.measure_index)
                                .map(|measure| score.playback_measure_beats(measure))
                                .sum::<f64>()
                                + end.offset.beats()
                                - positions[index].offset.beats()
                        })
                        .unwrap_or(f64::INFINITY))
                        > 2.0 + 1e-9))
        {
            return failure(
                "invalid-color-target",
                positions.get(index).map(|position| position.measure_index),
            );
        }
        prior_color = Some(index);
    }
    let mut search_request = request.proposal.clone();
    if (!request.cadences.is_empty() || !request.colors.is_empty())
        && (1..=4).contains(&search_request.max_candidates)
    {
        search_request.max_candidates = 4;
    }
    let mut reanchored_source_plan = None;
    let mut omitted_source_entries = Vec::new();
    let proposal = if let Some(mapping) = &request.source_reanchor {
        let Some(source) = &request.proposal.source_plan else {
            return failure("reanchor-requires-source-plan", None);
        };
        let reanchored = match reanchor_source(
            score,
            source,
            &positions,
            mapping,
            &request.locked_chord_entries,
            request.proposal.bass_mode,
        ) {
            Ok(mapped) => mapped,
            Err((code, measure)) => return failure(code, measure),
        };
        let mut proposal_request = search_request.clone();
        proposal_request.source_plan = Some(reanchored.plan.clone());
        let mut generated = suggest_chord_plans_with_locks_and_context(
            score,
            &proposal_request,
            &reanchored.locked_targets,
            &request.local_keys,
            &request.cadences,
            &request.colors,
        );
        let before_budget = generated.candidates.len();
        for candidate in &mut generated.candidates {
            candidate.changed_spans = changed_spans(score, &candidate.plan, Some(source));
        }
        if let Some(limit) = request.proposal.max_changed_spans {
            generated.candidates.retain(|candidate| {
                changed_original_regions(source, &candidate.changed_spans, count)
                    <= usize::from(limit)
            });
            if before_budget > 0 && generated.candidates.is_empty() {
                generated.diagnostics.push(ChordProposalDiagnostic {
                    code: "change-budget-exceeded-after-reanchor".into(),
                    measure_index: None,
                });
            }
        }
        for candidate in &mut generated.candidates {
            candidate
                .reason_codes
                .push("explicit-source-reanchor".into());
        }
        reanchored_source_plan = Some(reanchored.plan);
        omitted_source_entries = reanchored.omitted;
        generated
    } else if let Some(source) = &request.proposal.source_plan {
        let aligned = source.entries.len() == positions.len()
            && source
                .entries
                .iter()
                .zip(&positions)
                .all(|(entry, position)| {
                    entry.position.measure_index == position.measure_index
                        && (entry.position.offset.beats() - position.offset.beats()).abs() <= 1e-9
                });
        if !aligned {
            return failure("source-harmonic-positions-mismatch", None);
        }
        suggest_chord_plans_with_locks_and_context(
            score,
            &search_request,
            &request.locked_chord_entries,
            &request.local_keys,
            &request.cadences,
            &request.colors,
        )
    } else {
        if !request.locked_chord_entries.is_empty() {
            return failure("chord-lock-requires-source-plan", None);
        }
        suggest_chord_plans_at_positions_with_context(
            score,
            &search_request,
            &positions,
            &request.local_keys,
            &request.cadences,
            &request.colors,
        )
    };
    let mut proposal = proposal;
    if !request.cadences.is_empty() {
        let had_candidates = !proposal.candidates.is_empty();
        proposal.candidates.retain(|candidate| {
            matches_cadences(
                score,
                &candidate.plan,
                &positions,
                &request.cadences,
                &request.local_keys,
            )
        });
        if had_candidates && proposal.candidates.is_empty() {
            proposal.diagnostics.push(ChordProposalDiagnostic {
                code: "cadence-incompatible".into(),
                measure_index: None,
            });
        }
        for candidate in &mut proposal.candidates {
            candidate
                .reason_codes
                .push("caller-supplied-cadence".into());
            candidate.resolved.max_candidates = request.proposal.max_candidates;
        }
        if request.colors.is_empty() {
            proposal
                .candidates
                .truncate(usize::from(request.proposal.max_candidates));
        }
    }
    if !request.colors.is_empty() {
        let had_candidates = !proposal.candidates.is_empty();
        proposal.candidates.retain(|candidate| {
            matches_colors(
                score,
                &candidate.plan,
                &positions,
                &request.colors,
                &request.local_keys,
            )
        });
        if had_candidates && proposal.candidates.is_empty() {
            proposal.diagnostics.push(ChordProposalDiagnostic {
                code: "color-incompatible".into(),
                measure_index: None,
            });
        }
        for candidate in &mut proposal.candidates {
            candidate.reason_codes.push("caller-supplied-color".into());
            candidate.resolved.max_candidates = request.proposal.max_candidates;
        }
        proposal
            .candidates
            .truncate(usize::from(request.proposal.max_candidates));
    }
    if !request.local_keys.is_empty() {
        for candidate in &mut proposal.candidates {
            candidate
                .reason_codes
                .push("caller-supplied-local-key".into());
        }
    }
    PhraseHarmonyResult {
        positions,
        diagnostics: proposal.diagnostics.clone(),
        proposals: proposal,
        contract_version: 1,
        reanchored_source_plan,
        omitted_source_entries,
        applied_local_keys: request.local_keys.clone(),
        applied_cadences: request.cadences.clone(),
        applied_colors: request.colors.clone(),
    }
}
