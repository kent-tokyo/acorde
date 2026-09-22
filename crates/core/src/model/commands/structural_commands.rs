//! Measure-structure editing commands and their typed-spanner address transforms.

use super::spanner_remap::remap_spanners;
use super::{Barline, Error, JoinMeasuresCmd, NoteAddr, Score, SplitMeasureCmd};

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
            let expected = measure
                .time_sig
                .as_ref()
                .unwrap_or(&score.settings.time_signature)
                .total_beats();
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
