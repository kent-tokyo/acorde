# Archived evidence

This historical record is linked from the [evidence index](../verovio-engraving-baseline.md).

### Verovio-guided below-staff direction clearance (2026-10-31)

The same Quilter MXL puts `rall.` between the two piano staves in measure 9.
Verovio separates the word from nearby noteheads. In Acorde's page SVG,
the word started at x=324.85 and its conservative text box touched three
noteheads. The print renderer now checks its own low chord heads after vertical
placement and may shift a below-staff word right by at most three staff spaces.
On this score, `rall.` moves to x=342.89 (4.51 mm), with the same y=991.94;
the three box overlaps fall to zero. A fixture regression checks all page
noteheads against the word box. The longer `poco cresc.` and `poco rall.`
stay at their prior x positions rather than moving to the far end of a bar.

The one-page A4 Quilter PDF was rasterized at 1400 px and inspected after the
change. The three-page A4 *Aloha Oe* density PDF was regenerated; its first
page was inspected, with no new visible text/staff collision. The import
diagnostics remain 103 and 73 respectively. This narrow clearance result
does not establish complete collision freedom or Verovio-level engraving.

### Verovio-guided written metronome mark (2026-10-31)

The pinned CC0 Quilter Prelude writes `Moderato tranquillo e con tenerezza`
followed by a parenthesized quarter-note `=69`. Verovio 6.3.0 prints both on
the same opening line. Acorde already read the `<sound tempo="69"/>` playback
value, but discarded the separate MusicXML `<metronome>` notation. The typed
model now keeps its beat unit, dots, numeric rate, and parentheses. MusicXML
roundtrip and score patch preserve those fields; a mark with no `<sound>` stays
visual notation without an invented playback value.

The density A4 PDF still has one page with four systems (2+2+3+3), compared
with Verovio's three systems on the same source and paper. At 1400 px, the
new quarter-note equation is visible beside the opening words without touching
the first staff. The importer still reports 103 diagnostics. This closes one
missing printed symbol; note size, direction typography, spacing, and broad
print quality remain below the Verovio reference on this score.

### Verovio-guided common and cut meter glyphs (2026-10-31)

The pinned Quilter MXL has `<time symbol="common"><beats>4</beats><beat-type>4</beat-type>`.
Verovio 6.3.0 prints C on both piano staves; Acorde previously printed 4/4.
The [MusicXML time-symbol contract](https://www.w3.org/2021/06/musicxml40/tutorial/notation-basics/)
distinguishes the written common or cut sign from its 4/4 or 2/2 rhythmic
value. Acorde now keeps that distinction in the score, MusicXML roundtrip,
diff/patch, and SVG/PDF renderer. A change to cut time inside one system also
gets a dedicated aligned meter column. Unsupported or mismatched symbols are
reported on import and rendered as numerals rather than guessed.

The revised Quilter A4 PDF was rasterized at 1400 px and inspected. C appears
on both opening staves without touching the first notes. The page is still one
page with four systems (2+2+3+3), while the same-input Verovio page has three;
the 103 import diagnostics are unchanged. This fixes one notation mismatch,
not the remaining differences in note size, line filling, and typography.

### Verovio-guided slur side and taper after v1.3.7 (2026-10-31)

The same Quilter A4 input starts a long upper-staff slur with positive MusicXML
`bezier-y` but no explicit `placement`. Verovio 6.3.0 bows it above the notes;
Acorde v1.3.7 used the up-stem fallback and drew it below, through the phrase.
The importer now uses the starting Bezier control's sign only when `placement`
is absent; an explicit side still wins. The first lower-staff slur has a
negative control and remains below. MusicXML export/re-import retains both
resolved sides. The exact authored curve controls are not preserved.

The page SVG now draws a closed, tapered quadratic bow for both same-system
slurs and continuations. The updated A4 PDF was rasterized at 1400 px and
inspected against the same Verovio page. Quilter remains one page with four
systems (2+2+3+3), compared with Verovio's three. The import still reports
103 diagnostics. This fixes the direction and visibility of one prominent
slur; it does not prove clearance for every slur or print-quality parity.

### Verovio-guided partial beam stems and slurs after v1.3.7 (2026-10-31)

The self-authored MusicXML probe at
`crates/render-svg/tests/fixtures/verovio_partial_stem_slur.musicxml` has two
high eighth notes under one beam and slur. Only the first note explicitly says
`<stem>up</stem>`. Verovio 6.3.0 draws **both** stems up and the slur below.
The mirrored low-note probe with one `<stem>down</stem>` draws both stems down
and the slur above. Moving the sole up-stem instruction to the last note also
keeps both stems up in Verovio. Before this change Acorde drew the first probe
with one up stem and one down stem under the same beam.

The renderer now propagates a consistent authored direction to the other
unmarked notes in that beam. Explicit conflicting directions remain untouched.
The four up/down and first/last variants have screen-SVG assertions; the pinned
upper probe also has a physical print-page SVG assertion. The comparison SVGs
were rasterized and inspected. This corrects the beam/stem relation and keeps
the slur on the same side as Verovio. Acorde stretches this isolated one-bar
system much wider, making its slur longer; this probe does not establish equal
horizontal spacing or general slur geometry.

```sh
verovio -o /tmp/verovio-partial-stem.svg \
  crates/render-svg/tests/fixtures/verovio_partial_stem_slur.musicxml
cargo run -q -p acorde-cli --locked -- render \
  crates/render-svg/tests/fixtures/verovio_partial_stem_slur.musicxml \
  /tmp/acorde-partial-stem.svg --width 900 --staff-size 24
```

### Verovio-guided short final print system after v1.3.7 (2026-10-31)

The same self-authored MusicXML probe has only one short bar. On A4 with
16/14 mm margins, Verovio 6.3.0 gives its staff a 21.96 mm width. Before this
change, Acorde's one-system print SVG stretched it to 176.60 mm. The print
renderer now sizes a short final row from its notation demand and, when there
are preceding rows, their density. It still justifies earlier rows and a
final row with the same number of columns as the fullest preceding row. The
probe's Acorde staff is now 24.51 mm, with both notes and the slur inside it.
The print SVG and vector PDF were generated from the same input, and the PDF
was rasterized at 1400 px for inspection. The import has zero diagnostics.

The pinned *Aloha Oe* density example remains three A4 pages and five systems,
with 73 import diagnostics. Its last page was rasterized at 1400 px; the
changed rule did not visibly crowd that page. A dense single-measure ending
has a regression that leaves its final line full width. These checks concern
horizontal extent only: the probe's Acorde staff and publication type are
still smaller than Verovio's, and the two renderers do not yet have matching
overall print quality.

```sh
verovio -a --page-width 2100 --page-height 2970 \
  --page-margin-top 160 --page-margin-bottom 160 \
  --page-margin-left 140 --page-margin-right 140 \
  -o /tmp/verovio-partial-a4.svg \
  crates/render-svg/tests/fixtures/verovio_partial_stem_slur.musicxml
cargo run -q -p acorde-pdf --example print_mxl -- \
  crates/render-svg/tests/fixtures/verovio_partial_stem_slur.musicxml \
  crates/pdf/tests/fixtures/Tuffy.ttf /tmp/acorde-partial-a4.pdf
```

### Slur clearance at the middle of a phrase (2026-10-31)

For the self-authored ABC phrase `X:1`, `M:3/4`, `L:1/4`, `K:C`,
`(c a c) |]`, Acorde exported MusicXML and Verovio 6.3.0 rendered the
same file. Verovio stems the three notes down and bows the slur above the
central high note. Acorde previously put the quadratic control above that
note, but the actual curve crossed it. The control calculation now accounts
for the curve's midpoint. Regression tests evaluate the curve at the central
note for both a high peak and a low valley. This is a midpoint clearance
improvement; it does not guarantee collision-free slurs for off-center notes,
stems, chords, or system continuations.

### Slur clearance away from the middle (2026-10-31)

The self-authored 5/4 ABC phrase `(c a c c c) |]` was converted to MusicXML
and rendered by both Acorde and Verovio 6.3.0. Verovio's slur bows above the
second note; Acorde's prior midpoint-only calculation passed within 0.1 staff
space of that notehead in screen SVG. Acorde now evaluates each intermediate
same-voice note at its horizontal position on the quadratic bow, and adjusts
the control point to seek 0.8 staff spaces of clearance. Additional curvature
is capped at eight staff spaces, so very crowded phrases remain best-effort.
Screen tests cover peaks and valleys on either side of the midpoint; a physical
page SVG test covers the high second note. The matching A4 PDF was rasterized
at 1400 px and inspected. This does not solve collisions with stems, chord
extremes, other voices, or cross-system continuation segments.

The pinned Quilter Prelude was also regenerated in density mode: it remains
one A4 page with four systems and 103 import diagnostics. The PDF was
rasterized and inspected; the comparison does not establish general
collision-free or Verovio-equivalent typography and spacing.

### Slurs with mixed endpoint stems (2026-10-31)

Self-authored single-voice quarter-note MusicXML phrases `C4 C4 C5` and
`C5 C4 C4` were rendered by Verovio 6.3.0. Their endpoint stems point in
opposite directions, and both Verovio slurs bow above the notes. Acorde used
to select the side from the first stem alone, so it placed the ascending
example below. It now places mixed-endpoint single-voice slurs above, while
preserving explicit slur placement and the separate lower-voice rule. Screen
tests cover both directions and explicit placement; a page SVG test covers
the ascending case.

### Mixed stems inside a slur (2026-11-01)

Verovio 6.3.0 puts the slur above `C4 C5 C4`, whose endpoints stem up but
whose interior stem points down. The slur goes below in `C4 G4 C4` and in
`C4 rest C4`. Acorde now inspects intervening same-voice, same-staff notes and
uses the upper side when they include a down stem. Rests, stemless durations,
other voices, and cross-staff notes do not count as interior stems; explicit
slur placement still takes precedence. Screen regressions cover these cases
and a system break, and a page SVG regression covers `C4 C5 C4`.

These comparisons concern side selection for a few simple phrases; matching
Verovio's curve shape, stem clearance, chords, cross-staff notation, and
system-break geometry needs further comparisons.

### Chord stems with symmetric outer notes (2026-11-01)

In self-authored treble-clef quarter-note MusicXML, Verovio 6.3.0 stems the
chord `E4 F5 A4` up, but `E4 F5 C5` down. Its `E4 F5 A4 C5` stems down,
and `E4 F5 G4 C5` stems up. The outer E4 and F5 are equally distant from the
middle line in each case; the next outer pair determines the direction, with
an exact tie going down. Acorde previously always chose down when the outer
pair tied. Its isolated chord rule now examines successive pairs, and the
four examples are covered by screen SVG tests; the first also has a print
page regression.

The corresponding three eighth notes `E4 F5 G4`, explicitly beamed as one
group, all stem down in Verovio. Acorde retains its existing separate beam
group rule, with a regression for this example. These probes do not establish
the behavior of all chord and beam configurations or cross-staff notation.

### Beam clearance at large pitch leaps (2026-11-01)

Self-authored MusicXML eighth-note beams `C4 C6 C4` with explicitly upward
stems, and `C6 C4 C6` with explicitly downward stems, were rendered in
Verovio 6.3.0. Both beams stay on the requested side of every notehead. In
Acorde, the old clearance calculation used the absolute stem length, so an
interior head far beyond the beam could be mistaken for a sufficiently long
stem. Clearance now uses signed distance and shifts the beam to the correct
side. Unit and screen tests cover both directions; screen tests also exercise
the slur, and a page SVG test verifies stem direction in print. This checks
clearance, not equality of beam angle, stem length, or slur shape with Verovio.

### Horizontal beams for interior extrema (2026-11-01)

Verovio 6.3.0 renders the explicitly up-stemmed eighth-note group `C4 E4 D4`
with a horizontal beam; it slopes the monotonic `C4 D4 E4` group. Mirrored
down-stem groups `C5 A4 B4` and `C5 B4 A4` show the corresponding flat and
sloped behavior. Verovio also flattens `C4 D4 D4` with up-stems and
`C5 B4 B4` with down-stems. These are self-authored MusicXML probes.

Acorde previously calculated slope solely from the endpoints. It now flattens
when an interior stem-side head reaches or passes the outer endpoint on the
beam side. Tests cover both directions, equal-height plateaus, monotonic
groups, a slur, and physical page SVG. This is a limited contour rule: the
reference probes do not establish matching beam angles, exact stem lengths,
or behavior for all chords, cross-staff beams, and secondary levels.

### Two-note beam rise and remaining reference differences (2026-11-01)

In self-authored treble-clef MusicXML eighth-note pairs, Verovio 6.3.0 changed
the beam height by 0.25 staff space for C4–D4, D4–C4, C4–F4, F4–C4,
C5–D5, and C6–C3, with the sign following the pitch contour; equal notes
produced a horizontal beam. The probes included explicitly up and down stems.
This is a height difference, not a fixed angle: the angle also depends on the
horizontal distance. Acorde now clamps the rise of two-note groups to 0.25
staff space for eighth and sixteenth-note groups with short spacing. For C4–G4, Verovio uses
one staff space of rise with 32nd and 64th notes, so Acorde retains its prior
one-space cap for three or more beam levels. The
`verovio_partial_stem_slur.musicxml` reference is tested at two screen widths
and on a print page. Further inspection of the pinned Verovio source and a
justified C4–G4 probe show that the quarter-space step is limited to pairs
whose horizontal distance is at most three staff spaces: with a wider pair,
Verovio used one staff-space rise. Acorde now uses that spacing threshold
instead of applying the short step to all pairs. The broader sample has not
established that the rule matches all beam levels, pitches, or spacings.
Reference: [Verovio 6.3.0 beam geometry](https://github.com/rism-digital/verovio/blob/version-6.3.0/src/beam.cpp).

The same reference comparison exposes a remaining stem-length difference:
for down-stemmed E4–F5, Verovio's stems measured approximately 2.49 and 6.24
staff spaces, while the Acorde visual-regression case reaches approximately
3.5 and 7.25. The latter is still bounded, but is not a Verovio match. Stem
length changes with pitch and direction, so the available measurements do not
justify replacing the general length rule with a constant.

For a two-staff piano group with upper-staff C5, D5 and lower-staff G3, A3,
Verovio 6.3.0 produced a sloping cross-staff beam with a 0.75-space rise
between the first and last stem tips. This four-note case is still horizontal
in Acorde. Another alternating-staff probe produced a flat Verovio beam,
so a universal cross-staff slope would be unsupported. In addition,
`collect_beam_groups` currently collects within each measure, so a group
continuing across a barline or system is not represented as one cross-measure
beam. These cases remain open.
