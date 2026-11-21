//! Opt-in written beat grouping for performance editions.
use crate::{BeamState, Duration, Error, Note, Score, TimeSignature};

fn group_boundaries(meter: &TimeSignature, five_eight: [u8; 2]) -> Vec<f64> {
    let units = if (meter.numerator, meter.denominator) == (5, 8) {
        five_eight.to_vec()
    } else if meter.denominator >= 8 && meter.numerator >= 6 && meter.numerator.is_multiple_of(3) {
        vec![3; usize::from(meter.numerator / 3)]
    } else {
        vec![1; usize::from(meter.numerator)]
    };
    let mut end = 0.;
    units
        .into_iter()
        .map(|n| {
            end += f64::from(n) * meter.beat_unit_beats();
            end
        })
        .collect()
}

fn written_value(beats: f64) -> Option<(Duration, u8)> {
    let duration = Duration::whole_filling_beats(beats);
    (0..=2)
        .find(|&dots| (duration.beats(dots) - beats).abs() < 1e-9)
        .map(|dots| (duration, dots))
}

// Only divide ordinary notes. Comparing with a plain note also protects future
// notation fields whose attack/continuation ownership has not yet been defined.
fn split_at(note: &mut Note, left: f64) -> Option<Note> {
    let (first, first_dots) = written_value(left)?;
    let (second, second_dots) = written_value(note.beats() - left)?;
    let mut plain = if let Some(pitch) = note.pitches.first() {
        Note::new(pitch.clone(), note.duration.clone())
    } else {
        Note::rest(note.duration.clone())
    };
    plain.pitches.clone_from(&note.pitches);
    plain.dot_count = note.dot_count;
    plain.tie_start = note.tie_start;
    plain.tie_end = note.tie_end;
    plain.stem_up = note.stem_up;
    let new_id = std::mem::replace(&mut plain.id, note.id.clone());
    plain.chord_symbol.clone_from(&note.chord_symbol);
    if plain != *note {
        return None;
    }
    plain.id = new_id;
    plain.chord_symbol = None;
    plain.duration = second;
    plain.dot_count = second_dots;
    plain.tie_end = !note.is_rest;
    note.duration = first;
    note.dot_count = first_dots;
    note.tie_start = !note.is_rest;
    Some(plain)
}

fn beam_run(run: &mut [Note]) {
    if run.len() >= 2 {
        let last = run.len() - 1;
        run[0].beam = BeamState::Begin;
        for note in &mut run[1..last] {
            note.beam = BeamState::Continue;
        }
        run[last].beam = BeamState::End;
    }
}

fn engrave_voice(voice: &mut Vec<Note>, boundaries: &[f64], origin: f64, can_split: bool) {
    let mut beat = origin;
    let mut cleaned = Vec::with_capacity(voice.len());
    let mut run_start = 0;
    let mut previous_group = None;
    for mut note in voice.drain(..) {
        let end = beat + note.beats();
        let group = boundaries.partition_point(|b| *b <= beat + 1e-9);
        let next = boundaries.get(group).copied();
        let off_beat = beat > 1e-9 && !boundaries.iter().any(|b| (b - beat).abs() < 1e-9);
        let continuation = if can_split && off_beat && next.is_some_and(|b| b < end - 1e-9) {
            next.and_then(|boundary| split_at(&mut note, boundary - beat))
        } else {
            None
        };
        let beamable = !note.is_rest
            && note.tuplet.is_none()
            && !note.is_cue
            && !note.is_grace
            && !note.explicit_unbeamed
            && note.beam == BeamState::None
            && note.duration.beats(0) <= 0.5;
        if !beamable || previous_group != Some(group) {
            beam_run(&mut cleaned[run_start..]);
            run_start = cleaned.len();
        }
        cleaned.push(note);
        if !beamable {
            run_start = cleaned.len();
        }
        if let Some(tail) = continuation {
            beam_run(&mut cleaned[run_start..]);
            cleaned.push(tail);
            run_start = cleaned.len();
        }
        previous_group = Some(group);
        beat = end;
    }
    beam_run(&mut cleaned[run_start..]);
    *voice = cleaned;
}

/// Return an engraved copy. The two denominator-unit groups apply only to 5/8.
/// Tuplets and voices with an authored beam instruction retain their written rhythm.
pub fn engrave_rhythm(source: &Score, five_eight: [u8; 2]) -> Result<Score, Error> {
    if !matches!(five_eight, [2, 3] | [3, 2]) {
        return Err(Error::InvalidCommand(
            "5/8 grouping must be 2+3 or 3+2".into(),
        ));
    }
    if !crate::validate(source).is_valid() {
        return Err(Error::InvalidScore);
    }
    let mut score = source.clone();
    let can_split = score.spanners.is_empty()
        && score.object_style_overrides.is_empty()
        && !score
            .parts
            .iter()
            .flat_map(|p| &p.staves)
            .flat_map(|s| &s.measures)
            .flat_map(|m| &m.voices)
            .flatten()
            .any(|n| {
                n.chord_symbol
                    .as_ref()
                    .is_some_and(|c| c.range_end.is_some())
            });
    for part in &mut score.parts {
        for staff in &mut part.staves {
            let mut meter = score.settings.time_signature.clone();
            for (index, bar) in staff.measures.iter_mut().enumerate() {
                if let Some(change) = &bar.time_sig {
                    meter = change.clone();
                }
                let origin = if index == 0 {
                    (meter.total_beats() - bar.duration_beats(&meter)).max(0.)
                } else {
                    0.
                };
                let boundaries = group_boundaries(&meter, five_eight);
                let polyphonic = bar.voices.iter().filter(|v| !v.is_empty()).count() > 1;
                for (slot, voice) in bar.voices.iter_mut().enumerate() {
                    if voice.is_empty() {
                        continue;
                    }
                    if voice
                        .iter()
                        .all(|n| n.beam == BeamState::None && !n.explicit_unbeamed)
                    {
                        engrave_voice(voice, &boundaries, origin, can_split);
                    }
                    if polyphonic {
                        for note in voice {
                            if note.is_rest && note.offset_y.is_none() && note.relative_y.is_none()
                            {
                                note.relative_y = Some(if slot % 2 == 0 { 10. } else { -10. });
                            } else if !note.is_rest && note.stem_up.is_none() {
                                note.stem_up = Some(slot % 2 == 0);
                            }
                        }
                    }
                }
            }
        }
    }
    if !crate::validate(&score).is_valid() {
        return Err(Error::InvalidScore);
    }
    let performance = |s: &Score| {
        let mut events = crate::to_playback_events_bounded(s, &Default::default())?;
        for e in &mut events {
            e.address = None;
            if let Some(source) = &mut e.source {
                source.note = 0;
            }
        }
        Ok::<_, Error>(events)
    };
    if performance(source)? != performance(&score)? {
        return Err(Error::InvalidCommand(
            "rhythm engraving changed playback".into(),
        ));
    }
    Ok(score)
}
