//! Screen written standard-bass actions on the expanded playback timeline.
//! The caller supplies screening limits; this is not a fingering certificate.
use std::collections::{BTreeSet, HashMap};

use serde::{Deserialize, Serialize};

use super::{
    playback::{PlaybackEvent, to_playback_events_bounded},
    score::{NoteAddr, Score},
    standard_bass_arrangement::{
        StandardBassArrangementDiagnostic, StandardBassArrangementOptions,
    },
    standard_bass_pitch_selection::{
        MelodyPitchSelection, SelectedPitchArrangementResult,
        arrange_standard_bass_with_pitch_selection,
    },
};

/// Both limits are caller supplied. Positions use the profile's linear units.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct StandardBassTransitionPolicy {
    /// Maximum movement for either button between consecutive attacks.
    pub max_position_delta: u16,
    /// Minimum time between consecutive attacks, including a reattack of the same buttons.
    pub min_attack_interval_ms: u32,
}

/// One actual playback attack, including attacks reached by repeats/endings.
#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
pub struct StandardBassTransition {
    /// Address of the generated accompaniment note, not the original melody.
    pub source: NoteAddr,
    pub bass_button_id: String,
    pub chord_button_id: String,
    pub time_seconds: f64,
    /// None for the first attack.
    pub elapsed_ms: Option<f64>,
    /// Maximum of bass and chord position changes; None for the first attack.
    pub position_delta: Option<u16>,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct ScreenedStandardBassResult {
    pub arrangement: SelectedPitchArrangementResult,
    pub policy: StandardBassTransitionPolicy,
    /// Empty on any failure; an absent score is never a partial success.
    pub transitions: Vec<StandardBassTransition>,
}

pub(super) fn fail(
    mut result: ScreenedStandardBassResult,
    code: &str,
    message: impl Into<String>,
    measure_index: Option<usize>,
) -> ScreenedStandardBassResult {
    result.arrangement.arrangement.score = None;
    result.arrangement.arrangement.source_pitches.clear();
    result.arrangement.arrangement.actions.clear();
    result.arrangement.arrangement.diagnostics = vec![StandardBassArrangementDiagnostic {
        code: code.into(),
        message: message.into(),
        measure_index,
    }];
    result.transitions.clear();
    result
}

fn key(addr: &NoteAddr) -> (usize, usize, usize, usize, usize) {
    (addr.part, addr.staff, addr.measure, addr.voice, addr.note)
}

/// Generate a simplified arrangement and screen its button attacks in playback
/// order. This checks the candidate actually produced by the existing search;
/// rejecting it does not prove that no other fingering or arrangement exists.
pub fn arrange_standard_bass_screened(
    score: &Score,
    options: &StandardBassArrangementOptions,
    selections: &[MelodyPitchSelection],
    policy: &StandardBassTransitionPolicy,
) -> ScreenedStandardBassResult {
    let arrangement = arrange_standard_bass_with_pitch_selection(score, options, selections);
    screen_existing_standard_bass(arrangement, options, policy)
}

pub(super) fn screen_existing_standard_bass(
    arrangement: SelectedPitchArrangementResult,
    options: &StandardBassArrangementOptions,
    policy: &StandardBassTransitionPolicy,
) -> ScreenedStandardBassResult {
    let mut result = ScreenedStandardBassResult {
        arrangement,
        policy: policy.clone(),
        transitions: vec![],
    };
    let Some(output) = result.arrangement.arrangement.score.as_ref() else {
        return result;
    };
    let events = match to_playback_events_bounded(output, &Default::default()) {
        Ok(events) => events,
        Err(_) => {
            return fail(
                result,
                "transition-playback-too-large",
                "could not construct a bounded playback timeline",
                None,
            );
        }
    };
    let actions: HashMap<_, _> = result
        .arrangement
        .arrangement
        .actions
        .iter()
        .map(|action| (key(&action.target), action))
        .collect();
    let mut left: Vec<&PlaybackEvent> = events
        .iter()
        .filter(|event| {
            event
                .source
                .as_ref()
                .is_some_and(|addr| addr.part == 0 && addr.staff == 1 && addr.voice == 0)
        })
        .collect();
    if left.len() > 131_072 {
        return fail(
            result,
            "transition-playback-too-large",
            "too many accompaniment playback events",
            None,
        );
    }
    if left
        .iter()
        .any(|event| !event.time_secs.is_finite() || event.time_secs < 0.0)
    {
        return fail(
            result,
            "transition-invalid-time",
            "accompaniment playback has an invalid timestamp",
            None,
        );
    }
    left.sort_by(|a, b| {
        a.time_secs
            .total_cmp(&b.time_secs)
            .then_with(|| key(a.source.as_ref().unwrap()).cmp(&key(b.source.as_ref().unwrap())))
            .then_with(|| a.pitch_midi.cmp(&b.pitch_midi))
    });
    let mut index = 0;
    let mut previous: Option<(f64, i16, i16)> = None;
    while index < left.len() {
        let start = left[index];
        let address = start.source.as_ref().unwrap();
        let Some(action) = actions.get(&key(address)) else {
            return fail(
                result,
                "transition-unmapped-action",
                "playback attack has no button action",
                Some(address.measure),
            );
        };
        let Some(bass) = options
            .profile
            .bass_buttons
            .iter()
            .find(|button| button.id == action.candidate.bass_button_id)
        else {
            return fail(
                result,
                "transition-invalid-profile",
                "bass button is absent from the profile",
                Some(address.measure),
            );
        };
        let Some(chord) = options
            .profile
            .chord_buttons
            .iter()
            .find(|button| button.id == action.candidate.chord_button_id)
        else {
            return fail(
                result,
                "transition-invalid-profile",
                "chord button is absent from the profile",
                Some(address.measure),
            );
        };
        let mut actual = BTreeSet::new();
        let first_event_index = index;
        while index < left.len()
            && left[index].time_secs == start.time_secs
            && key(left[index].source.as_ref().unwrap()) == key(address)
        {
            actual.insert(left[index].pitch_midi);
            index += 1;
        }
        let expected: BTreeSet<_> = std::iter::once(action.candidate.bass_midi)
            .chain(action.candidate.chord_midi.iter().copied())
            .collect();
        if actual != expected || index - first_event_index != expected.len() {
            return fail(
                result,
                "transition-playback-mismatch",
                "button action and simultaneous sounding pitches disagree",
                Some(address.measure),
            );
        }
        let (elapsed_ms, position_delta) =
            if let Some((time, previous_bass, previous_chord)) = previous {
                let elapsed = (start.time_secs - time) * 1000.0;
                if elapsed <= 0.0 {
                    return fail(
                        result,
                        "transition-overlapping-attacks",
                        "separate button actions have the same playback time",
                        Some(address.measure),
                    );
                }
                let distance = (i32::from(bass.position) - i32::from(previous_bass))
                    .unsigned_abs()
                    .max((i32::from(chord.position) - i32::from(previous_chord)).unsigned_abs());
                if elapsed + 1e-7 < f64::from(policy.min_attack_interval_ms) {
                    return fail(
                        result,
                        "transition-too-fast",
                        format!(
                            "button attacks are {elapsed:.3} ms apart; policy requires {} ms",
                            policy.min_attack_interval_ms
                        ),
                        Some(address.measure),
                    );
                }
                if distance > u32::from(policy.max_position_delta) {
                    return fail(
                        result,
                        "transition-too-far",
                        format!(
                            "button movement is {distance} units; policy allows {}",
                            policy.max_position_delta
                        ),
                        Some(address.measure),
                    );
                }
                (Some(elapsed), Some(distance as u16))
            } else {
                (None, None)
            };
        previous = Some((start.time_secs, bass.position, chord.position));
        result.transitions.push(StandardBassTransition {
            source: address.clone(),
            bass_button_id: bass.id.clone(),
            chord_button_id: chord.id.clone(),
            time_seconds: start.time_secs,
            elapsed_ms,
            position_delta,
        });
    }
    if result.transitions.is_empty() {
        return fail(
            result,
            "transition-no-attacks",
            "no audible standard-bass attacks were generated",
            None,
        );
    }
    result
}
