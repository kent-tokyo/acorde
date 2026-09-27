//! Measure-structure editing commands and their typed-spanner address transforms.

use super::spanner_remap::remap_spanners;
use super::{Barline, Error, JoinMeasuresCmd, NoteAddr, Score, SplitMeasureCmd};
use crate::{Clef, MeasureLength, MidMeasureClef};

/// Changes before `at` beats, and those after it re-based to `at` plus any change exactly at it.
type SplitClefs = (Vec<MidMeasureClef>, (Vec<MidMeasureClef>, Option<Clef>));

fn split_mid_clefs(changes: &[MidMeasureClef], at: f64) -> SplitClefs {
    let mut left = Vec::new();
    let mut right = Vec::new();
    let mut at_split = None;
    for change in changes {
        let Some(offset) = change.offset.beats() else {
            continue;
        };
        if offset < at - 1e-9 {
            left.push(change.clone());
        } else if offset <= at + 1e-9 {
            at_split = Some(change.clef.clone());
        } else if let Some(offset) = MeasureLength::from_beats(offset - at) {
            right.push(MidMeasureClef {
                offset,
                clef: change.clef.clone(),
            });
        }
    }
    (left, (right, at_split))
}

pub(super) fn apply_split_measure(cmd: &SplitMeasureCmd, score: &mut Score) -> Result<(), Error> {
    if !cmd.split_at_beats.is_finite() || cmd.split_at_beats <= 0.0 {
        return Err(Error::InvalidCommand("split point must be positive".into()));
    }
    let mut boundaries = Vec::new();
    for (part_index, part) in score.parts.iter().enumerate() {
        for (staff_index, staff) in part.staves.iter().enumerate() {
            let measure = staff
                .measures
                .get(cmd.measure_index)
                .ok_or(Error::MeasureNotFound(cmd.measure_index))?;
            let expected = staff.measure_beats(cmd.measure_index, &score.settings.time_signature);
            if cmd.split_at_beats >= expected - 1e-9 {
                return Err(Error::InvalidCommand(
                    "split point must be inside measure".into(),
                ));
            }
            let mut indices = [0usize; 4];
            for (voice_index, voice) in measure.voices.iter().enumerate() {
                let mut beats = 0.0;
                let mut found = false;
                for (index, note) in voice.iter().enumerate() {
                    beats += note.beats();
                    if (beats - cmd.split_at_beats).abs() < 1e-9 {
                        indices[voice_index] = index + 1;
                        found = true;
                        break;
                    }
                    if beats > cmd.split_at_beats {
                        break;
                    }
                }
                if !voice.is_empty() && !found {
                    return Err(Error::InvalidCommand(
                        "split point must align with every voice note boundary".into(),
                    ));
                }
            }
            boundaries.push((part_index, staff_index, indices));
        }
    }
    for (part_index, staff_index, indices) in &boundaries {
        let measure =
            &mut score.parts[*part_index].staves[*staff_index].measures[cmd.measure_index];
        let mut right = measure.clone();
        for (voice_index, split_index) in indices.iter().enumerate() {
            right.voices[voice_index] = measure.voices[voice_index].split_off(*split_index);
        }
        // Mid-bar clefs stay with their half; one at the split point begins the right bar,
        // which otherwise continues in the clef in effect there (no restated clef).
        let (left_clefs, (right_clefs, at_split)) =
            split_mid_clefs(&measure.mid_clefs, cmd.split_at_beats);
        measure.mid_clefs = left_clefs;
        right.mid_clefs = right_clefs;
        right.clef = at_split;
        measure.barline_right = Barline::Normal;
        right.barline_left = Barline::Normal;
        score.parts[*part_index].staves[*staff_index]
            .measures
            .insert(cmd.measure_index + 1, right);
        for (index, measure) in score.parts[*part_index].staves[*staff_index]
            .measures
            .iter_mut()
            .enumerate()
        {
            measure.number = index as u32 + 1;
        }
    }
    remap_spanners(score, |address| {
        if address.measure < cmd.measure_index {
            return Some(address.clone());
        }
        if address.measure > cmd.measure_index {
            let mut shifted = address.clone();
            shifted.measure += 1;
            return Some(shifted);
        }
        let split = boundaries
            .iter()
            .find(|(part, staff, _)| *part == address.part && *staff == address.staff)?
            .2[address.voice];
        if address.note >= split {
            Some(NoteAddr {
                measure: cmd.measure_index + 1,
                note: address.note - split,
                ..address.clone()
            })
        } else {
            Some(address.clone())
        }
    });
    Ok(())
}

pub(super) fn apply_join_measures(cmd: &JoinMeasuresCmd, score: &mut Score) -> Result<(), Error> {
    let mut offsets = Vec::new();
    for (part_index, part) in score.parts.iter().enumerate() {
        for (staff_index, staff) in part.staves.iter().enumerate() {
            let left = staff
                .measures
                .get(cmd.measure_index)
                .ok_or(Error::MeasureNotFound(cmd.measure_index))?;
            staff
                .measures
                .get(cmd.measure_index + 1)
                .ok_or(Error::MeasureNotFound(cmd.measure_index + 1))?;
            let mut counts = [0usize; 4];
            for (voice, notes) in left.voices.iter().enumerate() {
                counts[voice] = notes.len();
            }
            offsets.push((part_index, staff_index, counts));
        }
    }
    for (part, staff, _) in &offsets {
        let measures = &mut score.parts[*part].staves[*staff].measures;
        let right = measures.remove(cmd.measure_index + 1);
        let left = &mut measures[cmd.measure_index];
        // The right bar's clef changes become mid-bar changes of the joined bar.
        // Where the right bar begins: the end of the left bar's written content.
        let left_beats = left
            .voices
            .iter()
            .map(|voice| {
                voice
                    .iter()
                    .filter(|note| !note.is_grace)
                    .map(|note| note.beats())
                    .sum::<f64>()
            })
            .fold(0.0, f64::max);
        let joined = right.clef.iter().map(|clef| (0.0, clef.clone())).chain(
            right
                .mid_clefs
                .iter()
                .filter_map(|change| Some((change.offset.beats()?, change.clef.clone()))),
        );
        for (offset, clef) in joined {
            if let Some(offset) = MeasureLength::from_beats(left_beats + offset) {
                left.mid_clefs.push(MidMeasureClef { offset, clef });
            }
        }
        for voice in 0..4 {
            left.voices[voice].extend(right.voices[voice].clone());
        }
        left.barline_right = right.barline_right;
        for (index, measure) in measures.iter_mut().enumerate() {
            measure.number = index as u32 + 1;
        }
    }
    remap_spanners(score, |address| {
        if address.measure < cmd.measure_index + 1 {
            return Some(address.clone());
        }
        if address.measure > cmd.measure_index + 1 {
            let mut shifted = address.clone();
            shifted.measure -= 1;
            return Some(shifted);
        }
        let offset = offsets
            .iter()
            .find(|(part, staff, _)| *part == address.part && *staff == address.staff)?
            .2[address.voice];
        Some(NoteAddr {
            measure: cmd.measure_index,
            note: address.note + offset,
            ..address.clone()
        })
    });
    Ok(())
}
