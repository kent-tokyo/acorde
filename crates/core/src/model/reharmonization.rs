//! Automatic reharmonization candidates for an existing chord plan.
//!
//! This module is behind the `unstable-arrangement` cargo feature. Its types may
//! change in a minor release.
//!
//! [`suggest_reharmonizations`] looks at every pair of adjacent chord-plan entries
//! inside an editable measure range. When the second entry is a major- or
//! minor-type arrival, it proposes replacing the first entry with an approach
//! chord that resolves to it:
//!
//! - **secondary dominant**: V7 of the arrival (E7 before Am, D7 before G);
//! - **tritone substitution**: the dominant a semitone above the arrival (Db7 before C);
//! - **two-five**: split the first entry's span into ii7 (or iiø7 before a minor
//!   arrival) and V7 (Dm7-G7 before C, Bø7-E7 before Am).
//!
//! The arrival and every entry outside the edited span stay unchanged. Each
//! candidate records how the melody notes under the new chords fit them (chord
//! tone, available tension, or avoid note) and is ranked by that fit.
//! [`apply_reharmonizations`] writes chosen candidates into a new plan.

use serde::{Deserialize, Serialize};

use super::chord_plan::{
    BeatFraction, ChordPlan, ChordPlanEntry, ChordPlanOrigin, ChordPlanPosition, note_length,
};
use super::harmony_expansion::{ExpandedHarmony, HarmonyPitchClass, expand_harmony};
use super::notation::ChordSymbol;
use super::pitch::Step;
use super::score::Score;

/// Version of the reharmonization request and result contract.
pub const REHARMONIZATION_CONTRACT_VERSION: u16 = 1;
/// Largest number of candidates one request may return.
pub const REHARMONIZATION_MAX_CANDIDATES: u8 = 64;
/// Largest chord plan one request may read.
pub const REHARMONIZATION_MAX_ENTRIES: usize = 512;

/// A reharmonization technique.
#[derive(Debug, Clone, Copy, PartialEq, Eq, PartialOrd, Ord, Serialize, Deserialize)]
#[serde(rename_all = "kebab-case")]
#[non_exhaustive]
pub enum ReharmonizationTechnique {
    SecondaryDominant,
    TritoneSubstitution,
    TwoFive,
}

const ALL_TECHNIQUES: [ReharmonizationTechnique; 3] = [
    ReharmonizationTechnique::SecondaryDominant,
    ReharmonizationTechnique::TritoneSubstitution,
    ReharmonizationTechnique::TwoFive,
];

/// What to search in [`suggest_reharmonizations`].
#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
pub struct ReharmonizationRequest {
    pub plan: ChordPlan,
    /// Part whose notes are checked against the new chords.
    pub melody_part: usize,
    /// Physical measures whose harmony may change, `start..end`.
    pub start_measure: usize,
    pub end_measure_exclusive: usize,
    /// Measures inside the range that must keep their harmony.
    #[serde(default)]
    pub locked_measures: Vec<usize>,
    /// Plan entry indices that must keep their chord.
    #[serde(default)]
    pub locked_entries: Vec<usize>,
    /// Techniques to try. Empty tries all.
    #[serde(default)]
    pub techniques: Vec<ReharmonizationTechnique>,
    /// Most candidates to return, 1 to [`REHARMONIZATION_MAX_CANDIDATES`].
    pub max_candidates: u8,
}

/// One melody note under a new chord that is not a chord tone or available tension.
#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
pub struct ReharmonizationAvoidNote {
    pub measure: usize,
    pub offset_beats: f64,
    pub duration_beats: f64,
    /// Sounding MIDI pitch of the melody note.
    pub pitch_midi: i16,
    /// The new chord it sounds against.
    pub chord: ChordSymbol,
}

/// How the melody fits the new chords of one candidate.
#[derive(Debug, Clone, Default, PartialEq, Serialize, Deserialize)]
pub struct ReharmonizationMelodyFit {
    pub melody_notes: usize,
    pub chord_tones: usize,
    pub tensions: usize,
    /// Notes that are neither chord tones nor available tensions.
    pub avoid_notes: Vec<ReharmonizationAvoidNote>,
    /// Total beats of avoid notes.
    pub avoid_beats: f64,
}

/// One chord written by a candidate.
#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
pub struct ReharmonizationChord {
    pub position: ChordPlanPosition,
    pub chord: ChordSymbol,
}

/// One proposed change: the chord(s) that replace one plan entry.
#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
pub struct ReharmonizationCandidate {
    /// `<technique>-<replaced entry>`, unique within a result.
    pub id: String,
    pub technique: ReharmonizationTechnique,
    /// Plan entry whose chord is replaced.
    pub replaced_entry: usize,
    pub original: ChordSymbol,
    /// The replacement chords in time order. The first sits at the replaced
    /// entry's position.
    pub chords: Vec<ReharmonizationChord>,
    /// Plan entry the new harmony resolves to. Its chord is unchanged.
    pub arrival_entry: usize,
    pub arrival: ChordSymbol,
    /// Root motion from the last new chord to the arrival, in semitones
    /// within -6..=5 (−7 ≡ +5 is a fifth down; −1 a semitone down).
    pub resolution_root_motion: i8,
    pub melody: ReharmonizationMelodyFit,
}

/// Result of [`suggest_reharmonizations`].
#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
pub struct ReharmonizationResult {
    pub contract_version: u16,
    /// Candidates ranked by avoid-note beats, then avoid-note count, technique
    /// and position. At most `max_candidates`.
    pub candidates: Vec<ReharmonizationCandidate>,
    /// Eligible candidates found before truncation.
    pub eligible: usize,
}

/// Why a reharmonization request was rejected. No partial result is returned.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize, thiserror::Error)]
#[serde(rename_all = "kebab-case", tag = "kind")]
#[non_exhaustive]
pub enum ReharmonizationError {
    #[error("melody part {part} does not exist")]
    UnknownPart { part: usize },
    #[error("measure range {start}..{end} is empty or outside the score")]
    InvalidRange { start: usize, end: usize },
    #[error("max_candidates must be between 1 and {max}")]
    InvalidCount { max: u8 },
    #[error("the plan has more than {max} entries")]
    PlanTooLarge { max: usize },
    #[error("plan entries are not in strictly increasing position order at entry {entry}")]
    UnorderedPlan { entry: usize },
    #[error("candidate {id} does not match the plan")]
    CandidateMismatch { id: String },
    #[error("candidates {first} and {second} replace or resolve to the same entry")]
    OverlappingCandidates { first: String, second: String },
}

fn key(position: &ChordPlanPosition) -> (usize, f64) {
    (position.measure_index, position.offset.beats())
}

fn before(a: &ChordPlanPosition, b: &ChordPlanPosition) -> bool {
    let (ma, oa) = key(a);
    let (mb, ob) = key(b);
    ma < mb || (ma == mb && oa < ob)
}

const STEPS: [Step; 7] = [
    Step::C,
    Step::D,
    Step::E,
    Step::F,
    Step::G,
    Step::A,
    Step::B,
];

fn step_index(step: &Step) -> usize {
    STEPS.iter().position(|s| s == step).unwrap_or(0)
}

/// Spell the pitch `steps` letters and `semitones` above `root`.
fn spell(root: &HarmonyPitchClass, steps: usize, semitones: i16) -> Option<HarmonyPitchClass> {
    let absolute = step_index(&root.step) + steps;
    let step = STEPS[absolute % 7].clone();
    let natural = i16::from(step.to_semitone()) + 12 * (absolute / 7) as i16;
    let target = i16::from(root.step.to_semitone()) + i16::from(root.alter) + semitones;
    let alter = target - natural;
    (-2..=2).contains(&alter).then_some(HarmonyPitchClass {
        step,
        alter: alter as i8,
    })
}

fn name(pitch: &HarmonyPitchClass) -> String {
    let accidental = if pitch.alter < 0 { "b" } else { "#" };
    format!(
        "{}{}",
        pitch.step.to_char(),
        accidental.repeat(usize::from(pitch.alter.unsigned_abs()))
    )
}

fn chord(root: &HarmonyPitchClass, kind: &str) -> Option<(ChordSymbol, ExpandedHarmony)> {
    let symbol = ChordSymbol {
        root: name(root),
        kind: kind.into(),
        bass: None,
        placement: None,
        extender: false,
        harmonic_degree: None,
        harmony_function: None,
        harmony_type: None,
        chord_ref: None,
        written: None,
        range_end: None,
        degrees: Vec::new(),
    };
    let expanded = expand_harmony(&symbol).ok()?;
    Some((symbol, expanded))
}

/// Major- or minor-type arrival: a third and a perfect fifth above the root.
/// Returns `true` for a minor third.
fn arrival_quality(expanded: &ExpandedHarmony) -> Option<bool> {
    let fifth = expanded
        .tones
        .iter()
        .any(|tone| tone.degree == 5 && tone.semitones_above_root.rem_euclid(12) == 7);
    let third = expanded
        .tones
        .iter()
        .find(|tone| tone.degree == 3)
        .map(|tone| tone.semitones_above_root.rem_euclid(12));
    match (fifth, third) {
        (true, Some(3)) => Some(true),
        (true, Some(4)) => Some(false),
        _ => None,
    }
}

/// Semitones above the root that count as available tensions for `kind`.
fn tensions(kind: &str) -> &'static [i16] {
    match kind {
        "dominant" => &[1, 2, 3, 6, 8, 9],
        "minor-seventh" => &[2, 5, 9],
        "half-diminished" => &[2, 5, 8],
        _ => &[],
    }
}

#[derive(Debug, Clone)]
struct MelodyNote {
    measure: usize,
    start: f64,
    end: f64,
    pitch: i16,
}

fn melody_notes(score: &Score, part: usize) -> Vec<MelodyNote> {
    let mut notes = Vec::new();
    for staff in &score.parts[part].staves {
        for (measure, bar) in staff.measures.iter().enumerate() {
            for voice in &bar.voices {
                let mut offset = BeatFraction::ZERO;
                for note in voice {
                    let Some(length) = note_length(note) else {
                        break;
                    };
                    if !note.is_rest && !note.is_unpitched && length.numerator > 0 {
                        for pitch in &note.pitches {
                            notes.push(MelodyNote {
                                measure,
                                start: offset.beats(),
                                end: offset.beats() + length.beats(),
                                pitch: pitch.to_midi(),
                            });
                        }
                    }
                    match offset.add(length) {
                        Some(next) => offset = next,
                        None => break,
                    }
                }
            }
        }
    }
    notes
}

/// Melody notes sounding in `[from, to)`, with the part of each note inside it.
fn notes_in<'a>(
    notes: &'a [MelodyNote],
    from: &ChordPlanPosition,
    to: Option<&ChordPlanPosition>,
) -> impl Iterator<Item = &'a MelodyNote> {
    let (from_measure, from_offset) = key(from);
    let to = to.map(key);
    notes.iter().filter(move |note| {
        let starts_before_end = match to {
            Some((measure, offset)) => {
                note.measure < measure || (note.measure == measure && note.start < offset)
            }
            None => true,
        };
        let ends_after_start =
            note.measure > from_measure || (note.measure == from_measure && note.end > from_offset);
        starts_before_end && ends_after_start
    })
}

fn fit(
    notes: &[MelodyNote],
    chords: &[(ReharmonizationChord, ExpandedHarmony)],
    end: Option<&ChordPlanPosition>,
) -> ReharmonizationMelodyFit {
    let mut result = ReharmonizationMelodyFit::default();
    for (index, (written, expanded)) in chords.iter().enumerate() {
        let to = chords.get(index + 1).map(|(c, _)| &c.position).or(end);
        let root = i16::from(expanded.root.pitch_class());
        let tones: Vec<i16> = expanded
            .tones
            .iter()
            .map(|tone| tone.semitones_above_root.rem_euclid(12))
            .collect();
        let available = tensions(&written.chord.kind);
        let (from_measure, from_offset) = key(&written.position);
        let to_key = to.map(key);
        for note in notes_in(notes, &written.position, to) {
            result.melody_notes += 1;
            let start = if note.measure == from_measure {
                note.start.max(from_offset)
            } else {
                note.start
            };
            let end = match to_key {
                Some((measure, offset)) if note.measure == measure => note.end.min(offset),
                _ => note.end,
            };
            let interval = (note.pitch - root).rem_euclid(12);
            if tones.contains(&interval) {
                result.chord_tones += 1;
            } else if available.contains(&interval) {
                result.tensions += 1;
            } else {
                let duration = end - start;
                result.avoid_beats += duration;
                result.avoid_notes.push(ReharmonizationAvoidNote {
                    measure: note.measure,
                    offset_beats: note.start,
                    duration_beats: duration,
                    pitch_midi: note.pitch,
                    chord: written.chord.clone(),
                });
            }
        }
    }
    result
}

fn validate(score: &Score, request: &ReharmonizationRequest) -> Result<(), ReharmonizationError> {
    if request.melody_part >= score.parts.len() {
        return Err(ReharmonizationError::UnknownPart {
            part: request.melody_part,
        });
    }
    let measures = score
        .parts
        .first()
        .and_then(|part| part.staves.first())
        .map_or(0, |staff| staff.measures.len());
    if request.start_measure >= request.end_measure_exclusive
        || request.end_measure_exclusive > measures
    {
        return Err(ReharmonizationError::InvalidRange {
            start: request.start_measure,
            end: request.end_measure_exclusive,
        });
    }
    if request.max_candidates == 0 || request.max_candidates > REHARMONIZATION_MAX_CANDIDATES {
        return Err(ReharmonizationError::InvalidCount {
            max: REHARMONIZATION_MAX_CANDIDATES,
        });
    }
    if request.plan.entries.len() > REHARMONIZATION_MAX_ENTRIES {
        return Err(ReharmonizationError::PlanTooLarge {
            max: REHARMONIZATION_MAX_ENTRIES,
        });
    }
    for (entry, pair) in request.plan.entries.windows(2).enumerate() {
        if !before(&pair[0].position, &pair[1].position) {
            return Err(ReharmonizationError::UnorderedPlan { entry: entry + 1 });
        }
    }
    Ok(())
}

/// Whether the span of entry `index` (up to the next entry) is editable.
fn editable(request: &ReharmonizationRequest, index: usize) -> bool {
    if request.locked_entries.contains(&index) {
        return false;
    }
    let entries = &request.plan.entries;
    let start = entries[index].position.measure_index;
    let next = &entries[index + 1].position;
    // The last measure the span touches.
    let last = if next.offset.numerator == 0 {
        next.measure_index.saturating_sub(1)
    } else {
        next.measure_index
    };
    let last = last.max(start);
    (start..=last).all(|measure| {
        measure >= request.start_measure
            && measure < request.end_measure_exclusive
            && !request.locked_measures.contains(&measure)
    })
}

/// The midpoint of entry `index`'s span when it lies in one measure and lasts
/// at least two beats on the 1/64-beat grid.
fn midpoint(score: &Score, entries: &[ChordPlanEntry], index: usize) -> Option<ChordPlanPosition> {
    let start = &entries[index].position;
    let next = &entries[index + 1].position;
    if !start.offset.on_preset_grid() || !next.offset.on_preset_grid() {
        return None;
    }
    let to_64 = |beats: f64| (beats * 64.0).round() as u64;
    let start_64 = to_64(start.offset.beats());
    let end_64 = if next.measure_index == start.measure_index {
        to_64(next.offset.beats())
    } else if next.measure_index == start.measure_index + 1 && next.offset.numerator == 0 {
        let beats = score.playback_measure_beats(start.measure_index);
        let end = to_64(beats);
        ((end as f64 - beats * 64.0).abs() < 1e-9).then_some(end)?
    } else {
        return None;
    };
    let length = end_64.checked_sub(start_64)?;
    if length < 128 || length % 2 != 0 {
        return None;
    }
    Some(ChordPlanPosition {
        measure_index: start.measure_index,
        offset: BeatFraction::new(start_64 + length / 2, 64)?,
    })
}

fn root_motion(from: &HarmonyPitchClass, to: &HarmonyPitchClass) -> i8 {
    let motion = (i16::from(to.pitch_class()) - i16::from(from.pitch_class())).rem_euclid(12);
    (if motion > 5 { motion - 12 } else { motion }) as i8
}

fn same_chord(a: &ChordSymbol, a_expanded: &ExpandedHarmony, b: &ChordSymbol) -> bool {
    expand_harmony(b).is_ok_and(|other| {
        other.root.pitch_class() == a_expanded.root.pitch_class()
            && a.kind == b.kind
            && a.bass == b.bass
    })
}

/// List reharmonization candidates for the editable span of `request.plan`.
pub fn suggest_reharmonizations(
    score: &Score,
    request: &ReharmonizationRequest,
) -> Result<ReharmonizationResult, ReharmonizationError> {
    validate(score, request)?;
    let notes = melody_notes(score, request.melody_part);
    let techniques: &[ReharmonizationTechnique] = if request.techniques.is_empty() {
        &ALL_TECHNIQUES
    } else {
        &request.techniques
    };
    let entries = &request.plan.entries;
    let mut candidates = Vec::new();
    for index in 0..entries.len().saturating_sub(1) {
        if !editable(request, index) {
            continue;
        }
        let arrival = &entries[index + 1];
        let Ok(arrival_expanded) = expand_harmony(&arrival.chord) else {
            continue;
        };
        let Some(minor) = arrival_quality(&arrival_expanded) else {
            continue;
        };
        let target = &arrival_expanded.root;
        let original = &entries[index];
        let end = Some(&arrival.position);
        let mut techniques = techniques.to_vec();
        techniques.sort();
        techniques.dedup();
        for technique in techniques {
            let written: Option<Vec<(ReharmonizationChord, ExpandedHarmony)>> = match technique {
                ReharmonizationTechnique::SecondaryDominant => spell(target, 4, 7)
                    .and_then(|root| chord(&root, "dominant"))
                    .map(|(symbol, expanded)| {
                        vec![(
                            ReharmonizationChord {
                                position: original.position.clone(),
                                chord: symbol,
                            },
                            expanded,
                        )]
                    }),
                ReharmonizationTechnique::TritoneSubstitution => spell(target, 1, 1)
                    .and_then(|root| chord(&root, "dominant"))
                    .map(|(symbol, expanded)| {
                        vec![(
                            ReharmonizationChord {
                                position: original.position.clone(),
                                chord: symbol,
                            },
                            expanded,
                        )]
                    }),
                ReharmonizationTechnique::TwoFive => {
                    let two_kind = if minor {
                        "half-diminished"
                    } else {
                        "minor-seventh"
                    };
                    match (
                        midpoint(score, entries, index),
                        spell(target, 1, 2).and_then(|root| chord(&root, two_kind)),
                        spell(target, 4, 7).and_then(|root| chord(&root, "dominant")),
                    ) {
                        (Some(mid), Some((two, two_x)), Some((five, five_x))) => Some(vec![
                            (
                                ReharmonizationChord {
                                    position: original.position.clone(),
                                    chord: two,
                                },
                                two_x,
                            ),
                            (
                                ReharmonizationChord {
                                    position: mid,
                                    chord: five,
                                },
                                five_x,
                            ),
                        ]),
                        _ => None,
                    }
                }
            };
            let Some(written) = written else {
                continue;
            };
            // A one-chord edit that reproduces the original is not a change.
            if written.len() == 1 && same_chord(&written[0].0.chord, &written[0].1, &original.chord)
            {
                continue;
            }
            let melody = fit(&notes, &written, end);
            let Some(last_root) = written.last().map(|(_, x)| x.root.clone()) else {
                continue;
            };
            candidates.push(ReharmonizationCandidate {
                id: format!("{}-{index}", technique_name(technique)),
                technique,
                replaced_entry: index,
                original: original.chord.clone(),
                chords: written.into_iter().map(|(c, _)| c).collect(),
                arrival_entry: index + 1,
                arrival: arrival.chord.clone(),
                resolution_root_motion: root_motion(&last_root, target),
                melody,
            });
        }
    }
    candidates.sort_by(|a, b| {
        a.melody
            .avoid_beats
            .total_cmp(&b.melody.avoid_beats)
            .then(a.melody.avoid_notes.len().cmp(&b.melody.avoid_notes.len()))
            .then(a.technique.cmp(&b.technique))
            .then(a.replaced_entry.cmp(&b.replaced_entry))
    });
    let eligible = candidates.len();
    candidates.truncate(usize::from(request.max_candidates));
    Ok(ReharmonizationResult {
        contract_version: REHARMONIZATION_CONTRACT_VERSION,
        candidates,
        eligible,
    })
}

fn technique_name(technique: ReharmonizationTechnique) -> &'static str {
    match technique {
        ReharmonizationTechnique::SecondaryDominant => "secondary-dominant",
        ReharmonizationTechnique::TritoneSubstitution => "tritone-substitution",
        ReharmonizationTechnique::TwoFive => "two-five",
    }
}

/// Write `candidates` into a copy of `plan`. Each candidate must have been made
/// from this plan; no two may replace the same entry, and no candidate may
/// replace another's arrival. Replaced and inserted entries are marked
/// [`ChordPlanOrigin::GeneratedReharmonization`] and keep the replaced entry's
/// sources.
pub fn apply_reharmonizations(
    plan: &ChordPlan,
    candidates: &[ReharmonizationCandidate],
) -> Result<ChordPlan, ReharmonizationError> {
    for (i, a) in candidates.iter().enumerate() {
        let entry = plan.entries.get(a.replaced_entry);
        let arrival = plan.entries.get(a.arrival_entry);
        let matches = entry.is_some_and(|e| e.chord == a.original)
            && arrival.is_some_and(|e| e.chord == a.arrival)
            && a.arrival_entry == a.replaced_entry + 1
            && a.chords.first().map(|c| &c.position) == entry.map(|e| &e.position)
            && a.chords
                .windows(2)
                .all(|w| before(&w[0].position, &w[1].position))
            && a.chords.last().zip(arrival).is_some_and(|(last, arrival)| {
                before(&last.position, &arrival.position) || a.chords.len() == 1
            });
        if !matches {
            return Err(ReharmonizationError::CandidateMismatch { id: a.id.clone() });
        }
        for b in &candidates[..i] {
            if a.replaced_entry == b.replaced_entry
                || a.replaced_entry == b.arrival_entry
                || b.replaced_entry == a.arrival_entry
            {
                return Err(ReharmonizationError::OverlappingCandidates {
                    first: b.id.clone(),
                    second: a.id.clone(),
                });
            }
        }
    }
    let mut order: Vec<&ReharmonizationCandidate> = candidates.iter().collect();
    order.sort_by_key(|c| std::cmp::Reverse(c.replaced_entry));
    let mut result = plan.clone();
    for candidate in order {
        let template = result.entries[candidate.replaced_entry].clone();
        let replacement: Vec<ChordPlanEntry> = candidate
            .chords
            .iter()
            .map(|written| ChordPlanEntry {
                position: written.position.clone(),
                offset_beats: written.position.offset.beats(),
                chord: written.chord.clone(),
                origin: ChordPlanOrigin::GeneratedReharmonization,
                sources: template.sources.clone(),
            })
            .collect();
        result.entries.splice(
            candidate.replaced_entry..=candidate.replaced_entry,
            replacement,
        );
    }
    Ok(result)
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::model::duration::Duration;
    use crate::model::pitch::Pitch;
    use crate::model::score::Note;

    fn symbol(root: &str, kind: &str) -> ChordSymbol {
        serde_json::from_value(serde_json::json!({"root": root, "kind": kind})).unwrap()
    }

    fn entry(measure: usize, beats: u64, root: &str, kind: &str) -> ChordPlanEntry {
        let offset = BeatFraction::new(beats, 1).unwrap();
        ChordPlanEntry {
            position: ChordPlanPosition {
                measure_index: measure,
                offset,
            },
            offset_beats: offset.beats(),
            chord: symbol(root, kind),
            origin: ChordPlanOrigin::HostAuthored,
            sources: Vec::new(),
        }
    }

    /// 4/4, one melody part of whole notes.
    fn score(melody: &[u8]) -> Score {
        let mut score = Score::new("t", 120, 4, 4, 0, melody.len() as u32);
        for (measure, pitch) in melody.iter().enumerate() {
            score.parts[0].staves[0].measures[measure].voices[0] =
                vec![Note::new(Pitch::from_midi(*pitch, false), Duration::Whole)];
        }
        score
    }

    fn request(plan: ChordPlan, start: usize, end: usize) -> ReharmonizationRequest {
        ReharmonizationRequest {
            plan,
            melody_part: 0,
            start_measure: start,
            end_measure_exclusive: end,
            locked_measures: Vec::new(),
            locked_entries: Vec::new(),
            techniques: Vec::new(),
            max_candidates: 64,
        }
    }

    fn plan(entries: Vec<ChordPlanEntry>) -> ChordPlan {
        ChordPlan {
            contract_version: 1,
            entries,
        }
    }

    fn chords(candidate: &ReharmonizationCandidate) -> Vec<(String, String)> {
        candidate
            .chords
            .iter()
            .map(|c| (c.chord.root.clone(), c.chord.kind.clone()))
            .collect()
    }

    fn find(result: &ReharmonizationResult, id: &str) -> ReharmonizationCandidate {
        result
            .candidates
            .iter()
            .find(|c| c.id == id)
            .unwrap_or_else(|| panic!("missing {id}: {:?}", result.candidates))
            .clone()
    }

    #[test]
    fn approach_chords_are_spelled_for_major_and_minor_arrivals() {
        // C | F | Am | C : G melody over every bar.
        let plan = plan(vec![
            entry(0, 0, "C", "major"),
            entry(1, 0, "F", "major"),
            entry(2, 0, "A", "minor"),
            entry(3, 0, "C", "major"),
        ]);
        let score = score(&[67, 69, 72, 67]);
        let result = suggest_reharmonizations(&score, &request(plan, 0, 4)).unwrap();
        // Before Am (entry 2): E7, Bb7, Bø7-E7.
        assert_eq!(
            chords(&find(&result, "secondary-dominant-1")),
            vec![("E".into(), "dominant".into())]
        );
        assert_eq!(
            chords(&find(&result, "tritone-substitution-1")),
            vec![("Bb".into(), "dominant".into())]
        );
        let two_five = find(&result, "two-five-1");
        assert_eq!(
            chords(&two_five),
            vec![
                ("B".into(), "half-diminished".into()),
                ("E".into(), "dominant".into())
            ]
        );
        assert_eq!(two_five.chords[1].position.offset.beats(), 2.0);
        assert_eq!(two_five.resolution_root_motion, 5, "E -> A is a fifth down");
        // Before C (entry 3): Db7 resolves down a semitone.
        let sub = find(&result, "tritone-substitution-2");
        assert_eq!(chords(&sub), vec![("Db".into(), "dominant".into())]);
        assert_eq!(sub.resolution_root_motion, -1);
        // Before F (entry 1): C7 replaces C.
        assert_eq!(
            chords(&find(&result, "secondary-dominant-0")),
            vec![("C".into(), "dominant".into())]
        );
    }

    #[test]
    fn an_existing_dominant_is_not_proposed_again() {
        let plan = plan(vec![
            entry(0, 0, "G", "dominant"),
            entry(1, 0, "C", "major"),
        ]);
        let result = suggest_reharmonizations(&score(&[67, 67]), &request(plan, 0, 2)).unwrap();
        assert!(
            result
                .candidates
                .iter()
                .all(|c| c.technique != ReharmonizationTechnique::SecondaryDominant)
        );
    }

    #[test]
    fn melody_fit_ranks_candidates_and_lists_avoid_notes() {
        // Melody C before F: root of C7, #11 of Gb7, 11 of Gm7 and root of C7.
        let to_f = plan(vec![entry(0, 0, "C", "major"), entry(1, 0, "F", "major")]);
        let result = suggest_reharmonizations(&score(&[72, 72]), &request(to_f, 0, 1)).unwrap();
        assert_eq!(result.eligible, 3);
        assert!(
            result
                .candidates
                .iter()
                .all(|c| c.melody.avoid_notes.is_empty())
        );

        // Melody C before Am: b13 of E7, 9 of Bb7, but b9 of Bø7 for the first
        // half of the two-five.
        let to_a = plan(vec![entry(0, 0, "C", "major"), entry(1, 0, "A", "minor")]);
        let result = suggest_reharmonizations(&score(&[72, 69]), &request(to_a, 0, 1)).unwrap();
        let two_five = find(&result, "two-five-0");
        assert_eq!(two_five.melody.melody_notes, 2);
        assert_eq!(two_five.melody.tensions, 1);
        assert_eq!(two_five.melody.avoid_beats, 2.0, "only the Bø7 half");
        assert_eq!(two_five.melody.avoid_notes[0].chord.root, "B");
        assert_eq!(two_five.melody.avoid_notes[0].duration_beats, 2.0);
        assert_eq!(find(&result, "secondary-dominant-0").melody.tensions, 1);
        assert_eq!(
            result.candidates.last().unwrap().id,
            "two-five-0",
            "most avoid beats last"
        );
    }

    #[test]
    fn locks_and_range_limit_the_edited_spans() {
        let plan = plan(vec![
            entry(0, 0, "C", "major"),
            entry(1, 0, "F", "major"),
            entry(2, 0, "G", "major"),
            entry(3, 0, "C", "major"),
        ]);
        let score = score(&[67, 69, 71, 72]);
        let mut req = request(plan, 0, 3);
        req.locked_measures = vec![1];
        req.locked_entries = vec![0];
        let result = suggest_reharmonizations(&score, &req).unwrap();
        // Entry 0 locked, entry 1 in a locked measure, entry 2 ends at the range end
        // (measure 3 is outside): its span is measure 2 only, so it is editable.
        assert!(result.candidates.iter().all(|c| c.replaced_entry == 2));
        assert!(!result.candidates.is_empty());
        req.end_measure_exclusive = 2;
        req.locked_measures.clear();
        let result = suggest_reharmonizations(&score, &req).unwrap();
        assert!(result.candidates.iter().all(|c| c.replaced_entry == 1));
    }

    #[test]
    fn applying_candidates_rewrites_only_their_entries() {
        let original = plan(vec![
            entry(0, 0, "C", "major"),
            entry(1, 0, "A", "minor"),
            entry(2, 0, "D", "minor"),
            entry(3, 0, "C", "major"),
        ]);
        let score = score(&[67, 69, 69, 67]);
        let result = suggest_reharmonizations(&score, &request(original.clone(), 0, 4)).unwrap();
        let a = find(&result, "two-five-0");
        let b = find(&result, "tritone-substitution-2");
        let applied = apply_reharmonizations(&original, &[b.clone(), a.clone()]).unwrap();
        let names: Vec<_> = applied
            .entries
            .iter()
            .map(|e| format!("{}{}", e.chord.root, e.chord.kind))
            .collect();
        assert_eq!(
            names,
            vec![
                "Bhalf-diminished",
                "Edominant",
                "Aminor",
                "Dbdominant",
                "Cmajor"
            ]
        );
        assert_eq!(
            applied.entries[1].origin,
            ChordPlanOrigin::GeneratedReharmonization
        );
        assert_eq!(applied.entries[2], original.entries[1]);
        assert_eq!(applied.entries[4], original.entries[3]);

        let c = find(&result, "secondary-dominant-1");
        assert!(matches!(
            apply_reharmonizations(&original, &[a.clone(), c]),
            Err(ReharmonizationError::OverlappingCandidates { .. })
        ));
        let mut changed = original.clone();
        changed.entries[0].chord = symbol("F", "major");
        assert!(matches!(
            apply_reharmonizations(&changed, &[a]),
            Err(ReharmonizationError::CandidateMismatch { .. })
        ));
    }

    #[test]
    fn invalid_requests_are_rejected() {
        let plan = plan(vec![entry(0, 0, "C", "major"), entry(1, 0, "F", "major")]);
        let score = score(&[67, 67]);
        let mut req = request(plan.clone(), 0, 3);
        assert!(matches!(
            suggest_reharmonizations(&score, &req),
            Err(ReharmonizationError::InvalidRange { .. })
        ));
        req.end_measure_exclusive = 2;
        req.max_candidates = 0;
        assert!(matches!(
            suggest_reharmonizations(&score, &req),
            Err(ReharmonizationError::InvalidCount { .. })
        ));
        req.max_candidates = 1;
        req.melody_part = 4;
        assert!(matches!(
            suggest_reharmonizations(&score, &req),
            Err(ReharmonizationError::UnknownPart { .. })
        ));
        req.melody_part = 0;
        req.plan.entries.swap(0, 1);
        assert!(matches!(
            suggest_reharmonizations(&score, &req),
            Err(ReharmonizationError::UnorderedPlan { .. })
        ));
    }
}
