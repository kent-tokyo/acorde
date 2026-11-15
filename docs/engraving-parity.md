# Engraving and print parity protocol

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
