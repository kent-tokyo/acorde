# Engraving and print parity protocol

## 2026-11-21 integration checkpoint

The measured-print branch was merged with main at `9fdfff1d`. Workspace
all-feature `cargo check --offline`, formatting, and the 264 IO round-trip
tests passed. The 24 retired development sources were fetched again with their
pinned Git blob identities and independently prepared as MusicXML. They remain
development data; the final reserved cohort has not been evaluated here.

The integrated debug `print_mxl` example rendered `chamber-sq14387632` on
28 A4 pages with measured print, an eight-measure limit and Tuffy. The shared
MusicXML SHA-256 is `924ce674731e2c15c613019051b3a7a6a8612ec3b86efffe824d936d5494f21f`,
the binary SHA-256 is `2bd10482d9c4abe7ce3164a9f0e84b1343838697d5d221059fab562cdc0702cd`,
and the PDF SHA-256 is `3431b3bca3fc627478e5ad031d7ed95b75eeee45781c99ffdaadc2a1806d4ec9`.
The fourth part's source measure 324 contains `p` two beats into a sustained
note. SVG measure ownership places that mark in the following measure, index
325; no dynamic was found in index 324. A separate `pp` in measure index 71
is also absent in the SVG ownership screen. This reproduces the timing defect
on the integrated revision. It requires a score-time representation and
round-trip, rendering, and playback checks. The screen alone does not verify
the other marks or the complete PDF.

The subsequent timed-dynamic development build records directions between note
onsets in the staff-local measure, exports their offsets, and applies them to
following playback events. The SVG ownership screen now locates one dynamic in
each of part 4's measure indices 71 and 324, with none in 325. On the rasterized
page 22 the assistant observed the `p` below the sustained cello note, within
the source measure. The new PDF SHA-256 is
`43c8f7bedf8ac3175bdc184dd3fa2109c3550deea41ae01a9fa178cc0d114b23`;
the debug example SHA-256 is
`993f1d83897683ffdcecaf6a27f1547861c14a70a253a728654d811d716fa88a`.
IO round-trip tests (264), dynamic-offset tests (6), measured-print tests (23),
and focused timed-dynamic command/playback tests passed. This is a single-score
development check, not the full-corpus or comparative page review.

Follow-up work preserves coincident imported directions instead of replacing
one with another, and retains timed dynamics in score-fragment attributes.
Focused round-trip and fragment tests cover those cases. Measured page margins
also reserve dynamic space and reject non-finite or negative timed positions.
The full IO round-trip suite (264) and measured-print suite (23) passed after
these changes. The PDF hashes above identify the preceding build; this later
source has not yet been rerendered for the 24-input comparison.

The [subsequent 24-input development rendering](archive/print-parity-development-20261121.json)
used the same prepared MusicXML for both engines. All 24 inputs produced output:
213 Acorde pages and 179 Verovio pages. This is a page inventory, not a visual
or notation correctness pass. For `vocal-lc6302395`, the outputs have 13 and
8 pages respectively. The assistant compared the first pages at matched page
width: Acorde has two systems and a large lower-page blank area, while Verovio
has three. Acorde's second system contains a long hairpin near the piano's
upper staff and additional crowding around the middle staves. Full source and
page review, collision classification, Letter, defaults, WASM and performance
checks remain pending. The source diff and output hashes in the receipt identify
the tested build; the preparation log separately records MuseScore export exits.

The [automated screen](archive/print-parity-screen-20261121.json) found no
Acorde page-edge, text/notation, text/text, lyric/staff, dynamic/staff or
text/curve candidates across the 24 inputs under that detector. It counted
926 elements outside the nominal page margins; these require classification.
The full candidate-level report remains in the local evaluation directory and
its SHA-256 is recorded in the summary. A zero count is not a visual pass.

The 13/8 vocal comparison above preserves the source's staff-size instructions.
Screening measured Verovio's smallest staff at 1.26 mm and Acorde's at 1.8 mm,
so those page counts do not compare equal staff sizes. A separate shared input
normalizes the authored staff-size to 100%, preserving the rest of the parsed
MusicXML. Its [receipt](archive/print-parity-vocal-equal-20261121.json) records
the hashes and measurements. Both outputs then measure approximately 1.8 mm
on every staff; Acorde uses 13 pages and Verovio 11. The two-page difference
and authored-size behavior remain open for review.

The [ten-input development screen](archive/print-parity-fixed-ten-20261121.json)
rendered 16 pages per engine. Six MSCZ-to-MusicXML MuseScore exports exited with
the known teardown crash after writing output; those inputs need independent source
fidelity review. The screen found one Acorde text/curve candidate: in OMR-1003,
`'No!'` overlapped the lower staff's staccato. The assistant confirmed it in the
PDF. Measured reflow now reserves articulation ink against adjacent staffs, and
the new fixture regression checks the lyric and articulation ink bounds. The
rerendered PDF (`734610528408f25226fe61c08b6a342ccf8f689066dc8051bc6d3279dd23a78b`)
no longer has that candidate; the assistant inspected the page. The full
measured-print suite passed 24 tests.

After this change the [24-input rerender](archive/print-parity-development-reflow-20261121.json)
completed for both engines, with 214 Acorde and 179 Verovio pages. The
[automated screen](archive/print-parity-screen-reflow-20261121.json) found no
Acorde page-edge or selected collision candidates; 931 marks fell outside the
nominal margins and require classification. One quartet increased from 18 to
19 Acorde pages. Comparing its before/after page 3 shows the same first two
systems with more space above the cello on the second system; the third system
moved to the next page. The page count increase is recorded for further review,
as are other layouts and all source fidelity checks.

The [34-input import inventory](archive/print-parity-event-audit-20261121.json)
compares shared MusicXML to the release CLI's score JSON by written onset,
duration, voice, staff, pitch spelling, grace/cue/visibility and ties. The raw
pitched inventory matches 33 inputs; OMR-1036 adds 38 model events because the
audit excludes source `<unpitched>` percussion. A second inventory using their
display-step and display-octave matches all 119 events in that input. Across all
34 inputs it matches 43,769 pitched or unpitched display events. This does not
establish percussion sounding pitch, rest or direction correctness, source-to-
MusicXML export fidelity, or final SVG/PDF visibility.

### Aloha Oe page review, articulation-reflow candidate

The assistant inspected all three Acorde PDF pages at 110 dpi and all three
Verovio SVG pages fitted to the same page width. The shared MusicXML SHA-256 is
`9e79bc1ed711b586f0b3ab4535544b3d77d9f0633c26c7eb395d4e3671e6d921`.
The screen counted 646 noteheads, 146 lyric syllables, five slurs and two ties
in Acorde, matching the shared MusicXML inventory for those categories. That
count and this page review are not a note-by-note original-source audit.

- Page 1: the previously observed lyric/staff collision is not apparent in
  this candidate. Acorde keeps Women and Men rest staves in the second system;
  Verovio hides them and fits a third system on the page.
- Page 2: Acorde has two systems and keeps the solo staff above the chorus after
  the chorus begins. Verovio uses two systems and hides the solo staff in its
  second system. The displayed lyric lines are readable in both at this scale.
- Page 3: Acorde uses two full-width systems. Verovio has one short final system,
  leaving substantially more blank space. Its earlier systems contain more bars,
  so this page is not a like-for-like density comparison.

The Acorde raster hashes in page order are
`0f718aa7ccb3b081699ee5c41c646740dc70f7debc9c9da95384e527175380a2`,
`6200fb3496a038265d6269f6ab985e0ab2d7fd627ae0b489fa5b4c751362d230`,
`14ddc2693495a1588cb150673b48b0efdd115b5b6bbfa22c80ee91de074ff790`.
The Verovio raster hashes are
`77c681828d74bda141321f479d510b66ec006b4e8b92a1e9ef760d579fbfee94`,
`d6e938dc4fdcd129cad9b87f6a2280e387cf40397bd1321d0b44a7cae9f96857`,
`3f79764c7b373e2314900a37ba039f3f585035812e0a46bb6b4454ce8aed7f5c`.
This is a single non-independent assistant review of the earlier candidate;
other input pages and the latest build remain to be reviewed.

After the articulation reflow, the [fixed ten rerender and screen](archive/print-parity-fixed-ten-reflow-20261121.json)
also completed for both engines. Source-to-SVG count discrepancies were examined
as candidates: the 28 hidden staccatos in `part-sq27449398` must remain hidden;
the Quilter MusicXML contains two wedge starts with no stops and both engines
omit them, while Acorde reports an unterminated hairpin. In `chamber-sq7551068`,
part 4, physical measure index 192 (source number 100), one `<dynamics>` group
contains `<p/>`, `other-dynamics` text `Das zweite mal`, and `<pp/>`. Acorde
printed the latter two components but lost the first in that candidate. An
intermediate parser revision reported `musicxml.multiple-dynamics-in-direction`
for this loss; that warning was superseded by the group preservation below.

The follow-up implementation preserves that dynamics group as ordered components
and serializes it back to MusicXML. In a rerender of the same source (SHA-256
`de11bdad52c1a6fc6cec1389ea5dda7a1204cb8c323208abbb90332637a7593d`),
the assistant inspected page 16 and observed `p Das zweite mal pp` below the
cello staff in source order. The PDF SHA-256 is
`ca8e7668163fc40dd612fc5ba1dfb7a28b8cb6ebc7877319e9eff6afa68a1f0c`;
the release example SHA-256 is
`5ef42614a885acdad06485a61ac420f33e433e460e0bd0889c45c04ba1061568`.
The first dynamic and the `pp` in this composite are currently drawn with text
outlines. The rendering still needs comparison of music-glyph styling and
spacing against the source. Playback retains the previous last-mark behavior;
it does not interpret `second time` as repeat-specific playback. Eight offset
and grouping tests, 264 IO round-trip tests, 25 measured-print tests and focused
library Clippy passed. The 34-input results above predate this group change.

The subsequent [24-input](archive/print-parity-development-composite-20261121.json)
and [ten-input](archive/print-parity-fixed-ten-composite-20261121.json) renders
completed in both engines after composite directions and cutaway visibility
changes. The 24 inputs used 212 Acorde pages and 179 Verovio pages; the ten
used 16 pages each. The [automatic screen](archive/print-parity-screen-composite-20261121.json)
found zero Acorde page-edge, text/notation, text/text, lyric/staff,
dynamic/staff or text/curve candidates across these inputs. It recorded 921
and 63 elements beyond nominal margins in the two sets, respectively. This is
not a substitute for classifying those elements or inspecting all pages.
An authored cutaway staff now remains visible for a timed dynamic or visible
pedal over rests, with native SVG and measured-print regression tests.

The [margin audit](archive/print-parity-margin-audit-20261121.json), produced by
[`audit_print_margins.py`](../scripts/audit_print_margins.py) with page hashes
checked, classifies the 984 prior Acorde margin candidates as 750 part labels,
228 page numbers, five titles and one volta line. These labels and page numbers
are deliberately placed in the nominal page margins. The leftmost part-label
ink is 3.04 mm from the paper edge and some long names are small; real printer
hardware and physical readability remain to be checked.

The single notation candidate was a continued volta on page 15 of
`chamber-sq8437358`, with its top line at 13.93 mm against a 16 mm top margin.
Measured preflight now reserves each continuation's actual top extent. The
rerendered score retained 16 pages, and the screen counted 80 margin elements
instead of 81, with zero page-edge or selected text collisions. The PDF SHA-256
is `759c73c0171c22c2cc567f97c10871f243f29c219b0f4e94c0a3e89cf0a6bce8`.
A multi-page regression checks every volta against the top margin; the test
failed without the reservation and passes with it. The latest 34-input output
has not yet been regenerated after this change.

The subsequent [24-input](archive/print-parity-development-volta-20261121.json)
and [ten-input](archive/print-parity-fixed-ten-volta-20261121.json) renders
completed in both engines. In the [repeat margin audit](archive/print-parity-margin-volta-20261121.json),
the 983 remaining candidates are all part labels (750), page numbers (228),
or titles (5). The part-label size and printer-edge clearance concerns above
remain unresolved. This is a margin classification, not a complete legibility
or collision audit of the new output.

### Part-name sizing experiment

The measured path now reserves the minimum extra left inset needed for the
widest unbreakable word at 2.7 mm, wraps names at word boundaries, and retains
the existing 3 mm paper-edge clearance. It reserves no extra space for short
names that already fit, or when part names are disabled. An automated test
checks `Violoncello` size and ink position and confirms `Piano` needs no inset.
The [24-input](archive/print-parity-development-labelfit-20261121.json) and
[ten-input](archive/print-parity-fixed-ten-labelfit-20261121.json) renders
all completed. The [margin audit](archive/print-parity-margin-labelfit-20261121.json)
measured the 759 part-label candidates outside nominal margins at a minimum
font size of 2.75 mm and a minimum ink-to-paper-edge distance of 3.06 mm.

This is not yet a satisfactory final layout. The 24-input Acorde total rose
from 212 to 218 pages: five inputs changed page counts, including
`piano-wq117n11` from one page to two. Its second page contains one system
and substantial blank space, observed in raster SHA-256
`f693e9273d6569242a9fe4869ee1b32492b7a6dfcd8e6069f98529943cd1ff35`.
The name lane is currently reserved on every system even though labels appear
only on each page's first system. Variable system widths and per-page label
requirements need further work before accepting this candidate. Actual printer
clearance also remains unverified.

### Opening-system label inset

The measured path now measures full and repeated names separately. When the
full name is wider than its abbreviation, only the opening system receives
the additional inset; later systems retain their width. The measured output
reports `first_system_inset_mm`, and collision preflight uses that inset as
the opening row's left boundary. This is still a partial solution: the widest
repeated name or fallback full name reserves space on *all* systems, rather
than just the first system on each page.

The [24-input rerender](archive/print-parity-development-first-row-20261121.json)
and [fixed-ten rerender](archive/print-parity-fixed-ten-first-row-20261121.json)
completed in both engines, using the same prepared inputs as the previous
experiment. The Acorde development total decreased from 218 to 217 pages;
`piano-wq117n11` returned from two pages to one with four systems. Its PDF
SHA-256 is `2e5b30676395daa104d91cf8293542b94dc2874702c419193c5b92d1d0aaff6d`.
The assistant reviewed its full page raster: the full name is on the opening
system and later systems start farther left, with no empty second page. This
is not a note-by-note original comparison. The other 23 development inputs
had unchanged page counts relative to the previous experiment. Relative to
the 212-page pre-label-reservation baseline, four chamber inputs still add
five pages in total.

The [margin audit](archive/print-parity-margin-first-row-20261121.json)
recorded 1,008 candidates across the 34 inputs: 770 part labels, 233 page
numbers, and five titles. Among the labels outside the nominal margins, the
minimum measured font size was 2.75 mm and the minimum ink-to-paper-edge
clearance was 3.06 mm. This is not a general collision audit, a full visual
review, or proof of printer clearance. The development inputs remain unsuitable
as a final unseen acceptance set.

### Page-start label insets

The next candidate reserves the full or repeated name only on each page's
first system. It discovers page starts after pagination and repeats system
breaking with the necessary insets. Previously observed starts remain reserved
in the solver, so the set grows monotonically and terminates even when page
boundaries move; stale starts can conservatively constrain later layouts.
`MeasuredPrintOutput.system_insets_mm` records the applied system offsets.
Cross-system ties, slurs, directions and tablature connections use the actual
edges of their respective systems. Regression tests cover a multipage score
and continuation ties and slurs on systems with different left edges.

Both engines rendered all [24 development inputs](archive/print-parity-development-page-insets-20261121.json)
and all [ten fixed inputs](archive/print-parity-fixed-ten-page-insets-20261121.json).
Acorde's development total decreased from 217 to 214 pages; compared with the
212-page baseline before label sizing, `chamber-sq7551068` and
`chamber-sq8812200` each remain one page longer. The 34-input
[margin audit](archive/print-parity-margin-page-insets-20261121.json) found
993 candidates: 758 part labels, 230 page numbers and five titles. The measured
labels outside nominal margins had a minimum size of 2.75 mm and minimum
paper-edge distance of 3.06 mm. The assistant inspected page 2 of
`chamber-sq8812200` (raster SHA-256
`44d985cf6086115db110daee321868a03d0be8a8649ebffc793874db1436a9ef`):
only its first system has the additional name inset. This is not a complete
visual or source-fidelity review, and physical printer clearance remains
unverified. The render report records the candidate source diff at render time;
a subsequently added regression test does not alter the rendering code.

### Balancing a one-system final page

The final page of `chamber-sq8812200` in the preceding default run had one
system. An experiment removing the initial two-sided timed-dynamic margin
reservation retained the 36-page and 20-page results for the two chamber
inputs and was reverted; the changed ink positions were not otherwise
validated. With the existing opt-in `FinalPagePolicy::Balance`, the 36-page
input also ended with one system. Its measured final-page heights showed
that a chain of legal boundary shifts could move a system onto the final page.

For measured layouts using `Balance`, pagination now tries that chain when
the last page has one system. It commits the chain only if the recipient pages
all fit their available heights and the chain ends at a page with a spare
system. Otherwise it retains the previous result. The default
`AllowSingleSystem` policy is unchanged; this does not remove the one-system
final page in the default run or prove a general solution for every possible
partition. Layout tests cover a feasible multi-page chain and an infeasible
move.

A same-input A4 trial with `Balance` retained 36 pages and changed the last
four page system counts from 3, 2, 2, 1 to 2, 2, 2, 2. Input MusicXML SHA-256
`9c1a19ede6f1eba3bb12760aa4f5f3639d677c84f4eeb52e7a745ed5b79bf3ad`,
configuration SHA-256
`7dbd1fa0544b38656baa3fa541d69aba37f7fee979826b266c2776fe0d0743e3`,
PDF SHA-256
`9c5f4c588d6643b66ccba82969f59ebf7d3b2749b68492153e21a97dc62e6d29`.
The assistant inspected the final-page raster SHA-256
`e011038c5f6ea5707428d14197333e1eed769183653bcfb568dcf9e146aa7867`:
it contains two systems, with no apparent overlap between them. The `pizz.`
mark near the second system's opening barline appears to touch the line; a
similar placement is visible in the earlier default rendering, but exact ink
bounds and original-source intent have not yet been assessed. This one-page
review is not a corpus-wide or original-to-output fidelity audit.

### Text clearance from spanning barlines

The apparent contact on the final page was confirmed at 300 dpi: the dot of
viola `pizz.` at measure 637 crossed the system barline. The independent MXL
source contains that text above the viola with `relative-x="-60.00"` and
`enclosure="rectangle"`; the latter is rendered by Verovio but currently
omitted by Acorde and remains a source-fidelity defect. This finding is based
on the original MXL content and Verovio SVG, not a comparison to the original
engraved page.

The measured preflight now includes actual barline strokes as horizontal
obstacles for text when their vertical ink ranges overlap. It preserves the
measure ownership and moves the text only as needed for the nearest available
clearance; later collision passes still apply. A licensed
[regression excerpt](../crates/render-svg/tests/fixtures/wolf_italian_serenade_pizz_excerpt.musicxml)
retains source measures 636–643, with initial attributes copied from measure 1.
The test failed on the prior code with a conservative italic-ink right edge of
196.70 against a barline at 194.59 internal units, and passes with the fix.
On the full score's default-layout final page, Chrome measured the outlined
text's right edge at 246.91 and the barline center at 247.98 in a 840-pixel
page view; the line stroke was 0.65 units wide. This verifies clearance for
this case, not every line and glyph in the corpus.

Both engines completed the [24 development](archive/print-parity-development-barline-20261121.json)
and [ten fixed](archive/print-parity-fixed-ten-barline-20261121.json) inputs;
each Acorde page count was unchanged from the preceding candidate, totaling
214 and 16 respectively. The [34-input margin audit](archive/print-parity-margin-barline-20261121.json)
again classified 993 candidates as 758 part labels, 230 page numbers and five
titles; label font size and paper-edge minima remained 2.75 mm and 3.06 mm.
This is not a complete collision or source-fidelity audit. The final-pass
PNG of the excerpt has SHA-256
`9ca26fd49ca51108fe84183c1fc4c2b6a1698bfae60cba59b78760df37081f5a`.

### MusicXML rectangular text enclosures

`StyledText.enclosure` now preserves the authored MusicXML enclosure value in
Score JSON and through MusicXML import/export. Its default is absent for old
JSON. The existing `SetMeasureText` command carries the field through
undo/redo. This adds a public Rust struct field: callers using struct literals
need to add it, so release versioning needs review. Other enclosure values are
preserved in MusicXML but currently cause an explicit renderer error; score-level
text enclosures are also not yet supported. Non-MusicXML interchange fidelity
for the new field has not been established.

For measure-level `rectangle`, measured print uses the supplied font's ink
bounds, synthetic italic slant and padding for the frame; collision preflight
moves the frame and its text together. Screen SVG has an approximate frame.
The source excerpt test checks JSON and MusicXML round trips and confirms the
frame and its stroke clear the barline. A 1500-pixel assistant review of the
full score's final page (PNG SHA-256
`406a3b1dc87a6fd14d5ca259abebe38742e177433065f7201b630b6f4baf51a0`)
shows the viola's boxed `pizz.`. Chrome measured its frame's right edge at
244.08 against the barline center at 247.98 in an 840-pixel page view; the
line stroke width was 0.65. The resulting box is to the left of the barline
that begins its owning measure, whereas the MusicXML attaches the direction
to the following measure and gives it a negative relative-x offset. Whether
this position communicates the intended onset requires comparison against the
original engraving; Verovio places the text within its owning measure. This
addresses the omitted rectangle, not that unresolved placement difference,
every possible enclosure, or a full original-page audit.

Both engines rendered all [24 development inputs](archive/print-parity-development-enclosure-20261121.json)
and [ten fixed inputs](archive/print-parity-fixed-ten-enclosure-20261121.json).
The Acorde page totals remain 214 and 16. The [margin audit](archive/print-parity-margin-enclosure-20261121.json)
again found 993 candidates: 758 part labels, 230 page numbers and five titles;
minimum label size and edge distance remained 2.75 mm and 3.06 mm. After this
render, the parser was corrected to keep words and rehearsal enclosures separate
when they share a direction; a scan found no such mixed direction in these 34
prepared inputs. This is not an exhaustive collision or cross-platform verification.

### Boxed direction and owning measure

The original OpenScore MXL assigns the boxed viola `pizz.` to measure 637
with `relative-x="-60.00"`. Verovio 6.4.1 places the box inside that measure.
An independent import of the same MXL by MuseScore Studio 4.7.5 also places it
inside measure 637. MuseScore wrote a readable 19-page PDF (SHA-256
`eae8c98efe735a1d0d66f468e01f302e0df592dcef40e9f679ae2741e91471d4`),
although its log reported a mutex exception at teardown; this is not a clean
export success. The assistant reviewed its last page (PNG SHA-256
`dfdd6020ed0a6090929a1d8327ef9f01d3abc307d0c403bcdb1f584b388db9be`)
and Verovio's last page (PNG SHA-256
`b4604ea4c85ef307c6bacb5d008be11233c6dcb8f042b67f0bb5cb03ac09c35f`).
The original typeset source referenced by the transcription is IMSLP #40911;
its [catalogue record](https://imslp.org/wiki/Italian_Serenade_(Wolf,_Hugo))
identifies the 1903 Lauterbach & Kuhn score, but that source PDF was not
available to the web tool and has not been visually inspected. MuseScore is
an independent rendering of the MusicXML, not the original engraved source.

Measured print now records each measure's physical left boundary. When the
center of a rectangular text enclosure would lie before the boundary of its
own measure, preflight keeps the frame inside that measure and clears the
barline. Other text and boxed text already centered in their measure retain
their existing positioning policy. This deliberately overrides a large
negative visual offset in the affected case; the MusicXML offset remains in
the model and serialization. The excerpt regression failed before the fix
and passes afterward. In the full Acorde final-page output, Chrome measured the
box left at 251.88 and its barline center at 247.98 in an 840-pixel page view
(barline stroke width 0.65). The assistant reviewed the Acorde final page
(PNG SHA-256 `faeb5f0a6e062ceb6f09a83a6a7e1599e9c282e3225cf29a22fb80702a2b7b66`).
This resolves the observed measure association in this input; it does not prove
that overriding every such authored offset is always musically appropriate.

The [24-input](archive/print-parity-development-owned-box-20261121.json) and
[fixed-ten](archive/print-parity-fixed-ten-owned-box-20261121.json) runs
completed in both engines with unchanged Acorde totals of 214 and 16 pages.
The [margin audit](archive/print-parity-margin-owned-box-20261121.json) found
993 candidates, classified as 758 part labels, 230 page numbers and five titles;
label minima were 2.75 mm and 3.06 mm to the paper edge. In the pictured
score Acorde still uses 36 pages and Verovio 25 under the recorded renderer
settings; the full-page comparison and same-staff-size attribution remain
open, as do the single-system final-page issue under the default policy and
the complete original-source review.

### One-shot staff-gap relaxation

On `chamber-sq8812200`, browser measurements of the first system showed a
1.8 mm staff space in both Acorde and Verovio. In Acorde, early crowded
placements caused staff-gap reservations to grow; after ink moved, the former
monotone algorithm retained reservations larger than the final ink required.
Simply shrinking on every pass failed to converge on two Gossec tests and was
reverted. The current measured path first converges with growth-only reflow,
then tries one shrink to observed ink requirements, bounded by the initial
gaps. Subsequent passes only grow gaps again. Each reserved system height
includes 0.05 mm for sub-threshold top/bottom adjustments. This can still
increase some gaps: on `sq8812200` page 1, the first inter-staff distance
increased from 25.58 to 27.84 mm, and on `sq14387632` some other gaps also
increased. The approach is a candidate, not an optimal packing proof.

All [24 development inputs](archive/print-parity-development-gap-relax-20261122.json)
and [ten fixed inputs](archive/print-parity-fixed-ten-gap-relax-20261122.json)
rendered in both engines. The Acorde development total changed from 214 to
208 pages, with four chamber inputs changing 28→27, 19→16, 20→19 and 36→35.
The fixed ten remained 16 pages. A [limited before/after collision screen](archive/print-parity-screen-gap-relax-20261122.json)
of the four changed scores found zero candidates for its text/notation,
text/text, lyric/staff, dynamic/staff and text/curve checks in either version.
It does not cover all possible engraving collisions. The [margin audit](archive/print-parity-margin-gap-relax-20261122.json)
found 963 candidates, comprising 734 part labels, 224 page numbers and five
titles; label minima were 2.75 mm and 3.06 mm from the edge.

Two alternating release runs per version on the same ARM64 macOS host and
`sq8812200` input are recorded in the [benchmark receipt](archive/print-parity-gap-benchmark-20261122.json).
Baseline elapsed 5.447–5.525 s with maximum RSS 257.5–270.1 MB; this candidate
elapsed 7.500–7.569 s with RSS 321.5–323.1 MB. These few runs do not establish
performance parity. The assistant inspected Acorde's first and final pages
(raster SHA-256 `4560cd265528b8a40dd370db4f203a0bc0c970a0200f6ba93166baae0fa61946`
and `9774bb0b214d5848fe1fe8195f0f266d4bf2afc06b9c6ffe1cccd4d5e40b6ace`);
the final page still has a single system under the default policy. The Gossec
regression checks that the second-violin-to-viola distance in the first system
stays below 21 mm while retaining its existing slur and fingering checks.
Complete page review, performance work, and final reserved validation remain open.

### Reuse of measured row geometry

Profiling the 643-measure chamber input showed repeated pagination attempts:
20 system-break passes, each previously remeasuring every row even when its
physical measure indices were unchanged. A per-engraving cache now reuses row
geometry for the same score and measure sequence. The cache is limited to
4,096 entries and an estimated 2 MiB of owned keys and geometry; on reaching
either limit, other rows are measured normally. Hash-map overhead and the rest
of engraving memory are not included in that estimate, and the cache-limit
fallback has not been stress-tested on a very large score.

The [same-input comparison](archive/print-parity-geometry-cache-exact-20261122.json)
rendered all 24 development inputs and ten fixed inputs. All 34 PDFs and
all 224 Acorde SVG pages matched the uncached renderer byte for byte. The
existing margin and limited collision screens therefore describe the same
output, but still do not amount to a complete visual or collision review.
The 33 measured-print tests passed, including the Gossec regression.

In the [same-host alternating benchmark](archive/print-parity-geometry-cache-benchmark-20261122.json),
the uncached reflow candidate took 7.698 and 8.510 s; the bounded cache took
6.623 and 6.981 s. Maximum RSS was 323.3–327.0 MB and 320.4–323.5 MB,
respectively. All four produced the same 35-page PDF hash. This is two runs
per version on one score; timing varies and this is not final performance or
memory qualification. The measured cached runs remain slower than the
5.447–5.525 s observed in the earlier benchmark before staff-gap relaxation,
which was a separate run and cannot establish a controlled percentage change.

This program targets conventional keyboard, vocal, chamber-score, and extracted-part
engraving against **Verovio 6.4.1**. The starting source is Acorde
`f48944024039cce2abb404af60d9ffc9dedc1502`, not a newly released artifact.
The user requested on 2026-11-15 that the implementing assistant perform the
visual assessment instead of waiting for human reviewers. Record that assessment
as a single assistant's review, never as independent human or statistical evidence.

## Fixed inputs and boundaries

- Development: the ten licensed inputs in
  [the existing comparison](archive/print-corpus-verovio-6-4-1.md).
- First reserved cohort, now retired to development after a failed frozen run:
  [24 selected inputs](../benchmarks/print/parity-holdout-selection-v1.json),
  six each for keyboard, vocal, chamber, and extracted-part output. The selection
  pins upstream revisions and Git blobs before opening source scores or renderings.
  The source contents and SHA-256 hashes must be frozen before algorithm changes.
- Retain full selected files. Some upstream files represent movements rather than
  whole multi-movement works; do not describe 24 files as 24 complete compositions.
  Keep related movements/cycle members in the same development or reserved cohort.
- MXL supplies independent MusicXML. Export MSCX through a pinned MuseScore build,
  preserving notation, text, lyrics, and credits. Do not use Acorde to prepare these
  held-out shared inputs. Record any exporter error and compare with original source.
- Preserve input loss and source ambiguity separately from layout defects. Do not
  drop a difficult input, omit a mark, or silently reinterpret a source to pass a gate.
- Source files remain in an external evaluation directory. Preserve upstream license,
  authorship, revision, hash, and attribution; no corpus downloads occur during tests.

## Physical comparison and success conditions

Use A4 portrait, 14 mm horizontal and 16 mm vertical margins, plus a separate Letter
check. Fix the same 1.8 mm staff space for the matched-size comparison. Also evaluate
the public default configuration independently. Music glyphs may differ in design;
the comparison must not require pixel identity with Verovio.

Proposed print typography floors, fixed before implementation: 4.05 mm font size for
lyrics, 3.24 mm for expression/tempo text and chord names, and 2.7 mm for measure and
ending numbers. These are role-specific engineering targets; font size alone does not
establish legibility. Record actual ink bounds and visual readability as well.

1. **Notation correctness:** zero unexplained omitted/wrong essential marks, wrong
   pitch/voice association, page-edge clipping, or confirmed reading-obstructing
   collisions on all development and final reserved inputs. Count split span segments
   by source ownership, not by raw SVG group counts. Review ambiguous source notation
   against the original rather than accepting either renderer as a gold standard.
2. **Legibility:** preserve requested staff space and typography floors without
   hiding marks. Inspect every page at physical-scale equivalent with detailed crops
   for dense regions. Count unintended text/staff-line intersections, stems, beams,
   articulations, slurs and ties as well as the existing text/notehead screen.
   Page count is secondary to correctness and readability; record utilization and
   avoid unexplained near-empty final pages or excessive inter-system stretching.
3. **Assistant review:** per score and page record source fidelity, spacing, typography,
   collisions and page use; compare randomized A/B filenames where practicable, but
   disclose prior familiarity and the single implementing reviewer's lack of independence.
   Do not compute human-preference confidence intervals from one assistant's ratings.
   Pass only if no unresolved material Acorde disadvantage remains in any category.
4. **Generalization and reproducibility:** freeze candidate source/binary/font hashes
   before opening final reserved renders. Require each of the four input groups to pass,
   record every exclusion/failure, and make no overall claim from an aggregate alone.
   After a reserved score informs a fix, mark the cohort as development and select a
   disjoint cohort before another final gate. Require native/WASM placement agreement,
   macOS/Linux/Windows tests, and same-host performance/memory measurements. Missing
   platform evidence remains pending, not a pass.

## Implementation sequence

1. Measured text and glyph bounds shared with rendering, in staff-space/mm units.
2. Minimum/ideal rhythmic-column and measure widths; deterministic measured system
   breaks that preserve authored breaks, pickup, multirests and keep-together ranges.
3. Per-system collision resolution and height measurement before pagination; fixed
   staff space, bounded reflow, explicit diagnostics for infeasible constraints.
4. Shared final placement for SVG/PDF, music-glyph outlines, text-font resources and
   full-page output verification. Preserve the existing layout ownership contracts.
5. Default and matched-size development runs, then frozen reserved evaluation.

Core retains score semantics only. Layout owns neutral geometry and page planning;
renderers provide glyph/font measurements without filesystem or OS dependencies.
New layout behavior begins opt-in. Preserve existing defaults until the gates pass.

## Baseline observations

The [2026-11-15 rerun](archive/print-parity-baseline-v1.json) reproduces 18 pages / 54
systems for Acorde and 16 / 40 for Verovio. Acorde lyrics remain 3.08–3.42 mm.
The run uses debug binaries, Tuffy.ttf and the existing eight-measure density option;
it is a geometry comparison, not a release-performance result.

Assistant inspection of Aloha Oe page 1 found lyrics from the Voice staff extending
into the following Women staff in the second system. The old bounding-box screen
reports zero text/notation candidates because staff lines are not included in its
notation selector. This is an unresolved baseline readability defect, and demonstrates
why candidate counts cannot substitute for visual inspection.

Current gate status: **1 pending, 2 pending, 3 pending, 4 pending**.

The frozen v20 run opened all 24 inputs. Fourteen produced Acorde PDFs and ten failed;
Verovio rendered all 24. The independent shared-MusicXML pitched-event audit matched
all 24, but this did not establish correctness of hidden objects, tuplets, text, or PDF
visibility. The complete cohort is now development data, before any fixes informed by
its outputs. See the [freeze](archive/print-parity-freeze-v20.json),
[results](archive/print-parity-holdout-v20.json), and
[retirement receipt](archive/print-parity-holdout-v20-status.json).
The next final gate requires a separately selected disjoint cohort.

A [second 24-input selection](../benchmarks/print/parity-holdout-selection-v2.json)
now pins six inputs per group, using a fixed SHA-256 ordering of metadata without
file-size or notation filters. Related works/cycles and matching imported fixtures
are excluded. The selection script and metadata hashes record the sampling rule.
[All source blobs and SHA-256 hashes](archive/print-parity-holdout-source-hashes-v2.json)
are verified; their score contents and renderings remain unopened until the next
candidate and font freeze. Selection is not an evaluation result.

Subsequent development exposed valid empty outline paths rejected by PDF, missing
inline music glyphs, lost Roman-numeral/custom harmony spelling, hidden rests printed
by the PDF backend, and lost tuplet display instructions. Cross-staff stems and beam
digits were incorrectly treated as independent staff-gap requirements, causing
unbounded reflow. These repairs remain under qualification. The reflow bound is now
16 passes. Cross-staff slur/hairpin crossings remain visible for review; they are not
silently excluded from the visual gate. Crowded directions may move outside the
grand staff while retaining their source part and temporal ownership.

## Retired-cohort development rerun (candidate v41)

The [v41 rerun](archive/print-parity-development-v41.json) produced all 24 Acorde
PDFs, versus 14 in v20. All 41,610 pitched events matched the shared MusicXML
inventory. Page-edge, text/notehead, text/text and lyric/dynamic staff-line screens
were zero, but 47 text/curve candidates remained (including articulations, ties,
beams and ornaments). These are unresolved screening candidates, not 47 confirmed
engraving defects. The assistant confirmed collisions in the C. P. E. Bach and
Schumann piano pages. Brahms used 16 Acorde pages versus 8 Verovio pages, with
excessive spacing still under investigation. This candidate does not pass the gates.

The [development repair receipt](archive/print-parity-development-repairs-v41.json)
records 1,166 tests in 45 suites before the later geometry repairs and 13 measured
print integration tests after them. The latter include shared filled unison heads,
visible tuplet counts, and sampled slur clearance on the full Brahms fixture.
Final native/WASM, cross-platform, default/Letter, performance, and complete visual
qualification remain pending. Frozen v41 binaries were retained with a separate
hash-checked copy receipt; v20 intermediate PDFs/HTML/native JSON were losslessly
compressed after checking they were not open, with original and archive hashes.

The [v43 four-input diagnostic](archive/print-parity-diagnostic-v43.json) clears
those four inputs' automatic screens after adding articulation/beam/tie obstacles
and per-staff spacing for interleaved rhythms. All 7,457 pitched events match.
C. P. E. Bach shrank from two pages to one; Brahms from sixteen to twelve versus
Verovio's eight. Two first pages were inspected; complete visual review and the
remaining corpus rerun are still pending. The v44 source only factors a type alias
and collapses an equivalent conditional after this diagnostic. Its first Clippy
attempt failed from disk exhaustion, so that attempt is not a successful check.
After losslessly archiving completed older-candidate files, the identical v44 retry
passed all-feature, all-target release Clippy for core, io, layout, render-svg, PDF
and WASM with warnings denied. The final cross-platform check is separate.

## Authored dynamic placement and integrated checks (candidate v47)

The v44 macOS release workspace run passed 2,296 tests in 94 suites, with one
existing ignored test. The branch then incorporated main at `ed6c6d0f`; the only
merge conflict was the two independent Unreleased changelog additions, both retained.
CI on that merge ran the tests but failed the three OS jobs during Clippy on two
Rust 1.100 `chunks_exact_to_as_chunks` warnings. The equivalent `as_chunks` calls
now replace those expressions; that CI failure is not a three-platform pass.

Development PDF inspection found that explicit MusicXML dynamic placement was
discarded. The optional `Note::dynamic_above` field and `SetDynamicPlacement`
preserve it through import, edit/undo/redo and export. Direction dynamics resolve
against the physical written staff, including cross-staff beams. Annotation bands
also follow that staff. In measured print, an unspecified expression on a lyric-free
upper staff of a multi-staff part starts below its staff; explicit sides retain
priority. Crowded interior text can still move outside that same part.
The targeted checks pass: five MusicXML dynamic/offset round-trip tests, the core
backward-compatibility/undo test, sixteen measured-print tests, and six-crate
all-feature/all-target release Clippy on local Rust 1.97. The CI lint repair still
needs its Rust 1.100 rerun. All-page, native/WASM and reserved-corpus gates remain pending.

Brahms remains twelve pages in this diagnostic. The first page's dynamic is restored
above the lower piano staff; `legato` is now below the piano rather than among the
viola annotations. This is one-page development inspection, not a complete review.
The shared source also has an explicitly 70%-sized viola staff in Verovio while
Acorde currently prints it at full size. Median staff-space reporting hides this
difference. Subsequent matched-size comparisons must report every staff's size and
separate authored-size output from a common physical-size comparison; the original
eight-page Verovio result is not an equal-all-staves-size paper-use benchmark.
Setting only explicit staff-size percentages to 100 in a separate shared input
produces eleven Verovio pages. The normalization receipt verifies that every other
parsed source element is unchanged. The authored input remains retained separately.
The screening HTML now declares UTF-8: earlier live-text Verovio screening could
misdecode non-ASCII lyrics, so its text-collision counts need rerunning. The separate
PDF review generator already declared UTF-8 and is not affected by this HTML fix.

## Equal-size rerun and hidden notation repairs (candidates v47–v49)

The [24-input rerun](archive/print-parity-development-v47.json) succeeds for both
engines at a shared 1.8 mm staff size: Acorde uses 210 pages and Verovio 182.
All 41,610 pitched events match. Acorde has no page-edge, text/notehead, text/text,
lyric/staff or dynamic/staff candidates, but three text/curve candidates remain.
The [original ten inputs](archive/print-parity-original-development-v47.json)
produce sixteen pages per engine and match 2,159 pitched events. Their historical
Acorde MSCZ conversion is retained; this does not establish independent MSCZ fidelity.

[Native/WASM placement checks](archive/print-parity-wasm-development-v47.json)
pass all 34 development inputs at tolerance 0.0001, with maximum coordinate
variation 0.00000733. This is geometric agreement, not a source-fidelity or visual
pass. [CI on d65041b3](archive/print-parity-ci-v47.json) passes the three OS test
jobs, browser contracts and measured-print Chrome tests. The WASM build job fails
its size gate: 7,338,641 bytes against 7,220,000. The next candidate records this
increase, narrows the full-profile gate to 7,380,000, and adds a separate release
build and size gate for opt-in measured print. These new gates still require CI.

The three candidates came from Gossec RH 189 page 8. Source inspection found
nonprinting notation containers: across the 34 inputs these contain 560 tuplet
endpoints and 91 articulation containers. The v49 repair preserves hidden
articulations as performance data and optional JSON indices, with validated editing,
undo/redo and separate hidden MusicXML containers on export. Hidden tuplets retain
their timing but suppress both number and bracket. This covers these two notation
kinds; it is not universal support for every possible hidden notation element.

Fingerings and technique text now participate in measured collision clearance.
Fingerings start next to their owning staff and move only to clear actual ink.
The [Gossec diagnostic](archive/print-parity-hidden-notations-v49.json) has no
text/curve candidates on all eighteen pages. Assistant inspection of page 8 confirms
that the three second-violin fingerings sit near that staff and the hidden
first-violin tuplets are absent. This is a one-page review, not an all-page pass.
The [subsequent page 1–2 review](archive/print-parity-gossec-review-v49.json)
finds additional material defects: above-staff bowing symbols are inverted, short
grace-note slurs reach the main-note stem tip, and system-start measure numbers
need clearance from clefs. The full-page review therefore fails this candidate.
The v49 workspace run also stopped at a missing embedded fixture mapping after
258 round-trip tests passed. That mapping is now supplied for both new fixtures;
v50 additionally preserves new note print fields in score diffs/patches and melodic
voice copying. Its full workspace run is pending.
Current-candidate full-corpus, native/WASM, default/Letter, final reserved input,
performance and complete assistant review remain pending. All four gates stay pending.

## Gossec page correction under review (post-v49 working candidate)

The renderer now keeps up-bow and down-bow shapes upright on either side of a
staff, puts system-start measure numbers clear of the clef, and anchors short
grace-note connectors at noteheads. Grace stems no longer pull the anchor of a
longer phrase slur to a distant tip. The span drawing and collision placement
pass share the same final curve. The Gossec regression checks both the short
connector and the longer, authored curve.

The source has two numbered slurs beginning on the same grace note in measure
2. Their MusicXML Bézier Y values imply a short curve below and a longer curve
above. The latter initially appeared unusually high in Acorde. Assistant
inspection of the source reversed an exploratory idea to put it below; the
working correction retains the authored sides and reduces its excess height.
This distinction matters because Verovio's preferred shape alone cannot
override the source.

Physical A4 PDFs of Gossec were regenerated with Verovio 6.4.1 and Acorde's
measured path. The [partial review and artifact hashes](archive/print-parity-gossec-working-review.json)
cover inspected Acorde pages 1, 2 and 8. These show the corrected bowing,
grace slurs, measure numbers and earlier fingering repair. The page-use result
remains open: Acorde uses 18 pages against Verovio's 16, and its opening page
has two systems against three. A trial reducing the initial staff gap to 4.5
staff spaces increased Acorde's total to 19 pages and was reverted. A full
source/ink audit and all-page physical review are still required. These pages
are development evidence only, not a reserved-corpus pass.

The [ea80a7d5 development rerun](archive/print-parity-development-ea80.json)
renders all 24 retired inputs from the [preparation receipt](archive/print-parity-development-preparation-ea80.json).
At the shared 1.8 mm staff size, Acorde uses **213 pages** and Verovio **179**.
The [independent pitched-event audit](archive/print-parity-development-audit-ea80.json)
matches 41,610 written events on the prepared MusicXML, including voice,
onset, duration, spelling, grace/cue/hidden state and per-pitch ties. Automatic
page-edge, text/notehead, text/text, lyric/staff, dynamic/staff and sampled
text/curve screens report zero Acorde candidates. Those screens do not inspect
all ink or prove the notation complete. Raw mark-count shortfalls appear in ten
scores and need source-ownership review; several scores also have substantially
more Acorde pages, including one vocal score at 13 versus 8 and a quartet at
36 versus 25. The current comparison is therefore **not a gate pass**.

MuseScore produced both extracted-part files and diagnostics for this
development rerun; eleven export processes ended with its known teardown crash
after writing nonempty files and were recorded as such, not as clean exits.
One extracted part has two pitched events assigned to a
different voice number in the exported MusicXML than in the source. The
prepared MusicXML is the common renderer input, so the 41,610-event agreement
does not resolve that source-to-export difference. Thirteen prepared input hashes
also differ from the historical v47 preparation; page-count changes between
those runs are not attributed solely to the renderer.

Follow-up on the ea80a7d5 shortfall list found outgoing ties with no matching
destination pitch or a following rest. The measured renderer had either
connected one chord tie to an unrelated note or omitted the open end. It now
draws one short partial arc for each tied pitch without asserting a false
connection. In the four affected development scores, raw Acorde tie counts
changed from **15 to 18**, **8 to 10**, **39 to 41**, and **16 to 17**; source
start counts are respectively 18, 10, 40, and 17. A split tie can yield more
than one drawn segment, so this is a candidate-screen repair rather than a
complete tie-fidelity audit. The full development rerun and source check must
be repeated on the new candidate. The other mark shortfalls remain open.

## Development progress (2026-11-15, candidate v18)

The opt-in `font-metrics` renderer and `measured-print` PDF/WASM feature accept
caller-supplied font bytes. Rustybuzz measures and outlines the same shaped text.
The measured API chooses measured system breaks with the configured measure count
as an upper bound and derives actual system heights. The legacy fixed-count/density
selector and nominal system height do not select another algorithm inside this API.
The existing public legacy rendering path remains separate.

Measured rhythmic columns now reserve accidentals on their actual left side.
Per-system reservations consider horizontal overlap before adding adjacent staff
extents. A bounded final-placement pass reserves slurs, tuplets, lyric baselines,
directions and hairpins before pagination. This covers selected generated geometry,
not a proof against every possible collision.

PDF review exposed Harumi 1.23.8 filling text contours independently. The adapter
tessellates compound nonzero fills with pinned Lyon 1.0.22, preserving transparent
counters without painting white over surrounding notation. Regression tests check
painted area and absence of bridge strokes.

Source investigations also fixed empty/ranged written metronome text and surrounding
word order, numbered cross-staff arpeggios, additional-verse lyric connectors, and
placement of numbered slurs sharing endpoints. Playback tempo remains independent
of an empty written metronome value. The model additions have serde defaults;
their public Rust struct-literal compatibility requires a release-version decision.

The [v18 development screen](archive/print-parity-development-v18.json) contains
ten successful outputs: **16 pages / 38 systems**, versus Verovio's 16 / 40.
Staff space is 1.8 mm and lyrics are 4.05 mm. Page-edge, text/notehead, text/text,
lyric/staff, dynamic/staff and sampled text/curve screens (including hairpins)
report zero candidates for Acorde. These are screens, not correctness proofs.

The [assistant review](archive/print-parity-review-v18.json) records all 16 Acorde
PDF pages inspected at physical-scale equivalent. All 16 Verovio page image hashes
match the reference pages inspected earlier. No unresolved reading-obstructing
defect was observed on these development pages. The implementing assistant was
unblinded and familiar with these inputs; this is not an independent preference study.
Disclosed observations include lost second-verse italic styling in Aloha Oe,
OMR-1035's 2.66 mm instrument label, source wedges with missing/duplicate endpoints,
and authored-break policy differences. These were not silently removed from the corpus.

The [independent shared-MusicXML audit](archive/print-parity-source-audit-v18.json)
matches all **2,159 pitched events** in spelling, physical staff, source voice,
onset, duration, grace/cue/hidden state and per-pitch ties. It does not audit every
rest, text or visual mark. The MSCZ development inputs still use Acorde conversion
to shared MusicXML, so this is not independent MSCZ import-fidelity evidence.

The [native/WASM rerun](archive/print-parity-wasm-development-v18.json) matches all
ten canonical scores, layouts and SVG tokens within 0.0001 renderer units;
maximum numerical delta is 0.0000047461. Debug timings are not release benchmarks.
Chrome 157 headless execution passed 67 WASM tests in 20 suites, including actual
measured engraving with the supplied font. The current macOS all-feature workspace
run passed 2,286 tests in 94 suites, with one ignored test. An earlier attempt ran
out of disk during linking; its failure is retained separately from the successful
rerun after removal of this task's unused build caches. Six-crate all-target Clippy
passed. Linux and Windows execution remain pending.

All ten inputs also rendered with default `PrintConfig` inside the measured API
and with Letter paper; their automatic screens are zero. The four-measure default
produces 20 pages and Letter/eight-measure mode 16 pages. These are distinct
configurations, not claims about the unchanged legacy default renderer.
The [variant review](archive/print-parity-variants-review-v18.json) covers 67
pages, reusing exact raster hashes for unchanged A4 Verovio pages. Magnified
inspection exposed repeat dots too close to the barline strokes in OMR-1033 and
Freilachs. This supersedes the earlier no-obstruction observation for those
marks: v18 is not a development pass. Candidate v20 separates the strokes and
dots and increases the measured trailing reservation; its rerender is pending.

The release WASM development benchmark records two warmups and nine samples
per engine/input in separate Node processes on the same host. Acorde median
parse/render times range from 5.1 to 36.4 ms versus 8.6 to 82.5 ms for Verovio;
Acorde process maximum RSS is higher (251–422 MiB versus 207–245 MiB).
Concurrent host activity was uncontrolled, and Acorde also serializes canonical
JSON and outlines text. These observations do not establish strict performance
superiority or memory parity. The saved benchmark is the pre-repeat-fix v18
candidate and must be distinguished from the final frozen candidate.

The v18 status above is historical. The entire first 24-input reserved cohort was
opened under the v20 freeze and retired after that run failed, as recorded above.
Corrections informed by those inputs require fresh qualification, a disjoint final
reserved cohort, final-candidate performance/memory and cross-platform execution,
and final artifact integration. Current overall gates remain pending.

## Numbered-wedge development check (after 0bf6a641)

MusicXML could contain two numbered wedges starting at one note, while the legacy
`Note::hairpin_start` held only one. The parser now records each complete wedge as
a typed spanner. Layout uses the typed endpoints without drawing the legacy flag
again; rendering uses each span's placement. A wedge attached to a note beamed
across staves is anchored to that note's displayed staff. MusicXML export retains
number and placement. The release example rendered three previously deficient
development inputs from the same prepared XML and Tuffy font. The [working
receipt](archive/print-parity-hairpin-working-review.json) pins inputs, binary,
PDFs and the one page inspected here.

Raw Acorde hairpin path counts changed from 10 to 11 against 11 source starts
for the piano input; from 102 to 110 against 105 for the vocal input; and from
127 to 139 against 139 for the chamber input. The five extra vocal paths are
continuation segments for wedges crossing system or page breaks. A separate pair
of wedges shares a start note in measure 48. A part/measure start-count screen has no
remaining deficit in these three inputs. This does not prove exact endpoint,
side, timing or page-wide notation fidelity. The assistant inspected vocal page
3 after the cross-staff fix: both short wedges in measures 21 and 22 are now
visible, but their vertical placement and the large lower-page whitespace need
comparison with Verovio and review of the entire score. The full 24-input
development corpus, default/Letter configurations and final reserved cohort have
not been rerun for this candidate. All four gates remain pending.

The vocal score currently has 25 Acorde systems on 13 A4 pages, versus 23
Verovio systems on 8 pages. Measured Acorde system heights on this input range
from about 85 to 106 mm within a 265 mm content height, so many pages cannot
hold a third system at the present staff size and spacing. A local trial reducing
the initial gap for scores with at least three staves from 6 to 4.5 staff spaces
still produced 13 pages; the trial was reverted. The remaining page-use gap
requires source-aware ink and inter-staff geometry review.
