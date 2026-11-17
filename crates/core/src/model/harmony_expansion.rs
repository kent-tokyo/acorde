//! Spelled chord tones for arrangement, before instrument-specific voicing.
//!
//! This opt-in expansion follows MusicXML's kind and degree semantics. In
//! particular, thirteenth kinds include the eleventh; a voicing policy may
//! subsequently omit it. An `alter` modifies the interval in the base kind,
//! while an `add` uses dominant-scale intervals (including a minor seventh).
//! https://www.w3.org/2021/06/musicxml40/musicxml-reference/elements/degree-alter/
//! Existing preset and chord-realization commands keep their legacy behavior.

use std::collections::BTreeSet;

use serde::{Deserialize, Serialize};

use super::notation::ChordSymbol;
use super::pitch::{Pitch, Step};

pub const HARMONY_EXPANSION_CONTRACT_VERSION: u16 = 1;
pub const HARMONY_EXPANSION_MAX_DEGREES: usize = 32;

const NATURALS: [i16; 7] = [0, 2, 4, 5, 7, 9, 11];
const STEPS: [Step; 7] = [
    Step::C,
    Step::D,
    Step::E,
    Step::F,
    Step::G,
    Step::A,
    Step::B,
];
const DOMINANT_DEGREES: [i16; 13] = [0, 2, 4, 5, 7, 9, 10, 12, 14, 16, 17, 19, 21];

#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
pub struct HarmonyPitchClass {
    pub step: Step,
    pub alter: i8,
}

impl HarmonyPitchClass {
    pub fn pitch_class(&self) -> u8 {
        (i16::from(self.step.to_semitone()) + i16::from(self.alter)).rem_euclid(12) as u8
    }
}

#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "kebab-case")]
pub enum HarmonyToneRole {
    Root,
    Third,
    Fifth,
    Sixth,
    Seventh,
    Suspension,
    Tension,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "kebab-case")]
pub enum HarmonyToneOrigin {
    Kind,
    Added,
    Altered,
}

#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
pub struct ExpandedHarmonyTone {
    /// Compound degree is retained: 9 differs from 2 even at the same pitch class.
    pub degree: u8,
    pub semitones_above_root: i16,
    pub spelling: HarmonyPitchClass,
    /// Diatonic octave displacement from the spelled root, before voicing.
    pub octave_from_root: i8,
    pub role: HarmonyToneRole,
    pub origin: HarmonyToneOrigin,
}

impl ExpandedHarmonyTone {
    /// Spell this tone with the root in the requested octave. No octave or
    /// accidental is clamped; MIDI-range failures are reported to the caller.
    pub fn pitch_at_root_octave(&self, root_octave: i8) -> Result<Pitch, HarmonyExpansionError> {
        let octave = i16::from(root_octave) + i16::from(self.octave_from_root);
        let octave = i8::try_from(octave).map_err(|_| HarmonyExpansionError::OutOfRange)?;
        let pitch = Pitch::with_alter(self.spelling.step.clone(), octave, self.spelling.alter);
        if !(0..=127).contains(&pitch.to_midi()) {
            return Err(HarmonyExpansionError::OutOfRange);
        }
        Ok(pitch)
    }
}

#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
pub struct ExpandedHarmony {
    pub contract_version: u16,
    pub root: HarmonyPitchClass,
    /// A slash bass remains separate; it is not silently added to every upper voicing.
    pub bass: Option<HarmonyPitchClass>,
    pub tones: Vec<ExpandedHarmonyTone>,
}

#[derive(Debug, Clone, PartialEq, Eq, thiserror::Error)]
pub enum HarmonyExpansionError {
    #[error("invalid chord root")]
    InvalidRoot,
    #[error("invalid slash bass")]
    InvalidBass,
    #[error("unsupported chord kind")]
    UnsupportedKind,
    #[error("chord has more than 32 degree directives")]
    TooManyDegrees,
    #[error("invalid degree directive at index {index}")]
    InvalidDegree { index: usize },
    #[error("degree {degree} is absent from the chord kind (directive {index})")]
    MissingDegree { index: usize, degree: u8 },
    #[error("multiple alterations or removals target degree {degree}")]
    ConflictingDegree { degree: u8 },
    #[error("chord has no tones after degree directives")]
    EmptyChord,
    #[error("spelled chord tone is outside the supported pitch range")]
    OutOfRange,
}

fn parse_name(name: &str) -> Option<(usize, HarmonyPitchClass)> {
    if name.len() > 16 {
        return None;
    }
    let mut chars = name.trim().chars();
    let letter = chars.next()?.to_ascii_uppercase();
    let index = "CDEFGAB".find(letter)?;
    let mut alter = 0_i8;
    let mut count = 0;
    for char in chars {
        count += 1;
        if count > 2 {
            return None;
        }
        let amount = match char {
            '#' | '♯' => 1,
            'b' | '♭' => -1,
            _ => return None,
        };
        if alter != 0 && alter.signum() != amount {
            return None;
        }
        alter += amount;
    }
    Some((
        index,
        HarmonyPitchClass {
            step: STEPS.get(index)?.clone(),
            alter,
        },
    ))
}

fn kind_tones(kind: &str) -> Option<Vec<(u8, i16)>> {
    let (third, fifth, seventh, sixth, extension) = match kind {
        "" | "major" => (4, 7, None, None, 0),
        "minor" => (3, 7, None, None, 0),
        "augmented" => (4, 8, None, None, 0),
        "diminished" => (3, 6, None, None, 0),
        "dominant" => (4, 7, Some(10), None, 0),
        "major-seventh" => (4, 7, Some(11), None, 0),
        "minor-seventh" => (3, 7, Some(10), None, 0),
        "half-diminished" => (3, 6, Some(10), None, 0),
        "diminished-seventh" => (3, 6, Some(9), None, 0),
        "augmented-seventh" => (4, 8, Some(10), None, 0),
        "major-minor" | "minor-major" | "minor-major-seventh" => (3, 7, Some(11), None, 0),
        "major-sixth" => (4, 7, None, Some(9), 0),
        "minor-sixth" => (3, 7, None, Some(9), 0),
        "major-add9" => (4, 7, None, None, 9),
        "minor-add9" => (3, 7, None, None, 9),
        "major-ninth" => (4, 7, Some(11), None, 9),
        "minor-ninth" => (3, 7, Some(10), None, 9),
        "dominant-ninth" => (4, 7, Some(10), None, 9),
        "major-11th" => (4, 7, Some(11), None, 11),
        "minor-11th" => (3, 7, Some(10), None, 11),
        "dominant-11th" => (4, 7, Some(10), None, 11),
        "major-13th" => (4, 7, Some(11), None, 13),
        "minor-13th" => (3, 7, Some(10), None, 13),
        "dominant-13th" => (4, 7, Some(10), None, 13),
        "dominant-flat-five" => (4, 6, Some(10), None, 0),
        "dominant-sharp-five" => (4, 8, Some(10), None, 0),
        "suspended-second" => return Some(vec![(1, 0), (2, 2), (5, 7)]),
        "suspended-fourth" => return Some(vec![(1, 0), (4, 5), (5, 7)]),
        "power" => return Some(vec![(1, 0), (5, 7)]),
        "pedal" => return Some(vec![(1, 0)]),
        "other" => return Some(Vec::new()),
        _ => return None,
    };
    let mut tones = vec![(1, 0), (3, third), (5, fifth)];
    if let Some(semitones) = sixth {
        tones.push((6, semitones));
    }
    if let Some(semitones) = seventh {
        tones.push((7, semitones));
    }
    for (degree, semitones) in [(9, 14), (11, 17), (13, 21)] {
        if extension >= degree {
            tones.push((degree, semitones));
        }
    }
    Some(tones)
}

/// Expand supported chord kinds and explicit degrees without choosing an
/// instrument, register, omitted voice or enharmonic replacement.
///
/// Degree edits refer to exact compound degrees in the base kind, independent
/// of input order. Repeated identical additions are deduplicated; distinct
/// alterations such as b9 and #9 may coexist as explicit additions. Unsupported
/// or contradictory directives return an error, never a partial harmony.
pub fn expand_harmony(chord: &ChordSymbol) -> Result<ExpandedHarmony, HarmonyExpansionError> {
    if chord.degrees.len() > HARMONY_EXPANSION_MAX_DEGREES {
        return Err(HarmonyExpansionError::TooManyDegrees);
    }
    let (root_index, root) = parse_name(&chord.root).ok_or(HarmonyExpansionError::InvalidRoot)?;
    let bass = chord
        .bass
        .as_deref()
        .map(|name| {
            parse_name(name)
                .map(|(_, pitch)| pitch)
                .ok_or(HarmonyExpansionError::InvalidBass)
        })
        .transpose()?;
    let base = kind_tones(&chord.kind).ok_or(HarmonyExpansionError::UnsupportedKind)?;
    let mut tones: Vec<_> = base
        .iter()
        .map(|&(degree, interval)| (degree, interval, HarmonyToneOrigin::Kind))
        .collect();
    let mut edited = BTreeSet::new();
    for (index, directive) in chord.degrees.iter().enumerate() {
        if !(1..=13).contains(&directive.value)
            || !(-2..=2).contains(&directive.alter)
            || !matches!(directive.kind.as_str(), "add" | "alter" | "subtract")
        {
            return Err(HarmonyExpansionError::InvalidDegree { index });
        }
        if directive.kind == "add" {
            continue;
        }
        if !edited.insert(directive.value) {
            return Err(HarmonyExpansionError::ConflictingDegree {
                degree: directive.value,
            });
        }
        let Some(&(_, original)) = base.iter().find(|(degree, _)| *degree == directive.value)
        else {
            return Err(HarmonyExpansionError::MissingDegree {
                index,
                degree: directive.value,
            });
        };
        if directive.kind == "subtract" {
            if directive.alter != 0 {
                return Err(HarmonyExpansionError::InvalidDegree { index });
            }
            tones.retain(|(degree, _, _)| *degree != directive.value);
        } else if let Some(tone) = tones
            .iter_mut()
            .find(|(degree, _, _)| *degree == directive.value)
        {
            tone.1 = original + i16::from(directive.alter);
            tone.2 = HarmonyToneOrigin::Altered;
        }
    }
    for directive in &chord.degrees {
        if directive.kind == "add" {
            let interval =
                DOMINANT_DEGREES[usize::from(directive.value - 1)] + i16::from(directive.alter);
            tones.push((directive.value, interval, HarmonyToneOrigin::Added));
        }
    }
    tones.sort_by_key(|&(degree, interval, _)| (degree, interval));
    tones.dedup_by_key(|tone| (tone.0, tone.1));
    if tones.is_empty() {
        return Err(HarmonyExpansionError::EmptyChord);
    }
    let mut expanded = Vec::with_capacity(tones.len());
    for (degree, semitones, origin) in tones {
        let letters = root_index + usize::from(degree - 1);
        let index = letters % 7;
        let octave_from_root = (letters / 7) as i8;
        let alter = NATURALS[root_index] + i16::from(root.alter) + semitones
            - NATURALS[index]
            - 12 * i16::from(octave_from_root);
        let role = match degree {
            1 => HarmonyToneRole::Root,
            2 if chord.kind == "suspended-second" => HarmonyToneRole::Suspension,
            4 if chord.kind == "suspended-fourth" => HarmonyToneRole::Suspension,
            3 => HarmonyToneRole::Third,
            5 => HarmonyToneRole::Fifth,
            6 => HarmonyToneRole::Sixth,
            7 => HarmonyToneRole::Seventh,
            _ => HarmonyToneRole::Tension,
        };
        expanded.push(ExpandedHarmonyTone {
            degree,
            semitones_above_root: semitones,
            spelling: HarmonyPitchClass {
                step: STEPS[index].clone(),
                alter: i8::try_from(alter).map_err(|_| HarmonyExpansionError::OutOfRange)?,
            },
            octave_from_root,
            role,
            origin,
        });
    }
    Ok(ExpandedHarmony {
        contract_version: HARMONY_EXPANSION_CONTRACT_VERSION,
        root,
        bass,
        tones: expanded,
    })
}
