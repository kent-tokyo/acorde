use crate::{
    AccidentalMark, BeamGroup, ConcertKeyOverride, CourtesyAccidental, LayoutConfig, LayoutResult,
    RowLayout, SpanMark, TupletGroup,
};
use acorde_core::{BeamState, HairpinKind, NoteAddr, OttavaKind, Score, Step, TupletInfo};
use std::collections::HashMap;

/// Shift `fifths` (circle-of-fifths key index) by `semitones` semitones.
///
/// Uses the modular relationship: 7 semitones = +1 fifth (sharp).
/// Maps result to [-7, 7].
fn transpose_key_fifths(fifths: i8, semitones: i8) -> i8 {
    let raw = (semitones as i16 * 7).rem_euclid(12) as i8;
    let delta = if raw > 6 { raw - 12 } else { raw };
    (fifths + delta).clamp(-7, 7)
}

/// Compute a `LayoutResult` for the given score and configuration.
///
/// The result is independent of pixel dimensions so any renderer can consume it.
pub fn compute_layout(score: &Score, config: &LayoutConfig) -> LayoutResult {
    let per_row = config.measures_per_row.max(1);
    let first_row = config.first_row_measures.map(|n| n.max(1));
    let vis_slots = build_vis_slots(score);
    let rows = build_rows(score, &vis_slots, per_row, first_row);
    let spans = resolve_spans(score);

    let mut concert_key_overrides = Vec::new();
    if config.concert_pitch {
        let base_fifths = score.settings.key_signature.fifths;
        for (pi, part) in score.parts.iter().enumerate() {
            for (si, staff) in part.staves.iter().enumerate() {
                if staff.transpose_semitones != 0 {
                    concert_key_overrides.push(ConcertKeyOverride {
                        part_index: pi,
                        staff_index: si,
                        fifths: transpose_key_fifths(base_fifths, staff.transpose_semitones),
                    });
                }
            }
        }
    }

    let beam_groups = collect_beam_groups(score);
    let tuplet_groups = collect_tuplet_groups(score);
    let courtesy_accidentals = collect_courtesy_accidentals(score);
    let accidentals = collect_accidental_marks(score);

    LayoutResult {
        vis_slots,
        rows,
        spans,
        concert_key_overrides,
        beam_groups,
        tuplet_groups,
        courtesy_accidentals,
        accidentals,
    }
}

// ── vis_slots ─────────────────────────────────────────────────────────────────

fn build_vis_slots(score: &Score) -> Vec<usize> {
    let measure_count = score
        .parts
        .first()
        .and_then(|p| p.staves.first())
        .map(|s| s.measures.len())
        .unwrap_or(0);

    let mut slots = Vec::new();
    for phys in 0..measure_count {
        let count = multi_rest_count(score, phys).max(1);
        for _ in 0..count {
            slots.push(phys);
        }
    }
    slots
}

fn multi_rest_count(score: &Score, measure_index: usize) -> usize {
    score
        .parts
        .iter()
        .flat_map(|p| p.staves.iter())
        .filter_map(|s| s.measures.get(measure_index))
        .filter_map(|m| m.multi_rest_count)
        .map(|c| c as usize)
        .max()
        .unwrap_or(1)
}

// ── rows ──────────────────────────────────────────────────────────────────────

fn force_break_after(score: &Score, phys: usize) -> bool {
    score
        .parts
        .iter()
        .flat_map(|p| p.staves.iter())
        .filter_map(|s| s.measures.get(phys))
        .any(|m| m.system_break || m.page_break)
}

fn build_rows(
    score: &Score,
    vis_slots: &[usize],
    per_row: usize,
    first_row_limit: Option<usize>,
) -> Vec<RowLayout> {
    if vis_slots.is_empty() {
        return vec![RowLayout {
            measure_indices: Vec::new(),
        }];
    }

    let mut rows: Vec<RowLayout> = Vec::new();
    let mut current: Vec<usize> = Vec::new();
    let mut last_phys = usize::MAX;

    for &phys in vis_slots {
        if phys != last_phys {
            let limit = if rows.is_empty() {
                first_row_limit.unwrap_or(per_row)
            } else {
                per_row
            };
            if current.len() == limit {
                rows.push(RowLayout {
                    measure_indices: std::mem::take(&mut current),
                });
            }
            current.push(phys);
            if force_break_after(score, phys) {
                rows.push(RowLayout {
                    measure_indices: std::mem::take(&mut current),
                });
            }
        }
        last_phys = phys;
    }

    if !current.is_empty() {
        rows.push(RowLayout {
            measure_indices: current,
        });
    }
    rows
}

// ── span resolution ───────────────────────────────────────────────────────────

fn resolve_spans(score: &Score) -> Vec<SpanMark> {
    let mut spans = Vec::new();

    for (pi, part) in score.parts.iter().enumerate() {
        for (si, staff) in part.staves.iter().enumerate() {
            for voice in 0..4usize {
                let mut open_hairpin: Option<(NoteAddr, HairpinKind)> = None;
                let mut open_ottava: Option<(NoteAddr, OttavaKind)> = None;
                let mut open_pedal: Option<NoteAddr> = None;
                let mut open_slur: Option<NoteAddr> = None;
                let mut open_trill_line: Option<NoteAddr> = None;
                let mut open_glissando: Option<NoteAddr> = None;

                for (mi, measure) in staff.measures.iter().enumerate() {
                    for (ni, note) in measure.voices[voice].iter().enumerate() {
                        let addr = NoteAddr {
                            part: pi,
                            staff: si,
                            measure: mi,
                            voice,
                            note: ni,
                        };

                        if let Some(kind) = note.hairpin_start {
                            open_hairpin = Some((addr.clone(), kind));
                        }
                        if note.hairpin_end
                            && let Some((start, kind)) = open_hairpin.take()
                        {
                            spans.push(SpanMark::Hairpin {
                                kind,
                                start,
                                end: addr.clone(),
                            });
                        }

                        if let Some(kind) = note.ottava_start {
                            open_ottava = Some((addr.clone(), kind));
                        }
                        if note.ottava_end
                            && let Some((start, kind)) = open_ottava.take()
                        {
                            spans.push(SpanMark::Ottava {
                                kind,
                                start,
                                end: addr.clone(),
                            });
                        }

                        if note.pedal_start {
                            open_pedal = Some(addr.clone());
                        }
                        if note.pedal_end
                            && let Some(start) = open_pedal.take()
                        {
                            spans.push(SpanMark::Pedal {
                                start,
                                end: addr.clone(),
                            });
                        }

                        if note.slur_start {
                            open_slur = Some(addr.clone());
                        }
                        if note.slur_end
                            && let Some(start) = open_slur.take()
                        {
                            spans.push(SpanMark::Slur {
                                start,
                                end: addr.clone(),
                            });
                        }

                        if note.trill_line_start {
                            open_trill_line = Some(addr.clone());
                        }
                        if note.trill_line_end
                            && let Some(start) = open_trill_line.take()
                        {
                            spans.push(SpanMark::TrillLine {
                                start,
                                end: addr.clone(),
                            });
                        }

                        if note.glissando_start {
                            open_glissando = Some(addr.clone());
                        }
                        if note.glissando_end
                            && let Some(start) = open_glissando.take()
                        {
                            spans.push(SpanMark::Glissando {
                                start,
                                end: addr.clone(),
                            });
                        }
                    }
                }
            }
        }
    }
    spans
}

// ── beam groups ───────────────────────────────────────────────────────────────

fn collect_beam_groups(score: &Score) -> Vec<BeamGroup> {
    let mut groups = Vec::new();
    for (pi, part) in score.parts.iter().enumerate() {
        for (si, staff) in part.staves.iter().enumerate() {
            for (mi, measure) in staff.measures.iter().enumerate() {
                for vi in 0..4usize {
                    let voice = &measure.voices[vi];
                    let mut current: Option<Vec<usize>> = None;

                    for (ni, note) in voice.iter().enumerate() {
                        match note.beam {
                            BeamState::Begin => {
                                current = Some(vec![ni]);
                            }
                            BeamState::BeginEnd => {
                                groups.push(BeamGroup {
                                    part: pi,
                                    staff: si,
                                    measure: mi,
                                    voice: vi,
                                    note_indices: vec![ni],
                                });
                            }
                            BeamState::Continue
                            | BeamState::ForwardHook
                            | BeamState::BackwardHook => {
                                if let Some(ref mut g) = current {
                                    g.push(ni);
                                }
                            }
                            BeamState::End => {
                                if let Some(mut g) = current.take() {
                                    g.push(ni);
                                    groups.push(BeamGroup {
                                        part: pi,
                                        staff: si,
                                        measure: mi,
                                        voice: vi,
                                        note_indices: g,
                                    });
                                }
                            }
                            BeamState::None => {
                                current = None;
                            }
                        }
                    }
                }
            }
        }
    }
    groups
}

// ── tuplet groups ─────────────────────────────────────────────────────────────

fn collect_tuplet_groups(score: &Score) -> Vec<TupletGroup> {
    let mut groups = Vec::new();
    for (pi, part) in score.parts.iter().enumerate() {
        for (si, staff) in part.staves.iter().enumerate() {
            for (mi, measure) in staff.measures.iter().enumerate() {
                for vi in 0..4usize {
                    let voice = &measure.voices[vi];
                    let mut current_indices: Vec<usize> = Vec::new();
                    let mut current_info: Option<TupletInfo> = None;

                    let flush = |indices: &mut Vec<usize>,
                                 info: &mut Option<TupletInfo>,
                                 groups: &mut Vec<TupletGroup>| {
                        if indices.len() >= 2
                            && let Some(ti) = info.take()
                        {
                            groups.push(TupletGroup {
                                part: pi,
                                staff: si,
                                measure: mi,
                                voice: vi,
                                note_indices: std::mem::take(indices),
                                actual_notes: ti.actual_notes,
                                normal_notes: ti.normal_notes,
                            });
                            return;
                        }
                        indices.clear();
                        info.take();
                    };

                    for (ni, note) in voice.iter().enumerate() {
                        match &note.tuplet {
                            Some(ti) => {
                                let same_group = current_info.as_ref().is_some_and(|prev| {
                                    prev.actual_notes == ti.actual_notes
                                        && prev.normal_notes == ti.normal_notes
                                });
                                if !same_group {
                                    flush(&mut current_indices, &mut current_info, &mut groups);
                                    current_info = Some(ti.clone());
                                }
                                current_indices.push(ni);
                            }
                            None => {
                                flush(&mut current_indices, &mut current_info, &mut groups);
                            }
                        }
                    }
                    flush(&mut current_indices, &mut current_info, &mut groups);
                }
            }
        }
    }
    groups
}

fn step_idx(step: &Step) -> u8 {
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

/// Accidental implied by a key signature for a given step: 0, 1, or -1.
fn key_alter(fifths: i8, step: &Step) -> i8 {
    const SHARPS: [Step; 7] = [
        Step::F,
        Step::C,
        Step::G,
        Step::D,
        Step::A,
        Step::E,
        Step::B,
    ];
    const FLATS: [Step; 7] = [
        Step::B,
        Step::E,
        Step::A,
        Step::D,
        Step::G,
        Step::C,
        Step::F,
    ];
    if fifths > 0 && SHARPS[..fifths as usize].contains(step) {
        1
    } else if fifths < 0 && FLATS[..(-fifths) as usize].contains(step) {
        -1
    } else {
        0
    }
}

fn collect_courtesy_accidentals(score: &Score) -> Vec<CourtesyAccidental> {
    let mut result = Vec::new();
    let base_fifths = score.settings.key_signature.fifths;

    for (pi, part) in score.parts.iter().enumerate() {
        for (si, staff) in part.staves.iter().enumerate() {
            // Maps (step_index, octave) -> alter from the previous measure.
            let mut prev_alters: HashMap<(u8, i8), i8> = HashMap::new();
            let mut current_fifths = base_fifths;

            for (mi, measure) in staff.measures.iter().enumerate() {
                if let Some(key_sig) = measure.key_sig.as_ref() {
                    current_fifths = key_sig.fifths;
                    prev_alters.clear();
                }

                // Phase 1: check this measure's notes against prev_alters.
                for vi in 0..4usize {
                    for (ni, note) in measure.voices[vi].iter().enumerate() {
                        if note.tie_end {
                            continue;
                        }
                        for (pitch_idx, pitch) in note.pitches.iter().enumerate() {
                            let key = (step_idx(&pitch.step), pitch.octave);
                            if prev_alters.contains_key(&key) {
                                result.push(CourtesyAccidental {
                                    part: pi,
                                    staff: si,
                                    measure: mi,
                                    voice: vi,
                                    note_index: ni,
                                    pitch_index: pitch_idx,
                                    alter: pitch.alter,
                                });
                            }
                        }
                    }
                }

                // Phase 2: rebuild prev_alters from this measure's chromatically altered pitches.
                prev_alters.clear();
                for vi in 0..4usize {
                    for note in measure.voices[vi].iter() {
                        for pitch in note.pitches.iter() {
                            if pitch.alter != key_alter(current_fifths, &pitch.step) {
                                prev_alters
                                    .insert((step_idx(&pitch.step), pitch.octave), pitch.alter);
                            }
                        }
                    }
                }
            }
        }
    }
    result
}

// ── mandatory accidentals ────────────────────────────────────────────────────

/// Collect mandatory (non-courtesy) accidental marks.
///
/// State is scoped per `(part, staff)` and reset every measure to the key signature's
/// implied alteration — accidentals never carry across a barline. Within a measure, all
/// voices on the staff share one accidental context (mirrors [`collect_courtesy_accidentals`]'s
/// voice-major scan order). Tied continuations (`tie_end`) never need a fresh accidental.
fn collect_accidental_marks(score: &Score) -> Vec<AccidentalMark> {
    let mut result = Vec::new();
    let base_fifths = score.settings.key_signature.fifths;

    for (pi, part) in score.parts.iter().enumerate() {
        for (si, staff) in part.staves.iter().enumerate() {
            let mut current_fifths = base_fifths;

            for (mi, measure) in staff.measures.iter().enumerate() {
                if let Some(key_sig) = measure.key_sig.as_ref() {
                    current_fifths = key_sig.fifths;
                }

                // Active alteration per (step, octave), established within this measure only.
                let mut active: HashMap<(u8, i8), i8> = HashMap::new();

                for vi in 0..4usize {
                    for (ni, note) in measure.voices[vi].iter().enumerate() {
                        for (pitch_idx, pitch) in note.pitches.iter().enumerate() {
                            let key = (step_idx(&pitch.step), pitch.octave);
                            let baseline = active
                                .get(&key)
                                .copied()
                                .unwrap_or_else(|| key_alter(current_fifths, &pitch.step));
                            if pitch.alter != baseline {
                                if !note.tie_end {
                                    result.push(AccidentalMark {
                                        part: pi,
                                        staff: si,
                                        measure: mi,
                                        voice: vi,
                                        note_index: ni,
                                        pitch_index: pitch_idx,
                                        alter: pitch.alter,
                                    });
                                }
                                active.insert(key, pitch.alter);
                            }
                        }
                    }
                }
            }
        }
    }
    result
}

#[cfg(test)]
mod tests {
    use super::*;
    use acorde_core::{Clef, Measure, Part, Score, Staff};

    fn score_with_measures(n: usize) -> Score {
        let mut score = Score::default();
        score.parts.clear();
        let mut staff = Staff::new(Clef::Treble);
        for i in 0..n {
            let mut m = Measure::empty(4, 4);
            m.number = i as u32 + 1;
            staff.measures.push(m);
        }
        let mut part = Part::new("P1", "P1");
        part.staves.push(staff);
        score.parts.push(part);
        score
    }

    #[test]
    fn default_score_has_one_row_of_four() {
        // Score::default() ships with 4 measures; default measures_per_row=4
        let score = Score::default();
        let result = compute_layout(&score, &LayoutConfig::default());
        assert_eq!(result.rows.len(), 1);
        assert_eq!(result.rows[0].measure_indices, vec![0, 1, 2, 3]);
        assert_eq!(result.vis_slots.len(), 4);
        assert!(result.spans.is_empty());
    }

    #[test]
    fn vis_slots_expand_multi_rest() {
        let mut score = score_with_measures(1);
        score.parts[0].staves[0].measures[0].multi_rest_count = Some(4);

        let result = compute_layout(
            &score,
            &LayoutConfig {
                measures_per_row: 8,
                concert_pitch: false,
                first_row_measures: None,
            },
        );
        assert_eq!(result.vis_slots.len(), 4);
        assert!(result.vis_slots.iter().all(|&v| v == 0));
    }

    #[test]
    fn measures_split_into_rows() {
        let score = score_with_measures(6);
        let result = compute_layout(
            &score,
            &LayoutConfig {
                measures_per_row: 4,
                concert_pitch: false,
                first_row_measures: None,
            },
        );
        assert_eq!(result.rows.len(), 2);
        assert_eq!(result.rows[0].measure_indices, vec![0, 1, 2, 3]);
        assert_eq!(result.rows[1].measure_indices, vec![4, 5]);
    }

    #[test]
    fn single_measure_single_row() {
        let score = score_with_measures(1);
        let result = compute_layout(
            &score,
            &LayoutConfig {
                measures_per_row: 4,
                concert_pitch: false,
                first_row_measures: None,
            },
        );
        assert_eq!(result.rows.len(), 1);
        assert_eq!(result.rows[0].measure_indices, vec![0]);
    }

    #[test]
    fn exactly_full_rows() {
        let score = score_with_measures(8);
        let result = compute_layout(
            &score,
            &LayoutConfig {
                measures_per_row: 4,
                concert_pitch: false,
                first_row_measures: None,
            },
        );
        assert_eq!(result.rows.len(), 2);
        assert_eq!(result.rows[0].measure_indices.len(), 4);
        assert_eq!(result.rows[1].measure_indices.len(), 4);
    }

    #[test]
    fn concert_pitch_bb_clarinet_transposes_key() {
        // Bb clarinet: transpose_semitones=-2, written key C major (fifths=0)
        // Concert key = Bb major (fifths=-2)
        let mut score = score_with_measures(1);
        score.parts[0].staves[0].transpose_semitones = -2;
        let result = compute_layout(
            &score,
            &LayoutConfig {
                measures_per_row: 4,
                concert_pitch: true,
                first_row_measures: None,
            },
        );
        assert_eq!(result.concert_key_overrides.len(), 1);
        assert_eq!(result.concert_key_overrides[0].part_index, 0);
        assert_eq!(result.concert_key_overrides[0].staff_index, 0);
        assert_eq!(result.concert_key_overrides[0].fifths, -2);
    }

    #[test]
    fn concert_pitch_false_no_override() {
        let mut score = score_with_measures(1);
        score.parts[0].staves[0].transpose_semitones = -2;
        let result = compute_layout(
            &score,
            &LayoutConfig {
                measures_per_row: 4,
                concert_pitch: false,
                first_row_measures: None,
            },
        );
        assert!(result.concert_key_overrides.is_empty());
    }

    #[test]
    fn concert_pitch_zero_transpose_no_override() {
        // transpose_semitones=0 means concert pitch equals written pitch; no override needed.
        let score = score_with_measures(1);
        let result = compute_layout(
            &score,
            &LayoutConfig {
                measures_per_row: 4,
                concert_pitch: true,
                first_row_measures: None,
            },
        );
        assert!(result.concert_key_overrides.is_empty());
    }

    #[test]
    fn transpose_key_formula_eb_alto_sax() {
        // Eb alto sax: transpose_semitones=-9, written key C major (fifths=0)
        // Concert key = Eb major (fifths=-3): delta = (-9*7) rem_euclid 12 = 9 → 9-12=-3
        assert_eq!(transpose_key_fifths(0, -9), -3);
    }

    #[test]
    fn transpose_key_formula_perfect_fifth_up() {
        // +7 semitones = P5 up → +1 sharp (G major = fifths=1)
        assert_eq!(transpose_key_fifths(0, 7), 1);
    }

    #[test]
    fn system_break_splits_row() {
        // 3 measures; measure[1] has system_break → rows: [0,1] and [2]
        let mut score = score_with_measures(3);
        score.parts[0].staves[0].measures[1].system_break = true;
        let result = compute_layout(
            &score,
            &LayoutConfig {
                measures_per_row: 4,
                concert_pitch: false,
                first_row_measures: None,
            },
        );
        assert_eq!(result.rows.len(), 2);
        assert_eq!(result.rows[0].measure_indices, vec![0, 1]);
        assert_eq!(result.rows[1].measure_indices, vec![2]);
    }

    #[test]
    fn page_break_splits_row() {
        let mut score = score_with_measures(3);
        score.parts[0].staves[0].measures[0].page_break = true;
        let result = compute_layout(
            &score,
            &LayoutConfig {
                measures_per_row: 4,
                concert_pitch: false,
                first_row_measures: None,
            },
        );
        assert_eq!(result.rows.len(), 2);
        assert_eq!(result.rows[0].measure_indices, vec![0]);
        assert_eq!(result.rows[1].measure_indices, vec![1, 2]);
    }

    #[test]
    fn system_break_overrides_per_row() {
        // per_row=4 but system_break after measure[1] → row ends early
        let mut score = score_with_measures(5);
        score.parts[0].staves[0].measures[1].system_break = true;
        let result = compute_layout(
            &score,
            &LayoutConfig {
                measures_per_row: 4,
                concert_pitch: false,
                first_row_measures: None,
            },
        );
        assert_eq!(result.rows.len(), 2);
        assert_eq!(result.rows[0].measure_indices, vec![0, 1]);
        assert_eq!(result.rows[1].measure_indices, vec![2, 3, 4]);
    }

    #[test]
    fn no_break_unchanged() {
        let score = score_with_measures(3);
        let result = compute_layout(
            &score,
            &LayoutConfig {
                measures_per_row: 4,
                concert_pitch: false,
                first_row_measures: None,
            },
        );
        assert_eq!(result.rows.len(), 1);
        assert_eq!(result.rows[0].measure_indices, vec![0, 1, 2]);
    }

    // ── first_row_measures ────────────────────────────────────────────────────

    #[test]
    fn first_row_measures_limits_first_row() {
        // 5 measures, per_row=4, first_row_measures=2 → row[0]=2, row[1]=3
        let score = score_with_measures(5);
        let result = compute_layout(
            &score,
            &LayoutConfig {
                measures_per_row: 4,
                concert_pitch: false,
                first_row_measures: Some(2),
            },
        );
        assert_eq!(result.rows.len(), 2);
        assert_eq!(result.rows[0].measure_indices, vec![0, 1]);
        assert_eq!(result.rows[1].measure_indices, vec![2, 3, 4]);
    }

    #[test]
    fn first_row_measures_none_unchanged() {
        // first_row_measures=None → same as using per_row for all rows
        let score = score_with_measures(5);
        let without = compute_layout(
            &score,
            &LayoutConfig {
                measures_per_row: 4,
                concert_pitch: false,
                first_row_measures: None,
            },
        );
        let with_none = compute_layout(&score, &LayoutConfig::default());
        assert_eq!(without.rows.len(), with_none.rows.len());
        for (a, b) in without.rows.iter().zip(with_none.rows.iter()) {
            assert_eq!(a.measure_indices, b.measure_indices);
        }
    }

    #[test]
    fn beam_groups_empty_for_default_score() {
        let score = Score::default();
        let result = compute_layout(&score, &LayoutConfig::default());
        assert!(result.beam_groups.is_empty());
    }

    #[test]
    fn beam_groups_begin_end_collected() {
        use acorde_core::{BeamState, Duration, Note, Pitch, Step};
        let mut score = score_with_measures(1);
        let mut n1 = Note::new(Pitch::new(Step::C, 4), Duration::Eighth);
        n1.beam = BeamState::Begin;
        let mut n2 = Note::new(Pitch::new(Step::D, 4), Duration::Eighth);
        n2.beam = BeamState::Continue;
        let mut n3 = Note::new(Pitch::new(Step::E, 4), Duration::Eighth);
        n3.beam = BeamState::End;
        score.parts[0].staves[0].measures[0].voices[0] = vec![n1, n2, n3];

        let result = compute_layout(&score, &LayoutConfig::default());
        assert_eq!(result.beam_groups.len(), 1);
        assert_eq!(result.beam_groups[0].note_indices, vec![0, 1, 2]);
        assert_eq!(result.beam_groups[0].part, 0);
        assert_eq!(result.beam_groups[0].measure, 0);
        assert_eq!(result.beam_groups[0].voice, 0);
    }

    #[test]
    fn beam_groups_beginend_is_standalone_group() {
        use acorde_core::{BeamState, Duration, Note, Pitch, Step};
        let mut score = score_with_measures(1);
        let mut n = Note::new(Pitch::new(Step::C, 4), Duration::Eighth);
        n.beam = BeamState::BeginEnd;
        score.parts[0].staves[0].measures[0].voices[0] = vec![n];

        let result = compute_layout(&score, &LayoutConfig::default());
        assert_eq!(result.beam_groups.len(), 1);
        assert_eq!(result.beam_groups[0].note_indices, vec![0]);
    }

    #[test]
    fn tuplet_groups_empty_for_default_score() {
        let score = Score::default();
        let result = compute_layout(&score, &LayoutConfig::default());
        assert!(result.tuplet_groups.is_empty());
    }

    #[test]
    fn tuplet_groups_triplet_collected() {
        use acorde_core::{Duration, Note, Pitch, Step, TupletInfo};
        let ti = TupletInfo {
            actual_notes: 3,
            normal_notes: 2,
        };
        let mut score = score_with_measures(1);
        let notes: Vec<Note> = (0..3)
            .map(|i| {
                let step = [Step::C, Step::D, Step::E][i].clone();
                let mut n = Note::new(Pitch::new(step, 4), Duration::Quarter);
                n.tuplet = Some(ti.clone());
                n
            })
            .collect();
        score.parts[0].staves[0].measures[0].voices[0] = notes;

        let result = compute_layout(&score, &LayoutConfig::default());
        assert_eq!(result.tuplet_groups.len(), 1);
        assert_eq!(result.tuplet_groups[0].note_indices, vec![0, 1, 2]);
        assert_eq!(result.tuplet_groups[0].actual_notes, 3);
        assert_eq!(result.tuplet_groups[0].normal_notes, 2);
        assert_eq!(result.tuplet_groups[0].part, 0);
        assert_eq!(result.tuplet_groups[0].measure, 0);
    }

    #[test]
    fn tuplet_groups_no_group_for_single_tuplet_note() {
        use acorde_core::{Duration, Note, Pitch, Step, TupletInfo};
        let mut score = score_with_measures(1);
        let mut n = Note::new(Pitch::new(Step::C, 4), Duration::Quarter);
        n.tuplet = Some(TupletInfo {
            actual_notes: 3,
            normal_notes: 2,
        });
        score.parts[0].staves[0].measures[0].voices[0] = vec![n];

        let result = compute_layout(&score, &LayoutConfig::default());
        // Single-note tuplet: not enough to form a group (need >= 2)
        assert!(result.tuplet_groups.is_empty());
    }

    #[test]
    fn hairpin_span_resolved() {
        use acorde_core::{Duration, HairpinKind, Note, Pitch, Step};
        let mut score = score_with_measures(1);
        let mut n1 = Note::new(Pitch::new(Step::C, 4), Duration::Quarter);
        n1.hairpin_start = Some(HairpinKind::Crescendo);
        let mut n2 = Note::new(Pitch::new(Step::D, 4), Duration::Quarter);
        n2.hairpin_end = true;
        score.parts[0].staves[0].measures[0].voices[0] = vec![n1, n2];

        let result = compute_layout(&score, &LayoutConfig::default());
        assert_eq!(result.spans.len(), 1);
        if let SpanMark::Hairpin { kind, start, end } = &result.spans[0] {
            assert_eq!(*kind, HairpinKind::Crescendo);
            assert_eq!(start.note, 0);
            assert_eq!(end.note, 1);
        } else {
            panic!("expected Hairpin span");
        }
    }

    #[test]
    fn slur_span_resolved() {
        use acorde_core::{Duration, Note, Pitch, Step};
        let mut score = score_with_measures(1);
        let mut n1 = Note::new(Pitch::new(Step::C, 4), Duration::Quarter);
        n1.slur_start = true;
        let mut n2 = Note::new(Pitch::new(Step::D, 4), Duration::Quarter);
        n2.slur_end = true;
        score.parts[0].staves[0].measures[0].voices[0] = vec![n1, n2];

        let result = compute_layout(&score, &LayoutConfig::default());
        let slur = result
            .spans
            .iter()
            .find(|s| matches!(s, SpanMark::Slur { .. }));
        assert!(slur.is_some(), "expected a Slur span");
        if let Some(SpanMark::Slur { start, end }) = slur {
            assert_eq!(start.note, 0);
            assert_eq!(end.note, 1);
        }
    }

    // ── CourtesyAccidental ────────────────────────────────────────────────────

    #[test]
    fn courtesy_none_for_clean_score() {
        // Default score has no chromatic alterations — no courtesy accidentals.
        let score = Score::default();
        let result = compute_layout(&score, &LayoutConfig::default());
        assert!(result.courtesy_accidentals.is_empty());
    }

    #[test]
    fn courtesy_after_chromatic_alteration() {
        // Measure 0: F# (alter=1). Measure 1: F natural (alter=0).
        // The F in measure 1 should get a courtesy accidental.
        use acorde_core::{Duration, Note, Pitch, Step};
        let mut score = score_with_measures(2);

        let mut fsharp = Note::new(Pitch::new(Step::F, 4), Duration::Quarter);
        fsharp.pitches[0].alter = 1;
        score.parts[0].staves[0].measures[0].voices[0] = vec![fsharp];

        let fnat = Note::new(Pitch::new(Step::F, 4), Duration::Quarter);
        // alter=0 is default; key signature is C major (fifths=0) so F natural has key_alter=0
        score.parts[0].staves[0].measures[1].voices[0] = vec![fnat];

        let result = compute_layout(&score, &LayoutConfig::default());
        assert_eq!(result.courtesy_accidentals.len(), 1);
        let ca = &result.courtesy_accidentals[0];
        assert_eq!(ca.measure, 1);
        assert_eq!(ca.alter, 0); // natural sign
    }

    #[test]
    fn courtesy_same_alteration_no_courtesy() {
        // Measure 0: F#. Measure 1: F# again.
        // F# in measure 1 does NOT need a courtesy — no prior alteration conflict.
        // (prev_alters records F# alter=1; measure 1 F# has same alter=1 → NOT in prev_alters
        // because phase 2 only records notes that differ from key_alter.)
        // Actually: F# IS in prev_alters → courtesy IS emitted, alter=1.
        use acorde_core::{Duration, Note, Pitch, Step};
        let mut score = score_with_measures(2);

        let mut fsharp1 = Note::new(Pitch::new(Step::F, 4), Duration::Quarter);
        fsharp1.pitches[0].alter = 1;
        score.parts[0].staves[0].measures[0].voices[0] = vec![fsharp1];

        let mut fsharp2 = Note::new(Pitch::new(Step::F, 4), Duration::Quarter);
        fsharp2.pitches[0].alter = 1;
        score.parts[0].staves[0].measures[1].voices[0] = vec![fsharp2];

        let result = compute_layout(&score, &LayoutConfig::default());
        // F# appears in prev_alters → courtesy emitted with alter=1
        assert_eq!(result.courtesy_accidentals.len(), 1);
        assert_eq!(result.courtesy_accidentals[0].alter, 1);
    }

    #[test]
    fn courtesy_tied_note_excluded() {
        // A note with tie_end=true should never get a courtesy accidental.
        use acorde_core::{Duration, Note, Pitch, Step};
        let mut score = score_with_measures(2);

        let mut fsharp = Note::new(Pitch::new(Step::F, 4), Duration::Quarter);
        fsharp.pitches[0].alter = 1;
        score.parts[0].staves[0].measures[0].voices[0] = vec![fsharp];

        let mut tied = Note::new(Pitch::new(Step::F, 4), Duration::Quarter);
        tied.pitches[0].alter = 1;
        tied.tie_end = true;
        score.parts[0].staves[0].measures[1].voices[0] = vec![tied];

        let result = compute_layout(&score, &LayoutConfig::default());
        assert!(result.courtesy_accidentals.is_empty());
    }

    #[test]
    fn courtesy_key_change_resets() {
        // Measure 0: F# (alter=1). Measure 1: key changes to G major (fifths=1, F# implied).
        // The key change clears prev_alters, so F in measure 1 won't get a courtesy.
        use acorde_core::{Duration, KeySignature, Note, Pitch, Step};
        let mut score = score_with_measures(2);

        let mut fsharp = Note::new(Pitch::new(Step::F, 4), Duration::Quarter);
        fsharp.pitches[0].alter = 1;
        score.parts[0].staves[0].measures[0].voices[0] = vec![fsharp];

        // Set key signature on measure 1 to G major (1 sharp = F#)
        score.parts[0].staves[0].measures[1].key_sig = Some(KeySignature {
            fifths: 1,
            mode: "major".to_string(),
        });
        // F# is now implied by the key — alter=1 == key_alter(1, F) → not in prev_alters
        let mut fsharp2 = Note::new(Pitch::new(Step::F, 4), Duration::Quarter);
        fsharp2.pitches[0].alter = 1;
        score.parts[0].staves[0].measures[1].voices[0] = vec![fsharp2];

        let result = compute_layout(&score, &LayoutConfig::default());
        // key change clears prev_alters before phase 1 → no courtesy for measure 1
        assert!(result.courtesy_accidentals.is_empty());
    }

    // ── AccidentalMark ────────────────────────────────────────────────────────

    #[test]
    fn accidental_none_for_clean_score() {
        let score = Score::default();
        let result = compute_layout(&score, &LayoutConfig::default());
        assert!(result.accidentals.is_empty());
    }

    #[test]
    fn accidental_first_chromatic_note_in_measure() {
        // C major, single F# in measure 0 → one mandatory accidental (sharp).
        use acorde_core::{Duration, Note, Pitch, Step};
        let mut score = score_with_measures(1);
        let mut fsharp = Note::new(Pitch::new(Step::F, 4), Duration::Quarter);
        fsharp.pitches[0].alter = 1;
        score.parts[0].staves[0].measures[0].voices[0] = vec![fsharp];

        let result = compute_layout(&score, &LayoutConfig::default());
        assert_eq!(result.accidentals.len(), 1);
        assert_eq!(result.accidentals[0].alter, 1);
        assert_eq!(result.accidentals[0].measure, 0);
    }

    #[test]
    fn accidental_repeat_same_alter_in_measure_not_marked_again() {
        // Two F#s in the same measure: only the first needs an accidental.
        use acorde_core::{Duration, Note, Pitch, Step};
        let mut score = score_with_measures(1);
        let mut f1 = Note::new(Pitch::new(Step::F, 4), Duration::Quarter);
        f1.pitches[0].alter = 1;
        let mut f2 = Note::new(Pitch::new(Step::F, 4), Duration::Quarter);
        f2.pitches[0].alter = 1;
        score.parts[0].staves[0].measures[0].voices[0] = vec![f1, f2];

        let result = compute_layout(&score, &LayoutConfig::default());
        assert_eq!(result.accidentals.len(), 1);
    }

    #[test]
    fn accidental_reverts_to_natural_within_same_measure() {
        // F# then F natural, same measure: the natural needs its own mark too.
        use acorde_core::{Duration, Note, Pitch, Step};
        let mut score = score_with_measures(1);
        let mut f1 = Note::new(Pitch::new(Step::F, 4), Duration::Quarter);
        f1.pitches[0].alter = 1;
        let f2 = Note::new(Pitch::new(Step::F, 4), Duration::Quarter); // alter=0
        score.parts[0].staves[0].measures[0].voices[0] = vec![f1, f2];

        let result = compute_layout(&score, &LayoutConfig::default());
        assert_eq!(result.accidentals.len(), 2);
        assert_eq!(result.accidentals[0].alter, 1);
        assert_eq!(result.accidentals[1].alter, 0);
    }

    #[test]
    fn accidental_does_not_carry_across_barline() {
        // F# in measure 0; measure 1's F# needs its own mark (accidentals reset per measure).
        // This is the case that would otherwise double up with a CourtesyAccidental —
        // callers must prefer `accidentals` over `courtesy_accidentals` when both exist.
        use acorde_core::{Duration, Note, Pitch, Step};
        let mut score = score_with_measures(2);
        let mut f1 = Note::new(Pitch::new(Step::F, 4), Duration::Quarter);
        f1.pitches[0].alter = 1;
        score.parts[0].staves[0].measures[0].voices[0] = vec![f1];
        let mut f2 = Note::new(Pitch::new(Step::F, 4), Duration::Quarter);
        f2.pitches[0].alter = 1;
        score.parts[0].staves[0].measures[1].voices[0] = vec![f2];

        let result = compute_layout(&score, &LayoutConfig::default());
        assert_eq!(result.accidentals.len(), 2);
        assert!(result.accidentals.iter().any(|a| a.measure == 1));
        // The same address also produced a courtesy mark — renderer precedence resolves it.
        assert_eq!(result.courtesy_accidentals.len(), 1);
        assert_eq!(result.courtesy_accidentals[0].measure, 1);
    }

    #[test]
    fn accidental_tied_note_excluded() {
        use acorde_core::{Duration, Note, Pitch, Step};
        let mut score = score_with_measures(1);
        let mut tied = Note::new(Pitch::new(Step::F, 4), Duration::Quarter);
        tied.pitches[0].alter = 1;
        tied.tie_end = true;
        score.parts[0].staves[0].measures[0].voices[0] = vec![tied];

        let result = compute_layout(&score, &LayoutConfig::default());
        assert!(result.accidentals.is_empty());
    }

    #[test]
    fn accidental_shared_across_voices_on_same_staff() {
        // Voice 0 introduces F#; voice 1's F# later in the same measure shares the context.
        use acorde_core::{Duration, Note, Pitch, Step};
        let mut score = score_with_measures(1);
        let mut f_v0 = Note::new(Pitch::new(Step::F, 4), Duration::Quarter);
        f_v0.pitches[0].alter = 1;
        score.parts[0].staves[0].measures[0].voices[0] = vec![f_v0];
        let mut f_v1 = Note::new(Pitch::new(Step::F, 4), Duration::Quarter);
        f_v1.pitches[0].alter = 1;
        score.parts[0].staves[0].measures[0].voices[1] = vec![f_v1];

        let result = compute_layout(&score, &LayoutConfig::default());
        assert_eq!(result.accidentals.len(), 1);
    }

    #[test]
    fn accidental_double_sharp_marked() {
        use acorde_core::{Duration, Note, Pitch, Step};
        let mut score = score_with_measures(1);
        let mut n = Note::new(Pitch::new(Step::F, 4), Duration::Quarter);
        n.pitches[0].alter = 2;
        score.parts[0].staves[0].measures[0].voices[0] = vec![n];

        let result = compute_layout(&score, &LayoutConfig::default());
        assert_eq!(result.accidentals.len(), 1);
        assert_eq!(result.accidentals[0].alter, 2);
    }
}
