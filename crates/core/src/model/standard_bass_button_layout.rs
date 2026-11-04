//! Caller-supplied physical labels for standard-bass candidate search.
use super::{
    notation::ChordSymbol,
    standard_bass::{
        StandardBassCandidate, StandardBassProfile, standard_bass_candidates,
        standard_bass_request_for_chord,
    },
};
use serde::{Deserialize, Serialize};
use std::collections::{BTreeMap, BTreeSet};

#[derive(Debug, Clone, Copy, PartialEq, Eq, PartialOrd, Ord, Serialize, Deserialize)]
#[serde(rename_all = "kebab-case")]
pub enum StandardBassButtonRow {
    Fundamental,
    CounterBass,
    Major,
    Minor,
    Seventh,
    Diminished,
    OtherBass,
    OtherChord,
}
impl StandardBassButtonRow {
    fn is_bass(self) -> bool {
        matches!(
            self,
            Self::Fundamental | Self::CounterBass | Self::OtherBass
        )
    }
}
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct StandardBassButtonLocation {
    pub id: String,
    pub row: StandardBassButtonRow,
    /// Caller-defined column coordinate; it is not a measurement of hand reach.
    pub column: i16,
}
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct LocatedStandardBassProfile {
    pub profile: StandardBassProfile,
    pub locations: Vec<StandardBassButtonLocation>,
}
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct PreviousStandardBassButtons {
    pub bass_button_id: String,
    pub chord_button_id: String,
}
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct LocatedStandardBassCandidate {
    pub candidate: StandardBassCandidate,
    pub bass_location: StandardBassButtonLocation,
    pub chord_location: StandardBassButtonLocation,
    /// Sum of absolute column changes, not a physical-playability certificate.
    pub transition_column_distance: Option<u32>,
    /// The supplied seventh row sounds the root, third and minor seventh.
    pub direct_seventh: bool,
}

/// List deterministic simultaneous actions using only explicitly supplied rows,
/// columns and actual pitches. Reach and registration still require review.
pub fn located_standard_bass_candidates(
    instrument: &LocatedStandardBassProfile,
    chord: &ChordSymbol,
    allow_omissions: bool,
    allow_additions: bool,
    previous: Option<&PreviousStandardBassButtons>,
) -> Result<Vec<LocatedStandardBassCandidate>, String> {
    let profile = &instrument.profile;
    if profile.bass_buttons.len() > 128
        || profile.chord_buttons.len() > 128
        || instrument.locations.len() != profile.bass_buttons.len() + profile.chord_buttons.len()
    {
        return Err("supply exactly one location for each button, at most 128 per kind".into());
    }
    let mut locations = BTreeMap::new();
    let mut occupied = BTreeSet::new();
    for location in &instrument.locations {
        if locations.insert(location.id.as_str(), location).is_some()
            || !occupied.insert((location.row, location.column))
        {
            return Err("button locations must have unique IDs and row/column pairs".into());
        }
    }
    for button in &profile.bass_buttons {
        if !locations
            .get(button.id.as_str())
            .is_some_and(|l| l.row.is_bass())
        {
            return Err("each bass button needs a bass-row location".into());
        }
    }
    for button in &profile.chord_buttons {
        if !locations
            .get(button.id.as_str())
            .is_some_and(|l| !l.row.is_bass())
        {
            return Err("each chord button needs a chord-row location".into());
        }
    }
    let previous_locations = if let Some(p) = previous {
        let b = locations
            .get(p.bass_button_id.as_str())
            .filter(|l| l.row.is_bass())
            .ok_or("unknown previous bass button")?;
        let c = locations
            .get(p.chord_button_id.as_str())
            .filter(|l| !l.row.is_bass())
            .ok_or("unknown previous chord button")?;
        Some((*b, *c))
    } else {
        None
    };
    let request = standard_bass_request_for_chord(chord, allow_omissions, allow_additions)?;
    let mut root_chord = chord.clone();
    root_chord.bass = None;
    let root = standard_bass_request_for_chord(&root_chord, false, false)?.bass_pitch_class;
    let candidates = standard_bass_candidates(profile, &request)?;
    let mut output = Vec::with_capacity(candidates.len());
    for candidate in candidates {
        let bass_location = (*locations[candidate.bass_button_id.as_str()]).clone();
        let chord_location = (*locations[candidate.chord_button_id.as_str()]).clone();
        let tones: BTreeSet<_> = candidate.chord_midi.iter().map(|m| m % 12).collect();
        let direct_seventh = chord.kind == "dominant"
            && request.bass_pitch_class == root
            && chord_location.row == StandardBassButtonRow::Seventh
            && [root, (root + 4) % 12, (root + 10) % 12]
                .iter()
                .all(|p| tones.contains(p))
            && candidate.added_pitch_classes.is_empty();
        let transition_column_distance = previous_locations.map(|(b, c)| {
            (i32::from(b.column) - i32::from(bass_location.column)).unsigned_abs()
                + (i32::from(c.column) - i32::from(chord_location.column)).unsigned_abs()
        });
        output.push(LocatedStandardBassCandidate {
            candidate,
            bass_location,
            chord_location,
            transition_column_distance,
            direct_seventh,
        });
    }
    output.sort_by(|a, b| {
        (
            !a.direct_seventh,
            a.candidate.omitted_pitch_classes.len(),
            a.candidate.added_pitch_classes.len(),
            a.transition_column_distance,
            a.candidate.distance,
            &a.candidate.bass_button_id,
            &a.candidate.chord_button_id,
        )
            .cmp(&(
                !b.direct_seventh,
                b.candidate.omitted_pitch_classes.len(),
                b.candidate.added_pitch_classes.len(),
                b.transition_column_distance,
                b.candidate.distance,
                &b.candidate.bass_button_id,
                &b.candidate.chord_button_id,
            ))
    });
    Ok(output)
}
