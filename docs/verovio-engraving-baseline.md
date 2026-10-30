# Verovio 6.3.0 engraving baseline

This is a reproducible starting point for the goal of exceeding Verovio on
layout, engraving, and print for ordinary piano and chamber scores. Page count
alone is not an engraving-quality score.

## Fixed input and setup

- Date: 2026-10-30.
- Input: `tests/fixtures/openscore_lieder_aloha_oe.mxl`, CC0-1.0, SHA-256
  `b195ae477a10cbaee281d9b728cac001ae1d50fe4527078ccae14092eee4936c`
  (see `tests/fixtures/manifest.json`).
- A4 portrait, 210 × 297 mm; top/bottom margins 16 mm, left/right 14 mm.
- Verovio: Homebrew CLI 6.3.0, default automatic breaks and bundled fonts.
- Acorde: `v1.3.3` plus the staff-aware height and physical page SVG changes on
  this development branch; default four measures per system, built-in vector notation,
  and the public-domain Tuffy test font for PDF text. Acorde's default now keeps the
  partial pickup with following measures.

```bash
verovio -a --page-width 2100 --page-height 2970 \
  --page-margin-top 160 --page-margin-bottom 160 \
  --page-margin-left 140 --page-margin-right 140 \
  -o /tmp/verovio-aloha-a4.svg tests/fixtures/openscore_lieder_aloha_oe.mxl

acorde print-report tests/fixtures/openscore_lieder_aloha_oe.mxl \
  --preset a4-score --measures-per-system 4

cargo run -p acorde-pdf --example print_mxl -- \
  tests/fixtures/openscore_lieder_aloha_oe.mxl \
  crates/pdf/tests/fixtures/Tuffy.ttf /tmp/acorde-aloha-print.pdf
pdftoppm -scale-to 1400 -png /tmp/acorde-aloha-print.pdf /tmp/acorde-aloha-page
```

| Result | Verovio 6.3.0 | Acorde v1.3.3 | Acorde with staff-aware height | Acorde with page SVG and pickup fix |
| --- | ---: | ---: | ---: | ---: |
| A4 pages | 3 rendered SVG pages | 1 logical page | 3 logical pages | 2 SVG/PDF pages |
| Systems | 6 | 7 | 7 | 6 |
| Reserved system height | renderer-managed | 24 mm | 88 mm (five staves) | 88 mm (five staves) |
| MusicXML import losses | not assessed | 73 | 73 | 73 |

The Acorde import diagnostics are mostly unsupported MusicXML text styling;
they remain unfixed. The PDF has a valid A4 media box on both pages. In the rendered
images, the page title and four part labels are visible, and the pickup is no longer
stretched into a standalone full-width system. Verovio uses three pages with visibly
larger lyrics. The follow-up staff-group change on this branch adds the missing
brace across the piano's two staves; the updated A4 PDF was rendered at 1400 pixels
and inspected. The follow-up print-row change on this branch also leaves a
proportionate final system without scaling glyphs. Acorde's lyrics are still very
small at that preview size.
The two-page Acorde result is therefore not evidence of superior print quality.
Verovio chooses its own system breaks and fonts; this is a same-input, same-paper
comparison rather than an identical-layout comparison.

## Chamber-score check: Mozart K.464

The second CC0 input is the OpenScore String Quartets MSCX pinned with its source
commit, Git blob, and SHA-256 in [#185](https://github.com/kent-tokyo/acorde/issues/185).
On 2026-10-31, Acorde imported its four main parts and exported MusicXML; the
export report found no additional loss. The MSCX import separately reported four
omitted linked excerpts and three hidden hairpins whose visibility is not preserved.
Both renderers then consumed that **same Acorde-exported MusicXML**. This comparison
tests engraving after a common interchange step, not either product's native MSCX
import. Both used A4 with 16 mm top/bottom and 14 mm left/right margins.

```bash
acorde convert /path/to/sq7070781.mscx /tmp/k464.musicxml
verovio -a --page-width 2100 --page-height 2970 \
  --page-margin-top 160 --page-margin-bottom 160 \
  --page-margin-left 140 --page-margin-right 140 \
  -o /tmp/verovio-k464.svg /tmp/k464.musicxml
cargo run -p acorde-pdf --example print_mxl -- \
  /tmp/k464.musicxml crates/pdf/tests/fixtures/Tuffy.ttf /tmp/acorde-k464.pdf
```

| Result | Acorde with page SVG/PDF | Verovio 6.3.0 |
| --- | ---: | ---: |
| A4 pages | 11 PDF pages | 7 SVG pages |
| Systems | 29 | 20 |
| Main parts | 4 | 4 |

The first and last pages were rasterized at the same 990 × 1400-pixel page size
and inspected. Acorde's fixed four-measure rows leave substantially more white
space, especially on the final page; Verovio fits more measures without making
the notes illegible in those previews. Verovio reported one highly compressed
system, so page count alone is not an endorsement of every Verovio break. Acorde
now leaves a ragged final system, but still needs density-aware print breaking
and a broader visual collision review before this quartet can support a quality claim.

## Gates before a quality claim

1. Keep page SVG/PDF generation and cross-page span continuation covered by tests.
2. Improve lyric size and publication
   typography against the visual baseline.
3. Pin a larger licensed piano/chamber corpus and compare missing symbols,
   clipped content, collisions, page geometry, and legibility at print size.
4. Run blind human review of the resulting PDFs and record the rubric and
   individual scores. Do not infer superiority from page count or preflight.
