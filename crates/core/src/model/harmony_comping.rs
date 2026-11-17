//! Exact written attacks, retaining the original harmonic reference.

use super::harmony_arrangement::{
    HarmonyArrangementError as Error, HarmonyArrangementRequest, Time,
};
use super::repeat::measure_sequence;
use super::score::Score;
use serde::{Deserialize, Serialize};

#[derive(Debug, Clone, Copy, Default, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "kebab-case")]
pub enum HarmonyCompingPattern {
    #[default]
    Held,
    OffbeatPulse,
}

#[derive(Debug, Clone, Copy, Default, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "kebab-case")]
pub enum HarmonyCompingAccent {
    #[default]
    None,
    Anticipations,
    AllAttacks,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "kebab-case")]
pub enum HarmonyAnticipationLead {
    Eighth,
    Sixteenth,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "kebab-case")]
pub enum HarmonyAnticipationJoin {
    Tie,
    Reattack,
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
#[serde(deny_unknown_fields)]
pub struct HarmonyAnticipation {
    pub chord_entry: usize,
    pub lead: HarmonyAnticipationLead,
    pub join: HarmonyAnticipationJoin,
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
#[serde(default, deny_unknown_fields)]
pub struct HarmonyCompingOptions {
    pub pattern: HarmonyCompingPattern,
    pub accent: HarmonyCompingAccent,
    pub anticipations: Vec<HarmonyAnticipation>,
    pub max_attacks_per_measure: u8,
}
impl Default for HarmonyCompingOptions {
    fn default() -> Self {
        Self {
            pattern: HarmonyCompingPattern::Held,
            accent: HarmonyCompingAccent::None,
            anticipations: Vec::new(),
            max_attacks_per_measure: 64,
        }
    }
}

#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "kebab-case")]
pub enum HarmonyCompingFailure {
    InvalidEntry,
    DuplicateEntry,
    BeforeSource,
    OutsideRange,
    LockedMeasure,
    OverlappingChanges,
    AmbiguousRepeat,
    AttackLimit,
}

#[derive(Clone)]
struct Attack {
    start: Time,
    end: Time,
    entry: usize,
}

pub(super) struct ScheduledSpan {
    pub measure: usize,
    pub start: Time,
    pub end: Time,
    pub entry: usize,
    pub attack: usize,
    pub accent: bool,
    pub tie_from_previous: bool,
    pub tie_to_next: bool,
}

fn fail(entry: usize, reason: HarmonyCompingFailure) -> Error {
    Error::Comping { entry, reason }
}

fn measure_at(starts: &[Time], position: Time) -> usize {
    starts
        .partition_point(|start| *start <= position)
        .saturating_sub(1)
}

/// The shared written anticipation must have the same performed destination.
/// A tied target must also have the same incoming predecessor on every visit.
fn safe_boundary(score: &Score, from: usize, to: usize, tie: bool) -> bool {
    if from == to {
        return true;
    }
    if from.checked_add(1) != Some(to) {
        return false;
    }
    let sequence = measure_sequence(score);
    let mut observed = false;
    for (index, &measure) in sequence.iter().enumerate() {
        if measure == from {
            observed = true;
            if sequence.get(index + 1) != Some(&to) {
                return false;
            }
        }
        if tie && measure == to && index.checked_sub(1).and_then(|i| sequence.get(i)) != Some(&from)
        {
            return false;
        }
    }
    observed
}

pub(super) fn schedule(
    score: &Score,
    request: &HarmonyArrangementRequest,
    starts: &[Time],
    positions: &[Time],
) -> Result<Vec<ScheduledSpan>, Error> {
    let options = &request.comping;
    if options.anticipations.len() > 64 || !(1..=64).contains(&options.max_attacks_per_measure) {
        return Err(Error::InvalidRequest);
    }
    let end_of_score = *starts.last().ok_or(Error::InvalidScore)?;
    let mut attacks = Vec::new();
    let half = Time::new(1, 2)?;
    let one = Time::new(1, 1)?;
    for measure in request.start_measure..request.end_measure_exclusive {
        if request.locked_measures.contains(&measure) {
            continue;
        }
        for (entry, &position) in positions.iter().enumerate() {
            let start = position.max(starts[measure]);
            let end = positions
                .get(entry + 1)
                .copied()
                .unwrap_or(end_of_score)
                .min(starts[measure + 1]);
            if start >= end {
                continue;
            }
            match options.pattern {
                HarmonyCompingPattern::Held => attacks.push(Attack { start, end, entry }),
                HarmonyCompingPattern::OffbeatPulse => {
                    let mut pulse = starts[measure].add(half)?;
                    while pulse < end {
                        let from = pulse.max(start);
                        let to = pulse.add(half)?.min(end);
                        if from < to {
                            attacks.push(Attack {
                                start: from,
                                end: to,
                                entry,
                            });
                        }
                        if attacks.len() > 2048 {
                            return Err(fail(entry, HarmonyCompingFailure::AttackLimit));
                        }
                        pulse = pulse.add(one)?;
                    }
                }
            }
        }
    }
    let mut anticipations = options.anticipations.clone();
    anticipations.sort_by_key(|a| a.chord_entry);
    for pair in anticipations.windows(2) {
        if pair[0].chord_entry == pair[1].chord_entry {
            return Err(fail(
                pair[0].chord_entry,
                HarmonyCompingFailure::DuplicateEntry,
            ));
        }
    }
    for anticipation in anticipations {
        let entry = anticipation.chord_entry;
        let &nominal = positions
            .get(entry)
            .ok_or_else(|| fail(entry, HarmonyCompingFailure::InvalidEntry))?;
        let lead = match anticipation.lead {
            HarmonyAnticipationLead::Eighth => half,
            HarmonyAnticipationLead::Sixteenth => Time::new(1, 4)?,
        };
        let start = nominal
            .sub(lead)
            .map_err(|_| fail(entry, HarmonyCompingFailure::BeforeSource))?;
        if entry == 0 || start < positions[entry - 1] {
            return Err(fail(entry, HarmonyCompingFailure::OverlappingChanges));
        }
        let from = measure_at(starts, start);
        let to = measure_at(starts, nominal);
        if from < request.start_measure || to >= request.end_measure_exclusive {
            return Err(fail(entry, HarmonyCompingFailure::OutsideRange));
        }
        if (from..=to).any(|m| request.locked_measures.contains(&m)) {
            return Err(fail(entry, HarmonyCompingFailure::LockedMeasure));
        }
        if !safe_boundary(
            score,
            from,
            to,
            anticipation.join == HarmonyAnticipationJoin::Tie,
        ) {
            return Err(fail(entry, HarmonyCompingFailure::AmbiguousRepeat));
        }
        let harmonic_end = positions
            .get(entry + 1)
            .copied()
            .unwrap_or(end_of_score)
            .min(starts[to + 1]);
        // Include the target's first held interval, or sustain until its next
        // offbeat. A reattack explicitly articulates the nominal chord boundary.
        let end = attacks
            .iter()
            .filter(|a| a.entry == entry && a.start >= nominal)
            .min_by_key(|a| a.start)
            .map_or(nominal.add(lead)?.min(harmonic_end), |a| {
                if a.start == nominal {
                    a.end
                } else {
                    a.start.min(harmonic_end)
                }
            });
        if end <= nominal {
            return Err(fail(entry, HarmonyCompingFailure::OverlappingChanges));
        }
        let mut retained = Vec::new();
        for attack in attacks {
            if attack.end <= start || attack.start >= end {
                retained.push(attack);
                continue;
            }
            if attack.start < start {
                retained.push(Attack {
                    end: start,
                    ..attack.clone()
                });
            }
            if attack.end > end {
                retained.push(Attack {
                    start: end,
                    ..attack
                });
            }
        }
        if anticipation.join == HarmonyAnticipationJoin::Reattack {
            retained.push(Attack {
                start,
                end: nominal,
                entry,
            });
            retained.push(Attack {
                start: nominal,
                end,
                entry,
            });
        } else {
            retained.push(Attack { start, end, entry });
        }
        attacks = retained;
    }
    attacks.sort_by_key(|a| (a.start, a.end, a.entry));
    if attacks.len() > 2048 {
        return Err(Error::InvalidRequest);
    }
    for pair in attacks.windows(2) {
        if pair[0].end > pair[1].start {
            return Err(fail(
                pair[1].entry,
                HarmonyCompingFailure::OverlappingChanges,
            ));
        }
    }
    let mut counts = vec![0u8; starts.len().saturating_sub(1)];
    let mut spans = Vec::new();
    for (attack, event) in attacks.iter().enumerate() {
        let first = measure_at(starts, event.start);
        counts[first] = counts[first]
            .checked_add(1)
            .ok_or_else(|| fail(event.entry, HarmonyCompingFailure::AttackLimit))?;
        if counts[first] > options.max_attacks_per_measure {
            return Err(fail(event.entry, HarmonyCompingFailure::AttackLimit));
        }
        for measure in first..request.end_measure_exclusive {
            let start = event.start.max(starts[measure]);
            let end = event.end.min(starts[measure + 1]);
            if start >= end {
                break;
            }
            let local_start = start.sub(starts[measure])?;
            let local_end = end.sub(starts[measure])?;
            local_start.ticks(measure)?;
            local_end.ticks(measure)?;
            spans.push(ScheduledSpan {
                measure,
                start: local_start,
                end: local_end,
                entry: event.entry,
                attack,
                accent: start == event.start
                    && match options.accent {
                        HarmonyCompingAccent::None => false,
                        HarmonyCompingAccent::Anticipations => event.start < positions[event.entry],
                        HarmonyCompingAccent::AllAttacks => true,
                    },
                tie_from_previous: start > event.start,
                tie_to_next: end < event.end,
            });
            if end == event.end {
                break;
            }
        }
    }
    Ok(spans)
}
