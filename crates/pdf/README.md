# acorde-pdf

`acorde-pdf` exports Acorde's deterministic SVG engraving as a vector PDF through
[Harumi](https://crates.io/crates/harumi). It is a separate, optional crate: score modelling,
I/O, logical layout, and SVG rendering do not depend on a PDF backend.

The crate accepts a caller-supplied TrueType or OpenType font for SVG text. Acorde's own
notation glyphs remain vector paths; no score bitmap is created during conversion.
`export_score_pdf` produces one PDF page from a rendered SVG viewport.
`export_svg_pages_pdf` joins up to 256 caller-supplied page SVGs into one vector PDF, preserving
their order and independent physical media boxes. The caller still needs to render each
`PrintLayoutResult` page as SVG. `export_print_layout_pdf` validates that layout and uses its
page count and physical dimensions when joining those SVGs. Publication typography and printer
integration remain follow-up work.

With Harumi 1.23.7 or newer, Acorde passes its SVG directly to the PDF backend: engraving
Bézier curves, elliptical arcs, and notehead ellipses remain native vector geometry rather than
being approximated as polygons.

```no_run
use acorde_core::Score;
use acorde_pdf::{PdfRenderOptions, export_score_pdf};

# fn main() -> Result<(), Box<dyn std::error::Error>> {
let font = std::fs::read("/path/to/text-font.ttf")?;
let options = PdfRenderOptions::new(&font);
let pdf = export_score_pdf(&Score::default(), &options)?;
std::fs::write("score.pdf", pdf)?;
# Ok(())
# }
```

`acorde-pdf` needs Rust 1.88 or newer because of its Harumi dependency. This is also the Acorde
workspace MSRV.
