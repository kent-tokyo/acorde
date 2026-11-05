# acorde-render-svg

Pure Rust/WASM SVG rendering driven by `acorde-core` and `acorde-layout`. The renderer is
deterministic, has no browser or DOM dependency, and never loads fonts. It exposes full-score,
layout-aware, per-row, physical print-page, metadata, preflight, and annotation rendering APIs.

Notation is emitted as font-independent geometry. Text, lyrics, chord symbols, and host annotation
marks remain SVG text, so callers own font choice, font metrics, embedding, and publication
typography. The renderer validates non-finite geometry, invalid XML characters, unsupported glyph
requests, malformed TAB positions, and bounded annotation/text inputs before emitting SVG.

```rust
use acorde_core::Score;
use acorde_render_svg::{render_svg, SvgRenderOptions};

let svg = render_svg(&Score::default(), &SvgRenderOptions::default())?;
# let _ = svg;
# Ok::<(), acorde_render_svg::RenderError>(())
```

## SMuFL music fonts (opt-in)

`SvgRenderConfig::new(options).with_smufl_font("Bravura")` draws music symbols as SMuFL code
points in `<text font-family="Bravura" font-size="…">` elements instead of vector paths. No font
is bundled: the host loads the family (for example with CSS `@font-face`). Without the setting the
output is byte-identical to the vector renderer.

- Mapped: G/F/C/percussion clefs; black, half, whole and breve noteheads plus X, plus, diamond,
  triangle and slash heads; whole to 128th and breve rests; flats, naturals, sharps, double sharps
  and double flats; combined 8th–128th flags; time-signature digits and common/cut time; dynamic
  letters; staccato, accent, tenuto, marcato, fermata; trill, mordent, inverted mordent, turn and
  inverted turn.
- Everything else stays vector: stems, beams, barlines, ledger lines, slurs, ties, the longa,
  longa rests, microtone markers, tuplet numbers and the remaining articulations.
- Each glyph sits on its SMuFL origin: baseline on the staff line or pitch, one em equal to four
  staff spaces. Articulations and ornaments are centred on the vector anchor using Bravura's
  bounding boxes, so other fonts may sit slightly higher or lower.
- Layout is unchanged: spacing still uses the vector glyph widths, so a wider or narrower font
  glyph can crowd or leave extra space next to its neighbour.
- `glyph_coverage_for(&SvgGlyphSource::Smufl { .. })` reports the resource id
  `smufl:<family>`. `render_print_page_svgs_with_glyphs` accepts the source for print pages. A page
  may then declare `GlyphResourcePolicy::HostProvided("smufl:<family>")`.
- PDF export does not support SMuFL text yet. `acorde-pdf` rejects such SVG with
  `PdfError::MusicFontText` instead of printing missing glyphs (see `docs/pdf-export.md`).

For source files with unspecified eighth-note beams, use
`render_svg_with_config(&score, &SvgRenderConfig::new(options).with_auto_beams())`.
This groups short notes by the active meter for display only. Authored beams and explicit
unbeamed marks are preserved; the score and interchange exports are unchanged.

## Host integration

Interactive output includes stable note-address hooks. `render_svg_metadata` supplies the matching
typed note, text, TAB, and technique data; call `svg_contract_version()` before consuming fields
added by later releases. `render_preflight()` reports source-located capability issues before a
host mounts SVG. Rows can be rendered independently for virtualized views.
`render_print_page_svgs()` accepts a validated `PrintLayoutResult` and emits one millimetre-sized
SVG per page using its system breaks and positions. Unsupported host resources return an error.
The matching vector PDF path is in `acorde-pdf`.

`RenderAnnotation` permits bounded, deterministic host text marks. Hosts supply their own
viewport geometry and can opt into the renderer's priority-based collision pass; the renderer does
not infer text metrics. Missing authored TAB positions render as an explicit marker rather than a
guessed string/fret.

Ties never cross a rest, microtones retain exact cents as explicit markers, and unpitched notes
retain display placement without inventing a percussion sound identity. These are bounded semantic
projections, not claims of font-aware publication engraving.

See the [browser contract](../../docs/browser-rendering.md) for the WASM boundary and the
[security contract](../../docs/security/threat-model.md) for SVG and annotation input rules.

[API documentation](https://docs.rs/acorde-render-svg) · [Repository](https://github.com/kent-tokyo/acorde)
