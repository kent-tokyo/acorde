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

use acorde_core::NoteHead;

/// Format a coordinate/length with fixed precision — keeps `sin`/`cos`-derived glyph
/// geometry stable across platforms (ULP-level differences vanish at 2 decimals).
pub(crate) fn f(v: f32) -> String {
    format!("{v:.2}")
}

fn arc_points(
    cx: f32,
    cy: f32,
    rx: f32,
    ry: f32,
    start_deg: f32,
    end_deg: f32,
    steps: u32,
) -> Vec<(f32, f32)> {
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

/// Stroke a path given in u units (x right, y down from the staff's bottom line) at `(ox, oy)`.
fn stroked(
    class: &str,
    d_units: &[(char, &[(f32, f32)])],
    ox: f32,
    oy: f32,
    space: f32,
    width_u: f32,
) -> String {
    let mut d = String::new();
    for (command, points) in d_units {
        d.push(*command);
        for &(x, y) in points.iter() {
            let _ = write!(d, " {},{}", f(ox + x * space), f(oy + y * space));
        }
        d.push(' ');
    }
    format!(
        r#"<path class="{class}" d="{}" fill="none" stroke="black" stroke-width="{}" stroke-linecap="round" stroke-linejoin="round"/>"#,
        d.trim_end(),
        f(width_u * space)
    )
}

/// Treble (G) clef, after the engraved shape: a spiral round the G4 line (two spaces above
/// the bottom line), a spine rising to a loop above the staff, and a tail ending in a ball
/// below it. `ox,oy` = the staff's bottom-line origin (px); `space` = staff_size.
pub(crate) fn clef_treble(ox: f32, oy: f32, space: f32) -> String {
    let path = stroked(
        "acorde-clef acorde-clef-treble",
        &[
            ('M', &[(1.05_f32, -1.55_f32)][..]),
            (
                'C',
                &[
                    (0.55, -1.6),
                    (0.5, -2.4),
                    (1.05, -2.45),
                    (1.75, -2.5),
                    (1.85, -1.05),
                    (1.0, -0.95),
                    (0.15, -0.9),
                    (-0.1, -2.2),
                    (0.55, -3.05),
                    (1.0, -3.65),
                    (1.5, -4.25),
                    (1.38, -5.05),
                    (1.3, -5.7),
                    (0.8, -5.6),
                    (0.72, -5.0),
                    (0.58, -4.1),
                    (0.85, -1.8),
                    (1.1, 0.6),
                    (1.15, 1.25),
                    (0.75, 1.5),
                    (0.5, 1.3),
                ],
            ),
        ],
        ox,
        oy,
        space,
        0.19,
    );
    let ball = dot_at(0.62, 1.12, ox, oy, space).replace(
        &format!(r#"r="{}""#, f(0.13 * space)),
        &format!(r#"r="{}""#, f(0.27 * space)),
    );
    format!(r#"<g class="acorde-clef acorde-clef-treble">{path}{ball}</g>"#)
}

/// Bass (F) clef: a ball on the F3 line (the fourth line), a hook sweeping right and down,
/// and two dots either side of the F line.
pub(crate) fn clef_bass(ox: f32, oy: f32, space: f32) -> String {
    let path = stroked(
        "acorde-clef-bass-hook",
        &[
            ('M', &[(0.25_f32, -3.05_f32)][..]),
            (
                'C',
                &[
                    (0.3, -4.0),
                    (1.75, -4.25),
                    (1.8, -3.0),
                    (1.85, -1.9),
                    (1.05, -0.85),
                    (0.1, -0.35),
                ],
            ),
        ],
        ox,
        oy,
        space,
        0.24,
    );
    let ball = format!(
        r#"<circle cx="{}" cy="{}" r="{}" fill="black"/>"#,
        f(ox + 0.42 * space),
        f(oy - 3.05 * space),
        f(0.3 * space)
    );
    let dot1 = dot_at(2.2, -3.5, ox, oy, space);
    let dot2 = dot_at(2.2, -2.5, ox, oy, space);
    format!(r#"<g class="acorde-clef acorde-clef-bass">{ball}{path}{dot1}{dot2}</g>"#)
}

/// C clef (alto/tenor): a thick and a thin bar spanning the staff, and two curled brackets
/// meeting at the reference line. `mid_position_u` is that line's height in staff spaces
/// above the bottom line (2.0 alto, 3.0 tenor).
pub(crate) fn clef_c(ox: f32, oy: f32, space: f32, mid_position_u: f32) -> String {
    let m = -mid_position_u;
    let upper = [
        (0.62, m),
        (1.0, m - 0.45),
        (1.75, m - 0.55),
        (1.75, m - 1.3),
        (1.75, m - 2.05),
        (1.05, m - 2.1),
        (0.95, m - 1.6),
    ];
    let lower = [
        (0.62, m),
        (1.0, m + 0.45),
        (1.75, m + 0.55),
        (1.75, m + 1.3),
        (1.75, m + 2.05),
        (1.05, m + 2.1),
        (0.95, m + 1.6),
    ];
    let brackets = stroked(
        "acorde-clef-c-brackets",
        &[
            ('M', &upper[..1]),
            ('C', &upper[1..]),
            ('M', &lower[..1]),
            ('C', &lower[1..]),
        ],
        ox,
        oy,
        space,
        0.2,
    );
    let top_y = oy - 4.0 * space;
    format!(
        r#"<g class="acorde-clef acorde-clef-c"><line x1="{x1}" y1="{ty}" x2="{x1}" y2="{by}" stroke="black" stroke-width="{w1}"/><line x1="{x2}" y1="{ty}" x2="{x2}" y2="{by}" stroke="black" stroke-width="{w2}"/>{brackets}</g>"#,
        x1 = f(ox + 0.18 * space),
        x2 = f(ox + 0.52 * space),
        ty = f(top_y),
        by = f(oy),
        w1 = f(0.34 * space),
        w2 = f(0.11 * space),
    )
}

/// Percussion clef: two deterministic vertical bars spanning the five-line staff.
pub(crate) fn clef_percussion(ox: f32, oy: f32, space: f32) -> String {
    let top_y = oy - 4.0 * space;
    let x1 = ox + 0.35 * space;
    let x2 = ox + 0.85 * space;
    let sw = f(0.18 * space);
    format!(
        r#"<g class="acorde-clef acorde-clef-percussion"><line x1="{x1}" y1="{top_y}" x2="{x1}" y2="{oy}" stroke="black" stroke-width="{sw}"/><line x1="{x2}" y1="{top_y}" x2="{x2}" y2="{oy}" stroke="black" stroke-width="{sw}"/></g>"#,
        x1 = f(x1),
        x2 = f(x2),
        top_y = f(top_y),
        oy = f(oy),
        sw = sw,
    )
}

fn dot_at(cx: f32, cy: f32, ox: f32, oy: f32, space: f32) -> String {
    format!(
        r#"<circle cx="{x}" cy="{y}" r="{r}" fill="black"/>"#,
        x = f(ox + cx * space),
        y = f(oy + cy * space),
        r = f(0.13 * space)
    )
}

// ── noteheads / stems / flags ───────────────────────────────────────────────────

/// Notehead ellipse centered at `(cx, cy)` px. `filled` = quarter/eighth (solid); otherwise
/// whole/half (hollow outline).
///
/// Proportions follow SMuFL's Bravura `noteheadBlack`: a tilted oval about 1.15 spaces wide and
/// 0.9 spaces tall, so a head fills the space between two staff lines.
pub(crate) fn notehead(cx: f32, cy: f32, space: f32, filled: bool) -> String {
    let rx = f(NOTEHEAD_OVAL_RX_U * space);
    let ry = f(NOTEHEAD_OVAL_RY_U * space);
    let tilt = format!("rotate(-20 {} {})", f(cx), f(cy));
    if filled {
        format!(
            r#"<ellipse class="acorde-notehead" cx="{x}" cy="{y}" rx="{rx}" ry="{ry}" transform="{tilt}" fill="black"/>"#,
            x = f(cx),
            y = f(cy)
        )
    } else {
        let sw = f(0.13 * space);
        format!(
            r#"<ellipse class="acorde-notehead" cx="{x}" cy="{y}" rx="{rx}" ry="{ry}" transform="{tilt}" fill="none" stroke="black" stroke-width="{sw}"/>"#,
            x = f(cx),
            y = f(cy)
        )
    }
}

/// Semi-axes of the (untilted) notehead oval, in staff spaces.
const NOTEHEAD_OVAL_RX_U: f32 = 0.56;
const NOTEHEAD_OVAL_RY_U: f32 = 0.4;

/// Render the model-selected notehead without relying on a notation font.
pub(crate) fn notehead_shape(
    head: &NoteHead,
    cx: f32,
    cy: f32,
    space: f32,
    filled: bool,
) -> String {
    match head {
        NoteHead::Normal => notehead(cx, cy, space, filled),
        NoteHead::Diamond => {
            let w = 0.6 * space;
            let h = 0.5 * space;
            let points = format!(
                "{},{} {},{} {},{} {},{}",
                f(cx),
                f(cy - h),
                f(cx + w),
                f(cy),
                f(cx),
                f(cy + h),
                f(cx - w),
                f(cy)
            );
            let fill = if filled { "black" } else { "none" };
            format!(
                r#"<polygon class="acorde-notehead acorde-notehead-diamond" points="{points}" fill="{fill}" stroke="black" stroke-width="{}"/>"#,
                f(0.12 * space)
            )
        }
        NoteHead::Triangle => {
            let points = format!(
                "{},{} {},{} {},{}",
                f(cx),
                f(cy - 0.5 * space),
                f(cx + 0.6 * space),
                f(cy + 0.42 * space),
                f(cx - 0.6 * space),
                f(cy + 0.42 * space)
            );
            let fill = if filled { "black" } else { "none" };
            format!(
                r#"<polygon class="acorde-notehead acorde-notehead-triangle" points="{points}" fill="{fill}" stroke="black" stroke-width="{}"/>"#,
                f(0.12 * space)
            )
        }
        NoteHead::X | NoteHead::Cross => {
            let r = if matches!(head, NoteHead::X) {
                0.45
            } else {
                0.58
            } * space;
            let sw = f(0.16 * space);
            format!(
                r#"<g class="acorde-notehead acorde-notehead-{}" stroke="black" stroke-width="{sw}" stroke-linecap="round"><line x1="{}" y1="{}" x2="{}" y2="{}"/><line x1="{}" y1="{}" x2="{}" y2="{}"/></g>"#,
                if matches!(head, NoteHead::X) {
                    "x"
                } else {
                    "cross"
                },
                f(cx - r),
                f(cy - r),
                f(cx + r),
                f(cy + r),
                f(cx - r),
                f(cy + r),
                f(cx + r),
                f(cy - r)
            )
        }
        NoteHead::Slash => {
            format!(
                r#"<line class="acorde-notehead acorde-notehead-slash" x1="{}" y1="{}" x2="{}" y2="{}" stroke="black" stroke-width="{}" stroke-linecap="round"/>"#,
                f(cx - 0.6 * space),
                f(cy + 0.5 * space),
                f(cx + 0.6 * space),
                f(cy - 0.5 * space),
                f(0.22 * space)
            )
        }
    }
}

/// Half the horizontal extent of the tilted `notehead()` oval, in staff spaces.
pub(crate) const NOTEHEAD_RX_U: f32 = 0.56;
pub(crate) const DEFAULT_STEM_LEN_U: f32 = 3.0;
/// Stem line width, in staff spaces.
pub(crate) const STEM_WIDTH_U: f32 = 0.11;

/// Stem of the default fixed length (unbeamed notes). Returns `(svg, tip_y)`.
pub(crate) fn stem(cx: f32, cy: f32, space: f32, up: bool) -> (String, f32) {
    let tip_y = if up {
        cy - DEFAULT_STEM_LEN_U * space
    } else {
        cy + DEFAULT_STEM_LEN_U * space
    };
    (stem_to(cx, cy, tip_y, space, up), tip_y)
}

/// Stem from the notehead to an explicit `tip_y` (beamed notes: the tip follows the beam
/// line, not the default fixed length).
pub(crate) fn stem_to(cx: f32, cy: f32, tip_y: f32, space: f32, up: bool) -> String {
    let x_off = NOTEHEAD_RX_U * space * 0.92;
    let x = if up { cx + x_off } else { cx - x_off };
    let sw = f(STEM_WIDTH_U * space);
    format!(
        r#"<line class="acorde-stem" x1="{x1}" y1="{y1}" x2="{x2}" y2="{y2}" stroke="black" stroke-width="{sw}"/>"#,
        x1 = f(x),
        y1 = f(cy),
        x2 = f(x),
        y2 = f(tip_y)
    )
}

/// Eighth-note flag at the stem tip: an S-shaped hook that leaves the stem on its right and
/// sweeps back toward the notehead (down from an up-stem, up from a down-stem), after the
/// engraved `flag8thUp`/`flag8thDown` shapes.
pub(crate) fn flag(stem_x: f32, tip_y: f32, space: f32, up: bool) -> String {
    let d = if up { 1.0 } else { -1.0 };
    let p = |dx: f32, dy: f32| format!("{},{}", f(stem_x + dx * space), f(tip_y + dy * d * space));
    let path = format!(
        "M {} C {} {} {} C {} {} {} Z",
        p(0.0, 0.0),
        p(0.05, 0.9),
        p(1.05, 1.25),
        p(0.8, 2.7),
        p(0.9, 1.7),
        p(0.35, 1.45),
        p(0.0, 1.2),
    );
    format!(r#"<path class="acorde-flag" d="{path}" fill="black" stroke="none"/>"#)
}

// ── ledger lines / barlines ─────────────────────────────────────────────────────

pub(crate) fn ledger_line(cx: f32, y: f32, space: f32) -> String {
    // A ledger line extends a little beyond the notehead on both sides.
    let half_w = (NOTEHEAD_RX_U + 0.22) * space;
    let sw = f(0.1 * space);
    format!(
        r#"<line class="acorde-ledger" x1="{x1}" y1="{y}" x2="{x2}" y2="{y}" stroke="black" stroke-width="{sw}"/>"#,
        x1 = f(cx - half_w),
        x2 = f(cx + half_w),
        y = f(y)
    )
}

pub(crate) fn barline(x: f32, top_y: f32, bottom_y: f32, space: f32, thick: bool) -> String {
    let sw = f(if thick { 0.3 * space } else { 0.09 * space });
    format!(
        r#"<line class="acorde-barline" x1="{x}" y1="{y1}" x2="{x}" y2="{y2}" stroke="black" stroke-width="{sw}"/>"#,
        x = f(x),
        y1 = f(top_y),
        y2 = f(bottom_y)
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
        x1 = f(x1),
        x2 = f(x2),
        y1a = f(y1 - ht),
        y2a = f(y2 - ht),
        y2b = f(y2 + ht),
        y1b = f(y1 + ht),
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
        x1 = f(x1),
        x2 = f(x2),
        yt = f(y_top),
        yb = f(y_bot),
        swv = sw_v,
        swh = sw_h,
        hx1 = f(cx - 0.3 * space),
        hx2 = f(cx + 0.3 * space),
        hy1 = f(cy - 0.32 * space),
        hy1a = f(cy - 0.42 * space),
        hy2 = f(cy + 0.42 * space),
        hy2a = f(cy + 0.32 * space),
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
        x = f(x),
        yt = f(y_top),
        yb = f(y_bot),
        sw = sw,
    )
}

fn natural(cx: f32, cy: f32, space: f32) -> String {
    let sw_v = f(0.08 * space);
    let sw_h = f(0.16 * space);
    let x1 = cx - 0.18 * space;
    let x2 = cx + 0.18 * space;
    format!(
        r#"<g class="acorde-accidental acorde-natural"><line x1="{x1}" y1="{y1t}" x2="{x1}" y2="{y1b}" stroke="black" stroke-width="{swv}"/><line x1="{x2}" y1="{y2t}" x2="{x2}" y2="{y2b}" stroke="black" stroke-width="{swv}"/><line x1="{x1}" y1="{hy1}" x2="{x2}" y2="{hy1b}" stroke="black" stroke-width="{swh}"/><line x1="{x1}" y1="{hy2}" x2="{x2}" y2="{hy2b}" stroke="black" stroke-width="{swh}"/></g>"#,
        x1 = f(x1),
        x2 = f(x2),
        y1t = f(cy - 0.3 * space),
        y1b = f(cy + 0.85 * space),
        y2t = f(cy - 0.85 * space),
        y2b = f(cy + 0.3 * space),
        hy1 = f(cy - 0.34 * space),
        hy1b = f(cy - 0.5 * space),
        hy2 = f(cy + 0.5 * space),
        hy2b = f(cy + 0.34 * space),
        swv = sw_v,
        swh = sw_h,
    )
}

fn double_sharp(cx: f32, cy: f32, space: f32) -> String {
    let sw = f(0.16 * space);
    let r = 0.3 * space;
    format!(
        r#"<g class="acorde-accidental acorde-double-sharp"><line x1="{x1}" y1="{y1}" x2="{x2}" y2="{y2}" stroke="black" stroke-width="{sw}" stroke-linecap="round"/><line x1="{x2}" y1="{y1}" x2="{x1}" y2="{y2}" stroke="black" stroke-width="{sw}" stroke-linecap="round"/></g>"#,
        x1 = f(cx - r),
        x2 = f(cx + r),
        y1 = f(cy - r),
        y2 = f(cy + r),
        sw = sw,
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
    // Hangs below the 4th line, one space above the middle line: a filled block.
    let y = staff_mid_y - space;
    rest_block(cx, y, space, true)
}

pub(crate) fn rest_half(cx: f32, staff_mid_y: f32, space: f32) -> String {
    // Sits on top of the middle line (position 4).
    rest_block(cx, staff_mid_y, space, false)
}

fn rest_block(cx: f32, line_y: f32, space: f32, hangs_below: bool) -> String {
    // SMuFL restWhole/restHalf proportions (Bravura): about 1.13 spaces wide, half a space tall.
    let w = 1.13 * space;
    let h = 0.5 * space;
    let y = if hangs_below { line_y } else { line_y - h };
    format!(
        r#"<rect x="{x}" y="{y}" width="{w}" height="{h}" fill="black"/>"#,
        x = f(cx - w / 2.0),
        y = f(y),
        w = f(w),
        h = f(h)
    )
}

pub(crate) fn rest_quarter(cx: f32, staff_mid_y: f32, space: f32) -> String {
    // Engraved quarter rest: a zigzag of two thick strokes with a curled foot, about three
    // spaces tall and centred on the middle line.
    let p = |x: f32, y: f32| format!("{},{}", f(cx + x * space), f(staff_mid_y + y * space));
    let d = format!(
        "M {} L {} C {} {} {} L {} C {} {} {}",
        p(-0.08, -1.45),
        p(0.38, -0.82),
        p(0.14, -0.56),
        p(0.04, -0.36),
        p(0.0, -0.24),
        p(0.42, 0.38),
        p(0.02, 0.24),
        p(-0.32, 0.56),
        p(0.12, 1.18),
    );
    format!(
        r#"<path class="acorde-rest-quarter" d="{d}" fill="none" stroke="black" stroke-width="{}" stroke-linecap="round" stroke-linejoin="round"/>"#,
        f(0.21 * space)
    )
}

/// Eighth, sixteenth, … rests: a slanted stem with one ball-ended hook per flag, the hooks
/// stepping down the stem (engraved `restEighth`/`rest16th` shapes).
pub(crate) fn rest_short(cx: f32, staff_mid_y: f32, space: f32, flags: usize) -> String {
    let flags = flags.max(1);
    let slant = 0.27; // horizontal run per space of fall
    let top = (0.4, -0.62);
    let fall = 1.55 + 0.85 * (flags - 1) as f32;
    let foot = (top.0 - slant * fall, top.1 + fall);
    let p = |x: f32, y: f32| format!("{},{}", f(cx + x * space), f(staff_mid_y + y * space));
    let mut out = format!(
        r#"<g class="acorde-rest-short"><path d="M {} L {}" fill="none" stroke="black" stroke-width="{}" stroke-linecap="round"/>"#,
        p(top.0, top.1),
        p(foot.0, foot.1),
        f(0.13 * space)
    );
    for index in 0..flags {
        let drop = 0.85 * index as f32;
        let (sx, sy) = (top.0 - slant * drop, top.1 + drop);
        let _ = write!(
            out,
            r#"<path class="acorde-rest-flag" d="M {} C {} {} {}" fill="none" stroke="black" stroke-width="{}" stroke-linecap="round"/><circle cx="{}" cy="{}" r="{}" fill="black"/>"#,
            p(sx, sy),
            p(sx - 0.2, sy + 0.32),
            p(sx - 0.45, sy + 0.3),
            p(sx - 0.62, sy + 0.12),
            f(0.12 * space),
            f(cx + (sx - 0.6) * space),
            f(staff_mid_y + (sy + 0.05) * space),
            f(0.19 * space)
        );
    }
    out.push_str("</g>");
    out
}

/// Augmentation dot.
pub(crate) fn augmentation_dot(cx: f32, cy: f32, space: f32) -> String {
    format!(
        r#"<circle cx="{x}" cy="{y}" r="{r}" fill="black"/>"#,
        x = f(cx),
        y = f(cy),
        r = f(0.11 * space)
    )
}

// ── digits (for time signatures) ────────────────────────────────────────────────

/// Centre-line strokes of the digits 0–9 in a box 1.2 spaces wide and 2 spaces tall (origin
/// top-left), drawn bold with round ends like engraved time-signature numerals.
fn digit_strokes(d: u8) -> Vec<(char, Vec<(f32, f32)>)> {
    let seg = |command: char, points: &[(f32, f32)]| (command, points.to_vec());
    match d {
        0 => vec![
            seg('M', &[(0.6, 0.2)]),
            seg(
                'C',
                &[
                    (0.12, 0.2),
                    (0.12, 1.8),
                    (0.6, 1.8),
                    (1.08, 1.8),
                    (1.08, 0.2),
                    (0.6, 0.2),
                ],
            ),
        ],
        1 => vec![
            seg('M', &[(0.3, 0.55)]),
            seg('L', &[(0.72, 0.2), (0.72, 1.8)]),
            seg('M', &[(0.35, 1.8)]),
            seg('L', &[(1.05, 1.8)]),
        ],
        2 => vec![
            seg('M', &[(0.2, 0.6)]),
            seg(
                'C',
                &[
                    (0.22, 0.1),
                    (1.0, 0.08),
                    (1.0, 0.62),
                    (1.0, 1.05),
                    (0.3, 1.3),
                    (0.2, 1.8),
                ],
            ),
            seg('L', &[(1.05, 1.8)]),
        ],
        3 => vec![
            seg('M', &[(0.22, 0.42)]),
            seg(
                'C',
                &[
                    (0.4, 0.08),
                    (1.0, 0.1),
                    (0.95, 0.5),
                    (0.92, 0.85),
                    (0.62, 0.95),
                    (0.5, 0.95),
                    (0.72, 0.95),
                    (1.02, 1.1),
                    (0.98, 1.42),
                    (0.95, 1.92),
                    (0.32, 1.9),
                    (0.18, 1.58),
                ],
            ),
        ],
        4 => vec![
            seg('M', &[(0.82, 1.85)]),
            seg('L', &[(0.82, 0.2), (0.15, 1.3), (1.12, 1.3)]),
        ],
        5 => vec![
            seg('M', &[(1.0, 0.2)]),
            seg('L', &[(0.3, 0.2), (0.25, 0.9)]),
            seg(
                'C',
                &[
                    (0.5, 0.72),
                    (1.04, 0.8),
                    (1.0, 1.3),
                    (0.98, 1.92),
                    (0.32, 1.9),
                    (0.18, 1.6),
                ],
            ),
        ],
        6 => vec![
            seg('M', &[(0.95, 0.35)]),
            seg(
                'C',
                &[
                    (0.75, 0.08),
                    (0.18, 0.2),
                    (0.2, 1.1),
                    (0.2, 1.92),
                    (1.02, 1.9),
                    (1.0, 1.3),
                    (0.98, 0.78),
                    (0.32, 0.8),
                    (0.22, 1.2),
                ],
            ),
        ],
        7 => vec![
            seg('M', &[(0.18, 0.2)]),
            seg('L', &[(1.05, 0.2)]),
            seg('C', &[(0.7, 0.7), (0.5, 1.2), (0.5, 1.82)]),
        ],
        8 => vec![
            seg('M', &[(0.6, 0.95)]),
            seg(
                'C',
                &[
                    (0.2, 0.85),
                    (0.25, 0.2),
                    (0.6, 0.2),
                    (0.95, 0.2),
                    (1.0, 0.85),
                    (0.6, 0.95),
                    (0.14, 1.05),
                    (0.14, 1.8),
                    (0.6, 1.8),
                    (1.06, 1.8),
                    (1.06, 1.05),
                    (0.6, 0.95),
                ],
            ),
        ],
        _ => vec![
            seg('M', &[(0.25, 1.65)]),
            seg(
                'C',
                &[
                    (0.45, 1.92),
                    (1.02, 1.8),
                    (1.0, 0.9),
                    (1.0, 0.08),
                    (0.18, 0.1),
                    (0.2, 0.7),
                    (0.22, 1.22),
                    (0.88, 1.2),
                    (0.98, 0.8),
                ],
            ),
        ],
    }
}

/// Digit glyph in a 1.2 × 2 space box (a staff half: the numerator fills the top two spaces),
/// with `ox,oy` at its top-left corner; plain vector strokes, no font.
pub(crate) fn digit(d: u8, ox: f32, oy: f32, space: f32) -> String {
    let strokes = digit_strokes(d.min(9));
    let borrowed: Vec<(char, &[(f32, f32)])> = strokes
        .iter()
        .map(|(command, points)| (*command, points.as_slice()))
        .collect();
    stroked("acorde-digit", &borrowed, ox, oy, space, 0.34)
}

/// Width (u) a single digit occupies, including trailing gap.
pub(crate) const DIGIT_WIDTH_U: f32 = 1.35;

// ── tuplets ──────────────────────────────────────────────────────────────────────

/// One bracket segment of a tuplet bracket (a hook or a horizontal run).
pub(crate) fn tuplet_line(x1: f32, y1: f32, x2: f32, y2: f32, space: f32) -> String {
    let sw = f(0.09 * space);
    format!(
        r#"<line class="acorde-tuplet-bracket" x1="{x1}" y1="{y1}" x2="{x2}" y2="{y2}" stroke="black" stroke-width="{sw}"/>"#,
        x1 = f(x1),
        y1 = f(y1),
        x2 = f(x2),
        y2 = f(y2)
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
            assert!(
                !digit(d, 0.0, 0.0, 20.0).is_empty(),
                "digit {d} produced no segments"
            );
        }
    }

    #[test]
    fn accidental_covers_supported_range() {
        for alter in [-2, -1, 0, 1, 2] {
            assert!(!accidental(alter, 0.0, 0.0, 20.0).is_empty());
        }
    }
}
