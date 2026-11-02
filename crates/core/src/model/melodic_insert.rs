//! Conservative, explicitly keyed melodic insertions with source provenance.
use super::{
    duration::Duration,
    notation::{BeamState, KeySignature},
    pitch::{Pitch, Step},
    score::{Note, NoteAddr, Score},
    validate::validate,
};
use serde::{Deserialize, Serialize};
use std::collections::HashSet;
use uuid::Uuid;

#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "kebab-case")]
pub enum MelodicInsertionKind {
    Passing,
    UpperNeighbor,
    LowerNeighbor,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "kebab-case")]
pub enum MelodicInsertionSource {
    ShortenNote,
    FillRest,
}

/// Explicit inputs; the measure range is zero-based and end-exclusive.
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct MelodicInsertionOptions {
    pub part: usize,
    pub staff: usize,
    pub voice: usize,
    pub start_measure: usize,
    pub end_measure: usize,
    pub key: KeySignature,
    pub kind: MelodicInsertionKind,
    pub source: MelodicInsertionSource,
    pub max_insertions: usize,
    pub min_midi: u8,
    pub max_midi: u8,
    pub max_leap_semitones: u8,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct MelodicSourceMapping {
    pub source: NoteAddr,
    pub output: NoteAddr,
}
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct MelodicInsertion {
    pub source_anchor: NoteAddr,
    pub output_anchor: NoteAddr,
    pub inserted: NoteAddr,
    pub original_duration: Duration,
    pub original_dot_count: u8,
    pub pitch: Pitch,
}
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct MelodicInsertionDiagnostic {
    pub code: String,
    pub measure: Option<usize>,
    pub message: String,
}
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct MelodicInsertionResult {
    pub score: Option<Score>,
    pub options: MelodicInsertionOptions,
    pub source_mapping: Vec<MelodicSourceMapping>,
    pub insertions: Vec<MelodicInsertion>,
    pub diagnostics: Vec<MelodicInsertionDiagnostic>,
}

fn diagnostic(code: &str, measure: Option<usize>, message: &str) -> MelodicInsertionDiagnostic {
    MelodicInsertionDiagnostic {
        code: code.into(),
        measure,
        message: message.into(),
    }
}
fn plain(note: &Note) -> bool {
    !note.is_unpitched
        && !note.is_grace
        && !note.is_cue
        && !note.tie_start
        && !note.tie_end
        && note.pitch_tie_starts.is_empty()
        && note.pitch_tie_ends.is_empty()
        && note.tuplet.is_none()
        && note.tuplet_boundary.is_none()
        && note.beam == BeamState::None
        && note.articulations.is_empty()
        && note.tremolo_pair.is_none()
        && note.hairpin_start.is_none()
        && !note.hairpin_end
        && note.ottava_start.is_none()
        && !note.ottava_end
        && !note.pedal_start
        && !note.pedal_end
        && note.cross_staff.is_none()
        && !note.slur_start
        && !note.slur_end
        && !note.glissando_start
        && !note.glissando_end
        && !note.trill_line_start
        && !note.trill_line_end
}
fn step_index(step: &Step) -> i16 {
    match step {
        Step::C => 0,
        Step::D => 1,
        Step::E => 2,
        Step::F => 3,
        Step::G => 4,
        Step::A => 5,
        Step::B => 6,
    }
}
fn diatonic_position(p: &Pitch) -> i16 {
    i16::from(p.octave) * 7 + step_index(&p.step)
}
fn pitch_at(position: i16, key: &KeySignature) -> Option<Pitch> {
    let octave = i8::try_from(position.div_euclid(7)).ok()?;
    let step = match position.rem_euclid(7) {
        0 => Step::C,
        1 => Step::D,
        2 => Step::E,
        3 => Step::F,
        4 => Step::G,
        5 => Step::A,
        _ => Step::B,
    };
    let alter = key.alter_for_step(&step);
    Some(Pitch::with_alter(step, octave, alter))
}
fn candidate(left: &Pitch, right: &Pitch, options: &MelodicInsertionOptions) -> Option<Pitch> {
    if left.microtone_cents != 0
        || right.microtone_cents != 0
        || !options.key.contains_pitch(left)
        || !options.key.contains_pitch(right)
    {
        return None;
    }
    let (a, b) = (diatonic_position(left), diatonic_position(right));
    let position = match options.kind {
        MelodicInsertionKind::Passing if (b - a).abs() == 2 => a + (b - a).signum(),
        MelodicInsertionKind::UpperNeighbor if a == b && left.to_midi() == right.to_midi() => a + 1,
        MelodicInsertionKind::LowerNeighbor if a == b && left.to_midi() == right.to_midi() => a - 1,
        _ => return None,
    };
    let pitch = pitch_at(position, &options.key)?;
    let midi = pitch.to_midi();
    if !(i16::from(options.min_midi)..=i16::from(options.max_midi)).contains(&midi)
        || (midi - left.to_midi()).abs() > i16::from(options.max_leap_semitones)
        || (midi - right.to_midi()).abs() > i16::from(options.max_leap_semitones)
    {
        return None;
    }
    Some(pitch)
}
fn generated_id(
    score: &Score,
    anchor: &Note,
    measure: usize,
    index: usize,
    kind: MelodicInsertionKind,
    source: MelodicInsertionSource,
) -> String {
    let input = format!(
        "melodic-insertion-v1:{}:{}:{measure}:{index}:{kind:?}:{source:?}",
        score.id, anchor.id
    );
    let hash = |seed: u64| {
        input
            .bytes()
            .fold(seed, |h, b| (h ^ u64::from(b)).wrapping_mul(0x100000001b3))
    };
    Uuid::from_u128(
        (u128::from(hash(0xcbf29ce484222325)) << 64) | u128::from(hash(0x84222325cbf29ce4)),
    )
    .to_string()
}

/// Produce a new score without mutating the source. Only simple, monophonic spans are edited;
/// ineligible bars are left unchanged and diagnosed. Keep the input score for undo.
pub fn insert_melodic_notes(
    score: &Score,
    options: MelodicInsertionOptions,
) -> MelodicInsertionResult {
    let mut result = MelodicInsertionResult {
        score: None,
        options: options.clone(),
        source_mapping: Vec::new(),
        insertions: Vec::new(),
        diagnostics: Vec::new(),
    };
    if !validate(score).is_valid()
        || !(-7..=7).contains(&options.key.fifths)
        || !matches!(options.key.mode.as_str(), "major" | "minor")
        || options.min_midi > options.max_midi
        || options.max_midi > 127
        || options.max_leap_semitones == 0
        || options.max_leap_semitones > 12
        || options.max_insertions > 128
        || options.start_measure >= options.end_measure
        || options.end_measure - options.start_measure > 128
    {
        result.diagnostics.push(diagnostic(
            "invalid-options",
            None,
            "invalid score, range, key, pitch or density limit",
        ));
        return result;
    }
    let Some(staff) = score
        .parts
        .get(options.part)
        .and_then(|p| p.staves.get(options.staff))
    else {
        result.diagnostics.push(diagnostic(
            "invalid-address",
            None,
            "part or staff does not exist",
        ));
        return result;
    };
    if options.end_measure > staff.measures.len() || options.voice >= 4 {
        result.diagnostics.push(diagnostic(
            "invalid-address",
            None,
            "measure or voice does not exist",
        ));
        return result;
    }
    let has_harmony_ranges = score
        .parts
        .iter()
        .flat_map(|p| &p.staves)
        .flat_map(|s| &s.measures)
        .flat_map(|m| &m.voices)
        .flatten()
        .any(|n| {
            n.chord_symbol
                .as_ref()
                .is_some_and(|h| h.range_end.is_some())
        });
    if !score.spanners.is_empty()
        || !score.object_style_overrides.is_empty()
        || !score.views.is_empty()
        || has_harmony_ranges
    {
        result.diagnostics.push(diagnostic(
            "unsupported-references",
            None,
            "typed spans, harmony ranges, object overrides and score views require address remapping",
        ));
        return result;
    }
    let mut used: HashSet<String> = score
        .parts
        .iter()
        .flat_map(|p| &p.staves)
        .flat_map(|s| &s.measures)
        .flat_map(|m| &m.voices)
        .flatten()
        .map(|n| n.id.clone())
        .collect();
    let mut output = score.clone();
    let mut key = score.settings.key_signature.clone();
    for mi in 0..options.end_measure {
        let measure = &staff.measures[mi];
        if let Some(k) = &measure.key_sig {
            key = k.clone();
        }
        if mi < options.start_measure {
            continue;
        }
        let voice = &measure.voices[options.voice];
        let incompatible = key != options.key
            || measure
                .voices
                .iter()
                .enumerate()
                .any(|(i, v)| i != options.voice && v.iter().any(|n| !n.is_rest))
            || voice.iter().any(|n| !plain(n));
        if incompatible {
            result.diagnostics.push(diagnostic(
                "unsupported-measure",
                Some(mi),
                "key differs or the measure contains polyphony, ties, tuplets, articulations, beams or spans",
            ));
        }
        let mut next_voice = Vec::with_capacity(voice.len() + 1);
        let mut changed = false;
        for (i, note) in voice.iter().enumerate() {
            let addr = |note| NoteAddr {
                part: options.part,
                staff: options.staff,
                measure: mi,
                voice: options.voice,
                note,
            };
            let output_index = next_voice.len();
            result.source_mapping.push(MelodicSourceMapping {
                source: addr(i),
                output: addr(output_index),
            });
            let mut anchor = note.clone();
            let split = if note.dot_count == 0 {
                match note.duration {
                    Duration::Half => Some((Duration::Quarter, Duration::Eighth)),
                    Duration::Quarter => Some((Duration::Eighth, Duration::Sixteenth)),
                    _ => None,
                }
            } else {
                None
            };
            let endpoints = match options.source {
                MelodicInsertionSource::ShortenNote if !note.is_rest && note.pitches.len() == 1 => {
                    voice
                        .get(i + 1)
                        .filter(|n| !n.is_rest && n.pitches.len() == 1)
                        .map(|n| (&note.pitches[0], &n.pitches[0]))
                }
                MelodicInsertionSource::FillRest if note.is_rest && i > 0 => voice
                    .get(i - 1)
                    .zip(voice.get(i + 1))
                    .filter(|(a, b)| {
                        !a.is_rest && !b.is_rest && a.pitches.len() == 1 && b.pitches.len() == 1
                    })
                    .map(|(a, b)| (&a.pitches[0], &b.pitches[0])),
                _ => None,
            };
            let attempted = !incompatible
                && result.insertions.len() < options.max_insertions
                && endpoints.is_some();
            let candidate = if attempted {
                endpoints.and_then(|(a, b)| candidate(a, b, &options))
            } else {
                None
            };
            if attempted && (split.is_none() || candidate.is_none()) {
                result.diagnostics.push(diagnostic(
                    "ineligible-candidate",
                    Some(mi),
                    "the interval, pitch limits or written duration cannot be edited safely",
                ));
            }
            if let (Some((short, added)), Some(pitch)) = (split, candidate) {
                let id = generated_id(score, note, mi, i, options.kind, options.source);
                if !used.insert(id.clone()) {
                    result.diagnostics.push(diagnostic(
                        "id-conflict",
                        Some(mi),
                        "generated note identity conflicts with an existing note",
                    ));
                } else {
                    anchor.duration = short;
                    anchor.dot_count = 1;
                    let mut inserted = Note::new(pitch.clone(), added);
                    inserted.id = id;
                    result.insertions.push(MelodicInsertion {
                        source_anchor: addr(i),
                        output_anchor: addr(output_index),
                        inserted: addr(output_index + 1),
                        original_duration: note.duration.clone(),
                        original_dot_count: note.dot_count,
                        pitch,
                    });
                    next_voice.push(anchor);
                    next_voice.push(inserted);
                    changed = true;
                    continue;
                }
            }
            next_voice.push(anchor);
        }
        if !changed && !incompatible {
            result.diagnostics.push(diagnostic(
                "no-candidate",
                Some(mi),
                "no eligible insertion at the selected density and musical constraints",
            ));
        }
        output.parts[options.part].staves[options.staff].measures[mi].voices[options.voice] =
            next_voice;
    }
    if !validate(&output).is_valid() {
        result.diagnostics.push(diagnostic(
            "invalid-output",
            None,
            "generated score failed validation",
        ));
        result.source_mapping.clear();
        result.insertions.clear();
        return result;
    }
    result.score = Some(output);
    result
}
