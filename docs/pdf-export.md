# PDF export

`acorde-pdf` is Acorde's optional, in-memory vector-PDF adapter. It converts one deterministic SVG
viewport from `acorde-render-svg` through [Harumi](https://github.com/kent-tokyo/harumi); it does
not change the score model, access the filesystem, or access printer APIs.

## Current contract

- `export_score_pdf` accepts an `acorde_core::Score` plus `SvgRenderOptions`.
  `export_svg_pdf` accepts only a bounded, self-contained Acorde SVG viewport; it is not a generic
  SVG sanitizer or renderer.
- Output is one `Vec<u8>` containing one vector PDF page. Passing `page_size_pt` scales the SVG
  viewport onto that media box.
- Callers provide an embeddable TrueType/OpenType font for lyric, text, and label SVG nodes.
  Acorde's notation geometry is retained as vector paths; no bitmap score is inserted.
- SVG is limited to 16 MiB. Harumi receives it unchanged, so Bézier paths, elliptical arcs, and
  ellipses remain native vector geometry rather than being approximated into polylines.

## Deliberate limits

This first slice is not a claim of publication-ready multipage engraving. It does not map
`PrintLayoutResult` pages and systems into independently scaled SVG viewports, resolve
publication font resources, embed music fonts, or integrate an OS print dialog. Those remain
the next `acorde-pdf` phases. The existing `acorde-layout::compute_print_layout` contract stays
the source of truth for page planning.

## Compatibility

`acorde-pdf` and the Acorde workspace require Rust 1.88 because Harumi does. Core Acorde crates
still do not depend on Harumi. Harumi 1.23.7 is the minimum supported backend and includes native
SVG curve, arc, ellipse, metadata, and CSS-color handling used by Acorde's renderer.
