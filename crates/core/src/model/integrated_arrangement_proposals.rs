//! Bounded selection across explicit harmony, rhythm and melody techniques.

use std::collections::{BTreeMap, BTreeSet};

use serde::{Deserialize, Serialize};

use super::arrange::AccordionBassMode;
use super::chord_plan::{ChordPlan, chord_plan_to_preset_selection};
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
    /// Requested technique choices relative to the first proposal.
    pub difference_codes: Vec<String>,
    /// Actual changed sounding roles and six observed arrangement axes.
    #[serde(default)]
    pub observed_difference_codes: Vec<String>,
    #[serde(default)]
    pub axis_distance_from_first: IntegratedArrangementAxisDistance,
    #[serde(default)]
    pub nearest_selected_distance: u32,
    pub changed_harmonies: usize,
    pub rhythm_attacks: usize,
    pub melody_anticipations: usize,
}

/// Jaccard distances in 0..=1000 on generated harmony pitch classes, attack
/// shape, exact bass events, four-semitone register bands, sounding roles by
/// measure, and written section boundaries. These are selection summaries,
/// not listening scores. Form remains zero when local edits preserve sections.
#[derive(Debug, Clone, Copy, Default, PartialEq, Eq, Serialize, Deserialize)]
pub struct IntegratedArrangementAxisDistance {
    pub harmony: u16,
    pub rhythm: u16,
    pub bass: u16,
    pub register: u16,
    pub roles: u16,
    pub form: u16,
}

impl IntegratedArrangementAxisDistance {
    fn total(self) -> u32 {
        u32::from(self.harmony)
            + u32::from(self.rhythm)
            + u32::from(self.bass)
            + u32::from(self.register)
            + u32::from(self.roles)
            + u32::from(self.form)
    }
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

#[derive(Debug, Clone, Copy, PartialEq, Eq, PartialOrd, Ord)]
enum ObservedRole {
    SourceMelody,
    Harmony,
    Bass,
    Kick,
    Ghost,
    Countermelody,
}

type ObservedEvent = (usize, u64, u64, i32, u8);

struct ObservedParts {
    source_melody: usize,
    harmony: usize,
    bass: usize,
    kick: usize,
    ghost: Option<usize>,
    countermelody: Option<usize>,
}

#[derive(Clone, Default)]
struct ObservedAxes {
    events: BTreeMap<ObservedRole, Vec<ObservedEvent>>,
    harmony: BTreeSet<(usize, i32)>,
    harmony_plan: BTreeSet<(usize, u64, String)>,
    rhythm: BTreeSet<(ObservedRole, u64, u64)>,
    bass: BTreeSet<(u64, u64, i32, u8)>,
    register: BTreeSet<(ObservedRole, i32)>,
    roles: BTreeSet<(usize, ObservedRole)>,
    form: BTreeSet<(usize, bool)>,
}

fn ticks(beats: f64) -> u64 {
    (beats * 480.0).round().max(0.0) as u64
}

fn jaccard<T: Ord>(left: &BTreeSet<T>, right: &BTreeSet<T>) -> u16 {
    let union = left.union(right).count();
    if union == 0 {
        return 0;
    }
    let common = left.intersection(right).count();
    (((union - common) * 1000 + union / 2) / union) as u16
}

impl ObservedAxes {
    fn collect(score: &Score, plan: &ChordPlan, parts: ObservedParts) -> Self {
        let mut axes = Self::default();
        let mut role_by_part = BTreeMap::from([
            (parts.source_melody, ObservedRole::SourceMelody),
            (parts.harmony, ObservedRole::Harmony),
            (parts.bass, ObservedRole::Bass),
            (parts.kick, ObservedRole::Kick),
        ]);
        if let Some(part) = parts.ghost {
            role_by_part.insert(part, ObservedRole::Ghost);
        }
        if let Some(part) = parts.countermelody {
            role_by_part.insert(part, ObservedRole::Countermelody);
        }
        for event in to_playback_events(score, &Default::default()) {
            let (Some(&role), Some(source)) = (role_by_part.get(&event.part_index), event.source)
            else {
                continue;
            };
            let onset = ticks(event.time_beats);
            let duration = ticks(event.duration_beats);
            let pitch = event.pitch_midi_cents;
            axes.events.entry(role).or_default().push((
                source.measure,
                onset,
                duration,
                pitch,
                event.velocity,
            ));
            axes.rhythm.insert((role, onset, duration));
            axes.register.insert((role, pitch.div_euclid(400)));
            axes.roles.insert((source.measure, role));
            if role == ObservedRole::Harmony {
                axes.harmony
                    .insert((source.measure, pitch.rem_euclid(1200)));
            } else if role == ObservedRole::Bass {
                axes.bass.insert((onset, duration, pitch, event.velocity));
            }
        }
        for events in axes.events.values_mut() {
            events.sort_unstable();
        }
        axes.harmony_plan = plan
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
        if let Some(staff) = score
            .parts
            .get(parts.source_melody)
            .and_then(|part| part.staves.first())
        {
            axes.form = staff
                .measures
                .iter()
                .enumerate()
                .map(|(measure, bar)| (measure, bar.section_break))
                .collect();
        }
        axes
    }

    fn distance(&self, other: &Self) -> IntegratedArrangementAxisDistance {
        IntegratedArrangementAxisDistance {
            harmony: jaccard(&self.harmony, &other.harmony)
                .max(jaccard(&self.harmony_plan, &other.harmony_plan)),
            rhythm: jaccard(&self.rhythm, &other.rhythm),
            bass: jaccard(&self.bass, &other.bass),
            register: jaccard(&self.register, &other.register),
            roles: jaccard(&self.roles, &other.roles),
            form: jaccard(&self.form, &other.form),
        }
    }

    fn difference_codes(&self, other: &Self) -> Vec<String> {
        let mut codes = Vec::new();
        for (role, code) in [
            (ObservedRole::SourceMelody, "source-melody-events-changed"),
            (ObservedRole::Harmony, "harmony-events-changed"),
            (ObservedRole::Bass, "bass-events-changed"),
            (ObservedRole::Kick, "kick-events-changed"),
            (ObservedRole::Ghost, "ghost-events-changed"),
            (ObservedRole::Countermelody, "countermelody-events-changed"),
        ] {
            if self.events.get(&role).map(Vec::as_slice).unwrap_or(&[])
                != other.events.get(&role).map(Vec::as_slice).unwrap_or(&[])
            {
                codes.push(code.into());
            }
        }
        let distance = self.distance(other);
        for (value, code) in [
            (distance.harmony, "harmony-pitch-classes-changed"),
            (distance.rhythm, "generated-rhythm-changed"),
            (distance.register, "generated-register-changed"),
            (distance.roles, "role-activity-changed"),
            (distance.form, "section-form-changed"),
        ] {
            if value > 0 {
                codes.push(code.into());
            }
        }
        codes
    }
}

struct PoolEntry {
    candidate: IntegratedArrangementCandidate,
    observed: ObservedAxes,
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
                let observed = ObservedAxes::collect(
                    &melody.score,
                    plan,
                    ObservedParts {
                        source_melody: request.melodies[0].countermelody.base.melody_part_index,
                        harmony: harmony.arrangement.added_part,
                        bass: rhythm.bass_part,
                        kick: rhythm.kick_part,
                        ghost: rhythm.ghost_part,
                        countermelody: melody
                            .countermelody
                            .generated_notes
                            .first()
                            .map(|note| note.part),
                    },
                );
                pool.push(PoolEntry {
                    candidate: IntegratedArrangementCandidate {
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
                        observed_difference_codes: Vec::new(),
                        axis_distance_from_first: IntegratedArrangementAxisDistance::default(),
                        nearest_selected_distance: 0,
                        changed_harmonies: harmony.changed_chord_entries.len(),
                        rhythm_attacks: attacks,
                        melody_anticipations: melody_request.anticipations.len(),
                    },
                    observed,
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
            let reference = &selected[0].candidate.resolved;
            if let Some(index) = pool.iter().position(|entry| {
                entry.candidate.resolved.harmony.arrangement.request.chords
                    != reference.harmony.arrangement.request.chords
                    && entry.candidate.resolved.rhythm_variant == reference.rhythm_variant
                    && entry.candidate.resolved.melody_variant == reference.melody_variant
            }) {
                selected.push(pool.remove(index));
                continue;
            }
        }
        let index = pool
            .iter()
            .enumerate()
            .max_by_key(|(index, entry)| {
                let harmony_new = !selected.iter().any(|prior: &PoolEntry| {
                    prior.candidate.resolved.harmony.arrangement.request.chords
                        == entry.candidate.resolved.harmony.arrangement.request.chords
                });
                let rhythm_new = !selected.iter().any(|prior| {
                    prior.candidate.resolved.rhythm_variant
                        == entry.candidate.resolved.rhythm_variant
                });
                let melody_new = !selected.iter().any(|prior| {
                    prior.candidate.resolved.melody_variant
                        == entry.candidate.resolved.melody_variant
                });
                let nearest = selected
                    .iter()
                    .map(|prior| entry.observed.distance(&prior.observed).total())
                    .min()
                    .unwrap_or(0);
                (
                    u8::from(harmony_new) + u8::from(rhythm_new) + u8::from(melody_new),
                    nearest,
                    std::cmp::Reverse(*index),
                )
            })
            .map(|(index, _)| index)
            .ok_or(IntegratedArrangementError::SelectionMismatch)?;
        selected.push(pool.remove(index));
    }
    let reference = selected[0].candidate.resolved.clone();
    let reference_observed = selected[0].observed.clone();
    for index in 0..selected.len() {
        let nearest = selected[..index]
            .iter()
            .map(|prior| selected[index].observed.distance(&prior.observed).total())
            .min()
            .unwrap_or(0);
        let observed_codes = selected[index]
            .observed
            .difference_codes(&reference_observed);
        let distance = selected[index].observed.distance(&reference_observed);
        let candidate = &mut selected[index].candidate;
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
        candidate.observed_difference_codes = observed_codes;
        candidate.axis_distance_from_first = distance;
        candidate.nearest_selected_distance = nearest;
    }
    Ok(IntegratedArrangementResult {
        candidates: selected.into_iter().map(|entry| entry.candidate).collect(),
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
