//! Bounded search for physically distinct buttons with the same sounding notes.
use std::collections::{BTreeSet, HashMap};

use super::{
    pitch::Pitch,
    playback::to_playback_events_bounded,
    score::{NoteAddr, Score},
    standard_bass::{
        StandardBassCandidate, standard_bass_candidates, standard_bass_request_for_chord,
    },
    standard_bass_arrangement::StandardBassArrangementOptions,
    standard_bass_pitch_selection::{
        MelodyPitchSelection, arrange_standard_bass_with_pitch_selection,
    },
    standard_bass_transition::{
        ScreenedStandardBassResult, StandardBassTransitionPolicy, fail,
        screen_existing_standard_bass,
    },
};

const MAX_GROUPS: usize = 256;
const MAX_CHOICES: usize = 32;
const MAX_ATTEMPTS: usize = 100_000;

type NoteKey = (usize, usize, usize, usize, usize);
fn key(a: &NoteAddr) -> NoteKey {
    (a.part, a.staff, a.measure, a.voice, a.note)
}

#[derive(Clone)]
struct LocatedCandidate {
    candidate: StandardBassCandidate,
    bass: i16,
    chord: i16,
}
fn movement(a: &LocatedCandidate, b: &LocatedCandidate) -> u32 {
    (i32::from(a.bass) - i32::from(b.bass))
        .unsigned_abs()
        .max((i32::from(a.chord) - i32::from(b.chord)).unsigned_abs())
}

/// Deterministic bounded constraint search. Err means the attempt limit was reached.
fn search(
    groups: &[Vec<LocatedCandidate>],
    edges: &[BTreeSet<usize>],
    max_delta: u16,
    order: &[usize],
    depth: usize,
    selected: &mut [Option<usize>],
    attempts: &mut usize,
) -> Result<bool, ()> {
    let Some(&group) = order.get(depth) else {
        return Ok(true);
    };
    for choice in 0..groups[group].len() {
        *attempts += 1;
        if *attempts > MAX_ATTEMPTS {
            return Err(());
        }
        if edges[group].iter().all(|&other| {
            selected[other].is_none_or(|index| {
                movement(&groups[group][choice], &groups[other][index]) <= u32::from(max_delta)
            })
        }) {
            selected[group] = Some(choice);
            if search(
                groups,
                edges,
                max_delta,
                order,
                depth + 1,
                selected,
                attempts,
            )? {
                return Ok(true);
            }
            selected[group] = None;
        }
    }
    Ok(false)
}

/// Find a screened arrangement using alternative buttons that sound exactly the
/// same MIDI pitches as the initially generated candidate. This bounded search
/// checks all expanded playback transitions, including repeat-back edges; it
/// does not infer instrument layout or certify fingering. If a search limit is
/// reached, the diagnostic does not claim the music is unplayable.
pub fn arrange_standard_bass_find_buttons(
    score: &Score,
    options: &StandardBassArrangementOptions,
    selections: &[MelodyPitchSelection],
    policy: &StandardBassTransitionPolicy,
) -> ScreenedStandardBassResult {
    find_buttons(score, options, selections, policy, false)
}

/// Find a button path that retains the same bass MIDI pitch and chord pitch
/// classes, while allowing different octave voicings. The returned score and
/// button actions contain the actual selected MIDI pitches. Melody playback is
/// checked again after the voicing change; all results need performer review.
pub fn arrange_standard_bass_find_voicings(
    score: &Score,
    options: &StandardBassArrangementOptions,
    selections: &[MelodyPitchSelection],
    policy: &StandardBassTransitionPolicy,
) -> ScreenedStandardBassResult {
    find_buttons(score, options, selections, policy, true)
}

fn find_buttons(
    score: &Score,
    options: &StandardBassArrangementOptions,
    selections: &[MelodyPitchSelection],
    policy: &StandardBassTransitionPolicy,
    allow_voicing_changes: bool,
) -> ScreenedStandardBassResult {
    let base = arrange_standard_bass_with_pitch_selection(score, options, selections);
    let initial = screen_existing_standard_bass(base.clone(), options, policy);
    if initial.arrangement.arrangement.score.is_some()
        || initial
            .arrangement
            .arrangement
            .diagnostics
            .first()
            .is_none_or(|d| d.code != "transition-too-far")
    {
        return initial;
    }
    let Some(output) = base.arrangement.score.as_ref() else {
        return initial;
    };
    let mut groups: Vec<Vec<LocatedCandidate>> = Vec::new();
    let mut action_groups = HashMap::<NoteKey, usize>::new();
    for action in &base.arrangement.actions {
        let address = &action.target;
        if !action.tied_from_previous {
            let Some(note) = output
                .parts
                .get(address.part)
                .and_then(|p| p.staves.get(address.staff))
                .and_then(|s| s.measures.get(address.measure))
                .and_then(|m| m.voices.get(address.voice))
                .and_then(|v| v.get(address.note))
            else {
                return fail(
                    initial,
                    "transition-search-invalid",
                    "generated action does not address a note",
                    Some(address.measure),
                );
            };
            let Some(chord) = note.chord_symbol.as_ref() else {
                return fail(
                    initial,
                    "transition-search-invalid",
                    "generated action has no explicit harmony",
                    Some(address.measure),
                );
            };
            let Ok(request) = standard_bass_request_for_chord(
                chord,
                options.allow_chord_omissions,
                options.allow_chord_additions,
            ) else {
                return initial;
            };
            let Ok(all) = standard_bass_candidates(&options.profile, &request) else {
                return initial;
            };
            let mut sounding = action.candidate.chord_midi.clone();
            sounding.sort_unstable();
            let sounding_classes: BTreeSet<_> = sounding.iter().map(|pitch| pitch % 12).collect();
            let mut choices = Vec::new();
            for candidate in all {
                let mut pitches = candidate.chord_midi.clone();
                pitches.sort_unstable();
                let pitch_classes: BTreeSet<_> = pitches.iter().map(|pitch| pitch % 12).collect();
                let mut unique = pitches.clone();
                unique.push(candidate.bass_midi);
                unique.sort_unstable();
                unique.dedup();
                if candidate.bass_midi != action.candidate.bass_midi
                    || (!allow_voicing_changes && pitches != sounding)
                    || (allow_voicing_changes && pitch_classes != sounding_classes)
                    || unique.len() != pitches.len() + 1
                    || unique.iter().any(|pitch| !(36..=84).contains(pitch))
                {
                    continue;
                }
                let Some(bass) = options
                    .profile
                    .bass_buttons
                    .iter()
                    .find(|button| button.id == candidate.bass_button_id)
                else {
                    return initial;
                };
                let Some(chord) = options
                    .profile
                    .chord_buttons
                    .iter()
                    .find(|button| button.id == candidate.chord_button_id)
                else {
                    return initial;
                };
                choices.push(LocatedCandidate {
                    candidate,
                    bass: bass.position,
                    chord: chord.position,
                });
            }
            if choices.len() > MAX_CHOICES {
                return fail(
                    initial,
                    "transition-search-limit",
                    "too many eligible button combinations; narrow the instrument profile",
                    Some(address.measure),
                );
            }
            if choices.is_empty() {
                return fail(
                    initial,
                    "transition-search-invalid",
                    "the original button action is absent from the profile",
                    Some(address.measure),
                );
            }
            groups.push(choices);
            if groups.len() > MAX_GROUPS {
                return fail(
                    initial,
                    "transition-search-limit",
                    "too many independent button regions for the bounded search",
                    Some(address.measure),
                );
            }
        }
        let Some(group) = groups.len().checked_sub(1) else {
            return fail(
                initial,
                "transition-search-invalid",
                "tied accompaniment has no preceding button action",
                Some(address.measure),
            );
        };
        action_groups.insert(key(address), group);
    }
    let Ok(events) = to_playback_events_bounded(output, &Default::default()) else {
        return initial;
    };
    let mut attacks: Vec<_> = events
        .iter()
        .filter_map(|event| {
            let address = event.source.as_ref()?;
            if address.part != 0 || address.staff != 1 || address.voice != 0 {
                return None;
            }
            Some((event.time_secs, key(address)))
        })
        .collect();
    if attacks.len() > 131_072
        || attacks
            .iter()
            .any(|(time, _)| !time.is_finite() || *time < 0.0)
    {
        return initial;
    }
    attacks.sort_by(|a, b| a.0.total_cmp(&b.0).then_with(|| a.1.cmp(&b.1)));
    attacks.dedup_by(|a, b| a.0 == b.0 && a.1 == b.1);
    let mut edges = vec![BTreeSet::new(); groups.len()];
    for pair in attacks.windows(2) {
        let elapsed = (pair[1].0 - pair[0].0) * 1000.0;
        if elapsed <= 0.0 || elapsed + 1e-7 < f64::from(policy.min_attack_interval_ms) {
            return fail(
                initial,
                "transition-too-fast",
                format!(
                    "button attacks are {elapsed:.3} ms apart; policy requires {} ms",
                    policy.min_attack_interval_ms
                ),
                Some(pair[1].1.2),
            );
        }
        let (Some(&a), Some(&b)) = (action_groups.get(&pair[0].1), action_groups.get(&pair[1].1))
        else {
            return initial;
        };
        if a != b {
            edges[a].insert(b);
            edges[b].insert(a);
        }
    }
    // Fix the ordering once: avoid re-scanning the full graph on every bounded
    // attempt. High-degree regions prune repeat constraints earlier.
    let mut order: Vec<usize> = (0..groups.len()).collect();
    order.sort_by_key(|&index| (std::cmp::Reverse(edges[index].len()), index));
    let mut selected = vec![None; groups.len()];
    let mut attempts = 0;
    match search(
        &groups,
        &edges,
        policy.max_position_delta,
        &order,
        0,
        &mut selected,
        &mut attempts,
    ) {
        Ok(true) => {}
        Ok(false) => {
            return fail(
                initial,
                if allow_voicing_changes {
                    "no-equivalent-voicing-path"
                } else {
                    "no-same-sounding-button-path"
                },
                if allow_voicing_changes {
                    "no permitted voicing path met the specified movement limit"
                } else {
                    "no same-sounding button path met the specified movement limit"
                },
                None,
            );
        }
        Err(()) => {
            return fail(
                initial,
                "transition-search-limit",
                "bounded button search reached its attempt limit",
                None,
            );
        }
    }
    // Compare the generated melody schedule before and after replacing bass
    // voicings, including repeat expansion and tempo-dependent seconds.
    let melody_signature = |score: &Score| {
        to_playback_events_bounded(score, &Default::default()).map(|events| {
            events
                .into_iter()
                .filter(|event| {
                    event
                        .source
                        .as_ref()
                        .is_some_and(|a| a.part == 0 && a.staff == 0 && a.voice == 0)
                })
                .map(|event| {
                    (
                        event.pitch_midi_cents,
                        event.time_beats,
                        event.duration_beats,
                        event.time_secs,
                        event.duration_secs,
                    )
                })
                .collect::<Vec<_>>()
        })
    };
    let before = if allow_voicing_changes {
        Some(melody_signature(output))
    } else {
        None
    };
    let mut updated = base;
    let arrangement = &mut updated.arrangement;
    let Some(updated_score) = arrangement.score.as_mut() else {
        return initial;
    };
    for action in &mut arrangement.actions {
        let group = action_groups[&key(&action.target)];
        action.candidate = groups[group][selected[group].unwrap()].candidate.clone();
        if allow_voicing_changes {
            let address = &action.target;
            let Some(note) = updated_score
                .parts
                .get_mut(address.part)
                .and_then(|p| p.staves.get_mut(address.staff))
                .and_then(|s| s.measures.get_mut(address.measure))
                .and_then(|m| m.voices.get_mut(address.voice))
                .and_then(|v| v.get_mut(address.note))
            else {
                return fail(
                    initial,
                    "transition-search-invalid",
                    "generated action does not address a note",
                    Some(address.measure),
                );
            };
            // The generated accompaniment uses uniform note-level ties. A
            // per-pitch representation would require explicit tie remapping.
            if !note.pitch_tie_starts.is_empty()
                || !note.pitch_tie_ends.is_empty()
                || !note.pitch_accidentals.is_empty()
                || !note.pitch_parentheses.is_empty()
                || !note.pitch_fingerings.is_empty()
            {
                return fail(
                    initial,
                    "unsupported-voicing",
                    "per-pitch accompaniment notation cannot be rewritten safely",
                    Some(address.measure),
                );
            }
            note.pitches = std::iter::once(action.candidate.bass_midi)
                .chain(action.candidate.chord_midi.iter().copied())
                .map(|midi| Pitch::from_midi(midi, false))
                .collect();
        }
    }
    if let Some(before) = before {
        let after = melody_signature(updated_score);
        if before.is_err() || after.is_err() || before.ok() != after.ok() {
            return fail(
                initial,
                "melody-playback-changed",
                "selected melody playback changed during voicing search",
                None,
            );
        }
        if !super::validate::validate(updated_score).is_valid() {
            return fail(
                initial,
                "invalid-output",
                "generated score failed validation after voicing search",
                None,
            );
        }
    }
    screen_existing_standard_bass(updated, options, policy)
}
