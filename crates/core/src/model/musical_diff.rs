//! Arrangement comparison by written musical position, separate from the edit patch diff.

use super::score::{Note, NoteAddr, Score};
use serde::{Deserialize, Serialize};
use serde_json::Value;
use std::collections::BTreeSet;

#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum StructurePolicy {
    /// Require identical part, staff and measure structure before matching by index.
    Strict,
    /// Explicitly pair the common index prefix; report notes in extra voices as additions/removals.
    IndexPrefix,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
pub struct VoiceAddr {
    pub part: usize,
    pub staff: usize,
    pub voice: usize,
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct VoiceCorrespondence {
    pub reference: VoiceAddr,
    pub current: VoiceAddr,
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct NoteCorrespondence {
    pub reference: NoteAddr,
    pub current: NoteAddr,
}

/// Explicit policy for comparison. Supplied note addresses take priority over position matching.
#[derive(Debug, Clone, Serialize, Deserialize)]
#[serde(default)]
pub struct MusicalComparisonOptions {
    pub structure_policy: StructurePolicy,
    pub voice_pairs: Vec<VoiceCorrespondence>,
    pub note_pairs: Vec<NoteCorrespondence>,
}

impl Default for MusicalComparisonOptions {
    fn default() -> Self {
        Self {
            structure_policy: StructurePolicy::Strict,
            voice_pairs: Vec::new(),
            note_pairs: Vec::new(),
        }
    }
}

#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum MusicalChangeKind {
    Added,
    Removed,
    Modified,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct MusicalChange {
    pub kind: MusicalChangeKind,
    pub reference: Option<NoteAddr>,
    pub current: Option<NoteAddr>,
    /// Stable field names. `onset_beats` is a derived rhythmic position.
    pub changed_fields: Vec<String>,
    /// `timing`, `pitch`, and/or `notation` for modified notes.
    pub domains: Vec<String>,
    pub reference_onset_beats: Option<f64>,
    pub current_onset_beats: Option<f64>,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct MusicalComparisonDiagnostic {
    pub code: String,
    pub reference_voice: Option<VoiceAddr>,
    pub current_voice: Option<VoiceAddr>,
    pub measure: Option<usize>,
}

#[derive(Debug, Clone, Default, Serialize, Deserialize)]
pub struct MusicalComparisonResult {
    pub changes: Vec<MusicalChange>,
    pub diagnostics: Vec<MusicalComparisonDiagnostic>,
}

struct Timed<'a> {
    note: &'a Note,
    index: usize,
    onset: f64,
}

fn timed(voice: &[Note]) -> Vec<Timed<'_>> {
    let mut onset = 0.0;
    voice
        .iter()
        .enumerate()
        .map(|(index, note)| {
            let entry = Timed { note, index, onset };
            onset += note.beats();
            entry
        })
        .collect()
}

fn voice_at(score: &Score, address: VoiceAddr, measure: usize) -> Option<&[Note]> {
    score
        .parts
        .get(address.part)?
        .staves
        .get(address.staff)?
        .measures
        .get(measure)?
        .voices
        .get(address.voice)
        .map(Vec::as_slice)
}

fn voice_exists(score: &Score, address: VoiceAddr) -> bool {
    address.voice < 4
        && score
            .parts
            .get(address.part)
            .and_then(|part| part.staves.get(address.staff))
            .is_some()
}

fn all_voices(score: &Score) -> Vec<VoiceAddr> {
    score
        .parts
        .iter()
        .enumerate()
        .flat_map(|(part, item)| {
            item.staves.iter().enumerate().flat_map(move |(staff, _)| {
                (0..4).map(move |voice| VoiceAddr { part, staff, voice })
            })
        })
        .collect()
}

fn same_structure(reference: &Score, current: &Score) -> bool {
    reference.parts.len() == current.parts.len()
        && reference.parts.iter().zip(&current.parts).all(|(a, b)| {
            a.staves.len() == b.staves.len()
                && a.staves
                    .iter()
                    .zip(&b.staves)
                    .all(|(x, y)| x.measures.len() == y.measures.len())
        })
}

fn diagnostic(
    code: &str,
    reference_voice: Option<VoiceAddr>,
    current_voice: Option<VoiceAddr>,
    measure: Option<usize>,
) -> MusicalComparisonDiagnostic {
    MusicalComparisonDiagnostic {
        code: code.into(),
        reference_voice,
        current_voice,
        measure,
    }
}

fn note_addr(voice: VoiceAddr, measure: usize, note: usize) -> NoteAddr {
    NoteAddr {
        part: voice.part,
        staff: voice.staff,
        measure,
        voice: voice.voice,
        note,
    }
}

fn canonical(value: &mut Value) {
    match value {
        Value::Object(fields) => {
            for name in ["id", "offset_x", "offset_y", "relative_x", "relative_y"] {
                fields.remove(name);
            }
            for value in fields.values_mut() {
                canonical(value);
            }
        }
        Value::Array(values) => {
            for value in values {
                canonical(value);
            }
        }
        _ => {}
    }
}

fn note_value(note: &Note) -> Value {
    let mut value = serde_json::to_value(note).unwrap_or(Value::Null);
    canonical(&mut value);
    value
}

fn cents(note: &Note) -> Vec<i32> {
    let mut pitches: Vec<_> = note
        .pitches
        .iter()
        .map(|pitch| pitch.to_midi_cents())
        .collect();
    pitches.sort_unstable();
    pitches
}

fn same_sound_identity(a: &Note, b: &Note) -> bool {
    a.is_rest == b.is_rest
        && a.is_grace == b.is_grace
        && a.is_cue == b.is_cue
        && cents(a) == cents(b)
}

fn same_onset(a: &Timed<'_>, b: &Timed<'_>) -> bool {
    (a.onset - b.onset).abs() <= 1e-8
}

fn match_unique(
    reference: &[Timed<'_>],
    current: &[Timed<'_>],
    used_reference: &mut [bool],
    used_current: &mut [bool],
    pairs: &mut Vec<(usize, usize)>,
    predicate: impl Fn(&Timed<'_>, &Timed<'_>) -> bool,
) {
    loop {
        let mut progress = false;
        for (ri, a) in reference.iter().enumerate() {
            if used_reference[ri] {
                continue;
            }
            let candidates: Vec<_> = current
                .iter()
                .enumerate()
                .filter(|(ci, b)| !used_current[*ci] && predicate(a, b))
                .map(|(ci, _)| ci)
                .collect();
            if candidates.len() != 1 {
                continue;
            }
            let ci = candidates[0];
            let reverse = reference
                .iter()
                .enumerate()
                .filter(|(other, r)| !used_reference[*other] && predicate(r, &current[ci]))
                .count();
            if reverse != 1 {
                continue;
            }
            used_reference[ri] = true;
            used_current[ci] = true;
            pairs.push((ri, ci));
            progress = true;
        }
        if !progress {
            break;
        }
    }
}

fn modified(
    reference: &Timed<'_>,
    current: &Timed<'_>,
    ra: NoteAddr,
    ca: NoteAddr,
) -> Option<MusicalChange> {
    let a = note_value(reference.note);
    let b = note_value(current.note);
    let mut fields = BTreeSet::new();
    if !same_onset(reference, current) {
        fields.insert("onset_beats".to_string());
    }
    if let (Some(x), Some(y)) = (a.as_object(), b.as_object()) {
        for key in x.keys().chain(y.keys()) {
            if x.get(key) != y.get(key) {
                fields.insert(key.clone());
            }
        }
    } else if a != b {
        fields.insert("note".to_string());
    }
    if fields.is_empty() {
        return None;
    }
    let mut domains = Vec::new();
    if fields.contains("onset_beats")
        || (reference.note.beats() - current.note.beats()).abs() > 1e-8
    {
        domains.push("timing".into());
    }
    if reference.note.is_rest != current.note.is_rest
        || cents(reference.note) != cents(current.note)
    {
        domains.push("pitch".into());
    }
    if fields
        .iter()
        .any(|field| !matches!(field.as_str(), "onset_beats" | "pitches"))
        || (fields.contains("pitches") && cents(reference.note) == cents(current.note))
    {
        domains.push("notation".into());
    }
    Some(MusicalChange {
        kind: MusicalChangeKind::Modified,
        reference: Some(ra),
        current: Some(ca),
        changed_fields: fields.into_iter().collect(),
        domains,
        reference_onset_beats: Some(reference.onset),
        current_onset_beats: Some(current.onset),
    })
}

fn unmatched_change(kind: MusicalChangeKind, address: NoteAddr, onset: f64) -> MusicalChange {
    let (reference, current, reference_onset_beats, current_onset_beats) = match kind {
        MusicalChangeKind::Added => (None, Some(address), None, Some(onset)),
        MusicalChangeKind::Removed => (Some(address), None, Some(onset), None),
        MusicalChangeKind::Modified => unreachable!(),
    };
    MusicalChange {
        kind,
        reference,
        current,
        changed_fields: Vec::new(),
        domains: Vec::new(),
        reference_onset_beats,
        current_onset_beats,
    }
}

fn whole_voice_changes(
    score: &Score,
    voice: VoiceAddr,
    kind: MusicalChangeKind,
    result: &mut MusicalComparisonResult,
) {
    let measures = &score.parts[voice.part].staves[voice.staff].measures;
    for (measure, _) in measures.iter().enumerate() {
        if let Some(notes) = voice_at(score, voice, measure) {
            for entry in timed(notes) {
                result.changes.push(unmatched_change(
                    kind,
                    note_addr(voice, measure, entry.index),
                    entry.onset,
                ));
            }
        }
    }
}

/// Compare reference and current notation at each measure's written onset.
///
/// This operation never mutates either score and does not change `diff` or patch semantics.
/// With default strict policy, structurally different scores return a diagnostic. Select
/// `index_prefix` or supply voice pairs to compare differently structured arrangements.
pub fn compare_musical_positions(
    reference: &Score,
    current: &Score,
    options: &MusicalComparisonOptions,
) -> MusicalComparisonResult {
    let mut result = MusicalComparisonResult::default();
    if options.voice_pairs.is_empty()
        && options.structure_policy == StructurePolicy::Strict
        && !same_structure(reference, current)
    {
        result
            .diagnostics
            .push(diagnostic("voice-structure-mismatch", None, None, None));
        return result;
    }
    let reference_voices = all_voices(reference);
    let current_voices = all_voices(current);
    let pairs = if !options.voice_pairs.is_empty() {
        options.voice_pairs.clone()
    } else {
        reference_voices
            .iter()
            .filter(|voice| current_voices.contains(voice))
            .map(|voice| VoiceCorrespondence {
                reference: *voice,
                current: *voice,
            })
            .collect()
    };
    let mut used_reference = Vec::new();
    let mut used_current = Vec::new();
    for pair in &pairs {
        if !voice_exists(reference, pair.reference)
            || !voice_exists(current, pair.current)
            || used_reference.contains(&pair.reference)
            || used_current.contains(&pair.current)
        {
            result.diagnostics.push(diagnostic(
                "invalid-voice-correspondence",
                Some(pair.reference),
                Some(pair.current),
                None,
            ));
            return result;
        }
        used_reference.push(pair.reference);
        used_current.push(pair.current);
    }
    let mut used_note_reference = Vec::new();
    let mut used_note_current = Vec::new();
    for mapping in &options.note_pairs {
        if !pairs.iter().any(|pair| {
            pair.reference.part == mapping.reference.part
                && pair.reference.staff == mapping.reference.staff
                && pair.reference.voice == mapping.reference.voice
                && pair.current.part == mapping.current.part
                && pair.current.staff == mapping.current.staff
                && pair.current.voice == mapping.current.voice
        }) || mapping.reference.measure != mapping.current.measure
            || voice_at(
                reference,
                VoiceAddr {
                    part: mapping.reference.part,
                    staff: mapping.reference.staff,
                    voice: mapping.reference.voice,
                },
                mapping.reference.measure,
            )
            .is_none_or(|notes| mapping.reference.note >= notes.len())
            || voice_at(
                current,
                VoiceAddr {
                    part: mapping.current.part,
                    staff: mapping.current.staff,
                    voice: mapping.current.voice,
                },
                mapping.current.measure,
            )
            .is_none_or(|notes| mapping.current.note >= notes.len())
            || used_note_reference.contains(&mapping.reference)
            || used_note_current.contains(&mapping.current)
        {
            result.diagnostics.push(diagnostic(
                "invalid-note-correspondence",
                None,
                None,
                Some(mapping.reference.measure),
            ));
            return result;
        }
        used_note_reference.push(mapping.reference.clone());
        used_note_current.push(mapping.current.clone());
    }
    for pair in &pairs {
        let reference_measures =
            &reference.parts[pair.reference.part].staves[pair.reference.staff].measures;
        let current_measures =
            &current.parts[pair.current.part].staves[pair.current.staff].measures;
        if reference_measures.len() != current_measures.len() {
            result.diagnostics.push(diagnostic(
                "measure-count-mismatch",
                Some(pair.reference),
                Some(pair.current),
                None,
            ));
            continue;
        }
        for measure in 0..reference_measures.len() {
            if reference_measures[measure].number != current_measures[measure].number {
                result.diagnostics.push(diagnostic(
                    "measure-number-mismatch",
                    Some(pair.reference),
                    Some(pair.current),
                    Some(measure),
                ));
                continue;
            }
            let a = timed(voice_at(reference, pair.reference, measure).unwrap_or(&[]));
            let b = timed(voice_at(current, pair.current, measure).unwrap_or(&[]));
            let mut used_a = vec![false; a.len()];
            let mut used_b = vec![false; b.len()];
            let mut matched = Vec::new();
            for identity in options.note_pairs.iter().filter(|identity| {
                identity.reference.measure == measure
                    && identity.reference.part == pair.reference.part
                    && identity.reference.staff == pair.reference.staff
                    && identity.reference.voice == pair.reference.voice
                    && identity.current.part == pair.current.part
                    && identity.current.staff == pair.current.staff
                    && identity.current.voice == pair.current.voice
            }) {
                let ri = identity.reference.note;
                let ci = identity.current.note;
                if ri >= a.len() || ci >= b.len() || used_a[ri] || used_b[ci] {
                    result.diagnostics.push(diagnostic(
                        "invalid-note-correspondence",
                        Some(pair.reference),
                        Some(pair.current),
                        Some(measure),
                    ));
                    continue;
                }
                used_a[ri] = true;
                used_b[ci] = true;
                matched.push((ri, ci));
            }
            match_unique(&a, &b, &mut used_a, &mut used_b, &mut matched, |x, y| {
                same_onset(x, y) && note_value(x.note) == note_value(y.note)
            });
            match_unique(&a, &b, &mut used_a, &mut used_b, &mut matched, |x, y| {
                same_onset(x, y) && same_sound_identity(x.note, y.note)
            });
            match_unique(&a, &b, &mut used_a, &mut used_b, &mut matched, |x, y| {
                same_sound_identity(x.note, y.note)
            });
            match_unique(&a, &b, &mut used_a, &mut used_b, &mut matched, same_onset);
            for (ri, ci) in matched {
                if let Some(change) = modified(
                    &a[ri],
                    &b[ci],
                    note_addr(pair.reference, measure, a[ri].index),
                    note_addr(pair.current, measure, b[ci].index),
                ) {
                    result.changes.push(change);
                }
            }
            let unmatched_a: Vec<_> = used_a
                .iter()
                .enumerate()
                .filter_map(|(i, used)| (!used).then_some(i))
                .collect();
            let unmatched_b: Vec<_> = used_b
                .iter()
                .enumerate()
                .filter_map(|(i, used)| (!used).then_some(i))
                .collect();
            if !unmatched_a.is_empty()
                && !unmatched_b.is_empty()
                && (unmatched_a.len() > 1 || unmatched_b.len() > 1)
            {
                result.diagnostics.push(diagnostic(
                    "ambiguous-note-alignment",
                    Some(pair.reference),
                    Some(pair.current),
                    Some(measure),
                ));
            }
            for ri in unmatched_a {
                result.changes.push(unmatched_change(
                    MusicalChangeKind::Removed,
                    note_addr(pair.reference, measure, a[ri].index),
                    a[ri].onset,
                ));
            }
            for ci in unmatched_b {
                result.changes.push(unmatched_change(
                    MusicalChangeKind::Added,
                    note_addr(pair.current, measure, b[ci].index),
                    b[ci].onset,
                ));
            }
        }
    }
    for voice in reference_voices {
        if !used_reference.contains(&voice) {
            whole_voice_changes(reference, voice, MusicalChangeKind::Removed, &mut result);
        }
    }
    for voice in current_voices {
        if !used_current.contains(&voice) {
            whole_voice_changes(current, voice, MusicalChangeKind::Added, &mut result);
        }
    }
    result
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::{Duration, Pitch, TupletBoundary, TupletInfo};

    fn note(midi: u8, duration: Duration) -> Note {
        Note::new(Pitch::from_midi(midi, false), duration)
    }

    fn score(notes: Vec<Note>) -> Score {
        let mut score = Score::new("comparison", 120, 4, 4, 0, 1);
        score.parts[0].staves[0].measures[0].voices[0] = notes;
        score
    }

    #[test]
    fn rearticulation_does_not_mark_later_notes() {
        let reference = score(vec![
            note(60, Duration::Quarter),
            note(62, Duration::Quarter),
            note(64, Duration::Quarter),
            note(65, Duration::Quarter),
        ]);
        let current = score(vec![
            note(60, Duration::Eighth),
            note(60, Duration::Eighth),
            note(62, Duration::Quarter),
            note(64, Duration::Quarter),
            note(65, Duration::Quarter),
        ]);
        let result = compare_musical_positions(&reference, &current, &Default::default());
        assert!(result.diagnostics.is_empty());
        let current_indices: Vec<_> = result
            .changes
            .iter()
            .filter_map(|change| change.current.as_ref().map(|address| address.note))
            .collect();
        assert_eq!(current_indices, vec![0, 1]);
        assert_eq!(result.changes[0].kind, MusicalChangeKind::Modified);
        assert_eq!(result.changes[0].reference.as_ref().unwrap().note, 0);
        assert_eq!(result.changes[1].kind, MusicalChangeKind::Added);
    }

    #[test]
    fn swing_tuplet_marks_only_the_changed_first_beat() {
        let reference = score(vec![
            note(60, Duration::Eighth),
            note(62, Duration::Eighth),
            note(64, Duration::Quarter),
            note(65, Duration::Quarter),
            note(67, Duration::Quarter),
        ]);
        let mut current = reference.clone();
        current.parts[0].staves[0].measures[0].voices[0][0].duration = Duration::Quarter;
        for index in 0..2 {
            current.parts[0].staves[0].measures[0].voices[0][index].tuplet = Some(TupletInfo {
                actual_notes: 3,
                normal_notes: 2,
            });
        }
        let result = compare_musical_positions(&reference, &current, &Default::default());
        let indices: Vec<_> = result
            .changes
            .iter()
            .filter_map(|change| change.current.as_ref().map(|address| address.note))
            .collect();
        assert_eq!(indices, vec![0, 1]);
        assert!(
            result
                .changes
                .iter()
                .all(|change| change.domains.contains(&"timing".into()))
        );
    }

    #[test]
    fn grace_insertion_and_notation_only_tuplet_boundary_are_distinct() {
        let reference = score(vec![
            note(60, Duration::Quarter),
            note(62, Duration::Quarter),
        ]);
        let mut current = reference.clone();
        let mut grace = note(67, Duration::Eighth);
        grace.is_grace = true;
        current.parts[0].staves[0].measures[0].voices[0].insert(0, grace);
        let result = compare_musical_positions(&reference, &current, &Default::default());
        assert_eq!(result.changes.len(), 1);
        assert_eq!(result.changes[0].kind, MusicalChangeKind::Added);
        assert_eq!(result.changes[0].current.as_ref().unwrap().note, 0);

        let mut notation = reference.clone();
        notation.parts[0].staves[0].measures[0].voices[0][0].tuplet_boundary =
            Some(TupletBoundary::Start);
        let result = compare_musical_positions(&reference, &notation, &Default::default());
        assert_eq!(result.changes.len(), 1);
        assert_eq!(result.changes[0].changed_fields, vec!["tuplet_boundary"]);
        assert_eq!(result.changes[0].domains, vec!["notation"]);
    }

    #[test]
    fn adjacent_authored_tuplets_do_not_move_later_notes() {
        let mut reference = score(vec![
            note(60, Duration::Eighth),
            note(62, Duration::Eighth),
            note(64, Duration::Eighth),
            note(65, Duration::Eighth),
            note(67, Duration::Quarter),
        ]);
        let voice = &mut reference.parts[0].staves[0].measures[0].voices[0];
        for note in voice.iter_mut().take(4) {
            note.tuplet = Some(TupletInfo {
                actual_notes: 3,
                normal_notes: 2,
            });
        }
        voice[0].tuplet_boundary = Some(TupletBoundary::Start);
        voice[1].tuplet_boundary = Some(TupletBoundary::Stop);
        voice[2].tuplet_boundary = Some(TupletBoundary::Start);
        voice[3].tuplet_boundary = Some(TupletBoundary::Stop);
        let mut current = reference.clone();
        current.parts[0].staves[0].measures[0].voices[0][1].tuplet_boundary = None;
        let result = compare_musical_positions(&reference, &current, &Default::default());
        assert_eq!(result.changes.len(), 1);
        assert_eq!(result.changes[0].current.as_ref().unwrap().note, 1);
        assert_eq!(result.changes[0].changed_fields, vec!["tuplet_boundary"]);
    }

    #[test]
    fn strict_shape_rejects_added_part_but_explicit_index_prefix_reports_its_notes() {
        let reference = score(vec![note(60, Duration::Whole)]);
        let mut current = reference.clone();
        let mut added = current.parts[0].clone();
        added.staves[0].measures[0].voices[0] = vec![note(48, Duration::Whole)];
        current.parts.push(added);
        let strict = compare_musical_positions(&reference, &current, &Default::default());
        assert!(strict.changes.is_empty());
        assert_eq!(strict.diagnostics[0].code, "voice-structure-mismatch");
        let options = MusicalComparisonOptions {
            structure_policy: StructurePolicy::IndexPrefix,
            ..Default::default()
        };
        let result = compare_musical_positions(&reference, &current, &options);
        assert_eq!(result.changes.len(), 1);
        assert_eq!(result.changes[0].kind, MusicalChangeKind::Added);
        assert_eq!(result.changes[0].current.as_ref().unwrap().part, 1);
    }

    #[test]
    fn rest_in_pickup_does_not_shift_the_next_short_measure() {
        let mut reference = score(vec![note(60, Duration::Quarter)]);
        reference.parts[0].staves[0].measures[0].actual_length = Some(crate::MeasureLength {
            numerator: 1,
            denominator: 4,
        });
        let mut second = reference.parts[0].staves[0].measures[0].clone();
        second.number = 2;
        second.actual_length = Some(crate::MeasureLength {
            numerator: 2,
            denominator: 4,
        });
        second.voices[0] = vec![note(62, Duration::Quarter), note(64, Duration::Quarter)];
        reference.parts[0].staves[0].measures.push(second);
        let mut current = reference.clone();
        current.parts[0].staves[0].measures[0].voices[0] =
            vec![Note::rest(Duration::Eighth), note(60, Duration::Eighth)];
        let result = compare_musical_positions(&reference, &current, &Default::default());
        assert!(result.changes.iter().all(|change| {
            change
                .current
                .as_ref()
                .is_none_or(|address| address.measure == 0)
        }));
        assert!(
            result
                .changes
                .iter()
                .any(|change| change.kind == MusicalChangeKind::Added)
        );
    }

    #[test]
    fn explicit_voice_and_note_correspondence_survive_inserted_part() {
        let reference = score(vec![note(60, Duration::Quarter)]);
        let mut current = reference.clone();
        current.parts.insert(0, reference.parts[0].clone());
        current.parts[0].staves[0].measures[0].voices[0] = vec![note(48, Duration::Quarter)];
        let reference_voice = VoiceAddr {
            part: 0,
            staff: 0,
            voice: 0,
        };
        let current_voice = VoiceAddr {
            part: 1,
            staff: 0,
            voice: 0,
        };
        let options = MusicalComparisonOptions {
            voice_pairs: vec![VoiceCorrespondence {
                reference: reference_voice,
                current: current_voice,
            }],
            note_pairs: vec![NoteCorrespondence {
                reference: note_addr(reference_voice, 0, 0),
                current: note_addr(current_voice, 0, 0),
            }],
            ..Default::default()
        };
        let result = compare_musical_positions(&reference, &current, &options);
        assert!(result.diagnostics.is_empty());
        assert_eq!(result.changes.len(), 1);
        assert_eq!(result.changes[0].kind, MusicalChangeKind::Added);
        assert_eq!(result.changes[0].current.as_ref().unwrap().part, 0);
    }
}
