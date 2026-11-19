//! Exact button screening for source-bound harmony/groove arrangements.
//!
//! This is a validation API. It never changes the melody or silently removes
//! inner voices. A failed screen leaves the unrestricted candidate available
//! as a free-bass edition, but does not certify any other fingering.

use std::collections::BTreeMap;

use serde::{Deserialize, Serialize};

use super::{
    harmony_groove_proposals::{
        HarmonyGrooveError, ResolvedHarmonyGrooveArrangement, replay_harmony_groove_arrangement,
    },
    playback::{PlaybackOptions, to_playback_events_bounded},
    score::{NoteAddr, Score},
    standard_bass::{StandardBassProfile, validate_standard_bass_profile},
};

const TICKS_PER_BEAT: f64 = 1_000_000.0;
const MAX_LEFT_HAND_EVENTS: usize = 4_096;

#[derive(Debug, thiserror::Error)]
pub enum HarmonyGrooveStandardBassError {
    #[error(transparent)]
    Replay(#[from] HarmonyGrooveError),
    #[error("invalid standard-bass profile: {0}")]
    Profile(String),
    #[error(transparent)]
    Playback(#[from] crate::Error),
    #[error("left-hand playback has {0} events; maximum is 4096")]
    TooManyEvents(usize),
    #[error("left-hand playback has an invalid event time")]
    InvalidPlaybackTime,
    #[error("left-hand playback event has no source note address")]
    InvalidPlaybackAddress,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "kebab-case")]
pub enum HarmonyGrooveButtonFailure {
    NoLeftHandEvents,
    UnsupportedMicrotoneOrBend,
    NoExactButtons,
}

#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
pub struct HarmonyGrooveButtonDiagnostic {
    pub reason: HarmonyGrooveButtonFailure,
    pub measure: Option<usize>,
    pub time_beats: Option<f64>,
    pub source_notes: Vec<NoteAddr>,
    /// Exact playback MIDI notes, including repeated unisons.
    pub sounding_midi: Vec<u8>,
}

/// One exact sounding state after an attack or release. A caller-provided bass
/// button ID may designate a counterbass; this API does not infer button rows.
#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
pub struct HarmonyGrooveButtonState {
    pub measure: usize,
    pub time_beats: f64,
    pub new_attack: bool,
    pub source_notes: Vec<NoteAddr>,
    pub sounding_midi: Vec<u8>,
    pub bass_button_id: Option<String>,
    pub chord_button_id: Option<String>,
    /// Pitches sounded by the selected buttons, equal to `sounding_midi`.
    pub button_midi: Vec<u8>,
}

#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
pub struct HarmonyGrooveStandardBassReport {
    pub playable_with_supplied_profile: bool,
    pub button_states: Vec<HarmonyGrooveButtonState>,
    pub diagnostics: Vec<HarmonyGrooveButtonDiagnostic>,
    pub profile: StandardBassProfile,
    pub requires_performer_review: bool,
}

#[derive(Clone)]
struct Sound {
    source: NoteAddr,
    midi: u8,
    cents: i32,
    has_bend: bool,
}

#[derive(Default)]
struct Boundary {
    starts: Vec<(usize, Sound)>,
    ends: Vec<usize>,
}

fn tick(beats: f64) -> Result<i64, HarmonyGrooveStandardBassError> {
    if !beats.is_finite() || !(0.0..=1_000_000_000.0).contains(&beats) {
        return Err(HarmonyGrooveStandardBassError::InvalidPlaybackTime);
    }
    Ok((beats * TICKS_PER_BEAT).round() as i64)
}

#[derive(Clone)]
struct ButtonChoice {
    bass_id: Option<String>,
    chord_id: Option<String>,
    pitches: Vec<u8>,
    kind: u8,
    distance: u32,
}

impl ButtonChoice {
    fn rank(&self) -> (u8, u32, &str, &str) {
        (
            self.kind,
            self.distance,
            self.bass_id.as_deref().unwrap_or(""),
            self.chord_id.as_deref().unwrap_or(""),
        )
    }
}

struct ButtonMatcher {
    by_pitches: BTreeMap<Vec<u8>, ButtonChoice>,
}

impl ButtonMatcher {
    fn insert(&mut self, choice: ButtonChoice) {
        let key = choice.pitches.clone();
        match self.by_pitches.entry(key) {
            std::collections::btree_map::Entry::Vacant(entry) => {
                entry.insert(choice);
            }
            std::collections::btree_map::Entry::Occupied(mut entry) => {
                if choice.rank() < entry.get().rank() {
                    entry.insert(choice);
                }
            }
        }
    }

    fn new(profile: &StandardBassProfile) -> Self {
        let mut matcher = Self {
            by_pitches: BTreeMap::new(),
        };
        for bass in &profile.bass_buttons {
            matcher.insert(ButtonChoice {
                bass_id: Some(bass.id.clone()),
                chord_id: None,
                pitches: vec![bass.midi],
                kind: 0,
                distance: 0,
            });
        }
        for chord in &profile.chord_buttons {
            let mut pitches = chord.midi.clone();
            pitches.sort_unstable();
            matcher.insert(ButtonChoice {
                bass_id: None,
                chord_id: Some(chord.id.clone()),
                pitches,
                kind: 1,
                distance: 0,
            });
        }
        for bass in &profile.bass_buttons {
            for chord in &profile.chord_buttons {
                let distance =
                    (i32::from(bass.position) - i32::from(chord.position)).unsigned_abs();
                if distance > u32::from(profile.max_distance)
                    || chord.midi.iter().any(|&pitch| pitch < bass.midi)
                {
                    continue;
                }
                let mut pitches = chord.midi.clone();
                pitches.push(bass.midi);
                pitches.sort_unstable();
                matcher.insert(ButtonChoice {
                    bass_id: Some(bass.id.clone()),
                    chord_id: Some(chord.id.clone()),
                    pitches,
                    kind: 2,
                    distance,
                });
            }
        }
        matcher
    }

    fn find(&self, sounding: &[u8]) -> Option<ButtonChoice> {
        self.by_pitches.get(sounding).cloned()
    }
}

/// Replay a saved candidate from its original source and check every left-hand
/// playback state against the supplied instrument's actual button pitches.
/// The added accompaniment part and the request's optional source bass staff
/// are left hand. Melody and all source parts remain untouched. A false result
/// means this two-button screen found no exact realization for this candidate
/// and profile. It does not prove the harmony impossible on all accordions.
pub fn screen_harmony_groove_standard_bass(
    source: &Score,
    resolved: &ResolvedHarmonyGrooveArrangement,
    profile: &StandardBassProfile,
) -> Result<HarmonyGrooveStandardBassReport, HarmonyGrooveStandardBassError> {
    validate_standard_bass_profile(profile).map_err(HarmonyGrooveStandardBassError::Profile)?;
    let matcher = ButtonMatcher::new(profile);
    let candidate = replay_harmony_groove_arrangement(source, resolved)?;
    let added = candidate.arrangement.added_part;
    let bass = resolved.request.base.bass;
    let playback =
        to_playback_events_bounded(&candidate.arrangement.score, &PlaybackOptions::default())?;
    let selected = playback
        .into_iter()
        .filter(|event| {
            event.source.as_ref().is_some_and(|address| {
                address.part == added
                    || (bass.is_some_and(|staff| {
                        address.part == staff.part && address.staff == staff.staff
                    }) && (resolved.request.base.start_measure
                        ..resolved.request.base.end_measure_exclusive)
                        .contains(&address.measure))
            })
        })
        .collect::<Vec<_>>();
    if selected.len() > MAX_LEFT_HAND_EVENTS {
        return Err(HarmonyGrooveStandardBassError::TooManyEvents(
            selected.len(),
        ));
    }
    let mut report = HarmonyGrooveStandardBassReport {
        playable_with_supplied_profile: false,
        button_states: Vec::new(),
        diagnostics: Vec::new(),
        profile: profile.clone(),
        requires_performer_review: true,
    };
    if selected.is_empty() {
        report.diagnostics.push(HarmonyGrooveButtonDiagnostic {
            reason: HarmonyGrooveButtonFailure::NoLeftHandEvents,
            measure: None,
            time_beats: None,
            source_notes: Vec::new(),
            sounding_midi: Vec::new(),
        });
        return Ok(report);
    }
    let mut boundaries = BTreeMap::<i64, Boundary>::new();
    for (index, event) in selected.into_iter().enumerate() {
        let start = tick(event.time_beats)?;
        let end = tick(event.time_beats + event.duration_beats)?;
        if end <= start {
            return Err(HarmonyGrooveStandardBassError::InvalidPlaybackTime);
        }
        let Some(address) = event.source else {
            return Err(HarmonyGrooveStandardBassError::InvalidPlaybackAddress);
        };
        boundaries.entry(start).or_default().starts.push((
            index,
            Sound {
                source: address,
                midi: event.pitch_midi,
                cents: event.pitch_midi_cents,
                has_bend: !event.pitch_bend_curve.is_empty(),
            },
        ));
        boundaries.entry(end).or_default().ends.push(index);
    }
    let mut active = BTreeMap::<usize, Sound>::new();
    for (at, boundary) in boundaries {
        for index in boundary.ends {
            active.remove(&index);
        }
        let new_attack = !boundary.starts.is_empty();
        let attack_measure = boundary
            .starts
            .first()
            .map(|(_, sound)| sound.source.measure);
        for (index, sound) in boundary.starts {
            active.insert(index, sound);
        }
        if active.is_empty() {
            continue;
        }
        let mut notes = active.values().map(|sound| sound.midi).collect::<Vec<_>>();
        notes.sort_unstable();
        let mut addresses = active
            .values()
            .map(|sound| sound.source.clone())
            .collect::<Vec<_>>();
        addresses.sort_by_key(|a| (a.part, a.staff, a.measure, a.voice, a.note));
        addresses.dedup();
        let measure = attack_measure
            .or_else(|| addresses.first().map(|address| address.measure))
            .ok_or(HarmonyGrooveStandardBassError::InvalidPlaybackAddress)?;
        let at_beats = at as f64 / TICKS_PER_BEAT;
        let unsupported_pitch = active
            .values()
            .any(|sound| sound.has_bend || sound.cents != i32::from(sound.midi) * 100);
        let buttons = (!unsupported_pitch).then(|| matcher.find(&notes)).flatten();
        let reason = if unsupported_pitch {
            Some(HarmonyGrooveButtonFailure::UnsupportedMicrotoneOrBend)
        } else if buttons.is_none() {
            Some(HarmonyGrooveButtonFailure::NoExactButtons)
        } else {
            None
        };
        if let Some(reason) = reason {
            report.diagnostics.push(HarmonyGrooveButtonDiagnostic {
                reason,
                measure: Some(measure),
                time_beats: Some(at_beats),
                source_notes: addresses,
                sounding_midi: notes,
            });
        } else if let Some(buttons) = buttons {
            report.button_states.push(HarmonyGrooveButtonState {
                measure,
                time_beats: at_beats,
                new_attack,
                source_notes: addresses,
                sounding_midi: notes,
                bass_button_id: buttons.bass_id,
                chord_button_id: buttons.chord_id,
                button_midi: buttons.pitches,
            });
        }
    }
    report.playable_with_supplied_profile = report.diagnostics.is_empty();
    Ok(report)
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::model::standard_bass::{StandardBassChordButton, StandardBassNoteButton};

    #[test]
    fn seventh_requires_exact_counterbass_and_chord_pitches() {
        let profile = StandardBassProfile {
            bass_buttons: vec![StandardBassNoteButton {
                id: "G counterbass".into(),
                position: 1,
                midi: 43,
            }],
            chord_buttons: vec![StandardBassChordButton {
                id: "B diminished".into(),
                position: 2,
                midi: vec![59, 62, 65],
            }],
            max_distance: 1,
        };
        let action = ButtonMatcher::new(&profile)
            .find(&[43, 59, 62, 65])
            .unwrap();
        assert_eq!(action.bass_id.as_deref(), Some("G counterbass"));
        assert_eq!(action.chord_id.as_deref(), Some("B diminished"));
        assert_eq!(action.pitches, vec![43, 59, 62, 65]);
        assert!(
            ButtonMatcher::new(&profile)
                .find(&[55, 59, 62, 65])
                .is_none()
        );
        assert!(
            ButtonMatcher::new(&profile)
                .find(&[43, 59, 62, 65, 67])
                .is_none()
        );
        let mut too_far = profile;
        too_far.max_distance = 0;
        assert!(
            ButtonMatcher::new(&too_far)
                .find(&[43, 59, 62, 65])
                .is_none()
        );
    }
}
