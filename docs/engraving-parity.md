# Engraving and print parity protocol

This page summarizes how Acorde's print output is compared with Verovio and which gates remain
open. The [full protocol and dated evidence](engraving-parity-history.md) keep the exact inputs,
hashes, commands, and per-score results. The [roadmap](../ROADMAP.md#1-engraving-and-print-verovio-641)
tracks current priorities. These development observations do not establish general parity with
Verovio.

## Current status

All four gates below are pending. The first reserved cohort (24 inputs) failed its frozen run
and is now development data; a disjoint frozen cohort is required before a broader parity claim.
Following the user's 2026-11-15 instruction ([#390](https://github.com/kent-tokyo/acorde/issues/390)),
the implementing assistant reviews every page at print size. That review is not independent
evidence. Page count alone is not a quality score.

## How a comparison is run

- **Inputs.** Development uses the ten licensed inputs of the
  [6.4.1 comparison](archive/print-corpus-verovio-6-4-1.md) and the retired 24-input cohort.
  Reserved inputs are pinned by upstream revision and Git blob, prepared without Acorde, and
  frozen before algorithm changes. See [Fixed inputs and boundaries](engraving-parity-history.md#fixed-inputs-and-boundaries).
- **Paper and sizes.** A4 portrait with 14 mm horizontal and 16 mm vertical margins, a separate
  Letter check, a matched 1.8 mm staff space, and the public default configuration evaluated on
  its own. Typography floors are 4.05 mm for lyrics, 3.24 mm for expression/tempo text and chord
  names, and 2.7 mm for measure and ending numbers. See
  [Physical comparison and success conditions](engraving-parity-history.md#physical-comparison-and-success-conditions).
- **Order of work.** Measured text and glyph bounds, measured system breaks, per-system collision
  resolution before pagination, shared SVG/PDF placement, then default and matched-size development
  runs before a frozen reserved evaluation. See [Implementation sequence](engraving-parity-history.md#implementation-sequence).

## Gates still to pass

1. **Notation correctness.** No unexplained omitted or wrong essential marks, wrong pitch/voice
   association, page-edge clipping, or confirmed reading-obstructing collisions on any input.
2. **Legibility.** Requested staff space and typography floors hold without hiding marks, with no
   unexplained near-empty final pages or excessive inter-system stretching.
3. **Assistant review.** Per score and page, no unresolved material Acorde disadvantage in source
   fidelity, spacing, typography, collisions, or page use.
4. **Generalization and reproducibility.** Each of the four input groups passes on a frozen reserved
   cohort, with native/WASM placement agreement, macOS/Linux/Windows tests, and same-host
   performance and memory measurements.

The full wording of each gate is in [the protocol](engraving-parity-history.md#physical-comparison-and-success-conditions).

## Evidence index

### 2026-11-21 integration checkpoint and follow-ups

Start with the [checkpoint itself](engraving-parity-history.md#2026-11-21-integration-checkpoint). Follow-up
observations by topic:

| Topic | Records |
| --- | --- |
| Page layout and balancing | [Aloha Oe page review, articulation-reflow candidate](engraving-parity-history.md#aloha-oe-page-review-articulation-reflow-candidate), [Balancing a one-system final page](engraving-parity-history.md#balancing-a-one-system-final-page), [One-shot staff-gap relaxation](engraving-parity-history.md#one-shot-staff-gap-relaxation), [Page balancing with explicit breaks](engraving-parity-history.md#page-balancing-with-explicit-breaks), [MusicXML print break boundaries](engraving-parity-history.md#musicxml-print-break-boundaries), [Reuse of measured row geometry](engraving-parity-history.md#reuse-of-measured-row-geometry) |
| Part names, labels and groups | [Part-name sizing experiment](engraving-parity-history.md#part-name-sizing-experiment), [Opening-system label inset](engraving-parity-history.md#opening-system-label-inset), [Page-start label insets](engraving-parity-history.md#page-start-label-insets), [First-page title credits](engraving-parity-history.md#first-page-title-credits), [Hidden part names and symbol-free groups](engraving-parity-history.md#hidden-part-names-and-symbol-free-groups), [MusicXML multistaff part symbols](engraving-parity-history.md#musicxml-multistaff-part-symbols), [Grouped barlines](engraving-parity-history.md#grouped-barlines), [Harpsichord grand-staff brace and page review](engraving-parity-history.md#harpsichord-grand-staff-brace-and-page-review), [Part-name margins and lyric/beam clearance](engraving-parity-history.md#part-name-margins-and-lyricbeam-clearance) |
| Text, directions and rehearsal marks | [Text clearance from spanning barlines](engraving-parity-history.md#text-clearance-from-spanning-barlines), [MusicXML rectangular text enclosures](engraving-parity-history.md#musicxml-rectangular-text-enclosures), [Boxed direction and owning measure](engraving-parity-history.md#boxed-direction-and-owning-measure), [Default rehearsal-mark enclosure](engraving-parity-history.md#default-rehearsal-mark-enclosure), [Multiple rehearsal directions in one measure](engraving-parity-history.md#multiple-rehearsal-directions-in-one-measure), [Rehearsal text components inside a direction](engraving-parity-history.md#rehearsal-text-components-inside-a-direction), [Rerender after rehearsal import changes](engraving-parity-history.md#rerender-after-rehearsal-import-changes), [Staff ownership of written directions](engraving-parity-history.md#staff-ownership-of-written-directions), [Outlined text in collision screening](engraving-parity-history.md#outlined-text-in-collision-screening) |
| Lyrics | [Lyric alignment under voice displacement](engraving-parity-history.md#lyric-alignment-under-voice-displacement), [Lyric-side data preservation](engraving-parity-history.md#lyric-side-data-preservation), [Above-staff lyrics in measured print](engraving-parity-history.md#above-staff-lyrics-in-measured-print), [Cross-staff lyric continuation checkpoint](engraving-parity-history.md#cross-staff-lyric-continuation-checkpoint), [Lyric extenders that change staff within a system](engraving-parity-history.md#lyric-extenders-that-change-staff-within-a-system) |
| Voices and ties | [Displaced voice spacing](engraving-parity-history.md#displaced-voice-spacing), [Tie starts at system breaks and voice changes](engraving-parity-history.md#tie-starts-at-system-breaks-and-voice-changes), [Voice-changing ties when the source voice rests](engraving-parity-history.md#voice-changing-ties-when-the-source-voice-rests), [Ties after a voice ends](engraving-parity-history.md#ties-after-a-voice-ends) |
| Test infrastructure | [Page-fragment parsing and default tests](engraving-parity-history.md#page-fragment-parsing-and-default-tests) |

### Earlier candidates

- [Baseline observations](engraving-parity-history.md#baseline-observations)
- [2026-11-15, candidate v18](engraving-parity-history.md#development-progress-2026-11-15-candidate-v18)
- [Candidate v41: retired-cohort development rerun](engraving-parity-history.md#retired-cohort-development-rerun-candidate-v41)
- [Candidate v47: authored dynamic placement and integrated checks](engraving-parity-history.md#authored-dynamic-placement-and-integrated-checks-candidate-v47)
- [Candidates v47–v49: equal-size rerun and hidden notation repairs](engraving-parity-history.md#equal-size-rerun-and-hidden-notation-repairs-candidates-v47v49)
- [Post-v49: Gossec page correction under review](engraving-parity-history.md#gossec-page-correction-under-review-post-v49-working-candidate)
- [After 0bf6a641: numbered-wedge development check](engraving-parity-history.md#numbered-wedge-development-check-after-0bf6a641)
