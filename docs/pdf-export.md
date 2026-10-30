# PDF export

`acorde-pdf` is Acorde's optional, in-memory vector-PDF adapter. It converts one deterministic SVG
viewport from `acorde-render-svg` through [Harumi](https://github.com/kent-tokyo/harumi); it does
not change the score model, access the filesystem, or access printer APIs.

## Current contract

- `export_score_pdf` accepts an `acorde_core::Score` plus `SvgRenderOptions`.
  `export_svg_pdf` accepts only a bounded, self-contained Acorde SVG viewport; it is not a generic
  SVG sanitizer or renderer.
- `export_score_pdf` and `export_svg_pdf` return one vector PDF page. Passing `page_size_pt`
  scales that SVG viewport onto the selected media box.
- `export_svg_pages_pdf` joins 1–256 independently sized page SVGs into one in-memory PDF. It
  retains page order and each page's physical media box, and rejects invalid sizes before any
  conversion. Callers can derive point sizes from `PageArtifact` millimetres with `POINTS_PER_MM`.
- `export_print_layout_pdf` validates a `PrintLayoutResult`, requires one SVG per layout page, and
  uses its page dimensions for the assembled PDF without recomputing pagination.
- `acorde-render-svg::render_print_page_svgs` consumes the score and validated layout, retaining
  its system breaks and physical page positions. `export_score_print_pdf` calls that renderer and
  assembles the resulting pages into vector PDF.
- Callers provide an embeddable TrueType/OpenType font for lyric, text, and label SVG nodes.
  Acorde's notation geometry is retained as vector paths; no bitmap score is inserted.
- SVG is limited to 16 MiB. Harumi receives it unchanged, so Bézier paths, elliptical arcs, and
  ellipses remain native vector geometry rather than being approximated into polylines.

## Deliberate limits

This is not a claim of publication-ready engraving or parity with Verovio. The page SVG path
rejects host glyphs, images, frames, color preservation, and crop marks. It relies on the caller's
text font for PDF and has no OS print dialog. The OpenScore *Aloha Oe* A4 comparison in
[`verovio-engraving-baseline.md`](verovio-engraving-baseline.md) still shows small lyrics and
missing grand-staff braces in Acorde's output. `acorde-layout::compute_print_layout` remains the
source of truth for system and page planning.

## Compatibility

`acorde-pdf` and the Acorde workspace require Rust 1.88 because Harumi does. Core Acorde crates
still do not depend on Harumi. Harumi 1.23.7 is the minimum supported backend and includes native
SVG curve, arc, ellipse, metadata, and CSS-color handling used by Acorde's renderer.
