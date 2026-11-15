//! Phrase-authored harmonic rhythm before bounded chord search.

use serde::{Deserialize, Serialize};

use super::chord_plan::{BeatFraction, ChordPlanPosition};
use super::chord_proposals::{
    ChordProposalDiagnostic, ChordProposalRequest, ChordProposalResult,
    suggest_chord_plans_at_positions, suggest_chord_plans_with_locks,
};
use super::chordal_pad::default_meter_groups;
use super::score::Score;

const MAX_PHRASES: usize = 64;
const MAX_POSITIONS: usize = 512;

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
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct PhraseHarmonyResult {
    pub positions: Vec<ChordPlanPosition>,
    pub proposals: ChordProposalResult,
    pub diagnostics: Vec<ChordProposalDiagnostic>,
    pub contract_version: u16,
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

/// Choose chord-change locations from a complete written phrase schedule,
/// then run the existing deterministic progression search. The caller's
/// supplied harmony and locks are accepted only at identical boundaries.
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
    let proposal = if let Some(source) = &request.proposal.source_plan {
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
        suggest_chord_plans_with_locks(score, &request.proposal, &request.locked_chord_entries)
    } else {
        if !request.locked_chord_entries.is_empty() {
            return failure("chord-lock-requires-source-plan", None);
        }
        suggest_chord_plans_at_positions(score, &request.proposal, &positions)
    };
    PhraseHarmonyResult {
        positions,
        diagnostics: proposal.diagnostics.clone(),
        proposals: proposal,
        contract_version: 1,
    }
}
