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

Before the two-verse clearance change below, this produced **2 A4 pages and 5
systems**, compared with 2 pages and 6 systems for fixed four-measure breaks
and Verovio's 3 pages and 6 systems.
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

### Print measure-width check (2026-10-31)

Physical page SVGs now allocate each row's width using both rhythmic duration
(40%) and the shared notation-demand estimate (60%). The screen SVG renderer's
rhythmic widths are unchanged. On Quilter's pinned first system, the second and
third measures previously each received about 213 SVG units; they now receive
about 176 and 219 units. The denser third measure gets 24% more width than the
second. A fixture regression asserts this relationship rather than exact
coordinates. The A4 page still has three systems and the same 3+4+3 breaks.

The new Quilter PDF was rasterized at 1400 px. Its opening has more room for
the busy bars, but text remains small and the page's lower area is largely
unused. At this stage the CC0 Aloha Oe density PDF was still two A4 pages /
five systems with 73 import diagnostics. The same-input K.464 quartet remains
nine pages / 25 systems with zero reimport diagnostics. No broad collision-free
or visual quality claim follows from these unchanged page counts. Increasing the
staff-aware system height in a separate trial enlarged the notation but left
visible lyric/staff collisions in Aloha Oe, so that trial was reverted.

### Quilter publication credits (2026-10-31)

The pinned Quilter MusicXML contains a work title, a separate movement title,
composer, and CC0 rights. Acorde previously imported only the work title. The
MusicXML importer now retains the other fields, and export preserves them on
roundtrip. The physical first-page SVG prints `Prelude` and `Roger Quilter`
above the first system; the rights remain in score metadata. The A4 PDF was
rasterized at 1400 px to check that the new text stays inside the page and does
not overlap the first system. The system grouping and one-page count are
unchanged. Verovio's publication type is still visibly larger and clearer, and
the 103 Acorde import diagnostics remain; restoring these credits is a semantic
and completeness fix, not a claim of print-quality parity.

### Two-verse vocal staff clearance (2026-10-31)

For scores with an additional lyric verse, the staff-aware print layout now
reserves 16 mm more per system and the physical SVG uses a nine-space gap
between staves instead of six. On the CC0 Aloha Oe fixture, the fixed mode is
three A4 pages / six systems and density mode is three pages / five systems.
Verovio 6.3.0 is three pages / six systems on the same source and paper. The
first page's staff space stays about 1.4 mm, while the gap from the first vocal
staff to the next rises from about 8.4 to 12.5 mm. A regression asserts at least
4 mm from a second-verse baseline to the following staff's top line.

All three density-mode PDF pages were rasterized at 1400 px and inspected. The
two lyric lines no longer run into the following staff in those previews, and
the page number and staves remain inside A4. The third page has a large unused
lower area; some dense syllables and notation are still small. The 73 Acorde
import diagnostics remain separate from this layout change. This is a
fixture-level clearance improvement, not a broad visual-quality result.

### Two-staff piano print size and density breaks (2026-10-31)

Staff-aware layouts for two staves now reserve at least 50 mm per system,
up from 40 mm. The notation-density row budget scales inversely with this
physical height, so larger notes are not packed into the old row lengths.
The pinned CC0 Quilter Prelude is still one A4 page, but its ten measures
now form four systems (2+3+3+2) rather than three (3+4+3). The first
staff's line spacing in the generated SVG rises from about 1.20 to
1.50 mm. The denser final measure on the second system receives over 10%
more width than its preceding measure.

The resulting PDF was rasterized at 1400 px and visually inspected beside
Verovio 6.3.0 on the same source. The larger staff and shorter rows improve
the Quilter preview, while the direction text is still tiny and the lower
part of the A4 page is largely empty. Individual chord and accidental
collisions have not been ruled out, and the import still reports 103
diagnostics. This change does not establish print-quality parity.

### Print text and grand-staff brace (2026-10-31)

The physical-page SVG now prints measure directions at 1.7 times the screen
SVG size, with a minimum above-staff lane for directions whose imported
placement hint would otherwise pull enlarged text into the notation. The
screen renderer keeps its established text size. On Quilter, the first tempo
direction is about 2.0 mm high and its baseline is about 6.0 mm above the
top staff line; the staff space remains about 1.5 mm. Publication title,
movement title, and composer sizes increase to 5.0, 3.8, and 3.0 mm. The
inferred piano brace uses a wider, more visible vector curve.

The Quilter density PDF remains one A4 page / four systems. All three Aloha
Oe density PDF pages were also rasterized at 1400 px and inspected; the page
count remains three pages / five systems. These fixture previews show clearer
directions and credits without a new visible overlap from the change. The
Quilter page still has unused lower space, dense chords need a measured
collision review, and neither score proves Verovio-level typography or
publication quality. Import diagnostics remain 103 and 73 respectively.

### Print direction legibility check after v1.3.5 (2026-10-31)

The physical-page renderer now uses a 3.0 measure-text scale in place of 1.7;
the regular screen renderer is unchanged. On the pinned Quilter Prelude, the
first tempo direction grows from about 2.0 to 3.5 mm. A page-SVG regression
requires at least 3.4 mm font size and 4 mm clearance from its baseline and
descender allowance to the top staff line. The five-staff Aloha Oe fixture has
a corresponding 3.1 mm and 4 mm regression. Its two-verse staff clearance
remains covered separately.

The Quilter (one A4 page, four systems), Aloha Oe (three pages, five density
systems), and Webern *So ich traurig bin* (two pages, five systems) PDFs were
rasterized at 1400 px and inspected. The larger directions are readable in
these previews without a new visible overlap, clipping, or changed system
break. Their MusicXML import diagnostics remain 103, 73, and 20 respectively.
The same-input Verovio 6.3.0 Quilter preview still has larger notation and
better page use. This focused text improvement does not establish superior
layout, engraving, or print quality.

### Verovio-guided title placement (2026-10-31)

The pinned Quilter MXL was rendered again with Verovio 6.3.0 and Acorde on
the same A4 paper and margins. Verovio uses three systems; Acorde's density
mode uses four. At 1400 px, Acorde's title and composer were close to the top
edge and visibly smaller. The print SVG now places the title 2 mm lower,
the movement title and composer 2.5 mm lower, and slightly enlarges those
three credits. A fixture regression checks that the title stays on the page,
the title and movement title have a gap, and the credits stay above the first
system. The updated one-page PDF was rasterized and inspected; its four
system breaks, staff size, and 103 MusicXML import diagnostics are unchanged.
The three-page Aloha Oe density PDF was regenerated; its first page was also
rasterized and inspected for title/composer overlap. Its 73 import diagnostics
and page/system counts are unchanged.

Two other trials were rejected after raster review. Filling the page's unused
vertical space only produced exaggerated gaps between small systems. Loosening
the density budget to get three systems made the first and last rows crowded.
Verovio remains more readable on this input: its notes and directions are
larger, and its layout uses three balanced systems. The new title placement
addresses only one print typography difference.

### Verovio-guided direction completeness and clearance (2026-10-31)

The same pinned Quilter MXL has two `L.H.` directions and `poco cresc.` in its
fourth measure. Verovio 6.3.0 prints all three. Acorde's importer previously
kept only the first words direction per measure; the later marks were absent
from its PDF and from a MusicXML roundtrip. The importer and exporter now retain
all three. The page SVG reserves an eight-space grand-staff gap and 56 mm system
height for a piano score with below-staff words. It also moves the words below
nearby noteheads when the source engraving's vertical hint would pull them into
the notation. A fixture regression checks both `L.H.` marks and clearance for
`poco cresc.` between the two staves.

The Acorde density PDF remains one A4 page, now with four systems grouped
2+2+3+3; Verovio uses one page with three systems. The Acorde PDF and the
same-input Verovio SVG were inspected at 1400 px. Acorde's directions are
present, but the two `L.H.` marks stack at one horizontal position because
measure text does not yet retain its beat position. Its notation and publication
type remain smaller than Verovio's. The MusicXML import still reports 103
diagnostics, so this is a focused completeness and clearance improvement.

## Gates before a quality claim

1. Keep page SVG/PDF generation and cross-page span continuation covered by tests.
2. Improve lyric size and publication
   typography against the visual baseline.
3. Pin a larger licensed piano/chamber corpus and compare missing symbols,
   clipped content, collisions, page geometry, and legibility at print size.
4. Run blind human review of the resulting PDFs and record the rubric and
   individual scores. Do not infer superiority from page count or preflight.
