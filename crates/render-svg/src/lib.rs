//! Pure-Rust/WASM SVG score renderer for [`acorde_core::Score`], driven by
//! [`acorde_layout`](https://docs.rs/acorde-layout).
//!
//! ```text
//! Score model (acorde-core) → logical layout (acorde-layout) → renderer (acorde-render-svg)
//! ```
//!
//! This crate has no browser/DOM dependency — [`render_svg`] produces a plain `String` that
//! renders identically whether you're on native Rust, in a WASM host, or on a server. It
//! consumes layout decisions ([`LayoutResult`]) rather than re-deriving them: beam grouping,
//! tuplet grouping, courtesy accidentals, and row breaks all stay in `acorde-layout`. This
//! crate places glyphs at pixel coordinates and nothing more.
//!
//! Glyphs (clefs, noteheads, accidentals, rests) are original hand-authored SVG paths — no
//! vendored font, no system-font dependency. See the crate README for the rationale.
//!
//! # Example
//!
//! ```
//! use acorde_core::Score;
//! use acorde_render_svg::{render_svg, SvgRenderOptions};
//!
//! let score = Score::default();
//! let svg = render_svg(&score, &SvgRenderOptions::default()).unwrap();
//! assert!(svg.starts_with("<svg"));
//! ```

mod beams;
mod geometry;
mod glyphs;
mod render;

use acorde_core::Score;
use acorde_layout::{compute_layout, LayoutConfig, LayoutResult};
use serde::{Deserialize, Serialize};

/// Options controlling SVG output. All fields have defaults — safe to deserialize from
/// partial JSON (e.g. `"{}"` from a WASM caller that only wants defaults).
#[derive(Debug, Clone, Serialize, Deserialize)]
#[serde(default)]
pub struct SvgRenderOptions {
    /// Total SVG width in pixels. Content is distributed across this width per row.
    pub width: f32,
    /// Distance between two adjacent staff lines, in pixels. Drives every other glyph
    /// dimension (noteheads, stems, clefs, accidentals are all proportional to this).
    pub staff_size: f32,
    /// How many measures to place per system/row.
    pub measures_per_system: usize,
    /// When `true`, emit stable `data-*` hooks (`data-acorde-kind`, `data-part`, `data-staff`,
    /// `data-measure`, `data-voice`, `data-note`, `data-note-addr`) for click-to-position
    /// interaction. When `false`, those attributes are omitted.
    pub interactive: bool,
}

impl Default for SvgRenderOptions {
    fn default() -> Self {
        Self { width: 900.0, staff_size: 24.0, measures_per_system: 4, interactive: true }
    }
}

/// Errors returned by [`render_svg`] / [`render_svg_with_layout`].
///
/// These are all system-boundary validation failures — an arbitrary [`Score`] can reference
/// notation this renderer does not (yet) support. Never fails silently: an unsupported clef or
/// an accidental beyond double-sharp/double-flat is reported, not dropped or approximated.
#[derive(Debug, Clone, PartialEq, Eq)]
pub enum RenderError {
    /// The score has no staves to render (no parts, or no parts with staves).
    EmptyScore,
    /// A staff uses a clef this renderer has no staff-position mapping for (percussion).
    UnsupportedClef,
    /// A pitch's `alter` is outside the supported range (`-2..=2`: double-flat..double-sharp).
    UnsupportedAccidental { alter: i8 },
}

impl std::fmt::Display for RenderError {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        match self {
            RenderError::EmptyScore => write!(f, "score has no staves to render"),
            RenderError::UnsupportedClef => write!(f, "unsupported clef (percussion has no staff-position mapping)"),
            RenderError::UnsupportedAccidental { alter } => {
                write!(f, "unsupported accidental alter={alter} (supported range is -2..=2)")
            }
        }
    }
}

impl std::error::Error for RenderError {}

/// Compute layout with `options` and render `score` to an SVG string.
///
/// Equivalent to calling [`acorde_layout::compute_layout`] with a [`LayoutConfig`] built from
/// `options.measures_per_system`, then [`render_svg_with_layout`]. Use `render_svg_with_layout`
/// directly when you already have a [`LayoutResult`] (e.g. computed once and reused, or built
/// with non-default `LayoutConfig` fields such as `concert_pitch`).
pub fn render_svg(score: &Score, options: &SvgRenderOptions) -> Result<String, RenderError> {
    let config = LayoutConfig {
        measures_per_row: options.measures_per_system.max(1),
        ..Default::default()
    };
    let layout = compute_layout(score, &config);
    render_svg_with_layout(score, &layout, options)
}

/// Render `score` to an SVG string using an already-computed [`LayoutResult`].
///
/// `layout` must have been computed from `score` (or a structurally identical score) —
/// mismatched inputs produce undefined visual output, not a panic.
pub fn render_svg_with_layout(
    score: &Score,
    layout: &LayoutResult,
    options: &SvgRenderOptions,
) -> Result<String, RenderError> {
    render::build_svg(score, layout, options)
}
