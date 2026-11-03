//! Explicit standard-bass button combinations for arrangement planning.
//!
//! Profiles describe the pitches that an individual instrument actually sounds;
//! this module does not infer a physical layout or generate score events.

use std::collections::BTreeSet;

use serde::{Deserialize, Serialize};

use super::notation::ChordSymbol;

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
    /// Readable action, e.g. `C bass + Em chord`; the musical chord label is separate.
    #[serde(default)]
    pub label: String,
    #[serde(default)]
    pub realization: StandardBassRealization,
}

/// `Combined` means the chord button does not itself contain the bass pitch class, so the
/// requested harmony needs the two buttons together (e.g. C bass + E minor for Cmaj7).
#[derive(Debug, Clone, Copy, Default, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "kebab-case")]
pub enum StandardBassRealization {
    #[default]
    Direct,
    Combined,
}

const NAMES: [&str; 12] = [
    "C", "C#", "D", "D#", "E", "F", "F#", "G", "G#", "A", "A#", "B",
];

/// Name a button by its sounding pitch classes (sharps), independent of its ID.
fn chord_button_name(tones: &BTreeSet<u8>) -> String {
    for &root in tones {
        let has = |i: u8| tones.contains(&((root + i) % 12));
        let suffix = match (tones.len(), has(4), has(3), has(7), has(6), has(8), has(10)) {
            (3, true, _, true, ..) => "",
            (3, _, true, true, ..) => "m",
            (3, _, true, _, true, ..) => "dim",
            (3, true, _, _, _, true, _) => "aug",
            (3, true, _, _, _, _, true) => "7",
            (4, true, _, true, _, _, true) => "7",
            _ => continue,
        };
        return format!("{}{suffix}", NAMES[usize::from(root)]);
    }
    let names: Vec<_> = tones.iter().map(|&p| NAMES[usize::from(p)]).collect();
    format!("({})", names.join(" "))
}

/// Translate a supported chord symbol into a pitch-class request. Slash basses
/// are part of the requested sounding set; altered degrees are rejected.
pub fn standard_bass_request_for_chord(
    chord: &ChordSymbol,
    allow_omissions: bool,
    allow_additions: bool,
) -> Result<StandardBassRequest, String> {
    use super::arrange::accordion_pitch_class as pitch_class;
    if !chord.degrees.is_empty() {
        return Err("chord degrees are not supported by this candidate search".into());
    }
    let root = pitch_class(&chord.root).ok_or("unsupported chord root spelling")?;
    let bass = match &chord.bass {
        Some(name) => pitch_class(name).ok_or("unsupported chord bass spelling")?,
        None => root,
    };
    let intervals: &[u8] = match chord.kind.as_str() {
        "major" => &[0, 4, 7],
        "minor" => &[0, 3, 7],
        "diminished" => &[0, 3, 6],
        "augmented" => &[0, 4, 8],
        "dominant" => &[0, 4, 7, 10],
        "major-seventh" => &[0, 4, 7, 11],
        "minor-seventh" => &[0, 3, 7, 10],
        _ => return Err("unsupported chord kind for this candidate search".into()),
    };
    let pitch_classes = intervals
        .iter()
        .map(|interval| (root + interval) % 12)
        .chain(std::iter::once(bass))
        .collect::<BTreeSet<_>>()
        .into_iter()
        .collect();
    Ok(StandardBassRequest {
        pitch_classes,
        bass_pitch_class: bass,
        allow_omissions,
        allow_additions,
    })
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
        || request.pitch_classes.len() > 12
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
        if button.id.is_empty()
            || button.id.len() > 128
            || !ids.insert(&button.id)
            || button.midi >= 128
        {
            return Err(
                "bass button IDs must be unique and pitches must be valid MIDI notes".into(),
            );
        }
    }
    for button in &profile.chord_buttons {
        if button.id.is_empty()
            || button.id.len() > 128
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
            let chord_tones: BTreeSet<u8> = chord.midi.iter().map(|pitch| pitch % 12).collect();
            candidates.push(StandardBassCandidate {
                label: format!(
                    "{} bass + {} chord",
                    NAMES[usize::from(bass.midi % 12)],
                    chord_button_name(&chord_tones)
                ),
                realization: if chord_tones.contains(&(bass.midi % 12)) {
                    StandardBassRealization::Direct
                } else {
                    StandardBassRealization::Combined
                },
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

/// Most two-chord-button candidates returned; the search is otherwise exhaustive.
pub const MAX_DOUBLE_CHORD_CANDIDATES: usize = 256;

/// A bass button with two different chord buttons pressed together.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct StandardBassDoubleChordCandidate {
    pub bass_button_id: String,
    pub chord_button_ids: [String; 2],
    pub bass_midi: u8,
    /// Sounding pitches of both chord buttons, sorted and de-duplicated.
    pub chord_midi: Vec<u8>,
    /// Largest position gap between any two pressed buttons.
    pub distance: u16,
    pub retained_pitch_classes: Vec<u8>,
    pub omitted_pitch_classes: Vec<u8>,
    pub added_pitch_classes: Vec<u8>,
    pub requires_performer_review: bool,
    pub label: String,
}

/// Opt-in search for one bass plus two chord buttons, for harmony that a single chord
/// button cannot give. Each chord button must contribute a requested pitch class the other
/// two buttons do not already sound; simultaneous sound and reach rules are those of
/// [`standard_bass_candidates`]. Results are capped at [`MAX_DOUBLE_CHORD_CANDIDATES`];
/// an empty list means this bounded search found nothing, not that the chord is impossible.
pub fn standard_bass_double_chord_candidates(
    profile: &StandardBassProfile,
    request: &StandardBassRequest,
) -> Result<Vec<StandardBassDoubleChordCandidate>, String> {
    // Reuse the single-button validation of the profile and request.
    standard_bass_candidates(profile, request)?;
    let target: BTreeSet<u8> = request.pitch_classes.iter().copied().collect();
    let tones =
        |b: &StandardBassChordButton| -> BTreeSet<u8> { b.midi.iter().map(|p| p % 12).collect() };
    let gap = |a: i16, b: i16| (i32::from(a) - i32::from(b)).unsigned_abs();
    let mut found = Vec::new();
    for bass in profile
        .bass_buttons
        .iter()
        .filter(|b| b.midi % 12 == request.bass_pitch_class)
    {
        let bass_pc = bass.midi % 12;
        for (i, first) in profile.chord_buttons.iter().enumerate() {
            for second in &profile.chord_buttons[i + 1..] {
                let (t1, t2) = (tones(first), tones(second));
                let distance = gap(bass.position, first.position)
                    .max(gap(bass.position, second.position))
                    .max(gap(first.position, second.position));
                if distance > u32::from(profile.max_distance)
                    || first
                        .midi
                        .iter()
                        .chain(&second.midi)
                        .any(|&p| p < bass.midi)
                {
                    continue;
                }
                let needed = |own: &BTreeSet<u8>, other: &BTreeSet<u8>| {
                    own.iter()
                        .any(|p| target.contains(p) && *p != bass_pc && !other.contains(p))
                };
                if !needed(&t1, &t2) || !needed(&t2, &t1) {
                    continue;
                }
                let sounding: BTreeSet<u8> = t1
                    .union(&t2)
                    .copied()
                    .chain(std::iter::once(bass_pc))
                    .collect();
                let omitted: Vec<_> = target.difference(&sounding).copied().collect();
                let added: Vec<_> = sounding.difference(&target).copied().collect();
                if (!request.allow_omissions && !omitted.is_empty())
                    || (!request.allow_additions && !added.is_empty())
                {
                    continue;
                }
                let chord_midi: BTreeSet<u8> =
                    first.midi.iter().chain(&second.midi).copied().collect();
                found.push(StandardBassDoubleChordCandidate {
                    label: format!(
                        "{} bass + {} chord + {} chord",
                        NAMES[usize::from(bass_pc)],
                        chord_button_name(&t1),
                        chord_button_name(&t2)
                    ),
                    bass_button_id: bass.id.clone(),
                    chord_button_ids: [first.id.clone(), second.id.clone()],
                    bass_midi: bass.midi,
                    chord_midi: chord_midi.into_iter().collect(),
                    distance: distance as u16,
                    retained_pitch_classes: target.intersection(&sounding).copied().collect(),
                    omitted_pitch_classes: omitted,
                    added_pitch_classes: added,
                    requires_performer_review: true,
                });
            }
        }
    }
    found.sort_by(|a, b| {
        (
            a.omitted_pitch_classes.len(),
            a.added_pitch_classes.len(),
            a.distance,
            &a.bass_button_id,
            &a.chord_button_ids,
        )
            .cmp(&(
                b.omitted_pitch_classes.len(),
                b.added_pitch_classes.len(),
                b.distance,
                &b.bass_button_id,
                &b.chord_button_ids,
            ))
    });
    found.truncate(MAX_DOUBLE_CHORD_CANDIDATES);
    Ok(found)
}
