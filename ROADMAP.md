# Acorde roadmap

This roadmap tracks work still needing evidence or implementation. The
[scorecard](docs/scorecard.md) inventories current contracts, the
[notation coverage matrix](docs/notation-coverage.md) describes tested format support, and the
[changelog](CHANGELOG.md) records releases. Issue acceptance criteria remain authoritative.

## Current baseline

Version 1.3.10 includes a Rust score model with undoable commands, bounded notation I/O,
SVG and print-page rendering, playback scheduling, analysis primitives, WASM bindings,
and the file-based CLI. MusicXML/MXL is the broadest interchange path; Humdrum `**kern` and
Plaine & Easie import are opt-in subsets. Import and export reports identify known losses,
but broad real-score round-trip compatibility remains an evidence question.

The CLI also provides `acorde mcp --root <dir>` over local stdio. Its inspection and
validation tools are read-only by default. Reviewed editing uses sessions, a validated
preview, undo/redo, and an explicit destination; file writing requires `--allow-write`.
The score-model and WASM library crates remain independent of the MCP transport and host
file access. A hosted HTTP MCP service is not part of this release.

## Competitor parity program

Compare pinned, stable competitors on the same licensed input and fixed output
settings. Keep import losses separate from each engine's layout, scheduling or
analysis errors. Save per-score results and commands. An improvement on the
development corpus needs a separate, frozen holdout before a general parity claim.

### 1. Engraving and print: Verovio 6.3.0

The [ten-score piano/chamber corpus](docs/archive/print-corpus-verovio.md)
already checks PDF page geometry, marks, collisions and font/staff sizes. The
next implementation work is to raise the small staff and text sizes without
introducing overlaps, then address remaining page-use and missing-mark cases.
Compare each revision on the same A4 inputs and verify suspicious collision
counts in the PDFs, including slurs, beams, lyrics and directions.

Complete the randomized, print-size A/B review with at least two independent
reviewers ([#390](https://github.com/kent-tokyo/acorde/issues/390)). Record
readability, spacing, typography, collisions, paper use and missing/wrong marks
per score and reviewer. The initial gate is no confirmed page-edge clipping or
unexplained missing marks on the fixed corpus, followed by no systematic
Verovio preference in the blind review. Repeat on an untouched piano/chamber
holdout before claiming broader parity. Page count alone is not a quality score.

### 2. Playback and audio: alphaTab 1.9.0

The [eight-score 1.9.0 observation in PR #401](https://github.com/kent-tokyo/acorde/pull/401)
uses the same scores, soundfont, sample rate and channel count. Its timing
reference comes from Acorde, so first build an independent expected-event
timeline from the source notation. Recheck note onset/release, tempo, repeats,
pedal, ornaments and instrument changes on sparse diagnostic excerpts before
using an audio onset detector on dense scores. Track import losses separately.

Then compare level-matched, complete audio outputs for clipping, truncation,
stuck notes and audible quality. Complete the existing 1.8.4 blind-listening
gate ([#388](https://github.com/kent-tokyo/acorde/issues/388)) and run a new
randomized A/B evaluation against 1.9.0 with at least two listeners. Initial
parity requires no missing or wrongly timed source events on the fixed corpus,
no clipped or prematurely ended output, and no systematic alphaTab preference
in the blind review. Recheck on an untouched holdout with instrument changes
and expressive playback before a broader claim.

### 3. Music analysis: music21 10.5.2

The [BCMH global-key baseline in PR #402](https://github.com/kent-tokyo/acorde/pull/402)
has 80 chorales with explicit labels, equal imported note counts and successful
parses in both tools: music21 selected 74 correct keys, while Acorde's legacy
first candidate selected 44. [PR #403](https://github.com/kent-tokyo/acorde/pull/403)
adds an opt-in tonal-profile ranking;
its development-corpus result must not be treated as holdout accuracy.

First resolve Acorde's eight additional parse failures
([PR #405](https://github.com/kent-tokyo/acorde/pull/405)) and four paired
note-count differences. Freeze a different labeled key corpus before tuning
further, and compare a single selected key per score with parse coverage shown.
Next add independently annotated harmony, cadence and voice-leading fixtures;
report precision, recall and error cases per task against music21 under the
same input notes. Task-scoped parity requires Acorde to match or exceed
music21 on each frozen holdout metric without lower parse coverage. A global
key tie on one development corpus is not overall analysis parity.

## Other evidence gates

Expand licensed real-score fixtures and external interoperability checks before
claiming broad format equivalence or practical standard-bass playability.
Preserve source-pitch omissions, harmony provenance, physical button actions,
and performer-review status in each result.

## MCP follow-up

Keep the local tool schemas and diagnostics aligned with the underlying Acorde APIs.
Test tool discovery and calls against a pinned score whenever the CLI or model changes.
Add remote transport only with an explicit host use case, authentication, per-request
authorization, and resource limits.

## Maintenance

Update this page when an issue's acceptance criteria are verified. Record implemented
behavior in the scorecard and coverage matrix, release changes in the changelog,
and fixed-corpus measurements in their evidence documents. A result on one corpus does
not establish general compatibility, engraving parity, or audio superiority.
