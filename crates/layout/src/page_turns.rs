//! Rest-aware pagination for one extracted part at a prescribed performance tempo.
use crate::{
    BreakReason, NotationBreakPolicy, PartLayoutPolicy, PrintConfig, PrintLayoutError,
    PrintLayoutResult, compute_print_layout,
};
use acorde_core::{Score, model::repeat::measure_sequence};
use serde::{Deserialize, Serialize};
use std::collections::BTreeMap;

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct PartPageTurn {
    pub from_measure: usize,
    pub to_measure: usize,
    /// Minimum written silence across playback visits to this edge.
    pub silence_ms: u32,
    pub sufficient: bool,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct PartPageTurnPlan {
    pub layout: PrintLayoutResult,
    pub turns: Vec<PartPageTurn>,
    /// Written measure endings selected by automatic reflow.
    pub preferred_breaks: Vec<usize>,
}

fn rest_windows(score: &Score, part: usize, bpm: u16) -> BTreeMap<(usize, usize), u32> {
    let visits = measure_sequence(score);
    let count = score.parts[part]
        .staves
        .first()
        .map_or(0, |s| s.measures.len());
    let mut bars = Vec::with_capacity(count);
    for i in 0..count {
        let beats = score.playback_measure_beats(i);
        let mut first = beats;
        let mut last = 0f64;
        for voice in score.parts[part]
            .staves
            .iter()
            .filter_map(|s| s.measures.get(i))
            .flat_map(|m| &m.voices)
        {
            let mut pos = 0.;
            for note in voice {
                let end = pos + note.beats();
                if !note.is_rest && !note.is_cue {
                    first = first.min(pos);
                    last = last.max(end);
                }
                pos = end;
            }
        }
        bars.push((beats, first, last));
    }
    let mut starts = Vec::with_capacity(visits.len());
    let mut end = 0.;
    let mut release = 0.;
    for &i in &visits {
        let Some(&(duration, _, last)) = bars.get(i) else {
            return BTreeMap::new();
        };
        if last > 0. {
            release = end + last;
        }
        starts.push((end, release));
        end += duration;
    }
    let mut result = BTreeMap::new();
    let mut next_attack = end;
    for idx in (1..visits.len()).rev() {
        let i = visits[idx];
        let (duration, first, last) = bars[i];
        if first < duration || last > 0. {
            next_attack = starts[idx].0 + first;
        }
        let silence = (next_attack - starts[idx - 1].1).max(0.);
        let ms = (silence * 60_000. / f64::from(bpm)).floor() as u32;
        result
            .entry((visits[idx - 1], i))
            .and_modify(|old: &mut u32| *old = (*old).min(ms))
            .or_insert(ms);
    }
    result
}

fn turn_report(
    layout: &PrintLayoutResult,
    windows: &BTreeMap<(usize, usize), u32>,
    threshold: u32,
) -> Vec<PartPageTurn> {
    let mut pages = BTreeMap::new();
    for page in &layout.pages {
        if let Some(span) = page.measure_span() {
            pages.extend((span.first_measure..=span.last_measure).map(|m| (m, page.page_index)));
        }
    }
    windows
        .iter()
        .filter(|((a, b), _)| pages.get(a) != pages.get(b))
        .map(|(&(from_measure, to_measure), &silence_ms)| PartPageTurn {
            from_measure,
            to_measure,
            silence_ms,
            sufficient: silence_ms >= threshold,
        })
        .collect()
}

fn cost(report: &[PartPageTurn], threshold: u32) -> (usize, u64) {
    (
        report.iter().filter(|turn| !turn.sufficient).count(),
        report
            .iter()
            .map(|turn| u64::from(threshold.saturating_sub(turn.silence_ms)))
            .sum(),
    )
}

/// Prefer an existing system ending with written silence. Keep manual breaks, page count,
/// and repeat/volta constraints. Report every residual short turn, including jumps back.
pub fn plan_part_page_turns(
    source: &Score,
    config: &PrintConfig,
    bpm: u16,
    minimum_ms: u32,
) -> Result<PartPageTurnPlan, PrintLayoutError> {
    let PartLayoutPolicy::ExtractedPart { part_index } = config.part_layout else {
        return Err(PrintLayoutError::InvalidPartIndex);
    };
    if bpm == 0 || minimum_ms == 0 {
        return Err(PrintLayoutError::InvalidPageTurnTiming);
    }
    let mut layout = compute_print_layout(source, config)?;
    let windows = rest_windows(source, part_index, bpm);
    let mut turns = turn_report(&layout, &windows, minimum_ms);
    let mut best = cost(&turns, minimum_ms);
    let repeat_ranges = if config.notation_break_policy == NotationBreakPolicy::KeepRepeatsTogether
    {
        super::print::repeat_ranges(source)
    } else {
        Vec::new()
    };
    let mut working = source.clone();
    let mut preferred_breaks = vec![];
    let mut attempts = 0;
    while best.0 > 0 && attempts < 32 {
        let candidates = layout
            .pages
            .iter()
            .flat_map(|page| page.systems.iter().rev().skip(1))
            .filter_map(|system| system.measure_spans.last().map(|s| s.last_measure))
            .filter(|&m| windows.get(&(m, m + 1)).copied().unwrap_or(0) > 0)
            .filter(|&m| {
                !repeat_ranges
                    .iter()
                    .any(|r| r.first_measure <= m && m < r.last_measure)
            })
            .collect::<Vec<_>>();
        let mut improved = false;
        for index in candidates {
            if attempts >= 32 {
                break;
            }
            let Some(bar) = working.parts[part_index]
                .staves
                .first_mut()
                .and_then(|s| s.measures.get_mut(index))
            else {
                continue;
            };
            if bar.page_break {
                continue;
            }
            attempts += 1;
            bar.page_break = true;
            if let Ok(mut candidate) = compute_print_layout(&working, config)
                && candidate.pages.len() <= layout.pages.len()
            {
                let report = turn_report(&candidate, &windows, minimum_ms);
                let candidate_cost = cost(&report, minimum_ms);
                if candidate_cost < best {
                    preferred_breaks.push(index);
                    // Synthetic choices remain automatic page boundaries to consumers.
                    for page in &mut candidate.pages {
                        if page
                            .measure_span()
                            .is_some_and(|span| preferred_breaks.contains(&span.last_measure))
                        {
                            page.break_reason = BreakReason::PageCapacity;
                            if let Some(system) = page.systems.last_mut() {
                                system.break_reason =
                                    if source.parts[part_index].staves.iter().any(|s| {
                                        s.measures
                                            .get(
                                                system.measure_indices.last().copied().unwrap_or(0),
                                            )
                                            .is_some_and(|m| m.system_break)
                                    }) {
                                        BreakReason::ExplicitSystemBreak
                                    } else {
                                        BreakReason::MeasureCapacity
                                    };
                            }
                        }
                    }
                    layout = candidate;
                    turns = report;
                    best = candidate_cost;
                    improved = true;
                    break;
                }
            }
            working.parts[part_index].staves[0].measures[index].page_break = false;
        }
        if !improved {
            break;
        }
    }
    Ok(PartPageTurnPlan {
        layout,
        turns,
        preferred_breaks,
    })
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::PrintPreset;
    use acorde_core::{Duration, Note, Pitch};

    fn fixture() -> (Score, PrintConfig) {
        let mut score = Score::new("Turns", 120, 4, 4, 0, 5);
        for m in &mut score.parts[0].staves[0].measures {
            m.voices[0] = vec![Note::new(Pitch::from_midi(60, false), Duration::Whole)];
        }
        let mut config = PrintPreset::A4Part { part_index: 0 }.config();
        config.measures_per_system = 1;
        config.systems_per_page = Some(3);
        (score, config)
    }

    #[test]
    fn prefers_silence_and_keeps_page_count() {
        let (mut score, config) = fixture();
        score.parts[0].staves[0].measures[1].voices[0] = vec![
            Note::new(Pitch::from_midi(60, false), Duration::Half),
            Note::rest(Duration::Half),
        ];
        let report = plan_part_page_turns(&score, &config, 120, 900).unwrap();
        assert_eq!(report.preferred_breaks, vec![1]);
        assert_eq!(report.layout.pages.len(), 2);
        report.layout.validate().unwrap();
        assert_eq!(report.turns.len(), 1);
        assert_eq!(report.turns[0].silence_ms, 1000);
        assert!(!score.parts[0].staves[0].measures[1].page_break);
    }

    #[test]
    fn reports_held_notes_and_honors_explicit_page_end() {
        let (mut score, config) = fixture();
        score.parts[0].staves[0].measures[2].page_break = true;
        score.parts[0].staves[0].measures[1].voices[0] = vec![Note::rest(Duration::Whole)];
        let report = plan_part_page_turns(&score, &config, 120, 1000).unwrap();
        assert_eq!(report.turns.len(), 1);
        assert_eq!(report.turns[0].from_measure, 2);
        assert_eq!(report.turns[0].silence_ms, 0);
        assert!(!report.turns[0].sufficient);
    }
}
