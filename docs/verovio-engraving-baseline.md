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

### Page-SVG WASM API and lyric-size check (2026-10-31)

The `render_print_page_svgs(score_json, layout_json)` WASM export now returns the
same physical pages as Rust's `render_print_page_svgs`. A browser test checks the
supplied A4 page dimensions, page number, distinct page bodies, and invalid layout
rejection. The Aloha Oe PDF was rebuilt and both pages were rasterized at 1400 px.
The voice lyric font grew from 0.72 to 1.4 staff spaces; on this fixture its first
SVG lyric is 1.97 mm high. The wider lyric collision estimate and verse spacing
were changed together. The lyrics are more legible, and visual inspection found
no clipped staves or missing page numbers on either page. Verovio still has
larger, clearer publication typography and 73 Acorde import diagnostics remain.

```bash
cargo run -q -p acorde-pdf --example print_mxl -- \
  tests/fixtures/openscore_lieder_aloha_oe.mxl \
  crates/pdf/tests/fixtures/Tuffy.ttf /tmp/acorde-aloha-lyric-review.pdf
pdftoppm -f 1 -l 2 -scale-to 1400 -png \
  /tmp/acorde-aloha-lyric-review.pdf /tmp/acorde-aloha-lyric-review
```

### Second piano score: Quilter, Prelude (2026-10-31)

The pinned CC0 `tests/fixtures/openscore_lieder_quilter_prelude.mxl` was rendered
with the same A4 paper and 16/14 mm margins using Verovio 6.3.0 and Acorde.
Verovio produced one SVG page with three systems; Acorde produced one PDF page
with three systems and a correct 595.276 × 841.89 pt media box. Acorde reported
103 MusicXML import diagnostics; Verovio separately warned of two open hairpins.
These diagnostics are not engraving measurements.

The Acorde PDF and Verovio SVG were rasterized at 1400 px and inspected. Acorde
uses fixed groups of four, four, and two measures, leaving the lower half of the
page mostly empty. Verovio uses groups of three, four, and three, with larger
notation and readable directions. Acorde's first system has cramped note and
direction text, though its staves and page number are inside the page box. A
trial that enlarged staff space without changing measure breaks created severe
overlaps and was discarded. This score does not meet a publication-quality
comparison against Verovio yet.

The subsequent opt-in density change reserves 15% of its budget on the first
system for clef, key, and meter. Quilter now breaks 3+4+3, matching
Verovio's measure groups, and the first row is less cramped. The A4 PDF was
rasterized again at 1400 px. Its staff and direction text remain much smaller
than Verovio's and the lower page remains mostly unused; this break change is
not a publication-quality result by itself. A trial with a 70 mm system height
and larger direction text also left floating text and collisions, so neither
change was adopted.

```bash
verovio -a --page-width 2100 --page-height 2970 \
  --page-margin-top 160 --page-margin-bottom 160 \
  --page-margin-left 140 --page-margin-right 140 \
  -o /tmp/verovio-quilter.svg tests/fixtures/openscore_lieder_quilter_prelude.mxl
cargo run -q -p acorde-pdf --example print_mxl -- \
  tests/fixtures/openscore_lieder_quilter_prelude.mxl \
  crates/pdf/tests/fixtures/Tuffy.ttf /tmp/acorde-quilter.pdf
pdftoppm -f 1 -l 1 -scale-to 1400 -png \
  /tmp/acorde-quilter.pdf /tmp/acorde-quilter
qlmanage -t -s 1400 -o /tmp /tmp/verovio-quilter.svg

# Inspect the opt-in first-system density adjustment with the same paper size.
cargo run -q -p acorde-pdf --example print_mxl -- \
  tests/fixtures/openscore_lieder_quilter_prelude.mxl \
  crates/pdf/tests/fixtures/Tuffy.ttf /tmp/acorde-quilter-density.pdf 4 density
```

### Density-aware print break experiment (2026-10-31)

The opt-in `PrintSystemBreakStrategy::NotationDensity` uses the shared SVG/layout
notation-demand estimate. Its budget is the 90th percentile of this score's
four-measure rows, scaled by available page width. `measures_per_system` is a
maximum rather than a fixed row length. Authored page and system breaks remain
in effect. The metric estimates noteheads, accidentals, lyrics, chord symbols,
articulations, and measure text; it does not measure final glyph collisions.

```bash
cargo run -p acorde-pdf --example print_mxl -- \
  tests/fixtures/openscore_lieder_aloha_oe.mxl \
  crates/pdf/tests/fixtures/Tuffy.ttf /tmp/acorde-aloha-density.pdf 7 density
```

This produces **2 A4 pages and 5 systems**, compared with 2 pages and 6
systems for fixed four-measure breaks and Verovio's 3 pages and 6 systems.
The second page was rasterized at 1400 pixels and inspected. Lyrics remain
too small at this size, so the more compact result is not evidence of better
engraving.

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
now leaves a ragged final system. The fixed-break result still needs a broader
visual collision review before this quartet can support a quality claim.

The opt-in density mode with a seven-measure maximum produces **9 pages and 25
systems** from the same MusicXML, improving on the fixed four-measure result's
11 pages and 29 systems. A fixed six-measure setting produces 8 pages but visibly
cramps the final dense measures. Density mode leaves measures 98–100 together
on a full-width system and the final three measures on page 9. Pages 1 and 8
were rasterized at 1400 pixels and inspected; they do not establish a
corpus-wide collision or legibility result. Verovio remains more compact at
7 pages and 20 systems on this input.

K.464 was also rerun after the first-system density change from the same
exported MusicXML with a seven-measure maximum; it remains at 9 A4 pages and
25 systems, with zero reimport diagnostics.

```bash
cargo run -p acorde-pdf --example print_mxl -- \
  /tmp/k464.musicxml crates/pdf/tests/fixtures/Tuffy.ttf \
  /tmp/acorde-k464-density.pdf 7 density
```

## Gates before a quality claim

1. Keep page SVG/PDF generation and cross-page span continuation covered by tests.
2. Improve lyric size and publication
   typography against the visual baseline.
3. Pin a larger licensed piano/chamber corpus and compare missing symbols,
   clipped content, collisions, page geometry, and legibility at print size.
4. Run blind human review of the resulting PDFs and record the rubric and
   individual scores. Do not infer superiority from page count or preflight.
