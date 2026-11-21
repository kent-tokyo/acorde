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

/// Returns whether reservations changed and the gaps required by the observed ink.
/// Text translations are applied to `pages` on every pass.
pub(crate) fn resolve(
    score: &Score,
    layout: &PrintLayoutResult,
    geometry: &mut [MeasuredRowGeometry],
    pages: &mut [String],
    font: &TextFont<'_>,
    system_insets_mm: &[f32],
) -> Result<(bool, Vec<Vec<f32>>), Error> {
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
                    .filter(|v| v.lyric.placement_above != Some(true))
                    .map(|v| v.verse)
                    .chain(
                        note.lyric
                            .as_ref()
                            .filter(|lyric| lyric.placement_above != Some(true))
                            .map(|_| 1),
                    )
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
    let mut ink_obstacles = Vec::new();
    let mut barlines = Vec::new();
    for svg in pages.iter() {
        let doc = roxmltree::Document::parse(svg).map_err(|_| Error::InvalidGeneratedGeometry)?;
        // System barlines can span the spaces between staves. Their geometry is
        // emitted outside measure groups, so associate each with its physical row.
        for line in doc
            .descendants()
            .filter(|n| n.attribute("class") == Some("acorde-barline"))
        {
            let number = |name: &str| {
                line.attribute(name)
                    .and_then(|v| v.parse::<f32>().ok())
                    .filter(|v| v.is_finite())
                    .ok_or(Error::InvalidGeneratedGeometry)
            };
            let x1 = number("x1")?;
            let x2 = number("x2")?;
            let y1 = number("y1")?;
            let y2 = number("y2")?;
            let half = number("stroke-width")? / 2.0;
            let (top, bottom) = (y1.min(y2), y1.max(y2));
            for row in 0..tops.len() {
                if top <= bottoms[row][bottoms[row].len() - 1] && bottom >= tops[row][0] {
                    barlines.push((row, x1.min(x2) - half, x1.max(x2) + half, top, bottom));
                }
            }
        }
        // Continued volta brackets are drawn outside their originating measure and can
        // extend above a later system's notes. Reserve the actual bracket height on
        // every system, including systems with no local start or stop marker.
        for volta in doc
            .descendants()
            .filter(|node| node.attribute("class") == Some("acorde-volta"))
        {
            let coordinates = volta
                .attribute("d")
                .unwrap_or("")
                .split(|c: char| c == ',' || c.is_whitespace())
                .filter_map(|value| value.parse::<f32>().ok())
                .collect::<Vec<_>>();
            if coordinates.len() < 4
                || coordinates.len() % 2 != 0
                || coordinates.iter().any(|value| !value.is_finite())
            {
                return Err(Error::InvalidGeneratedGeometry);
            }
            let top = coordinates
                .as_chunks::<2>()
                .0
                .iter()
                .map(|point| point[1])
                .fold(f32::INFINITY, f32::min);
            let row = tops
                .iter()
                .enumerate()
                .filter_map(|(row, staves)| {
                    staves
                        .first()
                        .map(|staff_top| (row, (staff_top - top).abs()))
                })
                .min_by(|left, right| left.1.total_cmp(&right.1))
                .map(|(row, _)| row)
                .ok_or(Error::InvalidGeneratedGeometry)?;
            above[row][0] = above[row][0].max((tops[row][0] - top) / SPACE + 0.2);
        }
        for mark in doc
            .descendants()
            .filter(|n| n.attribute("data-print-articulation") == Some("true"))
        {
            // Text ornaments are moved by the text pass; do not make their own
            // original boxes into obstacles against that same text.
            if mark.descendants().any(|n| n.has_tag_name("text")) {
                continue;
            }
            let row = *row_for_measure
                .get(&index(mark, "data-measure")?)
                .ok_or(Error::InvalidGeneratedGeometry)?;
            let number = |name: &str| {
                mark.attribute(name)
                    .and_then(|s| s.parse::<f32>().ok())
                    .filter(|v| v.is_finite())
                    .ok_or(Error::InvalidGeneratedGeometry)
            };
            let left = number("data-print-left")?;
            let right = number("data-print-right")?;
            let top = number("data-print-top")?;
            let bottom = number("data-print-bottom")?;
            ink_obstacles.push((row, left, right, top, bottom));
            let part = index(mark, "data-part")?;
            let source_staff = index(mark, "data-staff")?;
            let physical_staff = mark
                .attribute("data-print-note-addr")
                .and_then(|value| {
                    let fields = value
                        .split(':')
                        .map(str::parse::<usize>)
                        .collect::<Result<Vec<_>, _>>()
                        .ok()?;
                    let [p, s, m, v, n] = fields.as_slice() else {
                        return None;
                    };
                    score
                        .parts
                        .get(*p)?
                        .staves
                        .get(*s)?
                        .measures
                        .get(*m)?
                        .voices
                        .get(*v)?
                        .get(*n)?
                        .cross_staff
                        .as_ref()
                        .map(|cross| cross.target_staff)
                })
                .unwrap_or(source_staff);
            let staff = refs
                .iter()
                .position(|r| *r == (part, physical_staff))
                .ok_or(Error::InvalidGeneratedGeometry)?;
            reserve_ink(
                &mut upper_ink,
                &mut lower_ink,
                &tops,
                &bottoms,
                (row, staff, left, right, top, bottom),
            );
        }
        for beam in doc
            .descendants()
            .filter(|n| n.attribute("class") == Some("acorde-beam"))
        {
            let Some(owner) = beam
                .ancestors()
                .find(|n| n.attribute("data-acorde-kind") == Some("measure"))
            else {
                continue;
            };
            let row = *row_for_measure
                .get(&index(owner, "data-measure")?)
                .ok_or(Error::InvalidGeneratedGeometry)?;
            let numbers: Vec<f32> = beam
                .attribute("points")
                .unwrap_or("")
                .split(|c: char| c == ',' || c.is_whitespace())
                .filter_map(|s| s.parse().ok())
                .collect();
            if numbers.len() != 8 || numbers.iter().any(|v| !v.is_finite()) {
                return Err(Error::InvalidGeneratedGeometry);
            }
            let x = numbers.as_chunks::<2>().0.iter().map(|p| p[0]);
            let y = numbers.as_chunks::<2>().0.iter().map(|p| p[1]);
            let (left, right, top, bottom) = (
                x.clone().fold(f32::INFINITY, f32::min),
                x.fold(f32::NEG_INFINITY, f32::max),
                y.clone().fold(f32::INFINITY, f32::min),
                y.fold(f32::NEG_INFINITY, f32::max),
            );
            ink_obstacles.push((row, left, right, top, bottom));
            let part = index(owner, "data-part")?;
            let staff = index(owner, "data-staff")?;
            let measure = index(owner, "data-measure")?;
            let bar = score
                .parts
                .get(part)
                .and_then(|p| p.staves.get(staff))
                .and_then(|s| s.measures.get(measure))
                .ok_or(Error::InvalidGeneratedGeometry)?;
            // A beam joining staves belongs in their gap; assigning it to one
            // side as exterior ink could make reflow diverge. Ordinary beams
            // must reserve their actual overhang against neighboring lyrics.
            if !bar
                .voices
                .iter()
                .flatten()
                .any(|note| note.cross_staff.is_some())
            {
                let physical = refs
                    .iter()
                    .position(|r| *r == (part, staff))
                    .ok_or(Error::InvalidGeneratedGeometry)?;
                reserve_ink(
                    &mut upper_ink,
                    &mut lower_ink,
                    &tops,
                    &bottoms,
                    (row, physical, left, right, top, bottom),
                );
            }
        }
        // Individual syllables can be moved below the nominal verse baseline
        // by annotation collision resolution. Reserve their actual ink.
        for text in doc.descendants().filter(|n| {
            n.has_tag_name("text")
                && matches!(
                    n.attribute("class"),
                    Some("acorde-lyric" | "acorde-lyric-verse")
                )
        }) {
            let Some(owner) = text
                .ancestors()
                .find(|n| n.attribute("data-acorde-kind") == Some("measure"))
            else {
                continue;
            };
            let part = index(owner, "data-part")?;
            let staff = index(owner, "data-staff")?;
            let row = *row_for_measure
                .get(&index(owner, "data-measure")?)
                .ok_or(Error::InvalidGeneratedGeometry)?;
            let physical = refs
                .iter()
                .position(|r| *r == (part, staff))
                .ok_or(Error::InvalidGeneratedGeometry)?;
            let number = |name| {
                text.attribute(name)
                    .and_then(|v| v.parse::<f32>().ok())
                    .filter(|v| v.is_finite())
                    .ok_or(Error::InvalidGeneratedGeometry)
            };
            let (x, y, size) = (number("x")?, number("y")?, number("font-size")?);
            let ink = font.measure(text.text().unwrap_or(""), size)?;
            let start = x - match text.attribute("text-anchor") {
                Some("middle") => ink.advance_mm / 2.0,
                Some("end") => ink.advance_mm,
                _ => 0.0,
            };
            if ink.has_ink {
                above[row][physical] =
                    above[row][physical].max((tops[row][physical] - (y + ink.top_mm)) / SPACE);
                reserve_ink(
                    &mut upper_ink,
                    &mut lower_ink,
                    &tops,
                    &bottoms,
                    (
                        row,
                        physical,
                        start + ink.left_mm,
                        start + ink.right_mm,
                        y + ink.top_mm,
                        y + ink.bottom_mm,
                    ),
                );
            }
        }
        for tie in doc
            .descendants()
            .filter(|n| n.attribute("class") == Some("acorde-tie"))
        {
            let p: Vec<f32> = tie
                .attribute("d")
                .unwrap_or("")
                .split(|c: char| c == ',' || c.is_whitespace())
                .filter_map(|s| s.parse().ok())
                .collect();
            if p.len() != 14 || p.iter().any(|v| !v.is_finite()) {
                return Err(Error::InvalidGeneratedGeometry);
            }
            let Some(row) = tops
                .iter()
                .enumerate()
                .min_by(|(_, a), (_, b)| {
                    let distance = |staves: &Vec<f32>| {
                        staves
                            .iter()
                            .map(|y| (y + 2.0 * SPACE - (p[1] + p[7]) / 2.0).abs())
                            .fold(f32::INFINITY, f32::min)
                    };
                    distance(a).total_cmp(&distance(b))
                })
                .map(|(i, _)| i)
            else {
                continue;
            };
            let point = |t: f32| {
                let c = |offset| {
                    (1.0 - t).powi(3) * p[offset]
                        + 3.0 * (1.0 - t).powi(2) * t * p[offset + 2]
                        + 3.0 * (1.0 - t) * t * t * p[offset + 4]
                        + t.powi(3) * p[offset + 6]
                };
                (c(0), c(1))
            };
            for segment in 0..32 {
                let (x, y) = point(segment as f32 / 32.0);
                let (a, b) = point((segment + 1) as f32 / 32.0);
                ink_obstacles.push((
                    row,
                    x.min(a) - 0.15 * SPACE,
                    x.max(a) + 0.15 * SPACE,
                    y.min(b) - 0.15 * SPACE,
                    y.max(b) + 0.15 * SPACE,
                ));
            }
        }
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
            let y = line
                .attribute("y1")
                .and_then(|v| v.parse::<f32>().ok())
                .filter(|v| v.is_finite())
                .ok_or(Error::InvalidGeneratedGeometry)?;
            if y < tops[row][staff] {
                above[row][staff] = above[row][staff].max((tops[row][staff] - y) / SPACE + 0.1);
            } else {
                lyric_verses[row][staff] = lyric_verses[row][staff].max(verse);
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
            for point in values.as_chunks::<2>().0 {
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
            // A cross-staff stem may intersect an annotation attached to either
            // staff of the part. Its actual vertical extent decides the collision.
            stems.extend(
                refs.iter()
                    .enumerate()
                    .filter(|(_, (p, _))| *p == part)
                    .map(|(physical, _)| (row, physical, x, y1.min(y2), y1.max(y2))),
            );
            let cross_staff = stem
                .ancestors()
                .find(|n| n.attribute("data-acorde-kind") == Some("note"))
                .and_then(|note| {
                    let voice = index(note, "data-voice").ok()?;
                    let i = index(note, "data-note").ok()?;
                    let notes = score
                        .parts
                        .get(part)?
                        .staves
                        .get(refs[staff].1)?
                        .measures
                        .get(measure)?
                        .voices
                        .get(voice)?;
                    let note = notes.get(i)?;
                    let mut first = i;
                    let mut last = i;
                    if matches!(
                        note.beam,
                        acorde_core::BeamState::Begin
                            | acorde_core::BeamState::Continue
                            | acorde_core::BeamState::End
                    ) {
                        while first > 0
                            && matches!(
                                notes[first].beam,
                                acorde_core::BeamState::Continue | acorde_core::BeamState::End
                            )
                        {
                            first -= 1;
                        }
                        while last + 1 < notes.len()
                            && matches!(
                                notes[last].beam,
                                acorde_core::BeamState::Begin | acorde_core::BeamState::Continue
                            )
                        {
                            last += 1;
                        }
                    }
                    Some(notes[first..=last].iter().any(|n| n.cross_staff.is_some()))
                })
                .unwrap_or(false);
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
        // Tuplet digits and brackets must clear the final slur curves, including
        // curves connecting the two staves of a keyboard part. Place these before
        // directions/hairpins so subsequent marks see their final ink bounds.
        for mark in doc
            .descendants()
            .filter(|n| n.attribute("data-print-tuplet") == Some("true"))
        {
            let part = index(mark, "data-part")?;
            let under = mark.attribute("data-print-above") == Some("false");
            let source_staff = index(
                mark,
                if under {
                    "data-last-staff"
                } else {
                    "data-first-staff"
                },
            )?;
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
            let (left, right, top, bottom) = (
                number("data-print-left")?,
                number("data-print-right")?,
                number("data-print-top")?,
                number("data-print-bottom")?,
            );
            let mut shift = if under {
                (bottoms[row][staff] + CLEARANCE - top).max(0.0)
            } else {
                (tops[row][staff] - CLEARANCE - bottom).min(0.0)
            };
            for _ in 0..=curves.len().min(256) {
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
                if (shift - previous).abs() < 0.001 {
                    break;
                }
            }
            let ink = (row, staff, left, right, top + shift, bottom + shift);
            // Reserve a cross-staff tuplet against the exterior staff on its
            // authored side, rather than increasing the interior beam gap.
            above[row][staff] = above[row][staff].max((tops[row][staff] - ink.4) / SPACE);
            below[row][staff] = below[row][staff].max((ink.5 - bottoms[row][staff]) / SPACE);
            if lyric_verses[row][staff] > 0 {
                geometry[row].lyric_baselines_u[staff] = geometry[row].lyric_baselines_u[staff]
                    .max(
                        (ink.5 - bottoms[row][staff] + CLEARANCE) / SPACE
                            + crate::render::print_lyric_font_size_u(),
                    );
            }
            placed.push(ink);
            if shift.abs() > 0.001 {
                replacements.push((mark.range(), format!("<g data-print-tuplet-shift=\"{shift}\" transform=\"translate(0 {shift})\">{}</g>", &svg[mark.range()])));
            }
        }
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
            // The rendered span may be one of several numbered wedges starting on the
            // same note. Its actual side is encoded by the placement geometry; the
            // note's single legacy flag cannot describe every wedge.
            let under = top + bottom >= tops[row][staff] + bottoms[row][staff];
            let mut shift = if under {
                (bottoms[row][staff] + CLEARANCE - top).max(0.0)
            } else {
                (tops[row][staff] - CLEARANCE - bottom).min(0.0)
            };
            for _ in 0..=(curves.len() + note_bounds.len() + placed.len()).min(256) {
                let previous = shift;
                for curve in curves
                    .iter()
                    // Cross-staff slurs connect opposite sides of this gap. Pushing
                    // a horizontal wedge beyond both endpoints makes staff spacing
                    // diverge. Keep its authored lane; crossings remain reviewable.
                    .filter(|c| c.row == row && c.first_staff == staff && c.last_staff == staff)
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
                && n.attribute("class").is_some_and(|c| {
                    c.starts_with("acorde-measure-text")
                        || c.starts_with("acorde-articulation")
                        || matches!(
                            c,
                            "acorde-dynamic"
                                | "acorde-chord-symbol"
                                | "acorde-measure-number"
                                | "acorde-fingering"
                                | "acorde-technique-text"
                        )
                })
        }) {
            let Some(owner) = (text.attribute("class") == Some("acorde-measure-number"))
                .then_some(text)
                .or_else(|| {
                    text.ancestors()
                        .find(|n| n.attribute("data-acorde-kind") == Some("measure"))
                })
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
            if let Some(address) = text.ancestors().find_map(|n| {
                n.attribute("data-print-note-addr")
                    .or_else(|| n.attribute("data-note-addr"))
            }) {
                let address = address
                    .split(':')
                    .map(str::parse::<usize>)
                    .collect::<Result<Vec<_>, _>>()
                    .map_err(|_| Error::InvalidGeneratedGeometry)?;
                if let [part, source_staff, measure, voice, note] = address.as_slice()
                    && let Some(target) = score
                        .parts
                        .get(*part)
                        .and_then(|p| p.staves.get(*source_staff))
                        .and_then(|s| s.measures.get(*measure))
                        .and_then(|m| m.voices.get(*voice))
                        .and_then(|v| v.get(*note))
                        .and_then(|n| n.cross_staff.as_ref())
                        .map(|c| c.target_staff)
                {
                    staff = refs
                        .iter()
                        .position(|r| *r == (*part, target))
                        .ok_or(Error::InvalidGeneratedGeometry)?;
                }
            }
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
            if let Some(shape @ ("rectangle" | "square")) = text.attribute("data-acorde-enclosure")
            {
                let size = number("font-size")?;
                let slant = if matches!(text.attribute("font-style"), Some("italic" | "oblique")) {
                    -0.212_556_56
                } else {
                    0.0
                };
                let (left, top, right, bottom) = crate::font_metrics::text_enclosure_bounds(
                    &metrics,
                    size,
                    slant,
                    shape == "square",
                    0.18,
                );
                metrics.left_mm = left;
                metrics.top_mm = top;
                metrics.right_mm = right;
                metrics.bottom_mm = bottom;
            }
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
            let row_left = system_insets_mm
                .get(row)
                .copied()
                .ok_or(Error::InvalidGeneratedGeometry)?
                * 4.0;
            let mut minimum_left = row_left + 0.25 * SPACE;
            if matches!(
                text.attribute("data-acorde-enclosure"),
                Some("rectangle" | "square")
            ) && !text.attribute("class").is_some_and(|class| {
                class
                    .split_whitespace()
                    .any(|part| part == "acorde-measure-text-rehearsal-mark")
            }) && let Some(measure_left) = owner
                .attribute("data-print-measure-left")
                .and_then(|v| v.parse::<f32>().ok())
                .filter(|v| v.is_finite())
                && (left + right) / 2.0 < measure_left
            {
                // A boxed direction whose center falls before its own
                // barline can read as applying to the previous measure. Keep
                // its authored offset when it already points into this measure.
                minimum_left = minimum_left.max(measure_left + 0.25 * SPACE);
            }
            if right - left > row_widths[row] - minimum_left - 0.25 * SPACE {
                return Err(Error::DirectionTooWide);
            }
            let mut shift_x = (row_widths[row] - 0.25 * SPACE - right)
                .min(0.0)
                .max(minimum_left - left);
            left += shift_x;
            right += shift_x;
            let mut y = original_y;
            let mut under = original_y > bottoms[row][staff];
            if text.attribute("class") == Some("acorde-fingering") {
                // Start next to the owning staff instead of carrying the screen
                // renderer's fixed five-space note offset into a crowded score.
                // The following pass clears actual notes, stems and other marks.
                y = if under {
                    bottoms[row][staff] + CLEARANCE - metrics.top_mm
                } else {
                    tops[row][staff] - CLEARANCE - metrics.bottom_mm
                };
            }
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
            let mut staff_scoped_text = text
                .attribute("class")
                .is_some_and(|class| class.starts_with("acorde-measure-text"))
                && !under
                && staff > 0
                && refs[staff - 1].0 == p;
            for _ in 0..=(curves.len() + placed.len() + note_bounds.len() + rolls.len()).min(256) {
                let previous = y;
                let width = right - left;
                let mut blocked: Vec<_> = rolls
                    .iter()
                    .filter(|&&(r, _, _, t, b)| {
                        r == row
                            && y + metrics.bottom_mm + CLEARANCE > t
                            && y + metrics.top_mm - CLEARANCE < b
                    })
                    .map(|&(_, l, r, _, _)| (l - width - CLEARANCE, r + CLEARANCE))
                    .collect();
                for &(bar_row, bar_left, bar_right, bar_top, bar_bottom) in &barlines {
                    if bar_row == row
                        && y + metrics.bottom_mm + CLEARANCE > bar_top
                        && y + metrics.top_mm - CLEARANCE < bar_bottom
                    {
                        blocked.push((bar_left - width - CLEARANCE, bar_right + CLEARANCE));
                    }
                }
                for curve in curves.iter().filter(|c| {
                    c.row == row
                        && c.first_staff != c.last_staff
                        && c.first_staff <= staff
                        && staff <= c.last_staff
                }) {
                    for segment in 0..32 {
                        let a = curve.p[0] + (curve.p[4] - curve.p[0]) * segment as f32 / 32.0;
                        let b =
                            curve.p[0] + (curve.p[4] - curve.p[0]) * (segment + 1) as f32 / 32.0;
                        if let Some((top, bottom)) = curve.range(a.min(b), a.max(b))
                            && y + metrics.bottom_mm + CLEARANCE > top
                            && y + metrics.top_mm - CLEARANCE < bottom
                        {
                            blocked.push((a.min(b) - width - CLEARANCE, a.max(b) + CLEARANCE));
                        }
                    }
                }
                let ordinary_blocked = blocked.clone();
                if staff_scoped_text {
                    for &(r, s, x, top, bottom) in &stems {
                        if r == row
                            && s == staff
                            && y + metrics.bottom_mm + CLEARANCE > top
                            && y + metrics.top_mm - CLEARANCE < bottom
                        {
                            blocked.push((x - width - CLEARANCE, x + CLEARANCE));
                        }
                    }
                }
                let next = crate::render::nearest_text_gap(
                    left,
                    minimum_left,
                    row_widths[row] - 0.25 * SPACE - width,
                    blocked,
                )
                .or_else(|| {
                    // Keep the former outward escape when no horizontal lane exists.
                    // This remains a known placement limitation for crowded scores.
                    staff_scoped_text = false;
                    crate::render::nearest_text_gap(
                        left,
                        minimum_left,
                        row_widths[row] - 0.25 * SPACE - width,
                        ordinary_blocked,
                    )
                });
                if let Some(next) = next {
                    shift_x += next - left;
                    left = next;
                    right = left + width;
                } else {
                    return Err(Error::DirectionTooWide);
                }
                for curve in curves
                    .iter()
                    .filter(|c| c.row == row && c.first_staff == staff && c.last_staff == staff)
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
                for &(r, l, h, top, bottom) in &ink_obstacles {
                    if r == row
                        && left < h + CLEARANCE
                        && right > l - CLEARANCE
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
                // Note-owned marks can escape an interior lane when it cannot fit.
                // Measure text belongs to its written staff: reserve its ink against
                // that staff so reflow opens the lane instead of changing ownership.
                if staff_scoped_text
                    && under
                    && staff + 1 < refs.len()
                    && refs[staff + 1].0 == p
                    && y + metrics.bottom_mm + CLEARANCE > tops[row][staff + 1]
                {
                    y = bottoms[row][staff] + CLEARANCE - metrics.top_mm;
                } else if staff_scoped_text
                    && !under
                    && staff > 0
                    && refs[staff - 1].0 == p
                    && y + metrics.top_mm - CLEARANCE < bottoms[row][staff - 1]
                {
                    y = tops[row][staff] - CLEARANCE - metrics.bottom_mm;
                } else if !staff_scoped_text
                    && under
                    && staff + 1 < refs.len()
                    && refs[staff + 1].0 == p
                    && y + metrics.bottom_mm + CLEARANCE > tops[row][staff + 1]
                {
                    staff += 1;
                    y = y.max(bottoms[row][staff] + CLEARANCE - metrics.top_mm);
                } else if !staff_scoped_text
                    && !under
                    && staff > 0
                    && refs[staff - 1].0 == p
                    && y + metrics.top_mm - CLEARANCE < bottoms[row][staff - 1]
                {
                    staff -= 1;
                    y = y.min(tops[row][staff] - CLEARANCE - metrics.bottom_mm);
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
    let mut observed_gaps = Vec::with_capacity(geometry.len());
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
        let mut row_observed_gaps = Vec::with_capacity(g.gaps_u.len());
        for (staff, gap) in g.gaps_u.iter_mut().enumerate() {
            let required = adjacent_gap(&lower_ink[row][staff], &upper_ink[row][staff + 1]);
            let target = required.max(g.minimum_gaps_u[staff]);
            row_observed_gaps.push(target);
            if target > *gap + 0.01 {
                g.system_u += target - *gap;
                *gap = target;
            }
        }
        observed_gaps.push(row_observed_gaps);
        changed |= (g.top_u - old[row].top_u).abs() > 0.01
            || (g.bottom_u - old[row].bottom_u).abs() > 0.01
            || (g.system_u - old[row].system_u).abs() > 0.01
            || g.gaps_u
                .iter()
                .zip(&old[row].gaps_u)
                .any(|(a, b)| (a - b).abs() > 0.01)
            || g.lyric_baselines_u
                .iter()
                .zip(&old[row].lyric_baselines_u)
                .any(|(a, b)| (a - b).abs() > 0.01);
    }
    Ok((changed, observed_gaps))
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
