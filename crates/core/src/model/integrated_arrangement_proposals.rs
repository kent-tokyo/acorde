//! Bounded selection across explicit harmony, rhythm and melody techniques.

use serde::{Deserialize, Serialize};

use super::arrange::AccordionBassMode;
use super::chord_plan::chord_plan_to_preset_selection;
use super::chord_proposals::fingerprint;
use super::phrase_harmony::{
    PhraseHarmonyTargetError, PhraseHarmonyTargetRequest, ResolvedPhraseHarmonyTarget,
    replay_phrase_harmony_target, suggest_phrase_harmony_targets,
};
use super::phrase_melody::{PhraseMelodyRequest, ResolvedPhraseMelody, arrange_phrase_melody};
use super::playback::to_playback_events;
use super::rhythm_section::{ResolvedRhythmSection, RhythmSectionRequest, arrange_rhythm_section};
use super::score::Score;
use super::validate::validate;

pub const INTEGRATED_ARRANGEMENT_CONTRACT_VERSION: u16 = 1;
const MAX_VARIANTS: usize = 5;
const MAX_REALIZATIONS: u16 = 125;

#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
#[serde(deny_unknown_fields)]
pub struct IntegratedArrangementRequest {
    pub harmony: PhraseHarmonyTargetRequest,
    pub rhythms: Vec<RhythmSectionRequest>,
    pub melodies: Vec<PhraseMelodyRequest>,
    pub start_measure: usize,
    pub end_measure_exclusive: usize,
    pub max_changed_harmonies: u8,
    pub max_rhythm_attacks: u16,
    pub max_melody_anticipations: u8,
    pub count: u8,
    pub max_realizations: u16,
}

#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
pub struct ResolvedIntegratedArrangement {
    pub contract_version: u16,
    pub engine_version: String,
    pub source_fingerprint: String,
    pub request: IntegratedArrangementRequest,
    pub harmony: ResolvedPhraseHarmonyTarget,
    pub rhythm: ResolvedRhythmSection,
    pub melody: ResolvedPhraseMelody,
    pub rhythm_variant: usize,
    pub melody_variant: usize,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct IntegratedArrangementCandidate {
    pub score: Score,
    pub resolved: ResolvedIntegratedArrangement,
    pub difference_codes: Vec<String>,
    pub changed_harmonies: usize,
    pub rhythm_attacks: usize,
    pub melody_anticipations: usize,
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct IntegratedArrangementRejection {
    pub harmony: usize,
    pub rhythm: usize,
    pub melody: usize,
    pub code: String,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct IntegratedArrangementResult {
    pub candidates: Vec<IntegratedArrangementCandidate>,
    pub realizations: u16,
    pub search_limited: bool,
    pub rejections: Vec<IntegratedArrangementRejection>,
}

#[derive(Debug, thiserror::Error)]
pub enum IntegratedArrangementError {
    #[error("invalid or unbounded integrated arrangement request")]
    InvalidRequest,
    #[error("harmony candidate search failed: {0}")]
    Harmony(#[from] PhraseHarmonyTargetError),
    #[error("only {found} distinct integrated arrangements could be realized")]
    InsufficientDistinct {
        found: usize,
        search_limited: bool,
        rejections: Vec<IntegratedArrangementRejection>,
    },
    #[error("unsupported integrated arrangement contract")]
    UnsupportedContract,
    #[error("integrated arrangement source changed")]
    SourceMismatch,
    #[error("saved integrated arrangement choice changed")]
    SelectionMismatch,
}

fn valid_request(source: &Score, request: &IntegratedArrangementRequest) -> bool {
    let Some(count) = source
        .parts
        .first()
        .and_then(|part| part.staves.first())
        .map(|staff| staff.measures.len())
    else {
        return false;
    };
    (3..=5).contains(&request.count)
        && (1..=MAX_REALIZATIONS).contains(&request.max_realizations)
        && (1..=MAX_VARIANTS).contains(&request.rhythms.len())
        && (1..=MAX_VARIANTS).contains(&request.melodies.len())
        && request.start_measure < request.end_measure_exclusive
        && request.end_measure_exclusive <= count
        && request.harmony.base.start_measure == request.start_measure
        && request.harmony.base.end_measure_exclusive == request.end_measure_exclusive
        && request.harmony.max_changed_harmonies <= request.max_changed_harmonies
        && request.rhythms.iter().all(|rhythm| {
            rhythm.start_measure == request.start_measure
                && rhythm.end_measure_exclusive == request.end_measure_exclusive
                && rhythm.locked_measures == request.harmony.base.locked_measures
                && rhythm.bars.iter().all(|bar| {
                    (request.start_measure..request.end_measure_exclusive).contains(&bar.measure)
                })
        })
        && request.melodies.iter().all(|melody| {
            melody.countermelody.base.melody_part_index < source.parts.len()
                && melody.countermelody.base.melody_part_index
                    == request.melodies[0].countermelody.base.melody_part_index
                && melody.start_measure == request.start_measure
                && melody.end_measure_exclusive == request.end_measure_exclusive
                && melody.locked_measures == request.harmony.base.locked_measures
                && melody.anticipations.len() <= usize::from(request.max_melody_anticipations)
                && ((melody.countermelody_active_measures.is_empty()
                    && request.start_measure == 0
                    && request.end_measure_exclusive == count)
                    || (melody.countermelody_active_measures.len() == count
                        && melody.countermelody_active_measures.iter().enumerate().all(
                            |(measure, active)| {
                                !*active
                                    || (request.start_measure..request.end_measure_exclusive)
                                        .contains(&measure)
                            },
                        )))
        })
}

fn signature(score: &Score) -> Vec<(usize, u64, u64, i32, u8)> {
    to_playback_events(score, &Default::default())
        .into_iter()
        .filter_map(|event| {
            event.source.map(|source| {
                (
                    source.part,
                    (event.time_beats * 480.0).round() as u64,
                    (event.duration_beats * 480.0).round() as u64,
                    event.pitch_midi_cents,
                    event.velocity,
                )
            })
        })
        .collect()
}

fn stays_within_range(
    source: &Score,
    score: &Score,
    request: &IntegratedArrangementRequest,
) -> bool {
    if score.parts.len() < source.parts.len() {
        return false;
    }
    let mut original_parts = score.parts[..source.parts.len()].to_vec();
    let melody_part = request.melodies[0].countermelody.base.melody_part_index;
    let Some(melody_staff) = original_parts
        .get_mut(melody_part)
        .and_then(|part| part.staves.get_mut(0))
    else {
        return false;
    };
    let Some(source_staff) = source
        .parts
        .get(melody_part)
        .and_then(|part| part.staves.first())
    else {
        return false;
    };
    for measure in request.start_measure..request.end_measure_exclusive {
        let Some(original) = source_staff.measures.get(measure) else {
            return false;
        };
        let Some(actual) = melody_staff.measures.get_mut(measure) else {
            return false;
        };
        *actual = original.clone();
    }
    let source_ok = matches!(
        (serde_json::to_value(&original_parts), serde_json::to_value(&source.parts)),
        (Ok(actual), Ok(expected)) if actual == expected
    );
    let outside_ok = to_playback_events(score, &Default::default())
        .iter()
        .all(|event| {
            event.source.as_ref().is_none_or(|address| {
                address.part < source.parts.len()
                    || (request.start_measure..request.end_measure_exclusive)
                        .contains(&address.measure)
            })
        });
    source_ok && outside_ok
}

/// Select three to five distinct playable scores from bounded combinations.
/// The underlying generators retain their own source-bound replay contracts.
pub fn suggest_integrated_arrangements(
    source: &Score,
    request: &IntegratedArrangementRequest,
) -> Result<IntegratedArrangementResult, IntegratedArrangementError> {
    if !validate(source).is_valid() || !valid_request(source, request) {
        return Err(IntegratedArrangementError::InvalidRequest);
    }
    let source_fingerprint =
        fingerprint(source).ok_or(IntegratedArrangementError::InvalidRequest)?;
    let harmonies = suggest_phrase_harmony_targets(source, &request.harmony)?;
    let mut pool = Vec::new();
    let mut signatures = Vec::new();
    let mut rejections = Vec::new();
    let mut realizations = 0u16;
    let mut search_limited = harmonies.search_limited;
    'harmony: for (hi, harmony) in harmonies.candidates.iter().enumerate() {
        let plan = &harmony.resolved.arrangement.request.chords;
        if harmony.changed_chord_entries.len() > usize::from(request.max_changed_harmonies)
            || harmony.changed_chord_entries.iter().any(|&entry| {
                plan.entries.get(entry).is_none_or(|placed| {
                    !(request.start_measure..request.end_measure_exclusive)
                        .contains(&placed.position.measure_index)
                })
            })
        {
            continue;
        }
        let selection = chord_plan_to_preset_selection(plan, AccordionBassMode::FreeBass);
        let Some(chords) = selection.selection.map(|selected| selected.chords) else {
            rejections.push(IntegratedArrangementRejection {
                harmony: hi,
                rhythm: 0,
                melody: 0,
                code: "countermelody-chords-unavailable".into(),
            });
            continue;
        };
        for (ri, rhythm_template) in request.rhythms.iter().enumerate() {
            let mut rhythm_request = rhythm_template.clone();
            rhythm_request.chords = plan.clone();
            let rhythm = match arrange_rhythm_section(&harmony.arrangement.score, &rhythm_request) {
                Ok(value) => value,
                Err(error) => {
                    rejections.push(IntegratedArrangementRejection {
                        harmony: hi,
                        rhythm: ri,
                        melody: 0,
                        code: format!("rhythm:{error}"),
                    });
                    continue;
                }
            };
            let attacks = rhythm
                .attacks
                .iter()
                .filter(|attack| attack.kick.is_some() || attack.bass.is_some())
                .count();
            if attacks > usize::from(request.max_rhythm_attacks) {
                continue;
            }
            for (mi, melody_template) in request.melodies.iter().enumerate() {
                if realizations == request.max_realizations {
                    search_limited = true;
                    break 'harmony;
                }
                realizations += 1;
                let mut melody_request = melody_template.clone();
                melody_request.countermelody.base.chords = chords.clone();
                let melody = match arrange_phrase_melody(&rhythm.score, &melody_request) {
                    Ok(value) => value,
                    Err(error) => {
                        rejections.push(IntegratedArrangementRejection {
                            harmony: hi,
                            rhythm: ri,
                            melody: mi,
                            code: format!("melody:{error}"),
                        });
                        continue;
                    }
                };
                if !stays_within_range(source, &melody.score, request) {
                    rejections.push(IntegratedArrangementRejection {
                        harmony: hi,
                        rhythm: ri,
                        melody: mi,
                        code: "outside-range".into(),
                    });
                    continue;
                }
                let event_signature = signature(&melody.score);
                if signatures.contains(&event_signature) {
                    continue;
                }
                signatures.push(event_signature);
                pool.push(IntegratedArrangementCandidate {
                    score: melody.score,
                    resolved: ResolvedIntegratedArrangement {
                        contract_version: INTEGRATED_ARRANGEMENT_CONTRACT_VERSION,
                        engine_version: env!("CARGO_PKG_VERSION").into(),
                        source_fingerprint: source_fingerprint.clone(),
                        request: request.clone(),
                        harmony: harmony.resolved.clone(),
                        rhythm: rhythm.resolved.clone(),
                        melody: melody.resolved,
                        rhythm_variant: ri,
                        melody_variant: mi,
                    },
                    difference_codes: Vec::new(),
                    changed_harmonies: harmony.changed_chord_entries.len(),
                    rhythm_attacks: attacks,
                    melody_anticipations: melody_request.anticipations.len(),
                });
            }
        }
    }
    if pool.len() < usize::from(request.count) {
        return Err(IntegratedArrangementError::InsufficientDistinct {
            found: pool.len(),
            search_limited,
            rejections,
        });
    }
    let mut selected = Vec::new();
    selected.push(pool.remove(0));
    while selected.len() < usize::from(request.count) {
        // Keep a pure harmony comparison when the bounded search found one.
        // This lets the next proposal show the independent rhythm/melody axes.
        if selected.len() == 1 {
            let reference = &selected[0].resolved;
            if let Some(index) = pool.iter().position(|candidate| {
                candidate.resolved.harmony.arrangement.request.chords
                    != reference.harmony.arrangement.request.chords
                    && candidate.resolved.rhythm_variant == reference.rhythm_variant
                    && candidate.resolved.melody_variant == reference.melody_variant
            }) {
                selected.push(pool.remove(index));
                continue;
            }
        }
        let index = pool
            .iter()
            .enumerate()
            .max_by_key(|(index, candidate)| {
                let harmony_new = !selected
                    .iter()
                    .any(|prior: &IntegratedArrangementCandidate| {
                        prior.resolved.harmony.arrangement.request.chords
                            == candidate.resolved.harmony.arrangement.request.chords
                    });
                let rhythm_new = !selected.iter().any(|prior| {
                    prior.resolved.rhythm_variant == candidate.resolved.rhythm_variant
                });
                let melody_new = !selected.iter().any(|prior| {
                    prior.resolved.melody_variant == candidate.resolved.melody_variant
                });
                (
                    u8::from(harmony_new) + u8::from(rhythm_new) + u8::from(melody_new),
                    std::cmp::Reverse(*index),
                )
            })
            .map(|(index, _)| index)
            .ok_or(IntegratedArrangementError::SelectionMismatch)?;
        selected.push(pool.remove(index));
    }
    let reference = selected[0].resolved.clone();
    for candidate in &mut selected {
        let mut codes = Vec::new();
        if candidate.resolved.harmony.arrangement.request.chords
            != reference.harmony.arrangement.request.chords
        {
            codes.push("harmony-changed".into());
        }
        if candidate.resolved.rhythm_variant != reference.rhythm_variant {
            codes.push("rhythm-changed".into());
        }
        if candidate.resolved.melody_variant != reference.melody_variant {
            codes.push("melody-changed".into());
        }
        if codes.is_empty() {
            codes.push("reference".into());
        }
        candidate.difference_codes = codes;
    }
    Ok(IntegratedArrangementResult {
        candidates: selected,
        realizations,
        search_limited,
        rejections,
    })
}

/// Re-run bounded selection and require the saved choice to remain selected.
pub fn replay_integrated_arrangement(
    source: &Score,
    resolved: &ResolvedIntegratedArrangement,
) -> Result<Score, IntegratedArrangementError> {
    if resolved.contract_version != INTEGRATED_ARRANGEMENT_CONTRACT_VERSION
        || resolved.engine_version != env!("CARGO_PKG_VERSION")
    {
        return Err(IntegratedArrangementError::UnsupportedContract);
    }
    if fingerprint(source).as_deref() != Some(resolved.source_fingerprint.as_str()) {
        return Err(IntegratedArrangementError::SourceMismatch);
    }
    let selected = suggest_integrated_arrangements(source, &resolved.request)?;
    let candidate = selected
        .candidates
        .into_iter()
        .find(|candidate| candidate.resolved == *resolved)
        .ok_or(IntegratedArrangementError::SelectionMismatch)?;
    let replayed = replay_phrase_harmony_target(source, &resolved.harmony)?;
    if replayed.resolved != resolved.harmony {
        return Err(IntegratedArrangementError::SelectionMismatch);
    }
    Ok(candidate.score)
}
