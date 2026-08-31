# acorde-render-svg

Pure-Rust/WASM SVG renderer driven by acorde-core and acorde-layout.

The crate provides render_svg, render_svg_with_layout, render_svg_row, render_svg_metadata, and
render_svg_with_annotations. Host-defined `RenderAnnotation` providers can add safe text marks
using viewport coordinates; provider and mark IDs are validated and serialized deterministically.
Output is deterministic and may include stable data-note-addr hooks for host-side selection and
hit testing. The renderer has no browser/DOM dependency and uses font-independent SVG geometry
for notation glyphs.

~~~rust
use acorde_core::Score;
use acorde_render_svg::{render_svg, SvgRenderOptions};

let svg = render_svg(&Score::default(), &SvgRenderOptions::default())?;
~~~

Unsupported clefs, accidentals, rows, layouts, and invalid options return RenderError rather than
being silently dropped. See the [browser contract](https://github.com/kent-tokyo/acorde/blob/main/docs/browser-rendering.md).

[API documentation](https://docs.rs/acorde-render-svg) · [Repository](https://github.com/kent-tokyo/acorde)

SVG output and annotation trust-boundary rules are documented in the [security contract](../../docs/security/threat-model.md).
