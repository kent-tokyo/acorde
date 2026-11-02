//! Explicit standard-bass button combinations for arrangement planning.
//!
//! Profiles describe the pitches that an individual instrument actually sounds;
//! this module does not infer a physical layout or generate score events.

use std::collections::BTreeSet;

use serde::{Deserialize, Serialize};

/// One fundamental or counter-bass button and its actual sounding MIDI pitch.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct StandardBassNoteButton {
    pub id: String,
    pub position: i16,
    pub midi: u8,
}

/// One chord button and its actual sounding MIDI pitches.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct StandardBassChordButton {
    pub id: String,
    pub position: i16,
    pub midi: Vec<u8>,
}

/// Instrument-specific button inventory. Positions use a caller-defined linear
/// coordinate, so `max_distance` must use the same units.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct StandardBassProfile {
    pub bass_buttons: Vec<StandardBassNoteButton>,
    pub chord_buttons: Vec<StandardBassChordButton>,
    pub max_distance: u16,
}

/// Desired sounding pitch classes and bass; omissions and additions are opt-in.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct StandardBassRequest {
    pub pitch_classes: Vec<u8>,
    pub bass_pitch_class: u8,
    pub allow_omissions: bool,
    pub allow_additions: bool,
}

/// A simultaneous two-button candidate, requiring human performer review.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct StandardBassCandidate {
    pub bass_button_id: String,
    pub chord_button_id: String,
    pub bass_midi: u8,
    pub chord_midi: Vec<u8>,
    pub distance: u16,
    pub retained_pitch_classes: Vec<u8>,
    pub omitted_pitch_classes: Vec<u8>,
    pub added_pitch_classes: Vec<u8>,
    pub requires_performer_review: bool,
}

/// Find combinations from the supplied inventory, without changing a score.
///
/// This models simultaneous sound only. It does not imply that an alternating
/// accompaniment realizes the same harmony or that the reach is comfortable.
/// Invalid input is rejected; an empty result means no candidate was found.
pub fn standard_bass_candidates(
    profile: &StandardBassProfile,
    request: &StandardBassRequest,
) -> Result<Vec<StandardBassCandidate>, String> {
    if profile.bass_buttons.len() > 128 || profile.chord_buttons.len() > 128 {
        return Err("a profile may contain at most 128 buttons per row".into());
    }
    if request.pitch_classes.is_empty()
        || request.pitch_classes.iter().any(|&pitch| pitch >= 12)
        || request.bass_pitch_class >= 12
    {
        return Err(
            "pitch classes must be between 0 and 11 and the target must not be empty".into(),
        );
    }
    let target: BTreeSet<u8> = request.pitch_classes.iter().copied().collect();
    let mut ids = BTreeSet::new();
    for button in &profile.bass_buttons {
        if button.id.is_empty() || !ids.insert(&button.id) || button.midi >= 128 {
            return Err(
                "bass button IDs must be unique and pitches must be valid MIDI notes".into(),
            );
        }
    }
    for button in &profile.chord_buttons {
        if button.id.is_empty()
            || !ids.insert(&button.id)
            || button.midi.is_empty()
            || button.midi.len() > 16
            || button.midi.iter().any(|&pitch| pitch >= 128)
        {
            return Err(
                "chord button IDs must be unique and contain 1 to 16 valid MIDI notes".into(),
            );
        }
    }
    let mut candidates = Vec::new();
    for bass in &profile.bass_buttons {
        if bass.midi % 12 != request.bass_pitch_class {
            continue;
        }
        for chord in &profile.chord_buttons {
            let distance = (i32::from(bass.position) - i32::from(chord.position)).unsigned_abs();
            if distance > u32::from(profile.max_distance)
                || chord.midi.iter().any(|&pitch| pitch < bass.midi)
            {
                continue;
            }
            let sounding: BTreeSet<u8> = std::iter::once(bass.midi % 12)
                .chain(chord.midi.iter().map(|pitch| pitch % 12))
                .collect();
            let omitted: Vec<_> = target.difference(&sounding).copied().collect();
            let added: Vec<_> = sounding.difference(&target).copied().collect();
            if (!request.allow_omissions && !omitted.is_empty())
                || (!request.allow_additions && !added.is_empty())
            {
                continue;
            }
            candidates.push(StandardBassCandidate {
                bass_button_id: bass.id.clone(),
                chord_button_id: chord.id.clone(),
                bass_midi: bass.midi,
                chord_midi: chord.midi.clone(),
                distance: distance as u16,
                retained_pitch_classes: target.intersection(&sounding).copied().collect(),
                omitted_pitch_classes: omitted,
                added_pitch_classes: added,
                requires_performer_review: true,
            });
        }
    }
    candidates.sort_by(|a, b| {
        (
            a.omitted_pitch_classes.len(),
            a.added_pitch_classes.len(),
            a.distance,
            &a.bass_button_id,
            &a.chord_button_id,
        )
            .cmp(&(
                b.omitted_pitch_classes.len(),
                b.added_pitch_classes.len(),
                b.distance,
                &b.bass_button_id,
                &b.chord_button_id,
            ))
    });
    Ok(candidates)
}
