# Ten-score print corpus against Verovio 6.3.0 (2026-11-05)

The [6.4.1 rerun](print-corpus-verovio-6-4-1.md) uses the same licensed inputs
and A4 measurement protocol, with a separate versioned result.

Issue #183 asks for a licensed piano/chamber corpus, a same-input Verovio 6.3.0 comparison,
and checks for clipping, missing marks, collisions, legibility, page count and typography, with
import losses recorded separately. This record covers those checks on ten committed fixtures.
It is a screening pass. It does not rank the two renderers, and blind human review is not part
of it.

## Inputs and setup

All inputs are committed and licensed: four CC0 OpenScore Lieder MXL files, five CC0 OpenScore
OMR MuseScore files, and the public-domain *Freilachs No. 17* (see `tests/fixtures/manifest.json`).
Each MuseScore file was first converted with `acorde convert` to MusicXML. Both renderers then
read that same file, so import losses from the MuseScore source happen before either renderer
runs.

- Acorde: `print_mxl` on A4, notation-density breaks, at most four measures per system,
  `crates/pdf/tests/fixtures/Tuffy.ttf` for text. The vector PDF was rasterized with
  `pdftoppm -scale-to-x 990 -scale-to-y 1400`.
- Verovio 6.3.0: A4 (`--page-width 2100 --page-height 2970`), 14 mm side and 16 mm top/bottom
  margins, all pages (`-a`).
- Measurement: `scripts/compare_print_corpus.py` inlines each page SVG into one HTML page and
  measures it in headless Chrome with `getBoundingClientRect`. The same code runs for both
  renderers, and millimetres come from the A4 page width.

```bash
cargo build --release -p acorde-cli -p acorde-pdf --example print_mxl
python3 scripts/compare_print_corpus.py out.json work \
  aloha:tests/fixtures/openscore_lieder_aloha_oe.mxl \
  just-for-today:tests/fixtures/openscore_lieder_just_for_today.mxl \
  quilter:tests/fixtures/openscore_lieder_quilter_prelude.mxl \
  webern:tests/fixtures/openscore_lieder_webern_so_ich_traurig_bin.mxl \
  omr-1003:tests/fixtures/openscore_omr_score_1003.mscz \
  omr-1016:tests/fixtures/openscore_omr_score_1016.mscz \
  omr-1033:tests/fixtures/openscore_omr_score_1033.mscz \
  omr-1035:tests/fixtures/openscore_omr_score_1035.mscz \
  omr-1036:tests/fixtures/openscore_omr_score_1036.mscz \
  freilachs:tests/fixtures/freilachs_no_17.mscz
```

## Page geometry and import losses

Import diagnostics are Acorde's `acorde report` counts on the shared input. They are kept apart
from the engraving measurements below.

| Score | Source | Import diagnostics | Acorde pages / systems | Verovio pages / systems |
| --- | --- | ---: | ---: | ---: |
| aloha | `openscore_lieder_aloha_oe.mxl` | 73 | 3 / 6 | 3 / 6 |
| just-for-today | `openscore_lieder_just_for_today.mxl` | 32 | 4 / 11 | 3 / 7 |
| quilter | `openscore_lieder_quilter_prelude.mxl` | 104 | 1 / 4 | 1 / 3 |
| webern | `openscore_lieder_webern_so_ich_traurig_bin.mxl` | 23 | 2 / 5 | 2 / 4 |
| omr-1003 | `openscore_omr_score_1003.mscz` | 0 | 1 / 2 | 1 / 1 |
| omr-1016 | `openscore_omr_score_1016.mscz` | 0 | 2 / 7 | 1 / 4 |
| omr-1033 | `openscore_omr_score_1033.mscz` | 0 | 1 / 5 | 1 / 3 |
| omr-1035 | `openscore_omr_score_1035.mscz` | 1 | 1 / 5 | 1 / 4 |
| omr-1036 | `openscore_omr_score_1036.mscz` | 0 | 2 / 5 | 2 / 4 |
| freilachs | `freilachs_no_17.mscz` | 0 | 1 / 7 | 1 / 4 |

The Quilter and Webern diagnostic counts each rose by one: the new `musicxml.unterminated-hairpin`
and `musicxml.unsupported-overlapping-hairpin` codes now report wedges that were previously
dropped without a diagnostic.

## Clipping, marks, collisions and legibility

The checks, as `compare_print_corpus.py` defines them:

- *Past page edge*: a drawn element extends beyond the paper.
- *Past margins*: a drawn element extends beyond the 14/16 mm margin box. Page numbers and
  credits normally do this.
- *Text over notation / text*: a text box overlaps a notehead, accidental, rest or another text
  box by more than 0.5 mm². Text boxes include ascent and descent, so these counts are screening
  candidates, not confirmed collisions.
- *Shortfall*: the SVG has fewer marks of that kind than the source MusicXML. Spans split at
  system breaks can legitimately exceed the source count.

| Score | Renderer | Past page edge | Past margins | Text over notation | Text over text | Staff space mm | Lyric font mm | Median / min text font mm | Count shortfall vs source |
| --- | --- | ---: | ---: | ---: | ---: | ---: | ---: | ---: | --- |
| aloha | Acorde | 0 | 15 | 0 | 1 | 1.39 | 1.95 | 1.95 / 1.67 | dynamics 1 |
| aloha | Verovio | 0 | 35 | 2 | 75 | 1.8 | 4.05 | 4.05 / 3.24 | rests 25, dynamics 1 |
| just-for-today | Acorde | 0 | 12 | 0 | 0 | 1.35 | 1.89 | 1.89 / 1.62 | hairpins 1 |
| just-for-today | Verovio | 0 | 21 | 8 | 61 | 1.8 | 4.05 | 4.05 / 3.24 | noteheads 1 |
| quilter | Acorde | 0 | 2 | 1 | 0 | 1.66 | — | 3.87 / 1.99 | hairpins 2 |
| quilter | Verovio | 0 | 7 | 1 | 1 | 1.8 | — | 4.05 / 3.24 | — |
| webern | Acorde | 0 | 10 | 4 | 0 | 1.23 | 1.72 | 1.99 / 1.48 | dynamics 1, hairpins 1 |
| webern | Verovio | 0 | 11 | 0 | 1 | 1.8 | 4.05 | 4.05 / 3.24 | dynamics 1 |
| omr-1003 | Acorde | 0 | 2 | 0 | 0 | 1.69 | 2.37 | 2.37 / 2.04 | — |
| omr-1003 | Verovio | 0 | 0 | 0 | 0 | 1.8 | 4.05 | 4.05 / 4.05 | — |
| omr-1016 | Acorde | 0 | 4 | 0 | 0 | 1.69 | — | 2.52 / 2.04 | — |
| omr-1016 | Verovio | 0 | 0 | 0 | 0 | 1.8 | — | 4.05 / 3.24 | — |
| omr-1033 | Acorde | 0 | 3 | 0 | 0 | 1.69 | — | 1.22 / 1.22 | — |
| omr-1033 | Verovio | 0 | 0 | 0 | 0 | 1.8 | — | 4.05 / 3.24 | — |
| omr-1035 | Acorde | 0 | 3 | 0 | 0 | 1.63 | — | 1.95 / 1.44 | — |
| omr-1035 | Verovio | 0 | 0 | 0 | 0 | 1.8 | — | 4.05 / 3.24 | — |
| omr-1036 | Acorde | 6 | 5 | 0 | 0 | 1.49 | — | 2.63 / 1.79 | — |
| omr-1036 | Verovio | 0 | 2 | 0 | 0 | 1.8 | — | 4.05 / 3.24 | — |
| freilachs | Acorde | 0 | 2 | 0 | 0 | 1.13 | — | 1.47 / 0.81 | — |
| freilachs | Verovio | 0 | 1 | 0 | 0 | 1.8 | — | 4.05 / 3.03 | — |

## Defects found and fixed in this pass

1. **Every page carried the whole score.** Each physical page SVG contained the notation of
   every page and showed one page through its viewBox. Hidden content therefore went into each
   PDF page: each of Aloha Oe's three PDF page streams was about 830 KB, and the counts showed
   1,938 noteheads across three pages for a 646-notehead score. Each page now keeps only the
   top-level elements whose vertical extent reaches it. Aloha's noteheads are 646 across its
   pages, and a regression test checks this.
2. **Cross-voice chord ties were dropped.** In Quilter bar 8, a chord ties C♯3 and G3 into the
   other voice's chord. Only the same-voice tie was drawn (6 of 8 ties). Unmatched chord members
   now look for the same pitch at the same onset in another voice. This also applies to screen
   SVG.
3. **Part labels reached the paper edge.** Right-aligned labels were sized assuming 0.5 em per
   glyph. In the Tuffy PDF, "Γούντ Μπλόκ" (OMR 1036) began at the paper edge, and Webern's
   two-line "Gesang\nVoice" printed as one run. Labels now split at their own line breaks, or at
   the middle space when longer than eight characters. They are sized for 0.6 em per glyph, with
   3 mm of paper kept clear.
4. **Wedges were lost without a report.** A wedge start replaced by another before reaching a
   note, and a wedge with no stop, were dropped without a diagnostic. They now report
   `musicxml.unsupported-overlapping-hairpin` and `musicxml.unterminated-hairpin`. Hairpin start
   and stop state is also cleared at the end of each part. Previously it could carry into the
   next part.

The remaining Acorde shortfalls are reported import losses. The dynamics shortfalls are
`<other-dynamics>`, reported as `musicxml.unsupported-dynamic`. The Quilter hairpins are
unterminated source wedges. The Webern hairpin is a replaced start.

## Open observations (not fixed here)

- **Number-2 wedges pair wrongly.** In *Just for Today* bars 9 and 13, a crescendo ends exactly
  where a numbered diminuendo begins. The PDF shows zero-length hairpin ticks and long, thin
  wedges running to the system edge. Import keeps one hairpin per staff, so these overlapping
  pairs are mis-paired. This needs a hairpin model that pairs by wedge number.
- **Text size.** Acorde's lyrics are 1.7–2.4 mm. Verovio's measure 4.05 mm on the same input.
  OMR 1033 chord symbols are 1.22 mm and Freilachs volta and measure numbers are about 1.4 mm.
  At 1.13 mm, Freilachs's seven systems also have the smallest staff space. Verovio uses 1.8 mm
  throughout.
- **Paper use.** Acorde uses more systems than Verovio on eight of ten scores. On the
  *Just for Today* pages it leaves much of the lower page empty.
- **Missing brace for a localized piano name.** OMR 1036's two-staff "Πιάνο" part has no brace:
  the implicit piano brace matches only the English name.
- **Chrome font fallback.** Headless Chrome lays out Greek part labels with a wide fallback
  font, so the six OMR 1036 "past page edge" elements are those labels in Chrome. The Tuffy PDF
  raster shows them inside the paper with the 3 mm clearance.
- **Verovio counts were not visually confirmed.** These include Aloha's 25-rest shortfall and
  its 75 text-over-text candidates. They are listed for symmetry, not as Verovio defects.

- **Collisions confirmed in the rasters.** On Quilter, `poco rall.` in bar 8 overlaps noteheads
  and `Fine` crosses the final slur. On Webern page 1, lyrics such as `Fast` print over chord
  noteheads in the lower staff (its four text-over-notation candidates), and the opening tempo
  prints only as `ca 112)`.
- **Freilachs** uses half the page, with a 1.13 mm staff space and stray tiny marks inside the
  staff.

Rasterized Acorde pages inspected at 990 × 1400 px: Aloha page 1, *Just for Today* page 2,
Quilter, Webern page 1, and Freilachs, plus a 300 dpi crop of OMR 1036 page 1. Blind human
scoring of these PDFs is not part of this record.

## Re-measurement after #391 (2026-11-05)

Same inputs, settings and script, Acorde only (Verovio rows are unchanged). The changes are listed
in the CHANGELOG under #391. Before → after:

| Score | Pages / systems | Text over notation | Text over text | Staff space mm | Lyric font mm | Min text font mm | Count shortfall |
| --- | ---: | ---: | ---: | ---: | ---: | ---: | --- |
| aloha | 3 / 6 | 0 → 0 | 1 → 1 | 1.39 → 1.61 | 1.95 → 2.58 | 1.67 → 1.94 | dynamics 1 |
| just-for-today | 4 / 11 | 0 → 0 | 0 → 0 | 1.35 → 1.57 | 1.89 → 2.51 | 1.62 → 1.88 | hairpins 1 → — |
| quilter | 1 / 4 | 1 → 0 | 0 → 0 | 1.66 → 1.70 | — | 1.99 → 2.04 | hairpins 2 |
| webern | 2 / 5 | 4 → 0 | 0 → 0 | 1.23 → 1.22 | 1.72 → 1.95 | 1.48 → 1.46 | dynamics 1, hairpins 1 |
| omr-1003 | 1 / 2 | 0 → 0 | 0 → 0 | 1.69 → 1.70 | 2.37 → 2.72 | 2.04 → 2.04 | — |
| omr-1016 | 2 / 7 | 0 → 0 | 0 → 0 | 1.69 → 1.70 | — | 2.04 → 2.04 | — |
| omr-1033 | 1 / 5 | 0 → 0 | 0 → 0 | 1.69 → 1.70 | — | 1.22 → 1.70 | — |
| omr-1035 | 1 / 5 | 0 → 0 | 0 → 0 | 1.63 → 1.70 | — | 1.44 → 1.44 | — |
| omr-1036 | 2 / 5 | 0 → 0 | 0 → 0 | 1.49 → 1.50 | — | 1.79 → 1.80 | — |
| freilachs | 1 / 7 | 0 → 0 | 0 → 0 | 1.13 → 1.67 | — | 0.81 → 1.68 | — |

- **Fixed:** the *Just for Today* numbered wedges (the hairpin shortfall is gone), the OMR 1036
  piano brace, Webern's tempo (now `Sehr fliessend und zart ( ca 112)`; the source's metronome has
  no per-minute value, so no note glyph is drawn), and Webern's lyrics over the piano's high
  chords. Vertical justification raised the staff space on every score except Webern and
  removed Freilachs's half-empty page.
- **Trade-off:** the wider lyric staff gap applies to every staff pair in the score, so Webern's
  staff space is 1.22 mm, slightly below its 1.23 mm baseline.
- **Lyrics and directions:** Aloha's larger lyrics stay inside their barlines and off the staff
  below: measures keep their lyric width, and the first syllable clears the barline. Quilter's
  `poco rall.` (bar 8) no longer sits on the tied chord, and `Fine` sits above the final slur.
  The script tracks neither slurs nor ties, so these were checked in 300 dpi rasters.
- **Lyric spacing:** in a bar with lyrics, every staff shares one beat-to-x map. Each gap
  between consecutive onsets keeps the lyric clearance its events need, and only the room beyond
  that is shared by duration. All of Aloha's syllables sit on their verse line, and the voice's
  notes stay in column with the piano's notes on the same beat.
- **Not a collision:** Acorde still uses more systems than Verovio on eight of ten scores. That
  comes from the corpus setting of at most four measures per system.
