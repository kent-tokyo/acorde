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
