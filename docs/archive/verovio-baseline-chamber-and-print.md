# Archived evidence

This historical record is linked from the [evidence index](../verovio-engraving-baseline.md).

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
all three. The page SVG reserves a nine-space grand-staff gap and 60 mm system
height for a piano score with below-staff words. It also moves the words below
nearby noteheads when the source engraving's vertical hint would pull them into
the notation. A fixture regression checks both `L.H.` marks and clearance for
`poco cresc.` between the two staves.

The Acorde density PDF remains one A4 page, now with four systems grouped
2+2+3+3; Verovio uses one page with three systems. The Acorde PDF and the
same-input Verovio SVG were inspected at 1400 px. Acorde's directions are
present, but the two `L.H.` marks stack at one horizontal position because
measure text does not yet retain its beat position. Its notation and publication
type remain smaller than Verovio's. A follow-up PDF review found `rall.` too
close to the lower staff at the previous eight-space/56 mm setting; the
nine-space/60 mm setting leaves about 10 px below its baseline at 1400 px.
The MusicXML import still reports 103
diagnostics, so this is a focused completeness and clearance improvement.

### Repeated piano directions on one print line (2026-10-31)

The same pinned Quilter MXL places two `L.H.` words at the beginning of measure 4
and one quarter-note beat later. Verovio 6.3.0 prints them side by side. Acorde
kept both words but previously stacked them vertically at the measure start.
Its physical-page SVG now spreads repeated expression words without explicit
horizontal offsets across one print lane. The generated SVG moves the second
`L.H.` from the first one's x coordinate to 15.36 mm farther right; both
baselines remain equal. The A4 PDF was rasterized at 1400 px and the two marks
were inspected beside Verovio's A4 SVG. The screen SVG's existing placement
is unchanged. This is a deterministic engraving heuristic: the canonical
`StyledText` still does not retain the source direction's beat position, so
the x coordinate is not asserted to match Verovio's exact placement.

The `poco cresc.` direction remains close to a down-stem chord. Moving it
farther down made it collide with the lower staff, so that trial was reverted.
Quilter still uses four smaller systems where Verovio uses three. This change
addresses one legibility defect and does not establish layout or print parity.

### Verovio-guided direction beat placement (2026-10-31)

The same pinned CC0 Quilter MXL places its two `L.H.` directions at beat 0 and
beat 1 of measure 4. Acorde now retains the later beat in the score text model,
round-trips it through MusicXML `<offset>`, and places above-staff words against
the final note spacing on the physical print page. The first direction remains
at the measure start; the second is 46.91 SVG units (11.73 mm) to its right.
The earlier repeated-word heuristic placed it about 15.36 mm right. A regression
checks the source beats and roundtrip, and another checks interpolation on a
reflowed note row. Below-staff words retain their previous clearance position:
anchoring `poco cresc.` to its beat caused a grand-staff clearance regression.

The Acorde one-page A4 PDF and Verovio 6.3.0 SVG were regenerated from the
same MXL at the same paper size and viewed at 1400 pixels. Both `L.H.` marks are
separate and the existing `poco cresc.` clearance check passes. Acorde still
uses four smaller systems against Verovio's three, and its notation and type
are visibly smaller. The Acorde PDF media box is 595.276 × 841.89 pt. Acorde
reports 103 MusicXML import diagnostics; Verovio reports two open hairpins.
Those diagnostics are separate from the print observations. This change does
not establish Verovio-level print quality.

### Verovio-guided opening credit space (2026-10-31)

The same pinned Quilter MXL was rendered again at A4 with Verovio 6.3.0 and
Acorde's density print mode. For this review, both pages were rasterized at
990 × 1400 px with a white background: Verovio's SVG through CairoSVG and
Acorde's PDF through Poppler. The older Quick Look thumbnail was square
(1400 × 1400 px) and is not a common-scale image for judging notation size.
CairoSVG lacks the Verovio tempo-note font in this environment, so the one
square tempo glyph in that raster is a rasterizer limit, not an input or
engraving result. Verovio places the opening credits farther from the paper
top. Acorde's title baseline was 38 SVG units (9.5 mm) from the paper top,
and its first staff line was at 158.34 SVG
units. The print layout now reserves 16 mm before the first system when a
work title and movement or composer are present. Its title baseline becomes
102 units (25.5 mm) and first staff line 222.34 units. The first page PDF
remains A4 with one page and four systems, grouped 2+2+3+3. The publication
block is visibly less cramped at the top; Acorde's fourth system still differs
from Verovio's three systems. The 103 MusicXML import diagnostics remain unchanged.

The separate pinned *Aloha Oe* MXL still produces three A4 pages in density
mode, with six systems and 73 import diagnostics in this run. Its first and
last pages were rasterized at 1400 px and inspected for clipping, title overlap,
and page-number placement. The new first-page reserve keeps the title above
the notation; the final page still has substantial unused space. A synthetic
eight-system regression checks that the first page's smaller capacity moves
later systems to page two without crossing the bottom margin. This is a
page-geometry improvement, not a general engraving-quality result.

```bash
pdftoppm -f 1 -l 1 -scale-to 1400 -singlefile -png \
  /tmp/acorde-quilter-20261031-after.pdf /tmp/acorde-quilter-20261031-after
DYLD_LIBRARY_PATH=/opt/homebrew/lib python3 -c \
  'import cairosvg; cairosvg.svg2png(url="/tmp/verovio-quilter-20261031.svg", write_to="/tmp/verovio-quilter-20261031-a4.png", output_width=990, output_height=1400, background_color="white")'
```
