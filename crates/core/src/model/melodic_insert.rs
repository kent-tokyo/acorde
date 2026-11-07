//! Conservative, explicitly keyed melodic insertions with source provenance.
use super::{
    chord_plan::{CHORD_PLAN_CONTRACT_VERSION, ChordPlan, ChordPlanEntry},
    duration::Duration,
    notation::{BeamState, KeySignature},
    pitch::{Pitch, Step},
    score::{Note, NoteAddr, ObjectStyleTarget, Score},
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

#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "kebab-case")]
pub enum MelodicInsertionOutcome {
    Invalid,
    NoChange,
    Partial,
    Applied,
}

impl MelodicInsertionResult {
    pub fn outcome(&self) -> MelodicInsertionOutcome {
        if self.score.is_none() {
            MelodicInsertionOutcome::Invalid
        } else if self.insertions.is_empty() {
            MelodicInsertionOutcome::NoChange
        } else if self.diagnostics.is_empty() {
            MelodicInsertionOutcome::Applied
        } else {
            MelodicInsertionOutcome::Partial
        }
    }
}

/// An explicit local key, effective from `start_measure` until the next entry.
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct MelodicLocalKey {
    pub start_measure: usize,
    pub key: KeySignature,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct MelodicInsertionSelection {
    pub source_anchor: NoteAddr,
    pub pitch: Pitch,
}

/// Optional musical context. Empty fields retain the conservative legacy behavior.
#[derive(Debug, Clone, Default, Serialize, Deserialize)]
#[serde(default)]
pub struct MelodicInsertionContext {
    pub chord_plan: Option<ChordPlan>,
    pub local_keys: Vec<MelodicLocalKey>,
    pub phrase_end_measures: Vec<usize>,
    pub selections: Vec<MelodicInsertionSelection>,
    pub max_insertions_per_measure: Option<usize>,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct MelodicInsertionCandidate {
    pub source_anchor: NoteAddr,
    pub beat: f64,
    pub pitch: Pitch,
    pub kind: MelodicInsertionKind,
    pub score: i32,
    pub reason_codes: Vec<String>,
    pub constraint_violations: Vec<String>,
    pub selected: bool,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct MelodicInsertionContextResult {
    pub insertion: MelodicInsertionResult,
    pub candidates: Vec<MelodicInsertionCandidate>,
    pub outcome: MelodicInsertionOutcome,
}

fn diagnostic(code: &str, measure: Option<usize>, message: &str) -> MelodicInsertionDiagnostic {
    MelodicInsertionDiagnostic {
        code: code.into(),
        measure,
        message: message.into(),
    }
}
fn safe_anchor(note: &Note) -> bool {
    !note.is_unpitched
        && !note.is_grace
        && !note.is_cue
        && !note.tie_start
        && !note.tie_end
        && note.pitch_tie_starts.is_empty()
        && note.pitch_tie_ends.is_empty()
        && !note.pitch_incoming_partial_ties.iter().any(|tied| *tied)
        && !note.pitch_tie_sources.iter().any(Option::is_some)
        && note.tuplet.is_none()
        && note.tuplet_boundary.is_none()
        && note.beam == BeamState::None
        && !note
            .articulations
            .contains(&super::notation::Articulation::Fermata)
        && note.tremolo_pair.is_none()
        && note.cross_staff.is_none()
        && note.guitar_technique.is_none()
        && !note.hidden
}

fn remap_address(address: &mut NoteAddr, mapping: &[MelodicSourceMapping]) {
    if let Some(found) = mapping.iter().find(|entry| entry.source == *address) {
        *address = found.output.clone();
    }
}

fn remap_references(score: &mut Score, mapping: &[MelodicSourceMapping]) {
    for spanner in &mut score.spanners {
        remap_address(&mut spanner.start, mapping);
        remap_address(&mut spanner.end, mapping);
    }
    for style in &mut score.object_style_overrides {
        if let ObjectStyleTarget::Note { address } = &mut style.target {
            remap_address(address, mapping);
        }
    }
    for note in score
        .parts
        .iter_mut()
        .flat_map(|part| &mut part.staves)
        .flat_map(|staff| &mut staff.measures)
        .flat_map(|measure| &mut measure.voices)
        .flatten()
    {
        if let Some(end) = note
            .chord_symbol
            .as_mut()
            .and_then(|harmony| harmony.range_end.as_mut())
        {
            remap_address(end, mapping);
        }
    }
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
fn candidate(
    left: &Pitch,
    right: &Pitch,
    key: &KeySignature,
    options: &MelodicInsertionOptions,
) -> Option<Pitch> {
    if left.microtone_cents != 0
        || right.microtone_cents != 0
        || !key.contains_pitch(left)
        || !key.contains_pitch(right)
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
    let pitch = pitch_at(position, key)?;
    Some(pitch)
}

fn constraint_violations(
    pitch: &Pitch,
    left: &Pitch,
    right: &Pitch,
    options: &MelodicInsertionOptions,
) -> Vec<String> {
    let midi = pitch.to_midi();
    let mut violations = Vec::new();
    if !(i16::from(options.min_midi)..=i16::from(options.max_midi)).contains(&midi) {
        violations.push("pitch-range".into());
    }
    if (midi - left.to_midi()).abs() > i16::from(options.max_leap_semitones)
        || (midi - right.to_midi()).abs() > i16::from(options.max_leap_semitones)
    {
        violations.push("leap-limit".into());
    }
    violations
}

fn chord_tones(entry: &ChordPlanEntry) -> Option<Vec<i16>> {
    let root = entry.chord.root.as_bytes();
    let base = match root.first()?.to_ascii_uppercase() {
        b'C' => 0,
        b'D' => 2,
        b'E' => 4,
        b'F' => 5,
        b'G' => 7,
        b'A' => 9,
        b'B' => 11,
        _ => return None,
    };
    let alter = match root.get(1..) {
        Some(b"") => 0,
        Some(b"#") => 1,
        Some(b"b") => -1,
        _ => return None,
    };
    let intervals: &[i16] = match entry.chord.kind.as_str() {
        "major" => &[0, 4, 7],
        "minor" => &[0, 3, 7],
        "diminished" => &[0, 3, 6],
        "augmented" => &[0, 4, 8],
        "dominant" | "dominant-seventh" => &[0, 4, 7, 10],
        "major-seventh" => &[0, 4, 7, 11],
        "minor-seventh" => &[0, 3, 7, 10],
        "half-diminished" => &[0, 3, 6, 10],
        "diminished-seventh" => &[0, 3, 6, 9],
        _ => return None,
    };
    Some(
        intervals
            .iter()
            .map(|interval| (base + alter + *interval).rem_euclid(12))
            .collect(),
    )
}

fn harmony_at(plan: &ChordPlan, measure: usize, beat: f64) -> Option<&ChordPlanEntry> {
    plan.entries
        .iter()
        .filter(|entry| {
            entry.position.offset.denominator != 0
                && (entry.position.measure_index < measure
                    || (entry.position.measure_index == measure
                        && entry.position.offset.beats() <= beat + 1e-8))
        })
        .max_by(|a, b| {
            a.position
                .measure_index
                .cmp(&b.position.measure_index)
                .then_with(|| {
                    a.position
                        .offset
                        .beats()
                        .total_cmp(&b.position.offset.beats())
                })
        })
}

struct CandidateEvaluation<'a> {
    base: Pitch,
    left: &'a Pitch,
    right: &'a Pitch,
    beat: f64,
    source: NoteAddr,
    options: &'a MelodicInsertionOptions,
    harmony: Option<&'a ChordPlanEntry>,
    resolves: bool,
    phrase_end: bool,
}

fn ranked_candidates(evaluation: CandidateEvaluation<'_>) -> Vec<MelodicInsertionCandidate> {
    let CandidateEvaluation {
        base,
        left,
        right,
        beat,
        source,
        options,
        harmony,
        resolves,
        phrase_end,
    } = evaluation;
    let chord_pitches = harmony.and_then(chord_tones);
    let mut candidates = Vec::new();
    for delta in if harmony.is_some() {
        [-1, 0, 1].as_slice()
    } else {
        [0].as_slice()
    } {
        let mut pitch = base.clone();
        pitch.alter += delta;
        let midi = pitch.to_midi();
        let violations = constraint_violations(&pitch, left, right, options);
        let chord_tone = chord_pitches
            .as_ref()
            .is_some_and(|tones| tones.contains(&midi.rem_euclid(12)));
        let weak_beat = (beat - beat.round()).abs() > 1e-8;
        let mut score = if chord_tone {
            10
        } else if harmony.is_none() {
            5
        } else if weak_beat && resolves {
            6
        } else {
            -5
        };
        let mut reason_codes = vec![
            if chord_tone {
                "chord-tone"
            } else if harmony.is_none() {
                "diatonic"
            } else if weak_beat && resolves {
                "resolved-weak-beat-nonchord-tone"
            } else {
                "unresolved-or-strong-beat-dissonance"
            }
            .to_string(),
        ];
        if *delta != 0 {
            score -= 2;
            reason_codes.push("chromatic-alteration".into());
        }
        if phrase_end && !chord_tone && harmony.is_some() {
            score -= 8;
            reason_codes.push("phrase-ending-nonchord-tone".into());
        }
        if !violations.is_empty() {
            score = -100;
        }
        candidates.push(MelodicInsertionCandidate {
            source_anchor: source.clone(),
            beat,
            pitch,
            kind: options.kind,
            score,
            reason_codes,
            constraint_violations: violations,
            selected: false,
        });
    }
    candidates.sort_by(|a, b| {
        b.score
            .cmp(&a.score)
            .then_with(|| {
                (a.pitch.alter - base.alter)
                    .abs()
                    .cmp(&(b.pitch.alter - base.alter).abs())
            })
            .then_with(|| a.pitch.to_midi().cmp(&b.pitch.to_midi()))
    });
    candidates
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
    insert_melodic_notes_impl(score, options, None).insertion
}

/// Evaluate explicitly positioned harmony and local keys, returning preview candidates and
/// a deterministic edited copy. A selected candidate is reapplied through `context.selections`.
pub fn insert_melodic_notes_with_context(
    score: &Score,
    options: MelodicInsertionOptions,
    context: &MelodicInsertionContext,
) -> MelodicInsertionContextResult {
    insert_melodic_notes_impl(score, options, Some(context))
}

fn insert_melodic_notes_impl(
    score: &Score,
    options: MelodicInsertionOptions,
    context: Option<&MelodicInsertionContext>,
) -> MelodicInsertionContextResult {
    let mut result = MelodicInsertionResult {
        score: None,
        options: options.clone(),
        source_mapping: Vec::new(),
        insertions: Vec::new(),
        diagnostics: Vec::new(),
    };
    let mut candidates = Vec::new();
    macro_rules! done {
        () => {
            return MelodicInsertionContextResult {
                outcome: result.outcome(),
                insertion: result,
                candidates,
            }
        };
    }
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
        done!();
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
        done!();
    };
    if options.end_measure > staff.measures.len() || options.voice >= 4 {
        result.diagnostics.push(diagnostic(
            "invalid-address",
            None,
            "measure or voice does not exist",
        ));
        done!();
    }
    if let Some(context) = context {
        let valid_keys = context.local_keys.iter().all(|entry| {
            entry.start_measure < options.end_measure
                && (-7..=7).contains(&entry.key.fifths)
                && matches!(entry.key.mode.as_str(), "major" | "minor")
        }) && context
            .local_keys
            .windows(2)
            .all(|pair| pair[0].start_measure < pair[1].start_measure);
        let valid_plan = context.chord_plan.as_ref().is_none_or(|plan| {
            plan.contract_version == CHORD_PLAN_CONTRACT_VERSION
                && !plan.entries.is_empty()
                && plan.entries.len() <= 512
                && plan.entries.iter().all(|entry| {
                    entry.position.offset.denominator != 0
                        && entry.offset_beats.is_finite()
                        && entry.offset_beats <= 32.0
                        && (entry.offset_beats - entry.position.offset.beats()).abs() < 1e-8
                })
                && plan.entries.windows(2).all(|pair| {
                    pair[0].position.measure_index < pair[1].position.measure_index
                        || (pair[0].position.measure_index == pair[1].position.measure_index
                            && pair[0].position.offset.beats() < pair[1].position.offset.beats())
                })
        });
        let valid_selections = context.selections.len() <= 128
            && context
                .selections
                .iter()
                .enumerate()
                .all(|(index, selection)| {
                    !context.selections[..index]
                        .iter()
                        .any(|previous| previous.source_anchor == selection.source_anchor)
                });
        if !valid_keys
            || !valid_plan
            || !valid_selections
            || context
                .max_insertions_per_measure
                .is_some_and(|limit| limit > 16)
        {
            result.diagnostics.push(diagnostic(
                "invalid-context",
                None,
                "invalid local key, chord plan or selection limit",
            ));
            done!();
        }
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
        let measure_start_insertions = result.insertions.len();
        let explicit_local_key = context.and_then(|ctx| {
            ctx.local_keys
                .iter()
                .rev()
                .find(|entry| entry.start_measure <= mi)
        });
        let local_key = explicit_local_key.map_or(&key, |entry| &entry.key);
        let incompatible = (explicit_local_key.is_none() && key != options.key)
            || measure
                .voices
                .iter()
                .enumerate()
                .any(|(i, v)| i != options.voice && v.iter().any(|n| !n.is_rest));
        if incompatible {
            result.diagnostics.push(diagnostic(
                "unsupported-measure",
                Some(mi),
                &format!(
                    "part {} staff {} measure {} voice {}: key differs or the measure contains polyphony",
                    options.part, options.staff, mi, options.voice
                ),
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
                        .filter(|n| {
                            !n.is_rest
                                && !n.is_grace
                                && !n.is_cue
                                && !n.is_unpitched
                                && n.pitches.len() == 1
                        })
                        .map(|n| (&note.pitches[0], &n.pitches[0]))
                }
                MelodicInsertionSource::FillRest if note.is_rest && i > 0 => voice
                    .get(i - 1)
                    .zip(voice.get(i + 1))
                    .filter(|(a, b)| {
                        !a.is_rest
                            && !b.is_rest
                            && !a.is_grace
                            && !b.is_grace
                            && !a.is_cue
                            && !b.is_cue
                            && !a.is_unpitched
                            && !b.is_unpitched
                            && a.pitches.len() == 1
                            && b.pitches.len() == 1
                    })
                    .map(|(a, b)| (&a.pitches[0], &b.pitches[0])),
                _ => None,
            };
            let attempted = !incompatible
                && result.insertions.len() < options.max_insertions
                && context.is_none_or(|ctx| {
                    ctx.max_insertions_per_measure.is_none_or(|limit| {
                        result.insertions.len() - measure_start_insertions < limit
                    })
                })
                && endpoints.is_some();
            let selected_pitch = if attempted && split.is_some() {
                endpoints.filter(|_| safe_anchor(note)).and_then(|(a, b)| {
                    let base = candidate(a, b, local_key, &options)?;
                    let Some(context) = context else {
                        return constraint_violations(&base, a, b, &options)
                            .is_empty()
                            .then_some(base);
                    };
                    let beat = voice[..i].iter().map(Note::beats).sum::<f64>()
                        + split.as_ref().map_or(0.0, |(short, _)| {
                            let mut shortened = note.clone();
                            shortened.duration = short.clone();
                            shortened.dot_count = 1;
                            shortened.beats()
                        });
                    let harmony = context.chord_plan.as_ref().and_then(|plan| harmony_at(plan, mi, beat));
                    if context.chord_plan.is_some() && harmony.is_none() {
                        result.diagnostics.push(diagnostic("uncovered-harmony", Some(mi),
                            &format!("part {} staff {} measure {} voice {} note {}: no explicit chord at insertion beat", options.part, options.staff, mi, options.voice, i)));
                        return None;
                    }
                    if harmony.is_some_and(|entry| chord_tones(entry).is_none()) {
                        result.diagnostics.push(diagnostic("unsupported-harmony", Some(mi),
                            &format!("part {} staff {} measure {} voice {} note {}: unsupported chord root or kind", options.part, options.staff, mi, options.voice, i)));
                        return None;
                    }
                    let right_beat = voice[..=i].iter().map(Note::beats).sum::<f64>();
                    let resolves = context.chord_plan.as_ref()
                        .and_then(|plan| harmony_at(plan, mi, right_beat))
                        .and_then(chord_tones)
                        .is_some_and(|tones| tones.contains(&b.to_midi().rem_euclid(12)));
                    let mut ranked = ranked_candidates(CandidateEvaluation {
                        base, left: a, right: b, beat, source: addr(i), options: &options,
                        harmony, resolves,
                        phrase_end: context.phrase_end_measures.contains(&mi) && i + 1 == voice.len() - 1,
                    });
                    let selected = if let Some(choice) = context.selections.iter().find(|choice| choice.source_anchor == addr(i)) {
                        ranked.iter().position(|item| item.pitch == choice.pitch && item.score >= 0
                            && item.constraint_violations.is_empty())
                    } else {
                        ranked.iter().position(|item| item.score >= 0
                            && item.constraint_violations.is_empty())
                    };
                    let pitch = selected.map(|index| {
                        ranked[index].selected = true;
                        ranked[index].pitch.clone()
                    });
                    if pitch.is_none() {
                        result.diagnostics.push(diagnostic("no-safe-candidate", Some(mi),
                            &format!("part {} staff {} measure {} voice {} note {}: no allowed pitch satisfies the musical context", options.part, options.staff, mi, options.voice, i)));
                    }
                    candidates.extend(ranked);
                    pitch
                })
            } else {
                None
            };
            if attempted && (split.is_none() || selected_pitch.is_none()) {
                result.diagnostics.push(diagnostic(
                    "ineligible-candidate",
                    Some(mi),
                    &format!(
                        "part {} staff {} measure {} voice {} note {}: interval, pitch limits, references or written duration cannot be edited safely",
                        options.part, options.staff, mi, options.voice, i
                    ),
                ));
            }
            if let (Some((short, added)), Some(pitch)) = (split, selected_pitch) {
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
    remap_references(&mut output, &result.source_mapping);
    if !validate(&output).is_valid() {
        result.diagnostics.push(diagnostic(
            "invalid-output",
            None,
            "generated score failed validation",
        ));
        result.source_mapping.clear();
        result.insertions.clear();
        done!();
    }
    result.score = Some(output);
    MelodicInsertionContextResult {
        outcome: result.outcome(),
        insertion: result,
        candidates,
    }
}
