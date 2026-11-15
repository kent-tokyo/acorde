//! Bounded seeded selection over explicit written-measure sections.

use std::collections::BTreeMap;

use serde::{Deserialize, Serialize};

use super::arrange::{
    AccordionBassMode, AccordionMelodicVoice, AccordionPattern, PositionedAccordionChord,
};
use super::arrange_presets::input_fingerprint;
use super::arrangement_plan::{
    ACCORDION_PLAN_MAX_CHORDS, ACCORDION_PLAN_MAX_MEASURES, ACCORDION_PLAN_MAX_SOURCE_NOTES,
    AccordionPlanDiagnostic, AccordionPlanOptions, AccordionPlanResult, AccordionPlanSection,
    arrange_accordion_plan, failure,
};
use super::score::Score;

pub const ACCORDION_PLAN_SHUFFLE_CONTRACT_VERSION: u16 = 1;
pub const ACCORDION_PLAN_SHUFFLE_MAX_SECTIONS: usize = 8;
pub const ACCORDION_PLAN_SHUFFLE_MAX_CHOICES: usize = 8;
pub const ACCORDION_PLAN_SHUFFLE_MAX_POOL: usize = 65_536;
pub const ACCORDION_PLAN_SHUFFLE_MAX_ATTEMPTS: usize = 64;

#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
#[serde(deny_unknown_fields)]
pub struct AccordionPlanSectionChoices {
    /// Zero-based written measure range [start_measure, end_measure).
    pub start_measure: usize,
    pub end_measure: usize,
    /// Include null to permit no accompaniment; duplicates do not add weight.
    pub allowed_accompaniments: Vec<Option<AccordionPattern>>,
    pub allowed_melodic_voices: Vec<AccordionMelodicVoice>,
}

#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
#[serde(deny_unknown_fields)]
pub struct AccordionPlanShuffleRequest {
    pub chords: Vec<PositionedAccordionChord>,
    pub bass_mode: AccordionBassMode,
    pub sections: Vec<AccordionPlanSectionChoices>,
    /// Unsigned 64-bit decimal text; JSON numbers lose precision in JavaScript.
    pub seed: String,
    /// Caller-selected generation budget, 1..=64.
    pub max_attempts: usize,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct AccordionPlanShuffleResult {
    /// Plan fields retain the #433 JSON contract, including resolved_config and provenance.
    #[serde(flatten)]
    pub plan: AccordionPlanResult,
    pub selection_contract_version: u16,
    pub seed: String,
    pub pool_size: usize,
    pub attempted: usize,
    pub selected_pool_index: Option<usize>,
    /// Counts rejection codes seen before the selected candidate or budget boundary.
    pub rejected: BTreeMap<String, usize>,
}

fn diagnostic(
    code: &str,
    message: impl Into<String>,
    source: Option<&AccordionPlanDiagnostic>,
) -> AccordionPlanDiagnostic {
    AccordionPlanDiagnostic {
        code: code.into(),
        message: message.into(),
        section_index: source.and_then(|d| d.section_index),
        measure_index: source.and_then(|d| d.measure_index),
        source: source.and_then(|d| d.source.clone()),
    }
}

fn rejected(
    request: &AccordionPlanShuffleRequest,
    code: &str,
    message: impl Into<String>,
    source: Option<&AccordionPlanDiagnostic>,
    pool_size: usize,
    attempted: usize,
    counts: BTreeMap<String, usize>,
) -> AccordionPlanShuffleResult {
    AccordionPlanShuffleResult {
        plan: failure(vec![diagnostic(code, message, source)]),
        selection_contract_version: ACCORDION_PLAN_SHUFFLE_CONTRACT_VERSION,
        seed: request.seed.clone(),
        pool_size,
        attempted,
        selected_pool_index: None,
        rejected: counts,
    }
}

fn splitmix64(mut value: u64) -> u64 {
    value = value.wrapping_add(0x9e37_79b9_7f4a_7c15);
    value = (value ^ (value >> 30)).wrapping_mul(0xbf58_476d_1ce4_e5b9);
    value = (value ^ (value >> 27)).wrapping_mul(0x94d0_49bb_1331_11eb);
    value ^ (value >> 31)
}

fn gcd(mut a: usize, mut b: usize) -> usize {
    while b != 0 {
        (a, b) = (b, a % b);
    }
    a
}

fn canonical_choices<T: Copy + PartialEq + std::fmt::Debug>(choices: &[T]) -> Vec<T> {
    let mut values = choices.to_vec();
    values.sort_by_key(|value| format!("{value:?}"));
    values.dedup();
    values
}

fn global_failure(code: &str) -> bool {
    matches!(
        code,
        "unsupported-source"
            | "invalid-source"
            | "invalid-input"
            | "invalid-plan"
            | "invalid-range"
            | "overlapping-range"
            | "uncovered-measures"
            | "limit-exceeded"
            | "missing-chords"
            | "chord-position"
            | "unsupported-chord"
    )
}

/// Select the first realizable plan in a seeded permutation of a bounded pool.
/// A complete failed traversal returns `no-candidate`; an unfinished one returns
/// `search-limit`. Unsupported inputs never trigger a fallback pattern or bass mode.
pub fn shuffle_accordion_plan(
    score: &Score,
    request: AccordionPlanShuffleRequest,
) -> AccordionPlanShuffleResult {
    let Some(parsed_seed) = request.seed.parse::<u64>().ok() else {
        return rejected(
            &request,
            "unsupported",
            "seed must be unsigned 64-bit decimal text",
            None,
            0,
            0,
            BTreeMap::new(),
        );
    };
    if request.max_attempts == 0
        || request.max_attempts > ACCORDION_PLAN_SHUFFLE_MAX_ATTEMPTS
        || request.sections.is_empty()
        || request.sections.len() > ACCORDION_PLAN_SHUFFLE_MAX_SECTIONS
        || request.sections.iter().any(|section| {
            section.allowed_accompaniments.is_empty()
                || section.allowed_accompaniments.len() > ACCORDION_PLAN_SHUFFLE_MAX_CHOICES
                || section.allowed_melodic_voices.is_empty()
                || section.allowed_melodic_voices.len() > ACCORDION_PLAN_SHUFFLE_MAX_CHOICES
        })
    {
        return rejected(
            &request,
            "unsupported",
            "section, choice or attempt budget is outside the supported bounds",
            None,
            0,
            0,
            BTreeMap::new(),
        );
    }
    let choices: Vec<_> = request
        .sections
        .iter()
        .map(|section| {
            (
                canonical_choices(&section.allowed_accompaniments),
                canonical_choices(&section.allowed_melodic_voices),
            )
        })
        .collect();
    let mut pool_size = 1usize;
    for (accompaniments, voices) in &choices {
        let Some(size) = pool_size
            .checked_mul(accompaniments.len())
            .and_then(|size| size.checked_mul(voices.len()))
        else {
            return rejected(
                &request,
                "unsupported",
                "candidate pool exceeds the supported bound",
                None,
                0,
                0,
                BTreeMap::new(),
            );
        };
        if size > ACCORDION_PLAN_SHUFFLE_MAX_POOL {
            return rejected(
                &request,
                "unsupported",
                "candidate pool exceeds the supported bound",
                None,
                size,
                0,
                BTreeMap::new(),
            );
        }
        pool_size = size;
    }
    // Reject oversized sources before the fingerprint clones and serializes the score.
    if score.parts.len() != 1 || score.parts[0].staves.len() != 1 {
        return rejected(
            &request,
            "unsupported",
            "expected one melody part and one staff",
            None,
            pool_size,
            0,
            BTreeMap::new(),
        );
    }
    let measures = &score.parts[0].staves[0].measures;
    let source_notes = measures
        .iter()
        .flat_map(|m| &m.voices)
        .fold(0usize, |count, voice| count.saturating_add(voice.len()));
    if measures.len() > ACCORDION_PLAN_MAX_MEASURES
        || request.chords.len() > ACCORDION_PLAN_MAX_CHORDS
        || source_notes > ACCORDION_PLAN_MAX_SOURCE_NOTES
    {
        return rejected(
            &request,
            "unsupported",
            "plan exceeds the measure, chord or source-note budget",
            None,
            pool_size,
            0,
            BTreeMap::new(),
        );
    }
    if measures.is_empty() {
        return rejected(
            &request,
            "unsupported",
            "source has no measures",
            None,
            pool_size,
            0,
            BTreeMap::new(),
        );
    }
    let mut end = 0;
    for section in &request.sections {
        if section.start_measure != end
            || section.start_measure >= section.end_measure
            || section.end_measure > measures.len()
        {
            return rejected(
                &request,
                "unsupported",
                "sections must be ordered, adjacent and cover written measures exactly once",
                None,
                pool_size,
                0,
                BTreeMap::new(),
            );
        }
        end = section.end_measure;
    }
    if end != measures.len() {
        return rejected(
            &request,
            "unsupported",
            "sections must cover every written measure",
            None,
            pool_size,
            0,
            BTreeMap::new(),
        );
    }
    let Some(fingerprint) = input_fingerprint(score).ok() else {
        return rejected(
            &request,
            "unsupported",
            "source score cannot be fingerprinted",
            None,
            pool_size,
            0,
            BTreeMap::new(),
        );
    };
    let fingerprint_seed =
        u64::from_str_radix(fingerprint.strip_prefix("v2:").unwrap_or(&fingerprint), 16)
            .unwrap_or_default();
    let start = (splitmix64(parsed_seed ^ fingerprint_seed) % pool_size as u64) as usize;
    let mut stride =
        (splitmix64(parsed_seed.wrapping_add(1) ^ fingerprint_seed) % pool_size as u64) as usize;
    while gcd(stride, pool_size) != 1 {
        stride = (stride + 1) % pool_size;
    }
    let mut rejected_counts = BTreeMap::new();
    let limit = request.max_attempts.min(pool_size);
    for attempt in 0..limit {
        let selected = (start + attempt * stride) % pool_size;
        let mut remaining = selected;
        let mut sections = Vec::with_capacity(choices.len());
        for (section, (accompaniments, voices)) in request.sections.iter().zip(&choices).rev() {
            let melodic_voice = voices[remaining % voices.len()];
            remaining /= voices.len();
            let accompaniment = accompaniments[remaining % accompaniments.len()];
            remaining /= accompaniments.len();
            sections.push(AccordionPlanSection {
                start_measure: section.start_measure,
                end_measure: section.end_measure,
                accompaniment,
                melodic_voice,
            });
        }
        sections.reverse();
        let plan = arrange_accordion_plan(
            score,
            AccordionPlanOptions {
                chords: request.chords.clone(),
                bass_mode: request.bass_mode,
                sections,
            },
        );
        if plan.score.is_some() {
            return AccordionPlanShuffleResult {
                plan,
                selection_contract_version: ACCORDION_PLAN_SHUFFLE_CONTRACT_VERSION,
                seed: request.seed,
                pool_size,
                attempted: attempt + 1,
                selected_pool_index: Some(selected),
                rejected: rejected_counts,
            };
        }
        if let Some(reason) = plan.diagnostics.first() {
            if global_failure(&reason.code) {
                return rejected(
                    &request,
                    "unsupported",
                    format!("{}: {}", reason.code, reason.message),
                    Some(reason),
                    pool_size,
                    attempt + 1,
                    rejected_counts,
                );
            }
            *rejected_counts.entry(reason.code.clone()).or_insert(0) += 1;
        }
    }
    let code = if limit < pool_size {
        "search-limit"
    } else {
        "no-candidate"
    };
    rejected(
        &request,
        code,
        if limit < pool_size {
            "attempt budget ended before all allowed combinations were checked"
        } else {
            "all allowed combinations were incompatible with the source"
        },
        None,
        pool_size,
        limit,
        rejected_counts,
    )
}
