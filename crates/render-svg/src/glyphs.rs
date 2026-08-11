//! Original, hand-authored minimal vector glyphs — no vendored font, no system-font
//! dependency. Everything here is plain SVG (`<path>`, `<ellipse>`, `<circle>`, `<line>`,
//! `<rect>`) generated from parametric math (arcs sampled with `sin`/`cos`, straight
//! segments), so native Rust and WASM/browser output are byte-identical.
//!
//! All shapes are authored in "u" units — multiples of one staff space — with a local
//! origin, then placed via `ox`/`oy` (px) and scaled by `space` (px per staff space, i.e.
//! `SvgRenderOptions::staff_size`). Coordinates are formatted to 2 decimal places
//! everywhere (see [`f`]) to keep output stable across platforms despite using
//! floating-point trig.

use std::fmt::Write as _;

/// Format a coordinate/length with fixed precision — keeps `sin`/`cos`-derived glyph
/// geometry stable across platforms (ULP-level differences vanish at 2 decimals).
pub(crate) fn f(v: f32) -> String {
    format!("{v:.2}")
}

fn arc_points(cx: f32, cy: f32, rx: f32, ry: f32, start_deg: f32, end_deg: f32, steps: u32) -> Vec<(f32, f32)> {
    let mut pts = Vec::with_capacity(steps as usize + 1);
    for i in 0..=steps {
        let t = i as f32 / steps as f32;
        let deg = start_deg + (end_deg - start_deg) * t;
        let rad = deg.to_radians();
        pts.push((cx + rad.cos() * rx, cy + rad.sin() * ry));
    }
    pts
}

/// Build an SVG path `d` string from segments of u-unit points, placed at `(ox,oy)` px and
/// scaled by `space` (px per u).
fn path_from_segments(segments: &[Vec<(f32, f32)>], ox: f32, oy: f32, space: f32) -> String {
    let mut d = String::new();
    let mut first = true;
    for seg in segments {
        for &(x, y) in seg {
            let px = ox + x * space;
            let py = oy + y * space;
            let _ = write!(d, "{}{},{} ", if first { "M " } else { "L " }, f(px), f(py));
            first = false;
        }
    }
    d.trim_end().to_string()
}

// ── clefs ─────────────────────────────────────────────────────────────────────

/// Treble (G) clef. `ox,oy` = the staff's bottom-line origin (px); `space` = staff_size.
/// Spans roughly `[-5.0u, +1.3u]` — about 1 space above the top line to 1.3 spaces below
/// the bottom line, anchored so the belly loop crosses the G4 line (2 spaces above bottom).
pub(crate) fn clef_treble(ox: f32, oy: f32, space: f32) -> String {
    let cx = 0.7;
    let foot = arc_points(cx, 0.85, 0.42, 0.42, 200.0, 560.0, 24);
    let belly = arc_points(cx + 0.05, -1.55, 0.62, 1.05, 55.0, -305.0, 40);
    let top_curl = arc_points(cx - 0.5, -4.55, 0.42, 0.42, -10.0, 250.0, 24);
    let d = path_from_segments(&[foot, belly, top_curl], ox, oy, space);
    format!(
        r#"<path class="acorde-clef acorde-clef-treble" d="{d}" fill="none" stroke="black" stroke-width="{sw}" stroke-linecap="round" stroke-linejoin="round"/>"#,
        sw = f(0.15 * space)
    )
}

/// Bass (F) clef, anchored so the two dots straddle the F3 line (3 spaces above bottom).
pub(crate) fn clef_bass(ox: f32, oy: f32, space: f32) -> String {
    let cx = 0.15;
    let cy = -2.9;
    let hook = arc_points(cx, cy, 0.62, 1.55, -100.0, 95.0, 50);
    let tail_end = *hook.last().unwrap();
    let tail_flick = arc_points(tail_end.0 - 0.35, tail_end.1 - 0.35, 0.35, 0.35, 30.0, -180.0, 16);
    let d = path_from_segments(&[hook, tail_flick], ox, oy, space);
    let dot1 = dot_at(cx + 0.95, -3.5, ox, oy, space);
    let dot2 = dot_at(cx + 0.95, -2.5, ox, oy, space);
    let sw = f(0.15 * space);
    format!(
        r#"<g class="acorde-clef acorde-clef-bass"><path d="{d}" fill="none" stroke="black" stroke-width="{sw}" stroke-linecap="round" stroke-linejoin="round"/>{dot1}{dot2}</g>"#
    )
}

/// C clef (alto/tenor): two mirrored lens shapes meeting on the reference (middle) line,
/// flanked by two vertical bars spanning the staff height. `mid_position_u` is the
/// reference line's position in staff-space units above the bottom line (e.g. 2.0 for
/// alto clef's middle line).
pub(crate) fn clef_c(ox: f32, oy: f32, space: f32, mid_position_u: f32) -> String {
    let mid = -mid_position_u;
    let lens_top = arc_points(0.35, mid - 0.6, 0.35, 0.6, 90.0, 270.0, 20);
    let lens_bottom = arc_points(0.35, mid + 0.6, 0.35, 0.6, -90.0, 90.0, 20);
    let d = path_from_segments(&[lens_top, lens_bottom], ox, oy, space);
    let bar1_x = ox + 0.85 * space;
    let bar2_x = ox + 1.1 * space;
    let top_y = oy - 4.0 * space;
    let bottom_y = oy;
    let sw = f(0.15 * space);
    format!(
        r#"<g class="acorde-clef acorde-clef-c"><path d="{d}" fill="none" stroke="black" stroke-width="{sw}" stroke-linecap="round" stroke-linejoin="round"/><line x1="{bx1}" y1="{ty}" x2="{bx1}" y2="{by}" stroke="black" stroke-width="{sw}"/><line x1="{bx2}" y1="{ty}" x2="{bx2}" y2="{by}" stroke="black" stroke-width="{sw2}"/></g>"#,
        bx1 = f(bar1_x), bx2 = f(bar2_x), ty = f(top_y), by = f(bottom_y),
        sw2 = f(0.28 * space),
    )
}

fn dot_at(cx: f32, cy: f32, ox: f32, oy: f32, space: f32) -> String {
    format!(
        r#"<circle cx="{x}" cy="{y}" r="{r}" fill="black"/>"#,
        x = f(ox + cx * space), y = f(oy + cy * space), r = f(0.13 * space)
    )
}

// ── noteheads / stems / flags ───────────────────────────────────────────────────

/// Notehead ellipse centered at `(cx, cy)` px. `filled` = quarter/eighth (solid); otherwise
/// whole/half (hollow outline).
pub(crate) fn notehead(cx: f32, cy: f32, space: f32, filled: bool) -> String {
    let rx = f(0.62 * space * 0.5);
    let ry = f(0.48 * space * 0.5);
    if filled {
        format!(r#"<ellipse class="acorde-notehead" cx="{x}" cy="{y}" rx="{rx}" ry="{ry}" fill="black"/>"#, x = f(cx), y = f(cy))
    } else {
        let sw = f(0.16 * space * 0.5);
        format!(
            r#"<ellipse class="acorde-notehead" cx="{x}" cy="{y}" rx="{rx}" ry="{ry}" fill="none" stroke="black" stroke-width="{sw}"/>"#,
            x = f(cx), y = f(cy)
        )
    }
}

pub(crate) const NOTEHEAD_RX_U: f32 = 0.31; // half of 0.62u, matches `notehead()`
pub(crate) const DEFAULT_STEM_LEN_U: f32 = 3.0;

/// Stem of the default fixed length (unbeamed notes). Returns `(svg, tip_y)`.
pub(crate) fn stem(cx: f32, cy: f32, space: f32, up: bool) -> (String, f32) {
    let tip_y = if up { cy - DEFAULT_STEM_LEN_U * space } else { cy + DEFAULT_STEM_LEN_U * space };
    (stem_to(cx, cy, tip_y, space, up), tip_y)
}

/// Stem from the notehead to an explicit `tip_y` (beamed notes: the tip follows the beam
/// line, not the default fixed length).
pub(crate) fn stem_to(cx: f32, cy: f32, tip_y: f32, space: f32, up: bool) -> String {
    let x_off = NOTEHEAD_RX_U * space * 0.92;
    let x = if up { cx + x_off } else { cx - x_off };
    let sw = f(0.11 * space);
    format!(
        r#"<line class="acorde-stem" x1="{x1}" y1="{y1}" x2="{x2}" y2="{y2}" stroke="black" stroke-width="{sw}"/>"#,
        x1 = f(x), y1 = f(cy), x2 = f(x), y2 = f(tip_y)
    )
}

/// Eighth-note flag at the stem tip: a small filled wedge curling toward the notehead side.
pub(crate) fn flag(stem_x: f32, tip_y: f32, space: f32, up: bool) -> String {
    let s = if up { 1.0 } else { -1.0 };
    let d = format!(
        "M {ox},{oy} Q {cx1},{cy1} {ex},{ey} Q {cx2},{cy2} {ox},{oy} Z",
        ox = f(stem_x), oy = f(tip_y),
        cx1 = f(stem_x + 0.65 * space), cy1 = f(tip_y + 0.35 * space * s),
        ex = f(stem_x + 0.12 * space), ey = f(tip_y + 1.05 * space * s),
        cx2 = f(stem_x - 0.05 * space), cy2 = f(tip_y + 0.55 * space * s),
    );
    format!(r#"<path d="{d}" fill="black" stroke="none"/>"#)
}

// ── ledger lines / barlines ─────────────────────────────────────────────────────

pub(crate) fn ledger_line(cx: f32, y: f32, space: f32) -> String {
    let half_w = 0.5 * space;
    let sw = f(0.1 * space);
    format!(
        r#"<line class="acorde-ledger" x1="{x1}" y1="{y}" x2="{x2}" y2="{y}" stroke="black" stroke-width="{sw}"/>"#,
        x1 = f(cx - half_w), x2 = f(cx + half_w), y = f(y)
    )
}

pub(crate) fn barline(x: f32, top_y: f32, bottom_y: f32, space: f32, thick: bool) -> String {
    let sw = f(if thick { 0.3 * space } else { 0.09 * space });
    format!(
        r#"<line class="acorde-barline" x1="{x}" y1="{y1}" x2="{x}" y2="{y2}" stroke="black" stroke-width="{sw}"/>"#,
        x = f(x), y1 = f(top_y), y2 = f(bottom_y)
    )
}

// ── beams ────────────────────────────────────────────────────────────────────────

/// One beam segment (one beam level, spanning `(x1,y1)` to `(x2,y2)`), drawn as a filled
/// parallelogram of constant *vertical* thickness — not a true perpendicular offset, but
/// beam slopes are always shallow enough (see `beams::MAX_BEAM_RISE_U`) that the visual
/// difference is negligible, and this avoids trigonometry for a purely cosmetic gain.
pub(crate) fn beam_segment(x1: f32, y1: f32, x2: f32, y2: f32, thickness: f32) -> String {
    let ht = thickness / 2.0;
    format!(
        r#"<polygon class="acorde-beam" points="{x1},{y1a} {x2},{y2a} {x2},{y2b} {x1},{y1b}" fill="black"/>"#,
        x1 = f(x1), x2 = f(x2),
        y1a = f(y1 - ht), y2a = f(y2 - ht), y2b = f(y2 + ht), y1b = f(y1 + ht),
    )
}

// ── accidentals ──────────────────────────────────────────────────────────────────

/// Accidental glyph for `alter` (-2..=2), vertically centered on `cy` (the notehead's y).
/// Returns `None` for `alter == 0` when no explicit natural is being drawn by the caller
/// (callers decide whether alter=0 means "draw a natural sign" or "draw nothing").
pub(crate) fn accidental(alter: i8, cx: f32, cy: f32, space: f32) -> String {
    match alter {
        1 => sharp(cx, cy, space),
        -1 => flat(cx, cy, space),
        0 => natural(cx, cy, space),
        2 => double_sharp(cx, cy, space),
        -2 => double_flat(cx, cy, space),
        _ => String::new(), // unreachable: callers validate range before calling
    }
}

/// Horizontal footprint an accidental glyph occupies (u-units), for layout spacing.
pub(crate) fn accidental_width_u(alter: i8) -> f32 {
    match alter {
        -2 => 1.1,
        _ => 0.65,
    }
}

fn sharp(cx: f32, cy: f32, space: f32) -> String {
    let sw_v = f(0.09 * space);
    let sw_h = f(0.22 * space);
    let x1 = cx - 0.18 * space;
    let x2 = cx + 0.18 * space;
    let y_top = cy - 0.75 * space;
    let y_bot = cy + 0.75 * space;
    format!(
        // Single-line literal: a multi-line raw string here would embed this *source file's*
        // own line-ending bytes into the compiled string, making output diverge between
        // LF-checkout and CRLF-checkout platforms (see beams::tests and CI history).
        r#"<g class="acorde-accidental acorde-sharp"><line x1="{x1}" y1="{yt}" x2="{x1}" y2="{yb}" stroke="black" stroke-width="{swv}"/><line x1="{x2}" y1="{yt}" x2="{x2}" y2="{yb}" stroke="black" stroke-width="{swv}"/><line x1="{hx1}" y1="{hy1}" x2="{hx2}" y2="{hy1a}" stroke="black" stroke-width="{swh}"/><line x1="{hx1}" y1="{hy2}" x2="{hx2}" y2="{hy2a}" stroke="black" stroke-width="{swh}"/></g>"#,
        x1 = f(x1), x2 = f(x2), yt = f(y_top), yb = f(y_bot), swv = sw_v, swh = sw_h,
        hx1 = f(cx - 0.3 * space), hx2 = f(cx + 0.3 * space),
        hy1 = f(cy - 0.32 * space), hy1a = f(cy - 0.42 * space),
        hy2 = f(cy + 0.42 * space), hy2a = f(cy + 0.32 * space),
    )
}

fn flat(cx: f32, cy: f32, space: f32) -> String {
    let sw = f(0.1 * space);
    let x = cx - 0.22 * space;
    let y_top = cy - 0.85 * space;
    let y_bot = cy + 0.35 * space;
    let bowl = arc_points(0.0, 0.15, 0.32, 0.35, -90.0, 110.0, 16);
    let bowl_d = path_from_segments(&[bowl], x, cy, space);
    format!(
        r#"<g class="acorde-accidental acorde-flat"><line x1="{x}" y1="{yt}" x2="{x}" y2="{yb}" stroke="black" stroke-width="{sw}"/><path d="{bowl_d}" fill="none" stroke="black" stroke-width="{sw}" stroke-linecap="round"/></g>"#,
        x = f(x), yt = f(y_top), yb = f(y_bot), sw = sw,
    )
}

fn natural(cx: f32, cy: f32, space: f32) -> String {
    let sw_v = f(0.08 * space);
    let sw_h = f(0.16 * space);
    let x1 = cx - 0.18 * space;
    let x2 = cx + 0.18 * space;
    format!(
        r#"<g class="acorde-accidental acorde-natural"><line x1="{x1}" y1="{y1t}" x2="{x1}" y2="{y1b}" stroke="black" stroke-width="{swv}"/><line x1="{x2}" y1="{y2t}" x2="{x2}" y2="{y2b}" stroke="black" stroke-width="{swv}"/><line x1="{x1}" y1="{hy1}" x2="{x2}" y2="{hy1b}" stroke="black" stroke-width="{swh}"/><line x1="{x1}" y1="{hy2}" x2="{x2}" y2="{hy2b}" stroke="black" stroke-width="{swh}"/></g>"#,
        x1 = f(x1), x2 = f(x2),
        y1t = f(cy - 0.3 * space), y1b = f(cy + 0.85 * space),
        y2t = f(cy - 0.85 * space), y2b = f(cy + 0.3 * space),
        hy1 = f(cy - 0.34 * space), hy1b = f(cy - 0.5 * space),
        hy2 = f(cy + 0.5 * space), hy2b = f(cy + 0.34 * space),
        swv = sw_v, swh = sw_h,
    )
}

fn double_sharp(cx: f32, cy: f32, space: f32) -> String {
    let sw = f(0.16 * space);
    let r = 0.3 * space;
    format!(
        r#"<g class="acorde-accidental acorde-double-sharp"><line x1="{x1}" y1="{y1}" x2="{x2}" y2="{y2}" stroke="black" stroke-width="{sw}" stroke-linecap="round"/><line x1="{x2}" y1="{y1}" x2="{x1}" y2="{y2}" stroke="black" stroke-width="{sw}" stroke-linecap="round"/></g>"#,
        x1 = f(cx - r), x2 = f(cx + r), y1 = f(cy - r), y2 = f(cy + r), sw = sw,
    )
}

fn double_flat(cx: f32, cy: f32, space: f32) -> String {
    let left = flat(cx - 0.35 * space, cy, space);
    let right = flat(cx + 0.35 * space, cy, space);
    format!(r#"<g class="acorde-accidental acorde-double-flat">{left}{right}</g>"#)
}

// ── rests ────────────────────────────────────────────────────────────────────────

/// Rest glyph for a duration, centered horizontally at `cx`. `staff_mid_y` is the y of the
/// staff's middle line (position 4).
pub(crate) fn rest_whole(cx: f32, staff_mid_y: f32, space: f32) -> String {
    // Hangs below the 4th line (position 6): a filled block.
    let y = staff_mid_y - 2.0 * space;
    rest_block(cx, y, space, true)
}

pub(crate) fn rest_half(cx: f32, staff_mid_y: f32, space: f32) -> String {
    // Sits on top of the middle line (position 4).
    rest_block(cx, staff_mid_y, space, false)
}

fn rest_block(cx: f32, line_y: f32, space: f32, hangs_below: bool) -> String {
    let w = 0.6 * space;
    let h = 0.22 * space;
    let y = if hangs_below { line_y } else { line_y - h };
    format!(
        r#"<rect x="{x}" y="{y}" width="{w}" height="{h}" fill="black"/>"#,
        x = f(cx - w / 2.0), y = f(y), w = f(w), h = f(h)
    )
}

pub(crate) fn rest_quarter(cx: f32, staff_mid_y: f32, space: f32) -> String {
    // Simplified serpentine "squiggle", centered on the staff middle.
    let pts: Vec<(f32, f32)> = vec![
        (0.15, -1.1), (-0.15, -0.55), (0.2, -0.05), (-0.2, 0.55), (0.05, 0.75), (-0.15, 1.1),
    ];
    let d = path_from_segments(&[pts], cx, staff_mid_y, space);
    let sw = f(0.16 * space);
    format!(
        r#"<path d="{d}" fill="none" stroke="black" stroke-width="{sw}" stroke-linecap="round" stroke-linejoin="round"/>"#
    )
}

pub(crate) fn rest_eighth(cx: f32, staff_mid_y: f32, space: f32) -> String {
    let stroke = format!(
        r#"<line x1="{x1}" y1="{y1}" x2="{x2}" y2="{y2}" stroke="black" stroke-width="{sw}" stroke-linecap="round"/>"#,
        x1 = f(cx + 0.32 * space), y1 = f(staff_mid_y - 0.75 * space),
        x2 = f(cx - 0.28 * space), y2 = f(staff_mid_y + 0.75 * space),
        sw = f(0.13 * space),
    );
    let head = format!(
        r#"<circle cx="{x}" cy="{y}" r="{r}" fill="black"/>"#,
        x = f(cx + 0.32 * space), y = f(staff_mid_y - 0.6 * space), r = f(0.2 * space)
    );
    format!("{stroke}{head}")
}

/// Augmentation dot.
pub(crate) fn augmentation_dot(cx: f32, cy: f32, space: f32) -> String {
    format!(r#"<circle cx="{x}" cy="{y}" r="{r}" fill="black"/>"#, x = f(cx), y = f(cy), r = f(0.11 * space))
}

// ── digits (for time signatures) ────────────────────────────────────────────────

/// Segments lit for each digit, in 7-segment order: a,b,c,d,e,f,g
/// (a=top, b=top-right, c=bottom-right, d=bottom, e=bottom-left, f=top-left, g=middle).
const DIGIT_SEGMENTS: [[bool; 7]; 10] = [
    [true, true, true, true, true, true, false],    // 0
    [false, true, true, false, false, false, false], // 1
    [true, true, false, true, true, false, true],    // 2
    [true, true, true, true, false, false, true],    // 3
    [false, true, true, false, false, true, true],   // 4
    [true, false, true, true, false, true, true],    // 5
    [true, false, true, true, true, true, true],     // 6
    [true, true, true, false, false, false, false],  // 7
    [true, true, true, true, true, true, true],       // 8
    [true, true, true, true, false, true, true],      // 9
];

/// Digit glyph in a `0.6u` × `1.6u` box (7-segment style — plain vector geometry, no font).
/// `ox,oy` place the digit's top-left corner.
pub(crate) fn digit(d: u8, ox: f32, oy: f32, space: f32) -> String {
    let segs = DIGIT_SEGMENTS[(d.min(9)) as usize];
    let w = 0.55 * space;
    let h = 1.5 * space;
    let mid = h / 2.0;
    let sw = f(0.14 * space);
    let mut out = String::new();
    let mut seg_line = |on: bool, x1: f32, y1: f32, x2: f32, y2: f32| {
        if on {
            let _ = write!(
                out,
                r#"<line x1="{x1}" y1="{y1}" x2="{x2}" y2="{y2}" stroke="black" stroke-width="{sw}" stroke-linecap="square"/>"#,
                x1 = f(ox + x1), y1 = f(oy + y1), x2 = f(ox + x2), y2 = f(oy + y2)
            );
        }
    };
    seg_line(segs[0], 0.0, 0.0, w, 0.0); // a: top
    seg_line(segs[1], w, 0.0, w, mid); // b: top-right
    seg_line(segs[2], w, mid, w, h); // c: bottom-right
    seg_line(segs[3], 0.0, h, w, h); // d: bottom
    seg_line(segs[4], 0.0, mid, 0.0, h); // e: bottom-left
    seg_line(segs[5], 0.0, 0.0, 0.0, mid); // f: top-left
    seg_line(segs[6], 0.0, mid, w, mid); // g: middle
    out
}

/// Width (u) a single digit occupies, including trailing gap.
pub(crate) const DIGIT_WIDTH_U: f32 = 0.75;

// ── tuplets ──────────────────────────────────────────────────────────────────────

/// One bracket segment of a tuplet bracket (a hook or a horizontal run).
pub(crate) fn tuplet_line(x1: f32, y1: f32, x2: f32, y2: f32, space: f32) -> String {
    let sw = f(0.09 * space);
    format!(
        r#"<line class="acorde-tuplet-bracket" x1="{x1}" y1="{y1}" x2="{x2}" y2="{y2}" stroke="black" stroke-width="{sw}"/>"#,
        x1 = f(x1), y1 = f(y1), x2 = f(x2), y2 = f(y2)
    )
}

/// Tuplet ratio number (just `actual_notes` — e.g. "3" for a triplet — matching standard
/// notation practice; the full N:M ratio is implied by context and not printed), centered
/// horizontally on `cx` and vertically on `cy`. Reuses the same 7-segment `digit()` glyphs
/// as the time signature, at a smaller scale.
pub(crate) fn tuplet_number(n: u8, cx: f32, cy: f32, space: f32) -> String {
    let digit_space = 0.65 * space;
    let digits: Vec<u8> = if n == 0 {
        vec![0]
    } else {
        let mut d = Vec::new();
        let mut v = n;
        while v > 0 {
            d.push(v % 10);
            v /= 10;
        }
        d.reverse();
        d
    };
    let total_w = digits.len() as f32 * DIGIT_WIDTH_U * digit_space;
    let mut ox = cx - total_w / 2.0;
    let oy = cy - 0.75 * digit_space;
    let mut out = String::from(r#"<g class="acorde-tuplet-number">"#);
    for d in digits {
        out.push_str(&digit(d, ox, oy, digit_space));
        ox += DIGIT_WIDTH_U * digit_space;
    }
    out.push_str("</g>");
    out
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn f_formats_two_decimals() {
        assert_eq!(f(1.0), "1.00");
        assert_eq!(f(1.005), "1.00"); // ties-to-even at f32 precision, just check length/format
    }

    #[test]
    fn clef_treble_is_stable_across_calls() {
        assert_eq!(clef_treble(0.0, 0.0, 24.0), clef_treble(0.0, 0.0, 24.0));
    }

    #[test]
    fn digit_glyphs_nonempty_for_all_digits() {
        for d in 0..=9u8 {
            assert!(!digit(d, 0.0, 0.0, 20.0).is_empty(), "digit {d} produced no segments");
        }
    }

    #[test]
    fn accidental_covers_supported_range() {
        for alter in [-2, -1, 0, 1, 2] {
            assert!(!accidental(alter, 0.0, 0.0, 20.0).is_empty());
        }
    }
}
