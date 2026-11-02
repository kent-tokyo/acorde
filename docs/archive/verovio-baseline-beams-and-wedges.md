# Archived evidence

This historical record is linked from the [evidence index](../verovio-engraving-baseline.md).

### Chord heads and beam clearance (2026-11-01)

Self-authored MusicXML pairs with two-octave chords `C4+C6` and `D4+D6`,
with explicitly up and down stems, were rendered by Verovio 6.3.0. In both
directions its beam lies beyond the chord head closest to the beam; the stem
also connects to the opposite outer head. A two-staff probe with a down-stem
`C5+E5` chord and an up-stem `G3+B3` chord likewise keeps both chords between
their staff and the shared beam.

Acorde had used the stem's starting head for both drawing and beam clearance.
That could let a beam cross another chord head. It now uses the beam-side head
for beam planning and vertical extent calculation, while retaining the other
head as the stem's starting point. Screen regressions cover both stem
directions and cross-staff chords; a print-page SVG regression covers the
cross-staff example. Very tight staff gaps may still be infeasible because
staff separation is not yet automatically expanded for kneed beams. This
comparison does not establish equal cross-staff slope or exact stem lengths.

### Two-note cross-staff beam slope (2026-11-01)

The pinned `verovio_cross_staff_pair.musicxml` has a down-stem C5 on the
upper staff followed by an up-stem G3 on the lower staff. Verovio 6.3.0's
beam polygon rises by one staff space from left to right in SVG coordinates;
the tips end on opposite sides of the beam, 0.75 space apart. Reversing the
staff order reverses the direction in additional self-authored probes.
Different pitches gave the same result for these single-level two-note beams.

Acorde now inclines the beam center by one staff space for these pairs when
there is enough room for two-space clearance on both sides. If that slope
would violate clearance it retains the horizontal placement. In the pinned
Verovio probe and its reversed-staff variant, the down stem ends at the beam's
lower edge and the up stem at its center. Acorde now does the same for
single-level two-note cross-staff beams; the endpoint difference in the sloped pair is
0.75 staff space. Screen and print tests cover the shape, chords, both orders,
edge attachment, and the fallback.
This does not establish the correct slope for longer or system-spanning beams,
nor automatically increase a too-small staff gap.

### Two-note cross-staff secondary beams (2026-11-01)

The pinned `verovio_cross_staff_sixteenths.musicxml` uses the same notes
as the eighth-note cross-staff probe, with both notes changed to sixteenths.
Verovio 6.3.0 draws two parallel sloped beams, with their corresponding edges
0.75 staff space apart vertically at the same x. The down-stem on the first
note reaches the lower edge of the primary beam, while the up-stem on the
second reaches the top edge of the secondary beam. Reversing the note order
places the secondary beam on the other side. Additional self-authored 32nd
note probes have three parallel beams with the same separation.

Acorde now follows these relationships for cross-staff pairs using eighth-,
sixteenth-, or 32nd-note beam levels. It considers the
secondary beams when checking the required staff gap, and retains a horizontal
primary beam when the sloped placement would not fit. Screen and print SVG
tests cover the pinned sixteenth-note input; beam-planner tests cover both
orders, the 32nd-note case, edge positions and parallel spacing. Absolute stem
lengths, longer groups, and system-spanning beams
remain unverified against the reference.

### Cross-staff partial beams with unequal durations (2026-11-01)

In `verovio_cross_staff_mixed_durations.musicxml`, an upper-staff eighth
note joins a lower-staff sixteenth note. Verovio 6.3.0 slopes the primary
beam by one staff space and draws a backward hook toward the shorter note's
head. The hook is about 1.25 staff spaces long and its corresponding edge is
0.75 staff space from the primary beam at the same x. Mirrored staff order,
short note first, and a single 32nd note were also checked with self-authored
MusicXML probes. Their hooks point toward the other note and lie on the short
note's head side. Explicitly authored hook orientations other than these
examples have not been compared.

Acorde now applies this geometry to two-note cross-staff groups with these
beam levels and accounts for the hooks in the staff-gap clearance check.
Screen and print regressions exercise the pinned MusicXML input, and planner
tests cover both staff orders, either short-note position, and 32nd notes.
Absolute stem lengths and groups spanning measures or systems remain open.

## Gates before a quality claim

1. Keep page SVG/PDF generation and cross-page span continuation covered by tests.
2. Improve lyric size and publication
   typography against the visual baseline.
3. Pin a larger licensed piano/chamber corpus and compare missing symbols,
   clipped content, collisions, page geometry, and legibility at print size.
4. Run blind human review of the resulting PDFs and record the rubric and
   individual scores. Do not infer superiority from page count or preflight.

### Inter-staff wedge and high lower-staff notes

The self-authored `tests/fixtures/issue_295_interstaff_hairpin.musicxml` places a crescendo
between piano staves and high notes on the lower staff. On this single deliberately extreme
example, Acorde and Verovio 6.3.0 both place the wedge clear of the noteheads and stems. Acorde's
inter-staff space is visibly larger; this comparison does not establish engraving parity or
performance on ordinary scores. A default Acorde page SVG, produced by `compute_print_layout`
and `render_print_page_svgs`, was also visually checked on the same input and did not show the
wedge crossing a notehead or stem. This is not a size-matched print-quality comparison. The
screen outputs can be reproduced with:

```sh
cargo run -p acorde-cli --locked -- render --width 1000 \
  tests/fixtures/issue_295_interstaff_hairpin.musicxml /tmp/acorde-interstaff.svg
verovio -o /tmp/verovio-interstaff.svg tests/fixtures/issue_295_interstaff_hairpin.musicxml
```
