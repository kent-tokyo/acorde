//! Verse-aware lyric hyphens and melisma lines after measured note placement.
use crate::glyphs::f;
use crate::render::{NoteKey, NotePoint};
use acorde_core::{Lyric, Note, Score};
use acorde_layout::GlyphPlacement;
use std::{
    collections::{BTreeSet, HashMap},
    fmt::Write as _,
};

fn lyric(note: &Note, verse: u8) -> Option<&Lyric> {
    if verse == 1 {
        note.lyric.as_ref()
    } else {
        note.additional_lyrics
            .iter()
            .find(|v| v.verse == verse)
            .map(|v| &v.lyric)
    }
}

fn placement_key(key: NoteKey, verse: u8) -> String {
    let (p, s, m, v, n) = key;
    let mut result = format!("lyric:{p}:{s}:{m}:{v}:{n}");
    if verse > 1 {
        let _ = write!(result, ":verse{verse}");
    }
    result
}

#[allow(clippy::too_many_arguments)]
fn line(
    body: &mut String,
    kind: &str,
    verse: u8,
    start: NoteKey,
    end: NoteKey,
    row: usize,
    x1: f32,
    x2: f32,
    y: f32,
    space: f32,
) {
    if ![x1, x2, y].iter().all(|v| v.is_finite()) || x2 - x1 < 0.25 * space {
        return;
    }
    let (p, s, m, v, n) = start;
    let (_, _, em, _, en) = end;
    let _ = write!(
        body,
        r#"<line class="acorde-lyric-{kind}" data-verse="{verse}" data-row="{row}" data-part="{p}" data-staff="{s}" data-start-note-addr="{p}:{s}:{m}:{v}:{n}" data-end-note-addr="{p}:{s}:{em}:{v}:{en}" x1="{}" y1="{}" x2="{}" y2="{}" stroke="black" stroke-width="{}"/>"#,
        f(x1),
        f(y),
        f(x2),
        f(y),
        f(0.1 * space)
    );
}

struct Event<'a> {
    key: NoteKey,
    note: &'a Note,
    point: NotePoint,
}

pub(crate) fn render(
    body: &mut String,
    score: &Score,
    points: &HashMap<NoteKey, NotePoint>,
    placements: &[GlyphPlacement],
    extents: &[(f32, f32)],
    baselines: &HashMap<(usize, usize, usize), f32>,
    space: f32,
) {
    let placed: HashMap<_, _> = placements
        .iter()
        .filter(|p| p.resource_key.starts_with("lyric:"))
        .map(|p| (p.resource_key.as_str(), p))
        .collect();
    for (p, part) in score.parts.iter().enumerate() {
        for (s, staff) in part.staves.iter().enumerate() {
            let voice_count = staff
                .measures
                .iter()
                .map(|m| m.voices.len())
                .max()
                .unwrap_or(0);
            for v in 0..voice_count {
                let mut events = Vec::new();
                let mut verses = BTreeSet::new();
                for (m, bar) in staff.measures.iter().enumerate() {
                    if let Some(voice) = bar.voices.get(v) {
                        for (n, note) in voice.iter().enumerate() {
                            if note.is_grace || note.hidden {
                                continue;
                            }
                            if note.lyric.is_some() {
                                verses.insert(1);
                            }
                            verses.extend(note.additional_lyrics.iter().map(|l| l.verse));
                            let key = (p, s, m, v, n);
                            if let Some(&point) = points.get(&key) {
                                events.push(Event { key, note, point });
                            }
                        }
                    }
                }
                for verse in verses {
                    let width = |l: &Lyric| {
                        crate::font_metrics::print_text_width(
                            &l.text,
                            crate::render::print_lyric_font_size_u(),
                        )
                        .unwrap_or(0.0)
                            * space
                    };
                    for (i, event) in events.iter().enumerate() {
                        let Some(text) = lyric(event.note, verse) else {
                            continue;
                        };
                        let Some(start_placement) =
                            placed.get(placement_key(event.key, verse).as_str())
                        else {
                            continue;
                        };
                        let next = events[i + 1..]
                            .iter()
                            .find(|e| e.note.is_rest || lyric(e.note, verse).is_some());
                        let start_row = event.point.3;
                        let start_x = start_placement.x_mm + width(text) / 2.0 + 0.18 * space;
                        if matches!(text.syllabic.as_str(), "begin" | "middle")
                            && let Some(next) = next.filter(|e| !e.note.is_rest)
                            && let Some(next_text) = lyric(next.note, verse)
                            && let Some(next_placement) =
                                placed.get(placement_key(next.key, verse).as_str())
                        {
                            let end_row = next.point.3;
                            let end_x = next_placement.x_mm - width(next_text) / 2.0 - 0.18 * space;
                            let y = start_placement.y_mm - 0.3 * space;
                            if start_row == end_row {
                                let gap = end_x - start_x;
                                let count = ((gap / (5.0 * space)).floor() as usize).clamp(1, 32);
                                for index in 0..count {
                                    let centre =
                                        start_x + gap * (index as f32 + 0.5) / count as f32;
                                    let half = (0.3 * space).min(gap / 2.0);
                                    line(
                                        body,
                                        "hyphen",
                                        verse,
                                        event.key,
                                        next.key,
                                        start_row,
                                        centre - half,
                                        centre + half,
                                        y,
                                        space,
                                    );
                                }
                            } else if let Some(&(_, right)) = extents.get(start_row) {
                                line(
                                    body,
                                    "hyphen",
                                    verse,
                                    event.key,
                                    next.key,
                                    start_row,
                                    (right - 1.2 * space).max(start_x),
                                    right - 0.4 * space,
                                    y,
                                    space,
                                );
                            }
                        }
                        if !text.extend {
                            continue;
                        }
                        let last_index = next
                            .and_then(|n| events.iter().position(|e| e.key == n.key))
                            .unwrap_or(events.len());
                        let last = events.get(last_index.saturating_sub(1)).unwrap_or(event);
                        let end_row = last.point.3;
                        let end_key = last.key;
                        for row in start_row..=end_row {
                            let Some(&(left, right)) = extents.get(row) else {
                                continue;
                            };
                            // All verses on a staff have fixed baselines within each measured row.
                            let Some(&first_baseline) = baselines.get(&(p, s, row)) else {
                                continue;
                            };
                            let baseline = first_baseline
                                + f32::from(verse.saturating_sub(1))
                                    * crate::render::lyric_verse_spacing_u()
                                    * space;
                            let x1 = if row == start_row {
                                start_x
                            } else {
                                left + 0.3 * space
                            };
                            let x2 = if row < end_row {
                                right - 0.3 * space
                            } else if let Some(next) = next.filter(|n| n.point.3 == row) {
                                if let Some(text) = lyric(next.note, verse)
                                    && let Some(placement) =
                                        placed.get(placement_key(next.key, verse).as_str())
                                {
                                    placement.x_mm - width(text) / 2.0 - 0.18 * space
                                } else {
                                    next.point.0 - 0.65 * space
                                }
                            } else {
                                right - 0.3 * space
                            };
                            line(
                                body, "extender", verse, event.key, end_key, row, x1, x2, baseline,
                                space,
                            );
                        }
                    }
                }
            }
        }
    }
}
