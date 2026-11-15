//! Bounded dynamic programming over written harmonic regions. Scores are musical
//! heuristics, not probabilities or evidence of the composer's intended harmony.

use super::*;

struct Choice {
    chord: ChordSymbol,
    root: u8,
    tones: Vec<u8>,
    /// Applied dominants must resolve to this root in the next written region.
    target: Option<u8>,
    penalty: i64,
}

struct Region {
    key: KeySignature,
    home: u8,
    weights: [i64; 12],
    choices: Vec<Choice>,
}

#[derive(Clone, Copy)]
struct BackLink {
    choice: usize,
    used_changes: usize,
}

type BacktrackRow = Vec<Vec<Option<BackLink>>>;

fn choice(chord: ChordSymbol, target: Option<u8>, penalty: i64) -> Option<Choice> {
    Some(Choice {
        root: root_pc(&chord.root)?,
        tones: tones(&chord)?,
        chord,
        target,
        penalty,
    })
}

fn choices(
    key: &KeySignature,
    notes: &[MelodyEvent],
    style: &str,
    bass_mode: AccordionBassMode,
) -> Vec<Choice> {
    let home = tonic(key).unwrap_or(0);
    let diatonic = candidates_for_key(key, notes);
    let mut result = Vec::new();
    for (index, mut symbol) in diatonic.iter().cloned().enumerate() {
        if style == "jazz-color" && index < 7 {
            symbol.kind = match symbol.kind.as_str() {
                "major" if root_pc(&symbol.root) == Some((home + 7) % 12) => "dominant",
                "major" if key.mode == "minor" && index == 6 => "dominant",
                "major" => "major-seventh",
                "minor" => "minor-seventh",
                _ => "diminished",
            }
            .to_string();
        }
        if super::super::arrange::preset_chord_supported(&symbol, bass_mode)
            && let Some(candidate) =
                choice(symbol, None, if index < 7 { index as i64 * 2 } else { 64 })
        {
            result.push(candidate);
        }
    }
    if style == "jazz-color" {
        for target in diatonic
            .iter()
            .take(7)
            .filter(|symbol| symbol.kind != "diminished")
        {
            let Some(target) = root_pc(&target.root) else {
                continue;
            };
            let symbol = chord((target + 7) % 12, "dominant", key.fifths < 0);
            if !result.iter().any(|candidate| candidate.chord == symbol)
                && let Some(candidate) = choice(symbol, Some(target), 48)
            {
                result.push(candidate);
            }
        }
    }
    // Seven diatonic choices, at most two fallbacks for each of twelve pitch
    // classes, and at most six applied dominants: fewer than 40 choices/region.
    result
}

/// Integrate metrical weight over sounding duration, independently of note
/// segmentation. A tied note and its unsplit equivalent give the same evidence.
fn metrical_duration(score: &Score, measure: usize, start: f64, end: f64) -> f64 {
    let staff = &score.parts[0].staves[0];
    let meter = staff.measures[..=measure]
        .iter()
        .rev()
        .find_map(|bar| bar.time_sig.as_ref())
        .unwrap_or(&score.settings.time_signature);
    let compound =
        meter.denominator == 8 && meter.numerator > 3 && meter.numerator.is_multiple_of(3);
    let unit = if compound {
        1.5
    } else {
        4.0 / f64::from(meter.denominator)
    };
    let nominal = f64::from(meter.numerator) * 4.0 / f64::from(meter.denominator);
    let phase = if measure == 0 {
        (nominal - score.playback_measure_beats(0)).max(0.0)
    } else {
        0.0
    };
    let mut cursor = start + phase;
    let end = end + phase;
    let mut weight = 0.0;
    while cursor < end - 1e-9 {
        let beat = ((cursor + 1e-9) / unit).floor() as u32;
        let next = (f64::from(beat + 1) * unit).min(end);
        let strength = if beat == 0 {
            2.0
        } else if compound || (meter.numerator == 4 && beat == 2) {
            1.5
        } else {
            1.0
        };
        weight += (next - cursor) * strength;
        cursor = next;
    }
    weight
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
enum ToneRole {
    Structural,
    Passing,
    Neighbor,
    Suspension,
}

fn absolute_position(score: &Score, position: &ChordPlanPosition) -> f64 {
    (0..position.measure_index)
        .map(|measure| score.playback_measure_beats(measure))
        .sum::<f64>()
        + position.offset.beats()
}

fn weak_melodic_onset(score: &Score, event: &MelodyEvent) -> bool {
    let staff = &score.parts[0].staves[0];
    let meter = staff.measures[..=event.measure]
        .iter()
        .rev()
        .find_map(|bar| bar.time_sig.as_ref())
        .unwrap_or(&score.settings.time_signature);
    let compound =
        meter.denominator == 8 && meter.numerator > 3 && meter.numerator.is_multiple_of(3);
    let unit = if compound {
        1.5
    } else {
        4.0 / f64::from(meter.denominator)
    };
    let nominal = f64::from(meter.numerator) * 4.0 / f64::from(meter.denominator);
    let pickup_phase = if event.measure == 0 {
        (nominal - score.playback_measure_beats(0)).max(0.0)
    } else {
        0.0
    };
    let position = event.onset + pickup_phase;
    let group = (position / unit).floor() as u32;
    let within = position - f64::from(group) * unit;
    within > 1e-9 && within < unit - 1e-9
        || (!compound && group > 0 && !(meter.numerator == 4 && group == 2))
}

fn tone_role(
    score: &Score,
    events: &[MelodyEvent],
    index: usize,
    region_start: f64,
    region_end: f64,
) -> ToneRole {
    let Some(event) = events.get(index) else {
        return ToneRole::Structural;
    };
    let prior = index.checked_sub(1).and_then(|prior| events.get(prior));
    let next = events.get(index + 1);
    if let Some(next) = next {
        let carried = event.absolute_onset < region_start - 1e-9
            && event.absolute_end > region_start + 1e-9
            || event.tie_end
                && (event.absolute_onset - region_start).abs() < 1e-9
                && prior.is_some_and(|prior| {
                    prior.midi == event.midi
                        && (prior.absolute_end - event.absolute_onset).abs() < 1e-9
                });
        if carried
            && (next.absolute_onset - event.absolute_end).abs() < 1e-9
            && next.absolute_onset < region_end - 1e-9
            && (1..=2).contains(&(event.midi - next.midi))
        {
            return ToneRole::Suspension;
        }
    }
    let (Some(prior), Some(next)) = (prior, next) else {
        return ToneRole::Structural;
    };
    if event.tie_start
        || event.tie_end
        || event.absolute_end - event.absolute_onset > 1.0 + 1e-9
        || !weak_melodic_onset(score, event)
        || (prior.absolute_end - event.absolute_onset).abs() > 1e-9
        || (event.absolute_end - next.absolute_onset).abs() > 1e-9
    {
        return ToneRole::Structural;
    }
    let into = event.midi - prior.midi;
    let out = next.midi - event.midi;
    if (1..=2).contains(&into.abs()) && (1..=2).contains(&out.abs()) {
        if into.signum() == out.signum() {
            return ToneRole::Passing;
        }
        if prior.midi == next.midi {
            return ToneRole::Neighbor;
        }
    }
    ToneRole::Structural
}

fn weights(
    score: &Score,
    notes: &[MelodyEvent],
    all_events: &[MelodyEvent],
    start: &ChordPlanPosition,
    end: &ChordPlanPosition,
    contextual_nonchord_tones: bool,
) -> [i64; 12] {
    let mut weighted = [0.0_f64; 12];
    let region_start = absolute_position(score, start);
    let region_end = absolute_position(score, end);
    for note in notes {
        let from = if note.measure == start.measure_index {
            note.onset.max(start.offset.beats())
        } else {
            note.onset
        };
        let to = if note.measure == end.measure_index {
            note.end.min(end.offset.beats())
        } else {
            note.end
        };
        if to > from {
            let factor = if contextual_nonchord_tones {
                match tone_role(score, all_events, note.index, region_start, region_end) {
                    ToneRole::Structural => 1.0,
                    ToneRole::Passing | ToneRole::Neighbor => 0.25,
                    ToneRole::Suspension => 0.4,
                }
            } else {
                1.0
            };
            weighted[usize::from(note.pitch_class)] +=
                factor * metrical_duration(score, note.measure, from, to);
        }
    }
    weighted.map(|weight| (weight * 16.0).round() as i64)
}

fn melody_score(region: &Region, choice: &Choice, first: bool, last: bool) -> i64 {
    let mut score = -choice.penalty;
    for (pc, weight) in region.weights.iter().enumerate() {
        score += weight
            * if choice.tones.contains(&(pc as u8)) {
                6
            } else {
                -9
            };
        if pc as u8 == choice.root {
            score += weight;
        }
    }
    // An initial/final tonic is a preference, never a forced overwrite of the melody.
    if choice.root == region.home {
        score += if last {
            96
        } else if first {
            32
        } else {
            0
        };
    }
    score
}

fn transition(prior: &Choice, next: &Choice, same_key: bool, home: u8, style: &str) -> Option<i64> {
    if prior
        .target
        .is_some_and(|target| !same_key || next.root != target)
    {
        return None;
    }
    if !same_key {
        return Some(0);
    }
    let distance = (next.root + 12 - prior.root) % 12;
    let mut score = match distance {
        0 => 16,
        5 => 48,
        7 => 16,
        2 | 10 => 8,
        _ => -16,
    };
    if prior.root == (home + 7) % 12 && next.root == home {
        score += 48;
    }
    if prior.target.is_some() {
        score += 32;
    }
    if style == "simplified" && prior.chord != next.chord {
        score -= 128;
    }
    Some(score)
}

pub(super) fn choose_progression(
    score: &Score,
    events: &[MelodyEvent],
    positions: &[ChordPlanPosition],
    settings: ProgressionSettings<'_>,
    locked: &[Option<&ChordPlanEntry>],
    source: Option<&ChordPlan>,
) -> Option<Vec<ChordSymbol>> {
    let mut regions = Vec::with_capacity(positions.len());
    for (index, position) in positions.iter().enumerate() {
        let end = positions
            .get(index + 1)
            .cloned()
            .unwrap_or_else(|| end_position(score));
        let notes = events_in_region(events, position, &end);
        let key = key_at(score, position.measure_index);
        let mut available = match locked.get(index) {
            Some(Some(entry)) => vec![choice(entry.chord.clone(), None, 0)?],
            _ => choices(key, &notes, settings.style, settings.bass_mode),
        };
        if settings.max_changed_spans.is_some()
            && locked.get(index).is_none_or(Option::is_none)
            && let Some(entry) = source?.entries.get(index)
            && !available.iter().any(|item| item.chord == entry.chord)
        {
            available.push(choice(entry.chord.clone(), None, 64)?);
        }
        regions.push(Region {
            key: key.clone(),
            home: tonic(key)?,
            weights: weights(
                score,
                &notes,
                events,
                position,
                &end,
                settings.contextual_nonchord_tones,
            ),
            choices: available,
        });
    }
    let budget = usize::from(settings.max_changed_spans.unwrap_or(0));
    let mut costs: Vec<Vec<Option<i64>>> = Vec::new();
    let mut backtracks: Vec<BacktrackRow> = Vec::with_capacity(regions.len());
    for (index, region) in regions.iter().enumerate() {
        let last = index + 1 == regions.len();
        let mut next_costs = vec![vec![None; budget + 1]; region.choices.len()];
        let mut back = vec![vec![None; budget + 1]; region.choices.len()];
        for (j, current) in region.choices.iter().enumerate() {
            if last && current.target.is_some() {
                continue;
            }
            let change = if settings.max_changed_spans.is_some()
                && source?.entries.get(index)?.chord != current.chord
            {
                1
            } else {
                0
            };
            let emission = melody_score(region, current, index == 0, last);
            if index == 0 {
                if change <= budget {
                    next_costs[j][change] = Some(emission);
                }
                continue;
            }
            let prior_region = &regions[index - 1];
            for (k, prior) in prior_region.choices.iter().enumerate() {
                let Some(link) = transition(
                    prior,
                    current,
                    prior_region.key == region.key,
                    region.home,
                    settings.style,
                ) else {
                    continue;
                };
                for (used, cost) in costs[k].iter().enumerate() {
                    let Some(cost) = cost else { continue };
                    let next_used = used + change;
                    if next_used > budget {
                        continue;
                    }
                    let total = cost + link + emission;
                    // Stable candidate order breaks ties; no random musical output.
                    if next_costs[j][next_used].is_none_or(|best| total > best) {
                        next_costs[j][next_used] = Some(total);
                        back[j][next_used] = Some(BackLink {
                            choice: k,
                            used_changes: used,
                        });
                    }
                }
            }
        }
        costs = next_costs;
        backtracks.push(back);
    }
    let mut best = None;
    for (index, row) in costs.iter().enumerate() {
        for (used, cost) in row.iter().enumerate() {
            if let Some(cost) = cost
                && best.is_none_or(|(_, _, previous)| *cost > previous)
            {
                best = Some((index, used, *cost));
            }
        }
    }
    let (mut selected, mut used, _) = best?;
    let mut result = Vec::with_capacity(regions.len());
    for index in (0..regions.len()).rev() {
        result.push(regions[index].choices[selected].chord.clone());
        if index > 0 {
            let link = backtracks[index][selected][used]?;
            selected = link.choice;
            used = link.used_changes;
        }
    }
    result.reverse();
    Some(result)
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::{Duration, Note, Pitch};

    fn notes(pitches: &[u8]) -> Score {
        let mut score = Score::new("tone roles", 120, 4, 4, 0, 1);
        score.parts[0].staves[0].measures[0].voices[0] = pitches
            .iter()
            .map(|pitch| Note::new(Pitch::from_midi(*pitch, false), Duration::Quarter))
            .collect();
        score
    }

    fn position(measure_index: usize) -> ChordPlanPosition {
        ChordPlanPosition {
            measure_index,
            offset: BeatFraction::ZERO,
        }
    }

    #[test]
    fn short_weak_steps_are_passing_or_neighboring_but_strong_steps_remain_structural() {
        let passing = notes(&[72, 74, 76, 72]);
        let events = melody_events(&passing).unwrap();
        assert_eq!(tone_role(&passing, &events, 1, 0.0, 4.0), ToneRole::Passing);
        assert_eq!(
            tone_role(&passing, &events, 2, 0.0, 4.0),
            ToneRole::Structural
        );
        let literal = weights(
            &passing,
            &events,
            &events,
            &position(0),
            &position(1),
            false,
        );
        let contextual = weights(&passing, &events, &events, &position(0), &position(1), true);
        assert_eq!(literal[2], 16);
        assert_eq!(contextual[2], 4);
        assert_eq!(literal[4], contextual[4]);

        let neighbor = notes(&[72, 74, 72, 72]);
        let events = melody_events(&neighbor).unwrap();
        assert_eq!(
            tone_role(&neighbor, &events, 1, 0.0, 4.0),
            ToneRole::Neighbor
        );

        let mut strong = Score::new("strong step", 120, 4, 4, 0, 1);
        strong.parts[0].staves[0].measures[0].voices[0] = vec![
            Note::new(Pitch::from_midi(72, false), Duration::Half),
            Note::new(Pitch::from_midi(74, false), Duration::Quarter),
            Note::new(Pitch::from_midi(76, false), Duration::Quarter),
        ];
        let events = melody_events(&strong).unwrap();
        assert_eq!(
            tone_role(&strong, &events, 1, 0.0, 4.0),
            ToneRole::Structural
        );
    }

    #[test]
    fn tied_suspension_is_reduced_only_after_the_harmony_boundary() {
        let mut score = Score::new("suspension", 120, 4, 4, 0, 2);
        let mut held = Note::new(Pitch::from_midi(72, false), Duration::Whole);
        held.tie_start = true;
        let mut carried = Note::new(Pitch::from_midi(72, false), Duration::Quarter);
        carried.tie_end = true;
        score.parts[0].staves[0].measures[0].voices[0] = vec![held];
        score.parts[0].staves[0].measures[1].voices[0] = vec![
            carried,
            Note::new(Pitch::from_midi(71, false), Duration::Quarter),
            Note::rest(Duration::Half),
        ];
        let events = melody_events(&score).unwrap();
        assert_eq!(
            tone_role(&score, &events, 1, 4.0, 8.0),
            ToneRole::Suspension
        );
        let first = events_in_region(&events, &position(0), &position(1));
        let second = events_in_region(&events, &position(1), &position(2));
        let first_literal = weights(&score, &first, &events, &position(0), &position(1), false);
        let first_context = weights(&score, &first, &events, &position(0), &position(1), true);
        assert_eq!(first_literal, first_context);
        let second_literal = weights(&score, &second, &events, &position(1), &position(2), false);
        let second_context = weights(&score, &second, &events, &position(1), &position(2), true);
        assert!(second_context[0] < second_literal[0]);
        assert_eq!(second_context[11], second_literal[11]);
    }
}
