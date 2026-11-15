//! Final-curve clearance for measured print, before physical pagination is frozen.
//! Coordinates here are the renderer's internal units; only staff-space deltas leave this pass.
use crate::render::MeasuredRowGeometry;
use crate::{MeasuredEngravingError as Error, TextFont};
use acorde_core::Score;
use acorde_layout::PrintLayoutResult;

const SPACE: f32 = 1.8 * 4.0;
const CLEARANCE: f32 = 0.45 * SPACE;

/// Horizontal interval and outward distance from a staff edge, in renderer units.
type EdgeInk = (f32, f32, f32);

fn adjacent_gap(upper: &[EdgeInk], lower: &[EdgeInk]) -> f32 {
    let mut gap = upper
        .iter()
        .chain(lower)
        .map(|r| r.2)
        .fold(0.0_f32, f32::max);
    for &(left, right, below) in upper {
        for &(l, r, above) in lower {
            if left < r + CLEARANCE && right > l - CLEARANCE {
                gap = gap.max(below + above);
            }
        }
    }
    gap / SPACE + 0.6
}

fn reserve_ink(
    upper: &mut [Vec<Vec<EdgeInk>>],
    lower: &mut [Vec<Vec<EdgeInk>>],
    tops: &[Vec<f32>],
    bottoms: &[Vec<f32>],
    ink: (usize, usize, f32, f32, f32, f32),
) {
    let (row, staff, left, right, top, bottom) = ink;
    if top < tops[row][staff] {
        upper[row][staff].push((left, right, tops[row][staff] - top));
    }
    if bottom > bottoms[row][staff] {
        lower[row][staff].push((left, right, bottom - bottoms[row][staff]));
    }
}

struct Curve {
    row: usize,
    first_staff: usize,
    last_staff: usize,
    p: [f32; 6],
}

impl Curve {
    fn range(&self, left: f32, right: f32) -> Option<(f32, f32)> {
        let [x0, y0, _, cy, x2, y2] = self.p;
        if right < x0.min(x2) || left > x0.max(x2) || (x2 - x0).abs() < 0.001 {
            return None;
        }
        let ta = ((left - x0) / (x2 - x0)).clamp(0.0, 1.0);
        let tb = ((right - x0) / (x2 - x0)).clamp(0.0, 1.0);
        let (a, b) = (ta.min(tb), ta.max(tb));
        let y = |t: f32| (1.0 - t).powi(2) * y0 + 2.0 * t * (1.0 - t) * cy + t * t * y2;
        let (mut low, mut high) = (y(a).min(y(b)), y(a).max(y(b)));
        let divisor = y0 - 2.0 * cy + y2;
        if divisor.abs() > 0.00001 {
            let t = (y0 - cy) / divisor;
            if a < t && t < b {
                low = low.min(y(t));
                high = high.max(y(t));
            }
        }
        Some((low - 0.15 * SPACE, high + 0.15 * SPACE))
    }
}

fn index(node: roxmltree::Node<'_, '_>, name: &str) -> Result<usize, Error> {
    node.attribute(name)
        .and_then(|s| s.parse().ok())
        .ok_or(Error::InvalidGeneratedGeometry)
}

/// Returns true when the row reservations changed. Text translations are applied to `pages`
/// every pass, including the final pass whose reservations no longer change.
pub(crate) fn resolve(
    score: &Score,
    layout: &PrintLayoutResult,
    geometry: &mut [MeasuredRowGeometry],
    pages: &mut [String],
    font: &TextFont<'_>,
) -> Result<bool, Error> {
    let refs: Vec<_> = score
        .parts
        .iter()
        .enumerate()
        .flat_map(|(p, part)| (0..part.staves.len()).map(move |s| (p, s)))
        .collect();
    let old = geometry.to_vec();
    let mut tops = vec![Vec::new(); old.len()];
    let mut bottoms = tops.clone();
    let mut row_for_measure = std::collections::HashMap::new();
    let mut row_widths = vec![0.0; old.len()];
    let mut page_offset = 0.0;
    for page in &layout.pages {
        for system in &page.systems {
            let row = system.system_index;
            row_widths[row] = page.content_width_mm * 4.0;
            let g = old.get(row).ok_or(Error::InvalidGeneratedGeometry)?;
            let top = (page_offset + system.top_mm) * 4.0 + g.top_u * SPACE;
            for (_, offset, height) in crate::render::print_staff_offsets(score, &g.gaps_u) {
                tops[row].push(top + offset * SPACE);
                bottoms[row].push(top + (offset + height) * SPACE);
            }
            for &m in &system.measure_indices {
                row_for_measure.insert(m, row);
            }
        }
        page_offset += page.height_mm;
    }
    let mut above = vec![vec![0.0_f32; refs.len()]; old.len()];
    let mut below = above.clone();
    let mut upper_ink = vec![vec![Vec::new(); refs.len()]; old.len()];
    let mut lower_ink = upper_ink.clone();
    let mut lyric_verses = vec![vec![0_u8; refs.len()]; old.len()];
    for (staff, &(p, s)) in refs.iter().enumerate() {
        for (m, bar) in score.parts[p].staves[s].measures.iter().enumerate() {
            let Some(&row) = row_for_measure.get(&m) else {
                continue;
            };
            for note in bar.voices.iter().flatten() {
                let last = note
                    .additional_lyrics
                    .iter()
                    .map(|v| v.verse)
                    .chain(note.lyric.as_ref().map(|_| 1))
                    .max()
                    .unwrap_or(0);
                lyric_verses[row][staff] = lyric_verses[row][staff].max(last);
            }
        }
    }
    let mut curves = Vec::new();
    let mut stems = Vec::new();
    let mut note_bounds = Vec::new();
    let mut rolls = Vec::new();
    for svg in pages.iter() {
        let doc = roxmltree::Document::parse(svg).map_err(|_| Error::InvalidGeneratedGeometry)?;
        for line in doc.descendants().filter(|n| {
            matches!(
                n.attribute("class"),
                Some("acorde-lyric-extender" | "acorde-lyric-hyphen")
            )
        }) {
            let row = index(line, "data-row")?;
            let p = index(line, "data-part")?;
            let s = index(line, "data-staff")?;
            let staff = refs
                .iter()
                .position(|r| *r == (p, s))
                .ok_or(Error::InvalidGeneratedGeometry)?;
            let verse = u8::try_from(index(line, "data-verse")?)
                .map_err(|_| Error::InvalidGeneratedGeometry)?;
            lyric_verses[row][staff] = lyric_verses[row][staff].max(verse);
        }
        for mark in doc
            .descendants()
            .filter(|n| n.attribute("data-print-tuplet") == Some("true"))
        {
            let part = index(mark, "data-part")?;
            let source_staff = index(mark, "data-staff")?;
            let staff = refs
                .iter()
                .position(|r| *r == (part, source_staff))
                .ok_or(Error::InvalidGeneratedGeometry)?;
            let row = *row_for_measure
                .get(&index(mark, "data-measure")?)
                .ok_or(Error::InvalidGeneratedGeometry)?;
            let number = |name: &str| {
                mark.attribute(name)
                    .and_then(|v| v.parse::<f32>().ok())
                    .filter(|v| v.is_finite())
                    .ok_or(Error::InvalidGeneratedGeometry)
            };
            let (l, r, t, b) = (
                number("data-print-left")?,
                number("data-print-right")?,
                number("data-print-top")?,
                number("data-print-bottom")?,
            );
            note_bounds.push((row, staff, l, r, t, b));
            above[row][staff] = above[row][staff].max((tops[row][staff] - t) / SPACE);
            below[row][staff] = below[row][staff].max((b - bottoms[row][staff]) / SPACE);
            if lyric_verses[row][staff] > 0 {
                geometry[row].lyric_baselines_u[staff] = geometry[row].lyric_baselines_u[staff]
                    .max(
                        (b - bottoms[row][staff] + CLEARANCE) / SPACE
                            + crate::render::print_lyric_font_size_u(),
                    );
            }
        }
        for roll in doc
            .descendants()
            .filter(|n| n.attribute("class") == Some("acorde-arpeggio"))
        {
            let owner = roll
                .parent_element()
                .ok_or(Error::InvalidGeneratedGeometry)?;
            let row = *row_for_measure
                .get(&index(owner, "data-measure")?)
                .ok_or(Error::InvalidGeneratedGeometry)?;
            let values: Vec<f32> = roll
                .attribute("d")
                .unwrap_or("")
                .split_whitespace()
                .filter_map(|s| s.parse().ok())
                .collect();
            if values.len() < 4 || !values.len().is_multiple_of(2) {
                return Err(Error::InvalidGeneratedGeometry);
            }
            let (mut l, mut r, mut t, mut b) = (
                f32::INFINITY,
                f32::NEG_INFINITY,
                f32::INFINITY,
                f32::NEG_INFINITY,
            );
            for point in values.chunks_exact(2) {
                l = l.min(point[0]);
                r = r.max(point[0]);
                t = t.min(point[1]);
                b = b.max(point[1]);
            }
            rolls.push((row, l, r, t, b));
        }
        for mark in doc.descendants().filter(|n| {
            n.attribute("class")
                .is_some_and(|c| c == "acorde-notehead" || c.starts_with("acorde-accidental "))
        }) {
            if mark
                .ancestors()
                .any(|n| n.attribute("visibility") == Some("hidden"))
            {
                continue;
            }
            let Some(note) = mark
                .ancestors()
                .find(|n| n.attribute("data-acorde-kind") == Some("note"))
            else {
                continue;
            };
            let p = index(note, "data-part")?;
            let s = index(note, "data-staff")?;
            let m = index(note, "data-measure")?;
            let v = index(note, "data-voice")?;
            let i = index(note, "data-note")?;
            let source = score
                .parts
                .get(p)
                .and_then(|p| p.staves.get(s))
                .and_then(|s| s.measures.get(m))
                .and_then(|m| m.voices.get(v))
                .and_then(|v| v.get(i))
                .ok_or(Error::InvalidGeneratedGeometry)?;
            let physical = source.cross_staff.as_ref().map_or(s, |c| c.target_staff);
            let staff = refs
                .iter()
                .position(|r| *r == (p, physical))
                .ok_or(Error::InvalidGeneratedGeometry)?;
            let row = *row_for_measure
                .get(&m)
                .ok_or(Error::InvalidGeneratedGeometry)?;
            let num = |name: &str| {
                mark.attribute(name)
                    .and_then(|v| v.parse::<f32>().ok())
                    .filter(|v| v.is_finite())
                    .ok_or(Error::InvalidGeneratedGeometry)
            };
            let (x, y, rx, ry) = if mark.has_tag_name("ellipse") {
                let radius = num("rx")?.max(num("ry")?);
                (num("cx")?, num("cy")?, radius, radius)
            } else if mark.attribute("data-y").is_some() {
                (num("data-x")?, num("data-y")?, 0.55 * SPACE, 1.5 * SPACE)
            } else {
                continue;
            };
            note_bounds.push((row, staff, x - rx, x + rx, y - ry, y + ry));
            above[row][staff] = above[row][staff].max((tops[row][staff] - y + ry) / SPACE);
            below[row][staff] = below[row][staff].max((y + ry - bottoms[row][staff]) / SPACE);
        }
        for stem in doc
            .descendants()
            .filter(|n| n.attribute("class") == Some("acorde-stem"))
        {
            let Some(owner) = stem
                .ancestors()
                .find(|n| n.attribute("data-acorde-kind") == Some("measure"))
            else {
                continue;
            };
            let part = index(owner, "data-part")?;
            let staff = index(owner, "data-staff")?;
            let measure = index(owner, "data-measure")?;
            let row = *row_for_measure
                .get(&measure)
                .ok_or(Error::InvalidGeneratedGeometry)?;
            let staff = refs
                .iter()
                .position(|r| *r == (part, staff))
                .ok_or(Error::InvalidGeneratedGeometry)?;
            let value = |name: &str| {
                stem.attribute(name)
                    .and_then(|v| v.parse::<f32>().ok())
                    .filter(|v| v.is_finite())
                    .ok_or(Error::InvalidGeneratedGeometry)
            };
            let x = value("x1")?;
            let y1 = value("y1")?;
            let y2 = value("y2")?;
            stems.push((row, staff, x, y1.min(y2), y1.max(y2)));
            let cross_staff = stem
                .ancestors()
                .find(|n| n.attribute("data-acorde-kind") == Some("note"))
                .and_then(|note| {
                    let voice = index(note, "data-voice").ok()?;
                    let i = index(note, "data-note").ok()?;
                    score
                        .parts
                        .get(part)?
                        .staves
                        .get(refs[staff].1)?
                        .measures
                        .get(measure)?
                        .voices
                        .get(voice)?
                        .get(i)?
                        .cross_staff
                        .as_ref()
                })
                .is_some();
            if !cross_staff {
                reserve_ink(
                    &mut upper_ink,
                    &mut lower_ink,
                    &tops,
                    &bottoms,
                    (
                        row,
                        staff,
                        x - 0.3 * SPACE,
                        x + 0.3 * SPACE,
                        y1.min(y2) - 0.3 * SPACE,
                        y1.max(y2) + 0.3 * SPACE,
                    ),
                );
                above[row][staff] = above[row][staff].max((tops[row][staff] - y1.min(y2)) / SPACE);
                below[row][staff] =
                    below[row][staff].max((y1.max(y2) - bottoms[row][staff]) / SPACE);
            }
        }
        for group in doc
            .descendants()
            .filter(|n| n.attribute("data-print-slur") == Some("true"))
        {
            let row = index(group, "data-row")?;
            let part = index(group, "data-part")?;
            let s = index(group, "data-staff")?;
            let end = index(group, "data-end-staff")?;
            let first = refs
                .iter()
                .position(|r| *r == (part, s.min(end)))
                .ok_or(Error::InvalidGeneratedGeometry)?;
            let last = refs
                .iter()
                .position(|r| *r == (part, s.max(end)))
                .ok_or(Error::InvalidGeneratedGeometry)?;
            if row >= old.len() {
                return Err(Error::InvalidGeneratedGeometry);
            }
            for path in group
                .children()
                .filter(|n| n.attribute("class") == Some("acorde-slur"))
            {
                let values: Vec<f32> = path
                    .attribute("d")
                    .unwrap_or("")
                    .split(|c: char| c == ',' || c.is_whitespace())
                    .filter_map(|v| v.parse().ok())
                    .collect();
                if values.len() != 10 || values.iter().any(|v| !v.is_finite()) {
                    return Err(Error::InvalidGeneratedGeometry);
                }
                let curve = Curve {
                    row,
                    first_staff: first,
                    last_staff: last,
                    p: [
                        values[0], values[1], values[2], values[3], values[4], values[5],
                    ],
                };
                if let Some((low, high)) = curve.range(f32::NEG_INFINITY, f32::INFINITY) {
                    above[row][first] = above[row][first].max((tops[row][first] - low) / SPACE);
                    below[row][last] = below[row][last].max((high - bottoms[row][last]) / SPACE);
                    if first == last && lyric_verses[row][first] > 0 {
                        geometry[row].lyric_baselines_u[first] =
                            geometry[row].lyric_baselines_u[first].max(
                                (high - bottoms[row][first] + CLEARANCE) / SPACE
                                    + crate::render::print_lyric_font_size_u(),
                            );
                    }
                }
                for segment in 0..32 {
                    let left = curve.p[0] + (curve.p[4] - curve.p[0]) * segment as f32 / 32.0;
                    let right =
                        curve.p[0] + (curve.p[4] - curve.p[0]) * (segment + 1) as f32 / 32.0;
                    if let Some((top, bottom)) = curve.range(left.min(right), left.max(right)) {
                        // A cross-staff connector intentionally occupies the interior gap.
                        // Only its exterior edges reserve room beyond its endpoint staves.
                        reserve_ink(
                            &mut upper_ink,
                            &mut lower_ink,
                            &tops,
                            &bottoms,
                            (
                                row,
                                first,
                                left.min(right),
                                left.max(right),
                                top,
                                if first == last {
                                    bottom
                                } else {
                                    bottoms[row][first]
                                },
                            ),
                        );
                        if first != last {
                            reserve_ink(
                                &mut upper_ink,
                                &mut lower_ink,
                                &tops,
                                &bottoms,
                                (
                                    row,
                                    last,
                                    left.min(right),
                                    left.max(right),
                                    tops[row][last],
                                    bottom,
                                ),
                            );
                        }
                    }
                }
                curves.push(curve);
            }
        }
    }
    for &ink in &note_bounds {
        reserve_ink(&mut upper_ink, &mut lower_ink, &tops, &bottoms, ink);
    }
    for svg in pages.iter_mut() {
        let doc = roxmltree::Document::parse(svg).map_err(|_| Error::InvalidGeneratedGeometry)?;
        let mut replacements = Vec::new();
        let mut placed: Vec<(usize, usize, f32, f32, f32, f32)> = Vec::new();
        for hairpin in doc
            .descendants()
            .filter(|n| n.attribute("class") == Some("acorde-hairpin"))
        {
            let owner = hairpin;
            let address = |name| -> Result<Vec<usize>, Error> {
                owner
                    .attribute(name)
                    .ok_or(Error::InvalidGeneratedGeometry)?
                    .split(':')
                    .map(|v| v.parse().map_err(|_| Error::InvalidGeneratedGeometry))
                    .collect()
            };
            let start = address("data-start-note-addr")?;
            let end = address("data-end-note-addr")?;
            if start.len() != 5 || end.len() != 5 {
                return Err(Error::InvalidGeneratedGeometry);
            }
            let source = score
                .parts
                .get(start[0])
                .and_then(|p| p.staves.get(start[1]))
                .and_then(|s| s.measures.get(start[2]))
                .and_then(|m| m.voices.get(start[3]))
                .and_then(|v| v.get(start[4]))
                .ok_or(Error::InvalidGeneratedGeometry)?;
            let physical = source
                .cross_staff
                .as_ref()
                .map_or(start[1], |c| c.target_staff);
            let staff = refs
                .iter()
                .position(|r| *r == (start[0], physical))
                .ok_or(Error::InvalidGeneratedGeometry)?;
            let values: Vec<f32> = hairpin
                .attribute("d")
                .unwrap_or("")
                .split(|c: char| c == ',' || c.is_whitespace())
                .filter_map(|v| v.parse().ok())
                .collect();
            if values.len() != 8 || values.iter().any(|v| !v.is_finite()) {
                return Err(Error::InvalidGeneratedGeometry);
            }
            let left = values[0].min(values[2]);
            let right = values[0].max(values[2]);
            let top = values[1].min(values[3]).min(values[5]).min(values[7]) - 0.04 * SPACE;
            let bottom = values[1].max(values[3]).max(values[5]).max(values[7]) + 0.04 * SPACE;
            let first = *row_for_measure
                .get(&start[2])
                .ok_or(Error::InvalidGeneratedGeometry)?;
            let last = *row_for_measure
                .get(&end[2])
                .ok_or(Error::InvalidGeneratedGeometry)?;
            let row = (first..=last)
                .min_by(|&a, &b| {
                    let distance =
                        |r: usize| ((top + bottom) - (tops[r][staff] + bottoms[r][staff])).abs();
                    distance(a).total_cmp(&distance(b))
                })
                .ok_or(Error::InvalidGeneratedGeometry)?;
            let under = !source.hairpin_above.unwrap_or(false);
            let mut shift = if under {
                (bottoms[row][staff] + CLEARANCE - top).max(0.0)
            } else {
                (tops[row][staff] - CLEARANCE - bottom).min(0.0)
            };
            for _ in 0..=(curves.len() + note_bounds.len() + placed.len()).min(256) {
                let previous = shift;
                for curve in curves
                    .iter()
                    .filter(|c| c.row == row && c.first_staff <= staff && staff <= c.last_staff)
                {
                    if let Some((t, b)) = curve.range(left - CLEARANCE, right + CLEARANCE)
                        && top + shift - CLEARANCE < b
                        && bottom + shift + CLEARANCE > t
                    {
                        shift = if under {
                            b + CLEARANCE - top
                        } else {
                            t - CLEARANCE - bottom
                        };
                    }
                }
                for &(r, s, l, h, t, b) in note_bounds.iter().chain(&placed) {
                    if r == row
                        && s == staff
                        && left < h + CLEARANCE
                        && right > l - CLEARANCE
                        && top + shift - CLEARANCE < b
                        && bottom + shift + CLEARANCE > t
                    {
                        shift = if under {
                            b + CLEARANCE - top
                        } else {
                            t - CLEARANCE - bottom
                        };
                    }
                }
                let verses = lyric_verses[row][staff];
                if under && verses > 0 {
                    let lyric_bottom = bottoms[row][staff]
                        + (geometry[row].lyric_baselines_u[staff]
                            + f32::from(verses - 1) * crate::render::lyric_verse_spacing_u()
                            + 0.25 * crate::render::print_lyric_font_size_u())
                            * SPACE;
                    let lyric_top = bottoms[row][staff]
                        + (geometry[row].lyric_baselines_u[staff]
                            - crate::render::print_lyric_font_size_u())
                            * SPACE;
                    if top + shift - CLEARANCE < lyric_bottom
                        && bottom + shift + CLEARANCE > lyric_top
                    {
                        shift = lyric_bottom + CLEARANCE - top;
                    }
                }
                if (shift - previous).abs() < 0.001 {
                    break;
                }
            }
            above[row][staff] =
                above[row][staff].max((tops[row][staff] - top - shift) / SPACE + 0.45);
            below[row][staff] =
                below[row][staff].max((bottom + shift - bottoms[row][staff]) / SPACE + 0.45);
            placed.push((row, staff, left, right, top + shift, bottom + shift));
            if shift.abs() > 0.001 {
                replacements.push((hairpin.range(), format!("<g data-print-hairpin-shift=\"{shift}\" transform=\"translate(0 {shift})\">{}</g>",&svg[hairpin.range()])));
            }
        }
        for text in doc.descendants().filter(|n| {
            (n.has_tag_name("text") || n.attribute("class") == Some("acorde-dynamic"))
                && n.attribute("class")
                    .is_some_and(|c| c.starts_with("acorde-measure-text") || c == "acorde-dynamic")
        }) {
            let Some(owner) = text
                .ancestors()
                .find(|n| n.attribute("data-acorde-kind") == Some("measure"))
            else {
                continue;
            };
            let p = index(owner, "data-part")?;
            let s = index(owner, "data-staff")?;
            let m = index(owner, "data-measure")?;
            let mut staff = refs
                .iter()
                .position(|r| *r == (p, s))
                .ok_or(Error::InvalidGeneratedGeometry)?;
            let row = *row_for_measure
                .get(&m)
                .ok_or(Error::InvalidGeneratedGeometry)?;
            let number = |name: &str| {
                text.attribute(name)
                    .and_then(|v| v.parse::<f32>().ok())
                    .filter(|v| v.is_finite())
                    .ok_or(Error::InvalidGeneratedGeometry)
            };
            let is_glyph = text.has_tag_name("g");
            let x = number(if is_glyph { "data-x" } else { "x" })?;
            let original_y = number(if is_glyph { "data-baseline" } else { "y" })?;
            let mut metrics = if is_glyph {
                crate::TextRunMetrics {
                    advance_mm: 0.0,
                    left_mm: number("data-print-left")? - x,
                    right_mm: number("data-print-right")? - x,
                    top_mm: number("data-print-top")? - original_y,
                    bottom_mm: number("data-print-bottom")? - original_y,
                    has_ink: true,
                    glyph_count: 1,
                    cluster_count: 1,
                }
            } else {
                font.measure(text.text().unwrap_or(""), number("font-size")?)?
            };
            let anchor = match text.attribute("text-anchor") {
                Some("middle") => metrics.advance_mm / 2.0,
                Some("end") => metrics.advance_mm,
                _ => 0.0,
            };
            let (mut left, mut right) =
                (x - anchor + metrics.left_mm, x - anchor + metrics.right_mm);
            let mark = text
                .next_siblings()
                .skip(1)
                .find(|n| n.is_element())
                .filter(|n| n.attribute("class") == Some("acorde-metronome"));
            if let Some(mark) = mark {
                let value = |name: &str| {
                    mark.attribute(name)
                        .and_then(|v| v.parse::<f32>().ok())
                        .filter(|v| v.is_finite())
                        .ok_or(Error::InvalidGeneratedGeometry)
                };
                left = left.min(value("data-print-left")?);
                right = right.max(value("data-print-right")?);
                metrics.top_mm = metrics.top_mm.min(value("data-print-top")? - original_y);
                metrics.bottom_mm = metrics
                    .bottom_mm
                    .max(value("data-print-bottom")? - original_y);
            }
            if right - left > row_widths[row] - 0.5 * SPACE {
                return Err(Error::DirectionTooWide);
            }
            let mut shift_x = (row_widths[row] - 0.25 * SPACE - right)
                .min(0.0)
                .max(0.25 * SPACE - left);
            left += shift_x;
            right += shift_x;
            let mut y = original_y;
            let mut under = original_y > bottoms[row][staff];
            if text.attribute("class") == Some("acorde-dynamic") {
                // A note may keep its voice address while moving to another staff.
                // A dynamic inside that staff follows its physical note, not the voice's staff.
                for (candidate, &(part, _)) in refs.iter().enumerate() {
                    if part == p
                        && original_y + metrics.bottom_mm > tops[row][candidate]
                        && original_y + metrics.top_mm < bottoms[row][candidate]
                    {
                        staff = candidate;
                        under = original_y > (tops[row][staff] + bottoms[row][staff]) / 2.0;
                        y = if under {
                            bottoms[row][staff] + CLEARANCE - metrics.top_mm
                        } else {
                            tops[row][staff] - CLEARANCE - metrics.bottom_mm
                        };
                        break;
                    }
                }
            }
            for _ in 0..=(curves.len() + placed.len() + note_bounds.len() + rolls.len()).min(256) {
                let previous = y;
                let width = right - left;
                let blocked = rolls
                    .iter()
                    .filter(|&&(r, _, _, t, b)| {
                        r == row
                            && y + metrics.bottom_mm + CLEARANCE > t
                            && y + metrics.top_mm - CLEARANCE < b
                    })
                    .map(|&(_, l, r, _, _)| (l - width - CLEARANCE, r + CLEARANCE))
                    .collect();
                if let Some(next) = crate::render::nearest_text_gap(
                    left,
                    0.25 * SPACE,
                    row_widths[row] - 0.25 * SPACE - width,
                    blocked,
                ) {
                    shift_x += next - left;
                    left = next;
                    right = left + width;
                } else {
                    return Err(Error::DirectionTooWide);
                }
                for curve in curves
                    .iter()
                    .filter(|c| c.row == row && c.first_staff <= staff && staff <= c.last_staff)
                {
                    if let Some((low, high)) = curve.range(left - CLEARANCE, right + CLEARANCE)
                        && y + metrics.bottom_mm + CLEARANCE > low
                        && y + metrics.top_mm - CLEARANCE < high
                    {
                        y = if under {
                            high + CLEARANCE - metrics.top_mm
                        } else {
                            low - CLEARANCE - metrics.bottom_mm
                        };
                    }
                }
                for &(r, s, l, h, t, b) in placed.iter().chain(&note_bounds) {
                    if r == row
                        && s == staff
                        && left < h + CLEARANCE
                        && right > l - CLEARANCE
                        && y + metrics.bottom_mm > t - CLEARANCE
                        && y + metrics.top_mm < b + CLEARANCE
                    {
                        y = if under {
                            b + CLEARANCE - metrics.top_mm
                        } else {
                            t - CLEARANCE - metrics.bottom_mm
                        };
                    }
                }
                for &(r, s, x, top, bottom) in &stems {
                    if r == row
                        && s == staff
                        && left - CLEARANCE < x
                        && x < right + CLEARANCE
                        && y + metrics.bottom_mm + CLEARANCE > top
                        && y + metrics.top_mm - CLEARANCE < bottom
                    {
                        y = if under {
                            bottom + CLEARANCE - metrics.top_mm
                        } else {
                            top - CLEARANCE - metrics.bottom_mm
                        };
                    }
                }
                if (y - previous).abs() < 0.001 {
                    break;
                }
            }
            placed.push((
                row,
                staff,
                left,
                right,
                y + metrics.top_mm,
                y + metrics.bottom_mm,
            ));
            above[row][staff] =
                above[row][staff].max((tops[row][staff] - y - metrics.top_mm) / SPACE + 0.45);
            below[row][staff] =
                below[row][staff].max((y + metrics.bottom_mm - bottoms[row][staff]) / SPACE + 0.45);
            if (y - original_y).abs() > 0.001 || shift_x.abs() > 0.001 {
                let range = text.range().start..mark.map_or(text.range().end, |m| m.range().end);
                replacements.push((
                    range.clone(),
                    format!(
                        "<g transform=\"translate({shift_x} {})\">{}</g>",
                        y - original_y,
                        &svg[range]
                    ),
                ));
            }
        }
        for &ink in &placed {
            reserve_ink(&mut upper_ink, &mut lower_ink, &tops, &bottoms, ink);
        }
        replacements.sort_by_key(|(range, _)| std::cmp::Reverse(range.start));
        for (range, replacement) in replacements {
            svg.replace_range(range, &replacement);
        }
    }
    let mut changed = false;
    for (row, g) in geometry.iter_mut().enumerate() {
        for staff in 0..refs.len() {
            let verses = lyric_verses[row][staff];
            if verses > 0 {
                below[row][staff] = below[row][staff].max(
                    g.lyric_baselines_u[staff]
                        + f32::from(verses - 1) * crate::render::lyric_verse_spacing_u()
                        + 0.25 * crate::render::print_lyric_font_size_u(),
                );
                lower_ink[row][staff].push((
                    0.0,
                    row_widths[row],
                    (g.lyric_baselines_u[staff]
                        + f32::from(verses - 1) * crate::render::lyric_verse_spacing_u()
                        + 0.25 * crate::render::print_lyric_font_size_u())
                        * SPACE,
                ));
            }
        }
        g.top_u = g.top_u.max(above[row][0] + 0.6);
        g.bottom_u = g.bottom_u.max(below[row][refs.len() - 1] + 0.6);
        for (staff, gap) in g.gaps_u.iter_mut().enumerate() {
            let required = adjacent_gap(&lower_ink[row][staff], &upper_ink[row][staff + 1]);
            if required > *gap + 0.01 {
                let extra = required - *gap;
                *gap += extra;
                g.system_u += extra;
            }
        }
        changed |= (g.top_u - old[row].top_u).abs() > 0.01
            || (g.bottom_u - old[row].bottom_u).abs() > 0.01
            || (g.system_u - old[row].system_u).abs() > 0.01
            || g.lyric_baselines_u
                .iter()
                .zip(&old[row].lyric_baselines_u)
                .any(|(a, b)| (a - b).abs() > 0.01);
    }
    Ok(changed)
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn staff_gap_combines_only_horizontally_adjacent_ink() {
        let upper = [(0.0, 10.0, 4.0 * SPACE)];
        let lower_far = [(30.0, 40.0, 3.0 * SPACE)];
        let lower_near = [(9.0, 20.0, 3.0 * SPACE)];
        assert!((adjacent_gap(&upper, &lower_far) - 4.6).abs() < 0.001);
        assert!((adjacent_gap(&upper, &lower_near) - 7.6).abs() < 0.001);
    }
}
