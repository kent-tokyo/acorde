# acorde-render-svg

Pure-Rust/WASM SVG renderer driven by acorde-core and acorde-layout.

The crate provides render_svg, render_svg_with_layout, render_svg_row, render_svg_metadata, and
render_svg_with_annotations. Host-defined `RenderAnnotation` providers can add safe text marks
using viewport coordinates; provider and mark IDs are validated and serialized deterministically.
The renderer accepts at most 10,000 marks and 16 KiB of UTF-8 text per mark; excess input returns
a typed error before SVG emission.
Output is deterministic and may include stable data-note-addr hooks for host-side selection and
hit testing. `glyph_coverage()` reports the built-in resource ID, supported clefs, and accidental
range before rendering; unsupported clefs/accidentals return `RenderError` rather than a blank
fallback. The renderer has no browser/DOM dependency and uses font-independent SVG geometry for
notation glyphs. Tablature staffs render their configured line count, explicit string/fret
positions, and guitar bend/slide/hammer-on/pull-off technique labels; missing positions are shown
as an explicit `?` marker rather than inferred silently.

~~~rust
use acorde_core::Score;
use acorde_render_svg::{render_svg, SvgRenderOptions};

let svg = render_svg(&Score::default(), &SvgRenderOptions::default())?;
~~~

Unsupported clefs, accidentals, rows, layouts, and invalid options return RenderError rather than
being silently dropped. See the [browser contract](https://github.com/kent-tokyo/acorde/blob/main/docs/browser-rendering.md).

[API documentation](https://docs.rs/acorde-render-svg) · [Repository](https://github.com/kent-tokyo/acorde)

SVG output and annotation trust-boundary rules are documented in the [security contract](../../docs/security/threat-model.md).
