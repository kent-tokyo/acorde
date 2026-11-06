//! Opt-in SMuFL glyphs: music symbols emitted as `<text>` code points in a host-supplied font
//! (Bravura, Leipzig, Petaluma, …) instead of the built-in vector paths.
//!
//! Every SMuFL code point the renderer uses lives in this file. Each emitter returns `None` when
//! no SMuFL font is selected or the symbol has no mapping, and the caller then draws the vector
//! glyph unchanged — so default output stays byte-identical. Glyphs are placed at their SMuFL
//! origin (baseline at the staff-line / pitch anchor; font size = 4 staff spaces, the SMuFL em).
//! Horizontal advances still come from the vector metrics: layout never reflows for a font.

use std::cell::RefCell;

use acorde_core::{Articulation, Duration, NoteHead};

use crate::glyphs::{STEM_WIDTH_U, f};

thread_local! {
    /// The XML-escaped font family selected for the render in progress on this thread.
    static FAMILY: RefCell<Option<String>> = const { RefCell::new(None) };
}

/// Selects the SMuFL family for one render and restores the previous selection on drop.
// ponytail: thread-local instead of threading a parameter through ~60 glyph call sites. Renders
// are synchronous and the guard nests, so the ceiling is only a render that hands work to another
// thread mid-render; pass the source explicitly if that ever happens.
pub(crate) struct Scope(Option<String>);

pub(crate) fn scope(family: Option<&str>) -> Scope {
    let next = family.map(crate::render::escape_xml);
    Scope(FAMILY.with(|cell| cell.replace(next)))
}

impl Drop for Scope {
    fn drop(&mut self) {
        let previous = self.0.take();
        FAMILY.with(|cell| *cell.borrow_mut() = previous);
    }
}

/// One `<text>` element holding `glyphs` with its SMuFL origin at `(x, y)`.
/// `centered` anchors the glyph's advance on `x` instead of its left edge.
fn glyph_run(
    class: &str,
    glyphs: &[char],
    x: f32,
    y: f32,
    em: f32,
    centered: bool,
) -> Option<String> {
    FAMILY.with(|cell| {
        let family = cell.borrow();
        let family = family.as_deref()?;
        let class = if class.is_empty() {
            String::new()
        } else {
            format!(r#" class="{class}""#)
        };
        let anchor = if centered {
            r#" text-anchor="middle""#
        } else {
            ""
        };
        let mut code_points = String::new();
        for glyph in glyphs {
            code_points.push_str(&format!("&#x{:X};", u32::from(*glyph)));
        }
        Some(format!(
            r#"<text{class} x="{}" y="{}" font-family="{family}" font-size="{}"{anchor}>{code_points}</text>"#,
            f(x),
            f(y),
            f(em)
        ))
    })
}

/// A glyph at the staff size: one SMuFL em is four staff spaces.
fn glyph(class: &str, cp: char, x: f32, y: f32, space: f32, centered: bool) -> Option<String> {
    glyph_run(class, &[cp], x, y, 4.0 * space, centered)
}

// ── clefs: origin at the left edge, baseline on the clef's reference line ─────────────

/// `gClef`, baseline on the G line (one space above the bottom line).
pub(crate) fn clef_treble(ox: f32, oy: f32, space: f32) -> Option<String> {
    let class = "acorde-clef acorde-clef-treble";
    glyph(class, '\u{E050}', ox, oy - space, space, false)
}

/// `fClef`, baseline on the F line (the fourth line).
pub(crate) fn clef_bass(ox: f32, oy: f32, space: f32) -> Option<String> {
    let class = "acorde-clef acorde-clef-bass";
    glyph(class, '\u{E062}', ox, oy - 3.0 * space, space, false)
}

/// `cClef`, centred on its reference line.
pub(crate) fn clef_c(ox: f32, oy: f32, space: f32, mid_position_u: f32) -> Option<String> {
    let y = oy - mid_position_u * space;
    glyph("acorde-clef acorde-clef-c", '\u{E05C}', ox, y, space, false)
}

/// `unpitchedPercussionClef1`, centred on the middle line.
pub(crate) fn clef_percussion(ox: f32, oy: f32, space: f32) -> Option<String> {
    let class = "acorde-clef acorde-clef-percussion";
    glyph(class, '\u{E069}', ox, oy - 2.0 * space, space, false)
}

// ── noteheads, flags ────────────────────────────────────────────────────────────────

/// Notehead centred on `(cx, cy)`. Longa and non-normal breve heads fall back to vector.
pub(crate) fn notehead(
    head: &NoteHead,
    duration: &Duration,
    filled: bool,
    cx: f32,
    cy: f32,
    space: f32,
) -> Option<String> {
    let long = matches!(duration, Duration::Breve | Duration::Longa);
    let (class, cp) = match head {
        NoteHead::Normal => (
            "acorde-notehead",
            match duration {
                Duration::Longa => return None,
                Duration::Breve => '\u{E0A0}', // noteheadDoubleWhole
                Duration::Whole => '\u{E0A2}', // noteheadWhole
                _ if filled => '\u{E0A4}',     // noteheadBlack
                _ => '\u{E0A3}',               // noteheadHalf
            },
        ),
        _ if long => return None,
        NoteHead::Diamond => (
            "acorde-notehead acorde-notehead-diamond",
            if filled { '\u{E0DB}' } else { '\u{E0D9}' }, // noteheadDiamondBlack / Half
        ),
        NoteHead::Triangle => (
            "acorde-notehead acorde-notehead-triangle",
            if filled { '\u{E0BE}' } else { '\u{E0BC}' }, // noteheadTriangleUpBlack / Half
        ),
        NoteHead::X => ("acorde-notehead acorde-notehead-x", '\u{E0A9}'), // noteheadXBlack
        NoteHead::Cross => ("acorde-notehead acorde-notehead-cross", '\u{E0AF}'), // noteheadPlusBlack
        NoteHead::Slash => ("acorde-notehead acorde-notehead-slash", '\u{E101}'), // noteheadSlashHorizontalEnds
    };
    glyph(class, cp, cx, cy, space, true)
}

/// One combined `flagNth{Up,Down}` glyph for `count` flags, origin at the stem end's left edge.
pub(crate) fn flags(stem_x: f32, tip_y: f32, space: f32, up: bool, count: usize) -> Option<String> {
    let index = u32::try_from(count.checked_sub(1)?)
        .ok()
        .filter(|i| *i < 5)?;
    let cp = char::from_u32(0xE240 + 2 * index + u32::from(!up))?; // flag8thUp … flag128thDown
    let x = stem_x - STEM_WIDTH_U * space / 2.0;
    glyph("acorde-flag", cp, x, tip_y, space, false)
}

// ── rests: centred on `cx`, `mid_y` is the middle line ──────────────────────────────

pub(crate) fn rest(duration: &Duration, cx: f32, mid_y: f32, space: f32) -> Option<String> {
    let (class, cp, y) = match duration {
        Duration::Breve => ("acorde-rest-breve", '\u{E4E2}', mid_y),
        Duration::Whole => ("acorde-rest-whole", '\u{E4E3}', mid_y - space), // hangs from line 4
        Duration::Half => ("acorde-rest-half", '\u{E4E4}', mid_y),
        Duration::Quarter => ("acorde-rest-quarter", '\u{E4E5}', mid_y),
        Duration::Eighth => ("acorde-rest-short", '\u{E4E6}', mid_y),
        Duration::Sixteenth => ("acorde-rest-short", '\u{E4E7}', mid_y),
        Duration::ThirtySecond => ("acorde-rest-short", '\u{E4E8}', mid_y),
        Duration::SixtyFourth => ("acorde-rest-short", '\u{E4E9}', mid_y),
        Duration::HundredTwentyEighth => ("acorde-rest-short", '\u{E4EA}', mid_y),
        Duration::Longa => return None,
    };
    glyph(class, cp, cx, y, space, true)
}

// ── accidentals: centred on `cx`, baseline on the pitch line ───────────────────────

/// The bare accidental glyph; the caller keeps its `acorde-accidental` wrapper group.
pub(crate) fn accidental(alter: i8, cx: f32, cy: f32, space: f32) -> Option<String> {
    let cp = match alter {
        -2 => '\u{E264}', // accidentalDoubleFlat
        -1 => '\u{E260}', // accidentalFlat
        0 => '\u{E261}',  // accidentalNatural
        1 => '\u{E262}',  // accidentalSharp
        2 => '\u{E263}',  // accidentalDoubleSharp
        _ => return None,
    };
    glyph("", cp, cx, cy, space, true)
}

// ── time signatures ─────────────────────────────────────────────────────────────────

/// `timeSigN` in the vector digit's 1.2 × 2 space box (top-left `ox, oy`); SMuFL time-signature
/// digits are centred on their baseline.
pub(crate) fn time_digit(d: u8, ox: f32, oy: f32, space: f32) -> Option<String> {
    let cp = char::from_u32(0xE080 + u32::from(d.min(9)))?;
    glyph(
        "acorde-digit",
        cp,
        ox + 0.6 * space,
        oy + space,
        space,
        true,
    )
}

/// `timeSigCommon` / `timeSigCutCommon`, centred on the middle line.
pub(crate) fn time_symbol(cut: bool, x: f32, middle_y: f32, space: f32) -> Option<String> {
    let cp = if cut { '\u{E08B}' } else { '\u{E08A}' };
    glyph("", cp, x, middle_y, space, false)
}

// ── dynamics ────────────────────────────────────────────────────────────────────────

/// Dynamic letters as one run of `dynamic*` glyphs centred on `cx`.
pub(crate) fn dynamic_letters(text: &str, cx: f32, baseline: f32, space: f32) -> Option<String> {
    let glyphs = text
        .chars()
        .map(|letter| match letter {
            'p' => Some('\u{E520}'),
            'm' => Some('\u{E521}'),
            'f' => Some('\u{E522}'),
            'r' => Some('\u{E523}'),
            's' => Some('\u{E524}'),
            'z' => Some('\u{E525}'),
            'n' => Some('\u{E526}'),
            _ => None,
        })
        .collect::<Option<Vec<_>>>()?;
    let class = "acorde-dynamic-letter";
    glyph_run(class, &glyphs, cx, baseline, 4.0 * space, true)
}

// ── articulations and ornaments ─────────────────────────────────────────────────────

/// Articulation centred on `(x, y)`; `dir < 0` means above the note. `centre` is the glyph's
/// vertical bounding-box centre above its baseline (Bravura metrics, staff spaces), so the glyph
/// sits on the vector anchor regardless of whether it hangs from or sits on its baseline.
pub(crate) fn articulation(
    articulation: &Articulation,
    x: f32,
    y: f32,
    dir: f32,
    space: f32,
) -> Option<String> {
    // (class, above glyph, below glyph, bbox centre); ornaments use one glyph either side.
    let (class, above, below, centre) = match articulation {
        Articulation::Staccato => ("acorde-staccato", '\u{E4A2}', '\u{E4A3}', 0.168), // articStaccatoAbove/Below
        Articulation::Accent => ("acorde-accent", '\u{E4A0}', '\u{E4A1}', 0.49), // articAccentAbove/Below
        Articulation::Tenuto => ("acorde-tenuto", '\u{E4A4}', '\u{E4A5}', 0.096), // articTenutoAbove/Below
        Articulation::Marcato => (
            "acorde-accent acorde-marcato",
            '\u{E4AC}',
            '\u{E4AD}',
            0.506,
        ), // articMarcatoAbove/Below
        Articulation::Fermata => ("acorde-fermata", '\u{E4C0}', '\u{E4C1}', 0.658), // fermataAbove/Below
        Articulation::Trill => ("acorde-trill", '\u{E566}', '\u{E566}', 0.76),      // ornamentTrill
        Articulation::Turn => ("acorde-ornament acorde-turn", '\u{E567}', '\u{E567}', 0.436), // ornamentTurn
        Articulation::InvertedTurn => (
            "acorde-ornament acorde-inverted-turn",
            '\u{E568}',
            '\u{E568}',
            0.436,
        ), // ornamentTurnInverted
        // Acorde's `Mordent` is the lower mordent (with the vertical stroke).
        Articulation::Mordent => (
            "acorde-ornament acorde-mordent",
            '\u{E56D}',
            '\u{E56D}',
            0.492,
        ), // ornamentMordent
        Articulation::InvertedMordent => (
            "acorde-ornament acorde-inverted-mordent",
            '\u{E56C}',
            '\u{E56C}',
            0.49,
        ), // ornamentShortTrill
        _ => return None,
    };
    // An "above" glyph sits on its baseline; a "below" glyph hangs from it.
    let (cp, baseline) = if dir < 0.0 || above == below {
        (above, y + centre * space)
    } else {
        (below, y - centre * space)
    };
    let class = format!("acorde-articulation {class}");
    glyph(&class, cp, x, baseline, space, true)
}
