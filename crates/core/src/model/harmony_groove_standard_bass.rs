//! Exact button screening for source-bound harmony/groove arrangements.
//!
//! This is a validation API. It never changes the melody or silently removes
//! inner voices. A failed screen leaves the unrestricted candidate available
//! as a free-bass edition, but does not certify any other fingering.

use std::collections::{BTreeMap, BTreeSet};

use serde::{Deserialize, Serialize};

use super::{
    harmony_arrangement::HarmonyStaffRef,
    harmony_groove_proposals::{
        HarmonyGrooveError, ResolvedHarmonyGrooveArrangement, replay_harmony_groove_arrangement,
    },
    playback::{PlaybackEvent, PlaybackOptions, to_playback_events_bounded},
    score::{NotationSpannerKind, NoteAddr, Score},
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
    #[error("left-hand staff selection is empty, duplicated, too large, or outside the score")]
    InvalidStaffSelection,
    #[error("unsupported continuous pitch or sustain control in selected part {part}")]
    UnsupportedPartControls { part: usize },
    #[error("unsupported pitch or sustain effect at {address:?}")]
    UnsupportedScoreEffect { address: NoteAddr },
    #[error("score is invalid or exceeds the screening input limit")]
    InvalidScore,
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

fn matched_buttons<'a>(
    profile: &'a StandardBassProfile,
    sounding: &[u8],
) -> Option<(Option<&'a str>, Option<&'a str>)> {
    if sounding.len() == 1 {
        for bass in &profile.bass_buttons {
            if sounding[0] == bass.midi {
                return Some((Some(&bass.id), None));
            }
        }
    }
    for chord in &profile.chord_buttons {
        if chord.midi.len() != sounding.len() {
            continue;
        }
        let mut pitches = chord.midi.clone();
        pitches.sort_unstable();
        if pitches == sounding {
            return Some((None, Some(&chord.id)));
        }
    }
    for bass in &profile.bass_buttons {
        for chord in &profile.chord_buttons {
            if chord.midi.len() + 1 != sounding.len() {
                continue;
            }
            let distance = (i32::from(bass.position) - i32::from(chord.position)).unsigned_abs();
            if distance > u32::from(profile.max_distance)
                || chord.midi.iter().any(|&pitch| pitch < bass.midi)
            {
                continue;
            }
            let mut pitches = chord.midi.clone();
            pitches.push(bass.midi);
            pitches.sort_unstable();
            if pitches == sounding {
                return Some((Some(&bass.id), Some(&chord.id)));
            }
        }
    }
    None
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
    validate_profile(profile)?;
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
    screen_events(profile, selected)
}

fn validate_profile(profile: &StandardBassProfile) -> Result<(), HarmonyGrooveStandardBassError> {
    validate_standard_bass_profile(profile).map_err(HarmonyGrooveStandardBassError::Profile)?;
    if profile.bass_buttons.len() * profile.chord_buttons.len() > 8_192 {
        return Err(HarmonyGrooveStandardBassError::Profile(
            "the exact two-button screen permits at most 8192 bass/chord pairs".into(),
        ));
    }
    Ok(())
}

/// Screen the sounding events on explicitly selected left-hand staves of an
/// existing score, including transposed scores. This does not replay or
/// certify a saved proposal: use the source-bound API for that separate check.
/// Repeats follow playback order, exact pitches and simultaneous releases are
/// matched against the supplied profile, and performer review is still needed.
pub fn screen_standard_bass_staves(
    score: &Score,
    staves: &[HarmonyStaffRef],
    profile: &StandardBassProfile,
) -> Result<HarmonyGrooveStandardBassReport, HarmonyGrooveStandardBassError> {
    validate_profile(profile)?;
    let total_notes = score
        .parts
        .iter()
        .flat_map(|part| &part.staves)
        .flat_map(|staff| &staff.measures)
        .flat_map(|bar| &bar.voices)
        .fold(0usize, |count, voice| count.saturating_add(voice.len()));
    if score.parts.len() > 32
        || total_notes > 65_536
        || !super::validate::validate(score).is_valid()
    {
        return Err(HarmonyGrooveStandardBassError::InvalidScore);
    }
    if staves.is_empty() || staves.len() > 32 {
        return Err(HarmonyGrooveStandardBassError::InvalidStaffSelection);
    }
    let mut selected_staves = BTreeSet::new();
    for staff in staves {
        let Some(part) = score.parts.get(staff.part) else {
            return Err(HarmonyGrooveStandardBassError::InvalidStaffSelection);
        };
        let Some(stave) = part.staves.get(staff.staff) else {
            return Err(HarmonyGrooveStandardBassError::InvalidStaffSelection);
        };
        if !selected_staves.insert((staff.part, staff.staff)) {
            return Err(HarmonyGrooveStandardBassError::InvalidStaffSelection);
        }
        if !part.midi_pitch_bends.is_empty()
            || part
                .midi_control_changes
                .iter()
                .any(|cc| matches!(cc.controller, 64 | 66 | 120 | 123))
            || stave.measures.iter().any(|bar| {
                !bar.timed_pedals.is_empty()
                    || bar
                        .voices
                        .iter()
                        .flatten()
                        .any(|note| note.pedal_start || note.pedal_end)
            })
        {
            return Err(HarmonyGrooveStandardBassError::UnsupportedPartControls {
                part: staff.part,
            });
        }
    }
    for staff in staves {
        let stave = &score.parts[staff.part].staves[staff.staff];
        for (measure, bar) in stave.measures.iter().enumerate() {
            for (voice, notes) in bar.voices.iter().enumerate() {
                for (note, value) in notes.iter().enumerate() {
                    if value.guitar_bend_alter_cents.is_some_and(|c| c != 0)
                        || value.glissando_start
                        || value.glissando_end
                        || value.arpeggiate.is_some()
                    {
                        return Err(HarmonyGrooveStandardBassError::UnsupportedScoreEffect {
                            address: NoteAddr {
                                part: staff.part,
                                staff: staff.staff,
                                measure,
                                voice,
                                note,
                            },
                        });
                    }
                }
            }
        }
    }
    for span in &score.spanners {
        if matches!(
            span.kind,
            NotationSpannerKind::Glissando
                | NotationSpannerKind::Pedal
                | NotationSpannerKind::TrillLine
        ) && let Some(address) = [&span.start, &span.end]
            .into_iter()
            .find(|address| selected_staves.contains(&(address.part, address.staff)))
        {
            return Err(HarmonyGrooveStandardBassError::UnsupportedScoreEffect {
                address: address.clone(),
            });
        }
    }
    let events = to_playback_events_bounded(score, &PlaybackOptions::default())?;
    let selected = events
        .into_iter()
        .filter(|event| {
            event
                .source
                .as_ref()
                .is_some_and(|address| selected_staves.contains(&(address.part, address.staff)))
        })
        .collect();
    screen_events(profile, selected)
}

fn screen_events(
    profile: &StandardBassProfile,
    selected: Vec<PlaybackEvent>,
) -> Result<HarmonyGrooveStandardBassReport, HarmonyGrooveStandardBassError> {
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
    let mut active = Vec::<(usize, Sound)>::new();
    for (at, boundary) in boundaries {
        for index in boundary.ends {
            active.retain(|(active_index, _)| *active_index != index);
        }
        let new_attack = !boundary.starts.is_empty();
        let attack_measure = boundary
            .starts
            .first()
            .map(|(_, sound)| sound.source.measure);
        for (index, sound) in boundary.starts {
            active.push((index, sound));
        }
        if active.is_empty() {
            continue;
        }
        let mut notes = active
            .iter()
            .map(|(_, sound)| sound.midi)
            .collect::<Vec<_>>();
        notes.sort_unstable();
        let mut addresses = active
            .iter()
            .map(|(_, sound)| sound.source.clone())
            .collect::<Vec<_>>();
        addresses.sort_by_key(|a| (a.part, a.staff, a.measure, a.voice, a.note));
        addresses.dedup();
        let measure = attack_measure
            .or_else(|| addresses.first().map(|address| address.measure))
            .ok_or(HarmonyGrooveStandardBassError::InvalidPlaybackAddress)?;
        let at_beats = at as f64 / TICKS_PER_BEAT;
        let unsupported_pitch = active
            .iter()
            .any(|(_, sound)| sound.has_bend || sound.cents != i32::from(sound.midi) * 100);
        let buttons = (!unsupported_pitch)
            .then(|| matched_buttons(profile, &notes))
            .flatten();
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
        } else if let Some((bass_button_id, chord_button_id)) = buttons {
            report.button_states.push(HarmonyGrooveButtonState {
                measure,
                time_beats: at_beats,
                new_attack,
                source_notes: addresses,
                button_midi: notes.clone(),
                sounding_midi: notes,
                bass_button_id: bass_button_id.map(str::to_owned),
                chord_button_id: chord_button_id.map(str::to_owned),
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
        let action = matched_buttons(&profile, &[43, 59, 62, 65]).unwrap();
        assert_eq!(action.0, Some("G counterbass"));
        assert_eq!(action.1, Some("B diminished"));
        assert!(matched_buttons(&profile, &[55, 59, 62, 65]).is_none());
        assert!(matched_buttons(&profile, &[43, 59, 62, 65, 67]).is_none());
        let mut too_far = profile;
        too_far.max_distance = 0;
        assert!(matched_buttons(&too_far, &[43, 59, 62, 65]).is_none());
    }
}
