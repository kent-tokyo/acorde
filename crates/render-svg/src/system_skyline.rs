//! Vertical spacing between consecutive measured systems from their rendered ink.
//!
//! Each system's box normally reserves its tallest ink above the first staff and
//! below the last staff across the whole width. Two neighboring systems rarely
//! have their tallest ink at the same horizontal position, so this pass reads the
//! outlined page SVGs, builds a skyline of exterior ink for every row, and
//! reports how much of each row's bottom reservation the next row can reuse.
//! Coordinates are the renderer's internal units (4 per millimetre), as in
//! `print_preflight`.
use crate::MeasuredEngravingError as Error;
use crate::render::MeasuredRowGeometry;
use acorde_core::Score;
use acorde_layout::PrintLayoutResult;

const SPACE: f32 = 1.8 * 4.0;
/// Skyline column width in renderer units (0.5 mm).
const BIN: f32 = 2.0;
/// Horizontal clearance between ink of neighboring systems, in bins (1 mm).
const HORIZONTAL_CLEARANCE_BINS: usize = 2;
/// Vertical clearance added to the closest ink pair, in staff spaces.
const INK_CLEARANCE_U: f32 = 1.0;
/// Minimum distance from the last staff line of a system to the first staff line
/// of the next, in staff spaces. Inter-staff gaps inside a system start at 6.
pub(crate) const MIN_SYSTEM_DISTANCE_U: f32 = 7.0;

type Affine = [f32; 6];

const IDENTITY: Affine = [1.0, 0.0, 0.0, 1.0, 0.0, 0.0];

fn multiply(a: &Affine, b: &Affine) -> Affine {
    [
        a[0] * b[0] + a[2] * b[1],
        a[1] * b[0] + a[3] * b[1],
        a[0] * b[2] + a[2] * b[3],
        a[1] * b[2] + a[3] * b[3],
        a[0] * b[4] + a[2] * b[5] + a[4],
        a[1] * b[4] + a[3] * b[5] + a[5],
    ]
}

fn numbers(text: &str) -> Vec<f32> {
    let mut values = Vec::new();
    let bytes = text.as_bytes();
    let mut index = 0;
    while index < bytes.len() {
        let start = index;
        let c = bytes[index];
        if c == b'-' || c == b'+' || c == b'.' || c.is_ascii_digit() {
            index += 1;
            let mut seen_dot = c == b'.';
            while index < bytes.len() {
                let d = bytes[index];
                if d.is_ascii_digit() {
                    index += 1;
                } else if d == b'.' && !seen_dot {
                    seen_dot = true;
                    index += 1;
                } else if (d == b'e' || d == b'E') && index + 1 < bytes.len() {
                    index += 1;
                    if bytes[index] == b'-' || bytes[index] == b'+' {
                        index += 1;
                    }
                } else {
                    break;
                }
            }
            if let Ok(value) = text[start..index].parse::<f32>() {
                values.push(value);
            }
        } else {
            index += 1;
        }
    }
    values
}

fn parse_transform(text: &str) -> Result<Affine, Error> {
    let mut result = IDENTITY;
    for item in text.split(')') {
        let item = item.trim().trim_start_matches(',').trim();
        if item.is_empty() {
            continue;
        }
        let (name, args) = item
            .split_once('(')
            .ok_or(Error::InvalidGeneratedGeometry)?;
        let v = numbers(args);
        let m = match (name.trim(), v.as_slice()) {
            ("translate", [x]) => [1.0, 0.0, 0.0, 1.0, *x, 0.0],
            ("translate", [x, y]) => [1.0, 0.0, 0.0, 1.0, *x, *y],
            ("scale", [s]) => [*s, 0.0, 0.0, *s, 0.0, 0.0],
            ("scale", [x, y]) => [*x, 0.0, 0.0, *y, 0.0, 0.0],
            ("rotate", [a]) | ("rotate", [a, _, _]) => {
                let (sin, cos) = a.to_radians().sin_cos();
                let rotation = [cos, sin, -sin, cos, 0.0, 0.0];
                if let [_, cx, cy] = v.as_slice() {
                    let to = [1.0, 0.0, 0.0, 1.0, *cx, *cy];
                    let back = [1.0, 0.0, 0.0, 1.0, -*cx, -*cy];
                    multiply(&multiply(&to, &rotation), &back)
                } else {
                    rotation
                }
            }
            ("skewX", [a]) => [1.0, 0.0, a.to_radians().tan(), 1.0, 0.0, 0.0],
            ("skewY", [a]) => [1.0, a.to_radians().tan(), 0.0, 1.0, 0.0, 0.0],
            ("matrix", [a, b, c, d, e, f]) => [*a, *b, *c, *d, *e, *f],
            _ => return Err(Error::InvalidGeneratedGeometry),
        };
        result = multiply(&result, &m);
    }
    if result.iter().all(|v| v.is_finite()) {
        Ok(result)
    } else {
        Err(Error::InvalidGeneratedGeometry)
    }
}

/// Control points of a path. Curves lie inside the hull of their control points,
/// and each arc is padded by its radii, so the box is conservative.
fn path_points(d: &str) -> Vec<(f32, f32, f32)> {
    let mut points = Vec::new();
    let mut tokens: Vec<(char, Vec<f32>)> = Vec::new();
    let mut start = None;
    for (index, c) in d.char_indices() {
        if c.is_ascii_alphabetic() && c != 'e' && c != 'E' {
            if let Some((command, from)) = start {
                tokens.push((command, numbers(&d[from..index])));
            }
            start = Some((c, index + c.len_utf8()));
        }
    }
    if let Some((command, from)) = start {
        tokens.push((command, numbers(&d[from..])));
    }
    let (mut x, mut y) = (0.0_f32, 0.0_f32);
    let (mut sx, mut sy) = (0.0_f32, 0.0_f32);
    for (command, args) in tokens {
        let relative = command.is_ascii_lowercase();
        match command.to_ascii_uppercase() {
            'M' | 'L' | 'T' => {
                for (n, pair) in args.as_chunks::<2>().0.iter().enumerate() {
                    let (bx, by) = if relative { (x, y) } else { (0.0, 0.0) };
                    x = bx + pair[0];
                    y = by + pair[1];
                    if n == 0 && command.eq_ignore_ascii_case(&'M') {
                        sx = x;
                        sy = y;
                    }
                    points.push((x, y, 0.0));
                }
            }
            'H' => {
                for &v in &args {
                    x = if relative { x + v } else { v };
                    points.push((x, y, 0.0));
                }
            }
            'V' => {
                for &v in &args {
                    y = if relative { y + v } else { v };
                    points.push((x, y, 0.0));
                }
            }
            'C' | 'S' | 'Q' => {
                let size = match command.to_ascii_uppercase() {
                    'C' => 6,
                    _ => 4,
                };
                for chunk in args.chunks_exact(size) {
                    let (bx, by) = if relative { (x, y) } else { (0.0, 0.0) };
                    for pair in chunk.as_chunks::<2>().0 {
                        points.push((bx + pair[0], by + pair[1], 0.0));
                    }
                    x = bx + chunk[size - 2];
                    y = by + chunk[size - 1];
                }
            }
            'A' => {
                for chunk in args.as_chunks::<7>().0 {
                    let (bx, by) = if relative { (x, y) } else { (0.0, 0.0) };
                    let radius = chunk[0].abs().max(chunk[1].abs());
                    points.push((x, y, radius));
                    x = bx + chunk[5];
                    y = by + chunk[6];
                    points.push((x, y, radius));
                }
            }
            'Z' => {
                x = sx;
                y = sy;
            }
            _ => {}
        }
    }
    points
}

/// Untransformed bounding box `(left, top, right, bottom)` of one drawing element.
fn local_box(node: roxmltree::Node<'_, '_>) -> Option<[f32; 4]> {
    let f = |name: &str| {
        node.attribute(name)
            .and_then(|v| numbers(v).first().copied())
            .unwrap_or(0.0)
    };
    let from_points = |points: &[(f32, f32, f32)]| {
        if points.is_empty() {
            return None;
        }
        let mut b = [
            f32::INFINITY,
            f32::INFINITY,
            f32::NEG_INFINITY,
            f32::NEG_INFINITY,
        ];
        for &(x, y, r) in points {
            b[0] = b[0].min(x - r);
            b[1] = b[1].min(y - r);
            b[2] = b[2].max(x + r);
            b[3] = b[3].max(y + r);
        }
        Some(b)
    };
    let mut b = match node.tag_name().name() {
        "line" => {
            let (x1, y1, x2, y2) = (f("x1"), f("y1"), f("x2"), f("y2"));
            [x1.min(x2), y1.min(y2), x1.max(x2), y1.max(y2)]
        }
        "rect" => {
            let (x, y) = (f("x"), f("y"));
            [x, y, x + f("width"), y + f("height")]
        }
        "circle" => {
            let (cx, cy, r) = (f("cx"), f("cy"), f("r"));
            [cx - r, cy - r, cx + r, cy + r]
        }
        "ellipse" => {
            let (cx, cy, rx, ry) = (f("cx"), f("cy"), f("rx"), f("ry"));
            [cx - rx, cy - ry, cx + rx, cy + ry]
        }
        "polygon" | "polyline" => {
            let values = numbers(node.attribute("points").unwrap_or(""));
            let points: Vec<_> = values
                .as_chunks::<2>()
                .0
                .iter()
                .map(|p| (p[0], p[1], 0.0))
                .collect();
            from_points(&points)?
        }
        "path" => from_points(&path_points(node.attribute("d").unwrap_or("")))?,
        "text" => {
            // Outlined pages contain no text; keep a generous box if one remains.
            let size = f("font-size").max(1.0);
            let length = node.text().map_or(1, |t| t.chars().count()) as f32;
            let (x, y) = (f("x"), f("y"));
            [
                x - length * size * 0.6,
                y - size,
                x + length * size * 0.6,
                y + size * 0.3,
            ]
        }
        _ => return None,
    };
    // Half the stroke width, whether or not the shape is also filled.
    let stroke = node
        .ancestors()
        .find_map(|n| n.attribute("stroke-width"))
        .and_then(|v| numbers(v).first().copied())
        .unwrap_or(0.0)
        .abs()
        / 2.0;
    b[0] -= stroke;
    b[1] -= stroke;
    b[2] += stroke;
    b[3] += stroke;
    b.iter().all(|v| v.is_finite()).then_some(b)
}

fn transformed(b: [f32; 4], m: &Affine) -> [f32; 4] {
    let corners = [(b[0], b[1]), (b[2], b[1]), (b[0], b[3]), (b[2], b[3])];
    let mut out = [
        f32::INFINITY,
        f32::INFINITY,
        f32::NEG_INFINITY,
        f32::NEG_INFINITY,
    ];
    for (x, y) in corners {
        let tx = m[0] * x + m[2] * y + m[4];
        let ty = m[1] * x + m[3] * y + m[5];
        out[0] = out[0].min(tx);
        out[1] = out[1].min(ty);
        out[2] = out[2].max(tx);
        out[3] = out[3].max(ty);
    }
    out
}

/// Exterior ink of one row: the outward extent above the first staff line and
/// below the last staff line for each skyline column.
#[derive(Clone, Debug, Default, PartialEq)]
pub(crate) struct RowSkyline {
    pub above: Vec<f32>,
    pub below: Vec<f32>,
}

fn raise(profile: &mut Vec<f32>, left: f32, right: f32, extent: f32) {
    if extent <= 0.0 || !extent.is_finite() {
        return;
    }
    let first = (left.max(0.0) / BIN).floor() as usize;
    let last = (right.max(0.0) / BIN).ceil() as usize;
    if last >= profile.len() {
        profile.resize(last + 1, 0.0);
    }
    for value in &mut profile[first..=last] {
        *value = value.max(extent);
    }
}

/// Builds every row's skyline from outlined page SVGs. Ink is assigned to the row
/// that owns its measure when the markup names one, and otherwise to the nearest
/// row on the same page. Page credits outside the notation group are ignored.
pub(crate) fn row_skylines(
    score: &Score,
    layout: &PrintLayoutResult,
    geometry: &[MeasuredRowGeometry],
    pages: &[String],
) -> Result<Vec<RowSkyline>, Error> {
    if pages.len() != layout.pages.len() {
        return Err(Error::InvalidGeneratedGeometry);
    }
    let mut skylines = vec![RowSkyline::default(); geometry.len()];
    let mut row_for_measure = std::collections::HashMap::new();
    let mut page_offset = 0.0;
    for (page, svg) in layout.pages.iter().zip(pages) {
        // Staff-line spans of the rows on this page, in internal units.
        let mut spans = Vec::with_capacity(page.systems.len());
        for system in &page.systems {
            let row = system.system_index;
            let g = geometry.get(row).ok_or(Error::InvalidGeneratedGeometry)?;
            let top = (page_offset + system.top_mm) * 4.0 + g.top_u * SPACE;
            let staves = crate::render::print_staff_offsets(score, &g.gaps_u);
            let (_, offset, height) = staves.last().ok_or(Error::InvalidGeneratedGeometry)?;
            spans.push((row, top, top + (offset + height) * SPACE));
            for &m in &system.measure_indices {
                row_for_measure.insert(m, row);
            }
        }
        page_offset += page.height_mm;
        let doc = roxmltree::Document::parse(svg).map_err(|_| Error::InvalidGeneratedGeometry)?;
        let Some(notation) = doc.descendants().find(|n| {
            n.attribute("class")
                .is_some_and(|c| c.split_whitespace().any(|c| c == "acorde-print-notation"))
        }) else {
            continue;
        };
        for node in notation.descendants().filter(|n| n.is_element()) {
            if node
                .ancestors()
                .any(|n| n.has_tag_name("defs") || n.has_tag_name("title"))
            {
                continue;
            }
            let Some(local) = local_box(node) else {
                continue;
            };
            // Transforms below the notation group; the group itself maps internal
            // units to the page and is the same for every element on the page.
            let mut matrix = IDENTITY;
            for ancestor in node.ancestors().take_while(|n| *n != notation) {
                if let Some(t) = ancestor.attribute("transform") {
                    matrix = multiply(&parse_transform(t)?, &matrix);
                }
            }
            let [left, top, right, bottom] = transformed(local, &matrix);
            // Spanner segments name their system; other notation names its measure
            // or note. A spanner whose ends lie in different systems is placed by
            // position instead.
            let row_of_address = |address: &str| {
                address
                    .split(':')
                    .nth(2)
                    .and_then(|m| m.parse::<usize>().ok())
                    .and_then(|m| row_for_measure.get(&m).copied())
            };
            let owner = node
                .ancestors()
                .find_map(|n| {
                    if let Some(row) = n.attribute("data-row") {
                        return row.parse::<usize>().ok();
                    }
                    if let Some(measure) = n.attribute("data-measure") {
                        return measure
                            .parse::<usize>()
                            .ok()
                            .and_then(|m| row_for_measure.get(&m).copied());
                    }
                    if let (Some(start), Some(end)) = (
                        n.attribute("data-start-note-addr").map(row_of_address),
                        n.attribute("data-end-note-addr").map(row_of_address),
                    ) {
                        return start.filter(|_| start == end);
                    }
                    n.attribute("data-note-addr")
                        .or_else(|| n.attribute("data-print-note-addr"))
                        .and_then(row_of_address)
                })
                .filter(|row| spans.iter().any(|s| s.0 == *row));
            let center = (top + bottom) / 2.0;
            let span = match owner {
                Some(row) => spans.iter().find(|s| s.0 == row),
                None => spans.iter().min_by(|a, b| {
                    let distance =
                        |s: &&(usize, f32, f32)| (s.1 - center).max(center - s.2).max(0.0);
                    distance(a).total_cmp(&distance(b))
                }),
            };
            let Some(&(row, staff_top, staff_bottom)) = span else {
                continue;
            };
            raise(&mut skylines[row].above, left, right, staff_top - top);
            raise(&mut skylines[row].below, left, right, bottom - staff_bottom);
        }
    }
    Ok(skylines)
}

/// Distance in staff spaces needed from the last staff line of `upper` to the
/// first staff line of `lower` so their exterior ink keeps a clearance.
pub(crate) fn required_distance_u(upper: &RowSkyline, lower: &RowSkyline) -> f32 {
    let mut closest = 0.0_f32;
    for (column, &below) in upper.below.iter().enumerate() {
        let first = column.saturating_sub(HORIZONTAL_CLEARANCE_BINS);
        let last = (column + HORIZONTAL_CLEARANCE_BINS).min(lower.above.len().saturating_sub(1));
        let above = lower
            .above
            .get(first..=last)
            .map_or(0.0, |range| range.iter().copied().fold(0.0, f32::max));
        closest = closest.max(below + above);
    }
    // Ink of either row must also clear the other row's staff lines.
    closest = closest
        .max(upper.below.iter().copied().fold(0.0, f32::max))
        .max(lower.above.iter().copied().fold(0.0, f32::max));
    (closest / SPACE + INK_CLEARANCE_U).max(MIN_SYSTEM_DISTANCE_U)
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn path_points_follow_relative_and_absolute_commands() {
        let points = path_points("M 10 20 l 5 5 H 40 v -10 Q 50 0 60 10 z");
        let ys: Vec<f32> = points.iter().map(|p| p.1).collect();
        assert_eq!(ys, [20.0, 25.0, 25.0, 15.0, 0.0, 10.0]);
        assert_eq!(points.last().map(|p| p.0), Some(60.0));
        let arc = path_points("M 0 0 A 4 3 0 0 1 8 0");
        assert_eq!(arc[1], (0.0, 0.0, 4.0));
        assert_eq!(arc[2], (8.0, 0.0, 4.0));
    }

    #[test]
    fn transforms_compose_in_document_order() {
        let m = parse_transform("translate(10 0) scale(2)").unwrap();
        assert_eq!(
            transformed([1.0, 1.0, 2.0, 2.0], &m),
            [12.0, 2.0, 14.0, 4.0]
        );
        let r = parse_transform("rotate(90 5 5)").unwrap();
        let b = transformed([5.0, 5.0, 7.0, 5.0], &r);
        assert!((b[1] - 5.0).abs() < 1e-4 && (b[3] - 7.0).abs() < 1e-4);
        assert!(parse_transform("perspective(1)").is_err());
    }

    #[test]
    fn staggered_ink_needs_less_than_the_sum_of_maxima() {
        let mut upper = RowSkyline::default();
        let mut lower = RowSkyline::default();
        // A low note at the left of the upper system and a high one at the right
        // of the lower system.
        raise(&mut upper.below, 0.0, 20.0, 5.0 * SPACE);
        raise(&mut lower.above, 200.0, 220.0, 5.0 * SPACE);
        assert_eq!(required_distance_u(&upper, &lower), MIN_SYSTEM_DISTANCE_U);
        // The same ink stacked in one column needs both extents and a clearance.
        let mut stacked = RowSkyline::default();
        raise(&mut stacked.above, 10.0, 14.0, 5.0 * SPACE);
        assert!((required_distance_u(&upper, &stacked) - 11.0).abs() < 1e-4);
        // Columns within the horizontal clearance still count as stacked.
        let mut near = RowSkyline::default();
        raise(&mut near.above, 23.0, 30.0, 5.0 * SPACE);
        assert!((required_distance_u(&upper, &near) - 11.0).abs() < 1e-4);
    }
}
