//! Compose a melody from a motif, a chord progression and a phrase form.
//!
//! This module is behind the `unstable-arrangement` cargo feature. Its types may
//! change in a minor release.
//!
//! [`compose_melody`] writes a one-part score. Each measure takes a rhythm and a
//! contour of scale steps from the motif, or from a variant of it chosen by the
//! form. Notes on strong beats land on a tone of the chord sounding there, phrase
//! endings land on dominant or tonic tones, and every note stays inside the
//! requested range and leap limit. The same request, including its seed, always
//! produces the same melody. The report counts what the rules achieved, so a test
//! or host can check it rather than trust it.

use serde::{Deserialize, Serialize};

use super::duration::Duration;
use super::harmony_expansion::expand_harmony;
use super::notation::{ChordSymbol, KeySignature};
use super::pitch::Pitch;
use super::score::{Note, Score};

/// Version of the melody composition request and result contract.
pub const MELODY_COMPOSITION_CONTRACT_VERSION: u16 = 1;
/// Largest number of measures one request may compose.
pub const MELODY_COMPOSITION_MAX_MEASURES: u16 = 128;
/// Largest number of chord entries one request may supply.
pub const MELODY_COMPOSITION_MAX_CHORDS: usize = 512;

/// How phrases are grouped.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "kebab-case")]
#[non_exhaustive]
pub enum MelodyForm {
    /// Eight-bar period: a four-bar antecedent ending on a dominant tone, and a
    /// consequent that restates the motif and ends on the tonic.
    Period,
    /// Eight-bar sentence: a two-bar idea, its repetition, a fragmented
    /// continuation and a cadence on the tonic.
    Sentence,
}

/// One measure of motif: a rhythm in eighth notes and a contour in scale steps.
///
/// `steps[i]` is the scale-step move from the previous note to note `i`. The
/// first step is taken from the last note of the previous measure.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct MelodyMotif {
    pub rhythm_eighths: Vec<u8>,
    pub steps: Vec<i8>,
}

/// A chord and where it starts.
#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
pub struct MelodyChord {
    pub measure: usize,
    /// Offset from the start of the measure in eighth notes.
    pub offset_eighths: u8,
    pub chord: ChordSymbol,
}

/// What to compose.
#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
pub struct MelodyCompositionRequest {
    pub key: KeySignature,
    /// Supported meters: 2/4, 3/4, 4/4 and 6/8.
    pub time_numerator: u8,
    pub time_denominator: u8,
    /// A multiple of eight, up to [`MELODY_COMPOSITION_MAX_MEASURES`].
    pub measures: u16,
    pub form: MelodyForm,
    /// Chords in time order. The first must start at measure 0, offset 0.
    pub chords: Vec<MelodyChord>,
    /// One measure of motif. `None` derives one from the seed.
    #[serde(default)]
    pub motif: Option<MelodyMotif>,
    pub low_midi: u8,
    pub high_midi: u8,
    /// Largest move between adjacent notes, 2 to 12 semitones.
    pub max_leap_semitones: u8,
    pub seed: u64,
    #[serde(default = "default_tempo")]
    pub tempo_bpm: u16,
}

fn default_tempo() -> u16 {
    100
}

/// Which tones a phrase ending should land on.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "kebab-case")]
pub enum PhraseEndingTarget {
    /// Scale degree 2, 5 or 7.
    Dominant,
    /// Scale degree 1.
    Tonic,
}

/// The last note of one phrase.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct PhraseEnding {
    pub measure: usize,
    pub pitch_midi: u8,
    /// Scale degree 1-7, or 0 when the pitch is outside the scale.
    pub scale_degree: u8,
    pub target: PhraseEndingTarget,
    /// Whether the pitch is a target degree and a tone of the chord there.
    pub satisfied: bool,
}

/// What the composition rules achieved.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct MelodyCompositionReport {
    pub notes: usize,
    pub strong_beat_notes: usize,
    pub strong_beat_chord_tones: usize,
    pub lowest_midi: u8,
    pub highest_midi: u8,
    pub max_leap_semitones: u8,
    pub phrase_endings: Vec<PhraseEnding>,
}

/// The composed melody.
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct MelodyCompositionResult {
    pub contract_version: u16,
    /// One part, one staff, one voice.
    pub score: Score,
    /// The motif used, including one derived from the seed.
    pub motif: MelodyMotif,
    pub report: MelodyCompositionReport,
}

/// Why a request was rejected. No partial result is returned.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize, thiserror::Error)]
#[serde(rename_all = "kebab-case", tag = "kind")]
#[non_exhaustive]
pub enum MelodyCompositionError {
    #[error("meter {numerator}/{denominator} is not supported")]
    UnsupportedMeter { numerator: u8, denominator: u8 },
    #[error("measures must be a positive multiple of 8 up to {max}")]
    InvalidMeasureCount { max: u16 },
    #[error("key must have -7..=7 fifths and a major or minor mode")]
    InvalidKey,
    #[error("range {low_midi}..={high_midi} must span at least an octave")]
    InvalidRange { low_midi: u8, high_midi: u8 },
    #[error("max leap must be between 2 and 12 semitones")]
    InvalidLeap,
    #[error("chord {index} is out of order, outside the score, or unsupported")]
    InvalidChord { index: usize },
    #[error("motif rhythm must fill one measure and match its steps")]
    InvalidMotif,
}

struct Meter {
    eighths: u8,
    strong: &'static [u8],
    templates: &'static [&'static [u8]],
    ending: &'static [u8],
}

fn meter(numerator: u8, denominator: u8) -> Option<Meter> {
    Some(match (numerator, denominator) {
        (4, 4) => Meter {
            eighths: 8,
            strong: &[0, 4],
            templates: &[
                &[2, 2, 2, 2],
                &[2, 1, 1, 2, 2],
                &[3, 1, 2, 2],
                &[2, 2, 1, 1, 2],
            ],
            ending: &[2, 2, 4],
        },
        (3, 4) => Meter {
            eighths: 6,
            strong: &[0],
            templates: &[&[2, 2, 2], &[3, 1, 2], &[2, 1, 1, 2]],
            ending: &[2, 4],
        },
        (2, 4) => Meter {
            eighths: 4,
            strong: &[0],
            templates: &[&[2, 2], &[1, 1, 2], &[3, 1]],
            ending: &[4],
        },
        (6, 8) => Meter {
            eighths: 6,
            strong: &[0, 3],
            templates: &[&[3, 3], &[2, 1, 3], &[1, 1, 1, 3], &[3, 2, 1]],
            ending: &[6],
        },
        _ => return None,
    })
}

fn duration(eighths: u8) -> Option<(Duration, u8)> {
    Some(match eighths {
        1 => (Duration::Eighth, 0),
        2 => (Duration::Quarter, 0),
        3 => (Duration::Quarter, 1),
        4 => (Duration::Half, 0),
        6 => (Duration::Half, 1),
        8 => (Duration::Whole, 0),
        _ => return None,
    })
}

fn splitmix64(mut value: u64) -> u64 {
    value = value.wrapping_add(0x9e37_79b9_7f4a_7c15);
    value = (value ^ (value >> 30)).wrapping_mul(0xbf58_476d_1ce4_e5b9);
    value = (value ^ (value >> 27)).wrapping_mul(0x94d0_49bb_1331_11eb);
    value ^ (value >> 31)
}

struct Key {
    tonic: i16,
    scale: [i16; 7],
    flats: bool,
}

fn key(signature: &KeySignature) -> Option<Key> {
    if !(-7..=7).contains(&signature.fifths) {
        return None;
    }
    let major_tonic = (i16::from(signature.fifths) * 7).rem_euclid(12);
    let (tonic, scale) = match signature.mode.as_str() {
        "major" => (major_tonic, [0, 2, 4, 5, 7, 9, 11]),
        "minor" => ((major_tonic + 9) % 12, [0, 2, 3, 5, 7, 8, 10]),
        _ => return None,
    };
    Some(Key {
        tonic,
        scale,
        flats: signature.fifths < 0,
    })
}

impl Key {
    /// Scale degree 1-7 of a pitch, or 0 outside the scale. A raised seventh in
    /// minor counts as degree 7.
    fn degree(&self, pitch: i16) -> u8 {
        let interval = (pitch - self.tonic).rem_euclid(12);
        if let Some(index) = self.scale.iter().position(|&s| s == interval) {
            return index as u8 + 1;
        }
        if self.scale[6] == 10 && interval == 11 {
            return 7;
        }
        0
    }

    /// Pitch classes of the scale, adjusted to the chord: a scale tone a
    /// semitone from a chord tone outside the scale is replaced by it (G# for G
    /// under E7 in A minor).
    fn adjusted(&self, chord: &[i16]) -> Vec<i16> {
        self.scale
            .iter()
            .map(|&s| {
                let pc = (self.tonic + s) % 12;
                chord
                    .iter()
                    .copied()
                    .find(|&c| {
                        !self.scale.iter().any(|&t| (self.tonic + t) % 12 == c)
                            && ((c - pc).rem_euclid(12) == 1 || (pc - c).rem_euclid(12) == 1)
                    })
                    .unwrap_or(pc)
            })
            .collect()
    }
}

/// Move `steps` scale steps from `pitch` through the pitch classes `scale`.
fn step_from(pitch: i16, steps: i8, scale: &[i16]) -> i16 {
    let mut current = pitch;
    let direction: i16 = if steps < 0 { -1 } else { 1 };
    for _ in 0..steps.unsigned_abs() {
        let mut next = current + direction;
        while !scale.contains(&next.rem_euclid(12)) {
            next += direction;
        }
        current = next;
    }
    if steps == 0 && !scale.contains(&current.rem_euclid(12)) {
        return nearest(current, scale, current);
    }
    current
}

/// The pitch with a class in `classes` nearest to `target`; ties go toward `toward`.
fn nearest(target: i16, classes: &[i16], toward: i16) -> i16 {
    (0..=12)
        .flat_map(|distance| [target - distance, target + distance])
        .filter(|candidate| classes.contains(&candidate.rem_euclid(12)))
        .min_by_key(|&candidate| {
            (
                (candidate - target).abs(),
                (candidate - toward).abs(),
                candidate,
            )
        })
        .unwrap_or(target)
}

/// Bring `pitch` inside the range and within the leap limit of `previous`.
fn constrain(
    mut pitch: i16,
    previous: Option<i16>,
    low: i16,
    high: i16,
    max_leap: i16,
    classes: &[i16],
) -> i16 {
    if let Some(previous) = previous {
        while pitch - previous > max_leap && pitch - 12 >= low {
            pitch -= 12;
        }
        while previous - pitch > max_leap && pitch + 12 <= high {
            pitch += 12;
        }
    }
    while pitch > high {
        pitch -= 12;
    }
    while pitch < low {
        pitch += 12;
    }
    if let Some(previous) = previous
        && (pitch - previous).abs() > max_leap
    {
        // An octave fold could not help: take the class tone nearest to the
        // previous note that lies in range.
        let toward = previous;
        let mut best = None;
        for candidate in low..=high {
            if classes.contains(&candidate.rem_euclid(12))
                && (candidate - previous).abs() <= max_leap
            {
                let key = ((candidate - pitch).abs(), (candidate - toward).abs());
                if best.is_none_or(|(k, _)| key < k) {
                    best = Some((key, candidate));
                }
            }
        }
        if let Some((_, candidate)) = best {
            pitch = candidate;
        }
    }
    pitch
}

#[derive(Clone)]
struct Bar {
    rhythm: Vec<u8>,
    steps: Vec<i8>,
}

fn derive_motif(meter: &Meter, seed: u64) -> MelodyMotif {
    let mut state = splitmix64(seed);
    let rhythm = meter.templates[(state % meter.templates.len() as u64) as usize].to_vec();
    let mut steps = Vec::with_capacity(rhythm.len());
    const CHOICES: [i8; 7] = [-2, -1, -1, 1, 1, 2, 3];
    for _ in 0..rhythm.len() {
        state = splitmix64(state);
        let step = CHOICES[(state % CHOICES.len() as u64) as usize];
        steps.push(step);
    }
    MelodyMotif {
        rhythm_eighths: rhythm,
        steps,
    }
}

/// The eight bars of one form unit and the target of each phrase ending.
fn form_bars(
    form: MelodyForm,
    motif: &MelodyMotif,
    meter: &Meter,
) -> (Vec<Bar>, [PhraseEndingTarget; 2]) {
    let basic = Bar {
        rhythm: motif.rhythm_eighths.clone(),
        steps: motif.steps.clone(),
    };
    let inverted = Bar {
        rhythm: basic.rhythm.clone(),
        steps: basic.steps.iter().map(|s| -s).collect(),
    };
    // Fragment: the first half of the motif's notes, repeated a step lower.
    let half = fragment(&basic, meter.eighths);
    let ending = |last_step: i8| Bar {
        rhythm: meter.ending.to_vec(),
        steps: {
            let mut steps = vec![1; meter.ending.len()];
            steps[0] = -1;
            if let Some(last) = steps.last_mut() {
                *last = last_step;
            }
            steps
        },
    };
    match form {
        MelodyForm::Period => (
            vec![
                basic.clone(),
                inverted.clone(),
                basic.clone(),
                ending(1),
                basic.clone(),
                inverted,
                half,
                ending(-1),
            ],
            [PhraseEndingTarget::Dominant, PhraseEndingTarget::Tonic],
        ),
        MelodyForm::Sentence => (
            vec![
                basic.clone(),
                inverted.clone(),
                basic,
                inverted,
                half.clone(),
                half,
                ending(-1),
                ending(-1),
            ],
            [PhraseEndingTarget::Dominant, PhraseEndingTarget::Tonic],
        ),
    }
}

fn fragment(bar: &Bar, eighths: u8) -> Bar {
    let mut rhythm = Vec::new();
    let mut steps = Vec::new();
    let mut total = 0;
    for (length, step) in bar.rhythm.iter().zip(&bar.steps) {
        if total + length > eighths / 2 {
            break;
        }
        rhythm.push(*length);
        steps.push(*step);
        total += length;
    }
    if total * 2 != eighths || rhythm.is_empty() {
        return bar.clone();
    }
    let mut out_rhythm = rhythm.clone();
    out_rhythm.extend(&rhythm);
    let mut out_steps = steps.clone();
    let mut repeat = steps;
    repeat[0] = -1;
    out_steps.extend(repeat);
    Bar {
        rhythm: out_rhythm,
        steps: out_steps,
    }
}

fn chord_classes(chord: &ChordSymbol) -> Option<Vec<i16>> {
    let expanded = expand_harmony(chord).ok()?;
    let root = i16::from(expanded.root.pitch_class());
    Some(
        expanded
            .tones
            .iter()
            .map(|tone| (root + tone.semitones_above_root).rem_euclid(12))
            .collect(),
    )
}

fn validate(
    request: &MelodyCompositionRequest,
) -> Result<(Meter, Key, Vec<Vec<i16>>), MelodyCompositionError> {
    let meter = meter(request.time_numerator, request.time_denominator).ok_or(
        MelodyCompositionError::UnsupportedMeter {
            numerator: request.time_numerator,
            denominator: request.time_denominator,
        },
    )?;
    if request.measures == 0
        || !request.measures.is_multiple_of(8)
        || request.measures > MELODY_COMPOSITION_MAX_MEASURES
    {
        return Err(MelodyCompositionError::InvalidMeasureCount {
            max: MELODY_COMPOSITION_MAX_MEASURES,
        });
    }
    let key = key(&request.key).ok_or(MelodyCompositionError::InvalidKey)?;
    if request.high_midi > 127 || request.high_midi < request.low_midi.saturating_add(12) {
        return Err(MelodyCompositionError::InvalidRange {
            low_midi: request.low_midi,
            high_midi: request.high_midi,
        });
    }
    if !(2..=12).contains(&request.max_leap_semitones) {
        return Err(MelodyCompositionError::InvalidLeap);
    }
    if request.chords.is_empty() || request.chords.len() > MELODY_COMPOSITION_MAX_CHORDS {
        return Err(MelodyCompositionError::InvalidChord { index: 0 });
    }
    let mut classes = Vec::new();
    let mut previous = None;
    for (index, chord) in request.chords.iter().enumerate() {
        let position = (chord.measure, chord.offset_eighths);
        let ordered = match previous {
            None => position == (0, 0),
            Some(before) => position > before,
        };
        if !ordered
            || chord.measure >= usize::from(request.measures)
            || chord.offset_eighths >= meter.eighths
        {
            return Err(MelodyCompositionError::InvalidChord { index });
        }
        classes.push(
            chord_classes(&chord.chord).ok_or(MelodyCompositionError::InvalidChord { index })?,
        );
        previous = Some(position);
    }
    if let Some(motif) = &request.motif {
        let total: u32 = motif.rhythm_eighths.iter().map(|&e| u32::from(e)).sum();
        if total != u32::from(meter.eighths)
            || motif.rhythm_eighths.len() != motif.steps.len()
            || motif.rhythm_eighths.iter().any(|&e| duration(e).is_none())
            || motif.steps.iter().any(|s| !(-7..=7).contains(s))
        {
            return Err(MelodyCompositionError::InvalidMotif);
        }
    }
    Ok((meter, key, classes))
}

/// Compose a melody. See the module documentation.
pub fn compose_melody(
    request: &MelodyCompositionRequest,
) -> Result<MelodyCompositionResult, MelodyCompositionError> {
    let (meter, key, chord_classes) = validate(request)?;
    let motif = request
        .motif
        .clone()
        .unwrap_or_else(|| derive_motif(&meter, request.seed));
    let (unit, targets) = form_bars(request.form, &motif, &meter);
    let low = i16::from(request.low_midi);
    let high = i16::from(request.high_midi);
    let max_leap = i16::from(request.max_leap_semitones);
    let chord_at = |measure: usize, offset: u8| -> usize {
        request
            .chords
            .iter()
            .rposition(|c| (c.measure, c.offset_eighths) <= (measure, offset))
            .unwrap_or(0)
    };

    let mut score = Score::new(
        "Composed melody",
        request.tempo_bpm,
        request.time_numerator,
        request.time_denominator,
        request.key.fifths,
        u32::from(request.measures),
    );
    score.settings.key_signature.mode = request.key.mode.clone();

    let center = (low + high) / 2;
    let mut previous: Option<i16> = None;
    let mut report = MelodyCompositionReport {
        notes: 0,
        strong_beat_notes: 0,
        strong_beat_chord_tones: 0,
        lowest_midi: u8::MAX,
        highest_midi: 0,
        max_leap_semitones: 0,
        phrase_endings: Vec::new(),
    };

    for measure in 0..usize::from(request.measures) {
        let bar = &unit[measure % 8];
        let phrase_end = measure % 4 == 3;
        let target = targets[(measure % 8) / 4];
        let mut offset = 0u8;
        let mut voice = Vec::with_capacity(bar.rhythm.len());
        for (index, (&length, &step)) in bar.rhythm.iter().zip(&bar.steps).enumerate() {
            let chord = &chord_classes[chord_at(measure, offset)];
            let scale = key.adjusted(chord);
            let last = index + 1 == bar.rhythm.len();
            let mut pitch = match previous {
                None => nearest(center, &[key.tonic], center),
                Some(prev) => step_from(prev, step, &scale),
            };
            let strong = meter.strong.contains(&offset);
            if phrase_end && last {
                let degrees: &[usize] = match target {
                    PhraseEndingTarget::Dominant => &[1, 4, 6],
                    PhraseEndingTarget::Tonic => &[0],
                };
                let wanted: Vec<i16> = degrees
                    .iter()
                    .map(|&d| scale[d])
                    .filter(|pc| chord.contains(pc))
                    .collect();
                let classes = if wanted.is_empty() { chord } else { &wanted };
                pitch = nearest(pitch, classes, previous.unwrap_or(pitch));
                pitch = constrain(pitch, previous, low, high, max_leap, classes);
            } else if strong {
                pitch = nearest(pitch, chord, previous.unwrap_or(pitch));
                pitch = constrain(pitch, previous, low, high, max_leap, chord);
            } else {
                pitch = constrain(pitch, previous, low, high, max_leap, &scale);
            }
            if let Some(prev) = previous
                && (pitch - prev).abs() == 6
                && !(phrase_end && last)
            {
                // A tritone leap: take the nearest permitted tone that is not one.
                let classes: &[i16] = if strong { chord } else { &scale };
                if let Some(better) = (1..=5)
                    .flat_map(|d| [pitch - d, pitch + d])
                    .filter(|c| classes.contains(&c.rem_euclid(12)))
                    .filter(|c| (c - prev).abs() != 6 && (c - prev).abs() <= max_leap)
                    .find(|c| (low..=high).contains(c))
                {
                    pitch = better;
                }
            }
            if strong {
                report.strong_beat_notes += 1;
                if chord.contains(&pitch.rem_euclid(12)) {
                    report.strong_beat_chord_tones += 1;
                }
            }
            if phrase_end && last {
                let degree = key.degree(pitch);
                let target_degrees: &[u8] = match target {
                    PhraseEndingTarget::Dominant => &[2, 5, 7],
                    PhraseEndingTarget::Tonic => &[1],
                };
                report.phrase_endings.push(PhraseEnding {
                    measure,
                    pitch_midi: pitch as u8,
                    scale_degree: degree,
                    target,
                    satisfied: target_degrees.contains(&degree)
                        && chord.contains(&pitch.rem_euclid(12)),
                });
            }
            if let Some(prev) = previous {
                report.max_leap_semitones =
                    report.max_leap_semitones.max(pitch.abs_diff(prev) as u8);
            }
            report.lowest_midi = report.lowest_midi.min(pitch as u8);
            report.highest_midi = report.highest_midi.max(pitch as u8);
            report.notes += 1;
            let (value, dots) = duration(length).ok_or(MelodyCompositionError::InvalidMotif)?;
            let mut note = Note::new(Pitch::from_midi(pitch as u8, key.flats), value);
            note.dot_count = dots;
            voice.push(note);
            previous = Some(pitch);
            offset += length;
        }
        score.parts[0].staves[0].measures[measure].voices[0] = voice;
    }

    Ok(MelodyCompositionResult {
        contract_version: MELODY_COMPOSITION_CONTRACT_VERSION,
        score,
        motif,
        report,
    })
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::model::validate::validate as validate_score;

    fn chord(root: &str, kind: &str) -> ChordSymbol {
        serde_json::from_value(serde_json::json!({"root": root, "kind": kind})).unwrap()
    }

    /// I | IV | V | V | I | IV | V7 | I, one chord per bar.
    fn request(numerator: u8, denominator: u8, form: MelodyForm) -> MelodyCompositionRequest {
        let progression = [
            ("C", "major"),
            ("F", "major"),
            ("G", "major"),
            ("G", "major"),
            ("C", "major"),
            ("F", "major"),
            ("G", "dominant"),
            ("C", "major"),
        ];
        MelodyCompositionRequest {
            key: KeySignature {
                fifths: 0,
                mode: "major".into(),
            },
            time_numerator: numerator,
            time_denominator: denominator,
            measures: 8,
            form,
            chords: progression
                .iter()
                .enumerate()
                .map(|(measure, (root, kind))| MelodyChord {
                    measure,
                    offset_eighths: 0,
                    chord: chord(root, kind),
                })
                .collect(),
            motif: None,
            low_midi: 60,
            high_midi: 79,
            max_leap_semitones: 7,
            seed: 7,
            tempo_bpm: 100,
        }
    }

    fn pitches(score: &Score) -> Vec<i16> {
        score.parts[0].staves[0]
            .measures
            .iter()
            .flat_map(|m| m.voices[0].iter().map(|n| n.pitches[0].to_midi()))
            .collect()
    }

    #[test]
    fn every_meter_and_form_meets_its_rules() {
        for (n, d) in [(4, 4), (3, 4), (2, 4), (6, 8)] {
            for form in [MelodyForm::Period, MelodyForm::Sentence] {
                for seed in 0..16 {
                    let mut req = request(n, d, form);
                    req.seed = seed;
                    let result = compose_melody(&req).unwrap();
                    let label = format!("{n}/{d} {form:?} seed {seed}");
                    assert!(validate_score(&result.score).is_valid(), "{label}");
                    let r = &result.report;
                    assert_eq!(r.strong_beat_chord_tones, r.strong_beat_notes, "{label}");
                    assert!(r.lowest_midi >= 60 && r.highest_midi <= 79, "{label}");
                    assert!(r.max_leap_semitones <= 7, "{label}");
                    assert_eq!(r.phrase_endings.len(), 2, "{label}");
                    assert!(
                        r.phrase_endings.iter().all(|e| e.satisfied),
                        "{label}: {:?}",
                        r.phrase_endings
                    );
                    assert_eq!(r.phrase_endings[1].scale_degree, 1, "{label}");
                    let line = pitches(&result.score);
                    let tritones = line.windows(2).filter(|w| (w[1] - w[0]).abs() == 6).count();
                    // Only a forced phrase ending may arrive by tritone.
                    assert!(tritones <= 1, "{label}: {tritones} tritone leaps");
                }
            }
        }
    }

    #[test]
    fn the_same_request_reproduces_and_seeds_differ() {
        let req = request(4, 4, MelodyForm::Period);
        let a = compose_melody(&req).unwrap();
        let b = compose_melody(&req).unwrap();
        assert_eq!(pitches(&a.score), pitches(&b.score));
        assert_eq!(a.motif, b.motif);
        let distinct: std::collections::BTreeSet<_> = (0..16)
            .map(|seed| {
                let mut req = req.clone();
                req.seed = seed;
                pitches(&compose_melody(&req).unwrap().score)
            })
            .collect();
        assert!(distinct.len() >= 8, "{} distinct of 16", distinct.len());
    }

    #[test]
    fn a_supplied_motif_shapes_the_first_bar_and_the_consequent() {
        let mut req = request(4, 4, MelodyForm::Period);
        req.motif = Some(MelodyMotif {
            rhythm_eighths: vec![2, 2, 2, 2],
            steps: vec![0, 1, 1, 1],
        });
        let result = compose_melody(&req).unwrap();
        let bars = &result.score.parts[0].staves[0].measures;
        let first: Vec<_> = bars[0].voices[0]
            .iter()
            .map(|n| n.pitches[0].to_midi())
            .collect();
        // C E? No: the first note is the tonic, then one scale step up each,
        // with the strong beat on beat 3 snapped to a C-major chord tone.
        assert_eq!(first[0].rem_euclid(12), 0);
        assert!(first.windows(2).all(|w| w[1] > w[0]), "{first:?}");
        // Bar 5 restates the motif's rhythm.
        assert_eq!(bars[4].voices[0].len(), 4);
    }

    #[test]
    fn minor_keys_raise_the_leading_tone_under_the_dominant() {
        let mut req = request(4, 4, MelodyForm::Period);
        req.key = KeySignature {
            fifths: 0,
            mode: "minor".into(),
        };
        let progression = [
            ("A", "minor"),
            ("D", "minor"),
            ("E", "dominant"),
            ("E", "dominant"),
            ("A", "minor"),
            ("D", "minor"),
            ("E", "dominant"),
            ("A", "minor"),
        ];
        for (entry, (root, kind)) in req.chords.iter_mut().zip(progression) {
            entry.chord = chord(root, kind);
        }
        req.low_midi = 57;
        req.high_midi = 76;
        for seed in 0..16 {
            req.seed = seed;
            let result = compose_melody(&req).unwrap();
            let bars = &result.score.parts[0].staves[0].measures;
            for measure in [2, 3, 6] {
                for note in &bars[measure].voices[0] {
                    assert_ne!(
                        note.pitches[0].to_midi().rem_euclid(12),
                        7,
                        "no G natural under E7 (seed {seed})"
                    );
                }
            }
            assert!(
                result.report.phrase_endings.iter().all(|e| e.satisfied),
                "seed {seed}: {:?}",
                result.report.phrase_endings
            );
        }
    }

    #[test]
    fn invalid_requests_are_rejected() {
        let mut req = request(5, 8, MelodyForm::Period);
        assert!(matches!(
            compose_melody(&req),
            Err(MelodyCompositionError::UnsupportedMeter { .. })
        ));
        req.time_numerator = 4;
        req.time_denominator = 4;
        req.measures = 6;
        assert!(matches!(
            compose_melody(&req),
            Err(MelodyCompositionError::InvalidMeasureCount { .. })
        ));
        req.measures = 8;
        req.high_midi = 65;
        assert!(matches!(
            compose_melody(&req),
            Err(MelodyCompositionError::InvalidRange { .. })
        ));
        req.high_midi = 79;
        req.chords.swap(0, 1);
        assert!(matches!(
            compose_melody(&req),
            Err(MelodyCompositionError::InvalidChord { index: 0 })
        ));
        req.chords.swap(0, 1);
        req.motif = Some(MelodyMotif {
            rhythm_eighths: vec![2, 2, 2],
            steps: vec![0, 1, 1],
        });
        assert!(matches!(
            compose_melody(&req),
            Err(MelodyCompositionError::InvalidMotif)
        ));
    }
}
