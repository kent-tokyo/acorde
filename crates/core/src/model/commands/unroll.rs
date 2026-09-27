//! MuseScore's "Unroll repeats": write the bars out in playing order.

use super::super::notation::{Barline, Clef};
use super::super::repeat::measure_sequence;
use super::super::score::{Measure, NoteAddr, ObjectStyleTarget, Score};
use crate::Error;
use crate::{KeySignature, TimeSignature};

/// Key, meter and clef in force on a staff at the start of a bar (after its own changes) and at
/// its end (after mid-bar clefs).
#[derive(Clone, PartialEq)]
struct BarState {
    key: KeySignature,
    time: TimeSignature,
    clef: Clef,
}

fn staff_states(measures: &[Measure], initial: &BarState) -> (Vec<BarState>, Vec<BarState>) {
    let mut current = initial.clone();
    let mut starts = Vec::with_capacity(measures.len());
    let mut ends = Vec::with_capacity(measures.len());
    for measure in measures {
        if let Some(key) = &measure.key_sig {
            current.key = key.clone();
        }
        if let Some(time) = &measure.time_sig {
            current.time = time.clone();
        }
        if let Some(clef) = &measure.clef {
            current.clef = clef.clone();
        }
        starts.push(current.clone());
        if let Some(last) = measure.mid_clefs.last() {
            current.clef = last.clef.clone();
        }
        ends.push(current.clone());
    }
    (starts, ends)
}

/// Positions in `sequence` that play physical bar `measure`.
fn positions_of(sequence: &[usize], measure: usize) -> impl Iterator<Item = usize> + '_ {
    sequence
        .iter()
        .enumerate()
        .filter(move |(_, bar)| **bar == measure)
        .map(|(position, _)| position)
}

/// Where a span from `start` (at unrolled position `from`) to physical bar `end` lands: the first
/// position at or after `from` that plays `end`.
fn following_position(sequence: &[usize], from: usize, end: usize) -> Option<usize> {
    sequence
        .iter()
        .enumerate()
        .skip(from)
        .find(|(_, bar)| **bar == end)
        .map(|(position, _)| position)
}

/// Ticks (480 per quarter) at which each physical bar of the first staff starts.
fn bar_ticks(score: &Score) -> Vec<(u64, u64)> {
    let Some(staff) = score.parts.first().and_then(|part| part.staves.first()) else {
        return Vec::new();
    };
    let mut tick = 0u64;
    (0..staff.measures.len())
        .map(|index| {
            let length =
                (staff.measure_beats(index, &score.settings.time_signature) * 480.0).round() as u64;
            let start = tick;
            tick += length;
            (start, length)
        })
        .collect()
}

pub(super) fn apply_unroll_repeats(score: &mut Score) -> Result<(), Error> {
    let sequence = measure_sequence(score);
    let bar_count = score
        .parts
        .first()
        .and_then(|part| part.staves.first())
        .map_or(0, |staff| staff.measures.len());
    let written_out = sequence.iter().copied().eq(0..bar_count);
    let has_marks = score
        .parts
        .iter()
        .flat_map(|part| &part.staves)
        .any(|staff| {
            staff.measures.iter().any(|measure| {
                measure.volta.is_some()
                    || measure.navigation.is_some()
                    || !matches!(
                        (&measure.barline_left, &measure.barline_right),
                        (
                            Barline::Normal
                                | Barline::Double
                                | Barline::Final
                                | Barline::Dashed
                                | Barline::Dotted
                                | Barline::Invisible,
                            Barline::Normal
                                | Barline::Double
                                | Barline::Final
                                | Barline::Dashed
                                | Barline::Dotted
                                | Barline::Invisible,
                        )
                    )
            })
        });
    if sequence.is_empty() || (written_out && !has_marks) {
        return Ok(());
    }

    // MIDI automation is timed in ticks of the written bars: each event follows its bar to
    // every place the bar is played.
    let old_ticks = bar_ticks(score);
    let mut new_starts = Vec::with_capacity(sequence.len());
    let mut tick = 0u64;
    for &bar in &sequence {
        new_starts.push(tick);
        tick += old_ticks.get(bar).map_or(0, |(_, length)| *length);
    }
    let remap_tick = |old: u64| -> Vec<u64> {
        let Some(bar) = old_ticks
            .iter()
            .position(|(start, length)| old >= *start && old < start + length.max(&1))
        else {
            return vec![old];
        };
        let offset = old - old_ticks[bar].0;
        positions_of(&sequence, bar)
            .map(|position| new_starts[position] + offset)
            .collect()
    };

    let last = sequence.len() - 1;
    for part in &mut score.parts {
        for staff in &mut part.staves {
            let initial = BarState {
                key: score.settings.key_signature.clone(),
                time: score.settings.time_signature.clone(),
                clef: staff.clef.clone(),
            };
            let (starts, ends) = staff_states(&staff.measures, &initial);
            let mut unrolled: Vec<Measure> = Vec::with_capacity(sequence.len());
            for (position, &bar) in sequence.iter().enumerate() {
                let Some(source) = staff.measures.get(bar) else {
                    continue;
                };
                let mut measure = source.clone();
                // What was in force just before this bar in playing order.
                let before = if position == 0 {
                    None
                } else {
                    ends.get(sequence[position - 1])
                };
                let state = &starts[bar];
                match before {
                    None => {}
                    Some(before) => {
                        // Restate what a jump changes; drop restatements of what it keeps.
                        measure.key_sig = (before.key != state.key).then(|| state.key.clone());
                        measure.time_sig = (before.time != state.time).then(|| state.time.clone());
                        measure.clef = (before.clef != state.clef).then(|| state.clef.clone());
                    }
                }
                measure.volta = None;
                measure.navigation = None;
                if matches!(
                    measure.barline_left,
                    Barline::RepeatStart | Barline::RepeatBoth
                ) {
                    measure.barline_left = Barline::Normal;
                }
                if matches!(
                    measure.barline_right,
                    Barline::RepeatEnd | Barline::RepeatBoth
                ) || (matches!(measure.barline_right, Barline::Final) && position != last)
                {
                    measure.barline_right = Barline::Normal;
                }
                measure.number = position as u32 + 1;
                // A chord symbol's range ends at the next playing of its end bar.
                for note in measure.voices.iter_mut().flatten() {
                    if let Some(end) = note
                        .chord_symbol
                        .as_mut()
                        .and_then(|chord| chord.range_end.as_mut())
                    {
                        match following_position(&sequence, position, end.measure) {
                            Some(to) => end.measure = to,
                            None => {
                                if let Some(chord) = note.chord_symbol.as_mut() {
                                    chord.range_end = None;
                                }
                            }
                        }
                    }
                }
                unrolled.push(measure);
            }
            staff.measures = unrolled;
        }
        for bend in std::mem::take(&mut part.midi_pitch_bends) {
            for tick in remap_tick(bend.tick) {
                part.midi_pitch_bends
                    .push(super::super::score::MidiPitchBend {
                        tick,
                        ..bend.clone()
                    });
            }
        }
        for change in std::mem::take(&mut part.midi_control_changes) {
            for tick in remap_tick(change.tick) {
                part.midi_control_changes
                    .push(super::super::score::MidiControlChange {
                        tick,
                        ..change.clone()
                    });
            }
        }
        for change in std::mem::take(&mut part.midi_program_changes) {
            for tick in remap_tick(change.tick) {
                part.midi_program_changes
                    .push(super::super::score::MidiProgramChange {
                        tick,
                        ..change.clone()
                    });
            }
        }
        for touch in std::mem::take(&mut part.midi_aftertouch) {
            for tick in remap_tick(touch.tick) {
                part.midi_aftertouch
                    .push(super::super::score::MidiAftertouch {
                        tick,
                        ..touch.clone()
                    });
            }
        }
        part.midi_pitch_bends.sort_by_key(|event| event.tick);
        part.midi_control_changes.sort_by_key(|event| event.tick);
        part.midi_program_changes.sort_by_key(|event| event.tick);
        part.midi_aftertouch.sort_by_key(|event| event.tick);
    }

    // Spanners follow each playing of their first bar to the next playing of their last one.
    let at = |address: &NoteAddr, measure: usize| NoteAddr {
        measure,
        ..address.clone()
    };
    let mut spanners = Vec::new();
    for spanner in std::mem::take(&mut score.spanners) {
        for (copy, from) in positions_of(&sequence, spanner.start.measure).enumerate() {
            let Some(to) = following_position(&sequence, from, spanner.end.measure) else {
                continue;
            };
            let mut unrolled = spanner.clone();
            unrolled.start = at(&spanner.start, from);
            unrolled.end = at(&spanner.end, to);
            if copy > 0 {
                unrolled.id = format!("{}-{}", spanner.id, copy + 1);
            }
            spanners.push(unrolled);
        }
    }
    score.spanners = spanners;

    let mut overrides = Vec::new();
    for style in std::mem::take(&mut score.object_style_overrides) {
        let measure = match &style.target {
            ObjectStyleTarget::MeasureText { measure, .. } => Some(*measure),
            ObjectStyleTarget::Note { address } => Some(address.measure),
            _ => None,
        };
        let Some(measure) = measure else {
            overrides.push(style);
            continue;
        };
        for position in positions_of(&sequence, measure) {
            let mut copy = style.clone();
            match &mut copy.target {
                ObjectStyleTarget::MeasureText { measure, .. } => *measure = position,
                ObjectStyleTarget::Note { address } => address.measure = position,
                _ => {}
            }
            overrides.push(copy);
        }
    }
    score.object_style_overrides = overrides;

    // Layout breaks of linked views name physical bars that no longer exist as such.
    for view in &mut score.views {
        view.layout.system_breaks.clear();
        view.layout.page_breaks.clear();
    }
    Ok(())
}

#[cfg(test)]
mod tests {
    use super::super::{Command, CommandStack, UnrollRepeatsCmd};
    use crate::{Barline, Duration, KeySignature, Note, Pitch, Score, Step, VoltaBracket};

    fn note(step: Step) -> Note {
        Note::new(Pitch::new(step, 4), Duration::Whole)
    }

    #[test]
    fn unroll_writes_repeats_and_endings_out_in_playing_order() {
        // |: C | D (1. :| E (2. | F || with the second ending in G major.
        let mut score = Score::new("unroll", 120, 4, 4, 0, 4);
        let staff = &mut score.parts[0].staves[0];
        for (measure, step) in staff
            .measures
            .iter_mut()
            .zip([Step::C, Step::D, Step::E, Step::F])
        {
            measure.voices[0] = vec![note(step)];
        }
        staff.measures[0].barline_left = Barline::RepeatStart;
        staff.measures[1].volta = Some(VoltaBracket {
            number: 1,
            kind: "begin_end".into(),
        });
        staff.measures[1].barline_right = Barline::RepeatEnd;
        staff.measures[2].volta = Some(VoltaBracket {
            number: 2,
            kind: "begin_end".into(),
        });
        staff.measures[2].key_sig = Some(KeySignature {
            fifths: 1,
            mode: "major".into(),
        });
        staff.measures[3].barline_right = Barline::Final;
        score.spanners.push(
            serde_json::from_value(serde_json::json!({
                "id": "slur", "kind": "Slur",
                "start": {"part": 0, "staff": 0, "measure": 0, "voice": 0, "note": 0},
                "end": {"part": 0, "staff": 0, "measure": 1, "voice": 0, "note": 0}
            }))
            .expect("spanner"),
        );
        let before = score.clone();

        let mut stack = CommandStack::new(10);
        stack
            .execute(Command::UnrollRepeats(UnrollRepeatsCmd {}), &mut score)
            .expect("unrolls");
        let measures = &score.parts[0].staves[0].measures;
        let steps: Vec<_> = measures
            .iter()
            .map(|m| m.voices[0][0].pitches[0].step.clone())
            .collect();
        assert_eq!(steps, vec![Step::C, Step::D, Step::C, Step::E, Step::F]);
        assert!(measures.iter().all(|m| m.volta.is_none()));
        assert!(measures.iter().all(|m| {
            !matches!(m.barline_left, Barline::RepeatStart)
                && !matches!(m.barline_right, Barline::RepeatEnd)
        }));
        assert_eq!(measures[4].barline_right, Barline::Final);
        assert_eq!(measures[3].key_sig.as_ref().map(|k| k.fifths), Some(1));
        assert_eq!(
            measures.iter().map(|m| m.number).collect::<Vec<_>>(),
            vec![1, 2, 3, 4, 5]
        );
        // The slur over bars 1–2 is played once: the second pass leaves bar 1 for the second
        // ending before reaching bar 2.
        let slurs: Vec<_> = score
            .spanners
            .iter()
            .map(|s| (s.start.measure, s.end.measure))
            .collect();
        assert_eq!(slurs, vec![(0, 1)]);
        assert_eq!(crate::measure_sequence(&score), vec![0, 1, 2, 3, 4]);
        stack.undo(&mut score).expect("undoes");
        assert_eq!(score.parts[0].staves[0].measures.len(), 4);
        assert_eq!(
            serde_json::to_value(&score).expect("json"),
            serde_json::to_value(&before).expect("json")
        );
    }
}
