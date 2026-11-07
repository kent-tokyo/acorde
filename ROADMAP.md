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

### 1. Engraving and print: Verovio 6.4.1

The [ten-score piano/chamber corpus](docs/archive/print-corpus-verovio.md)
checks PDF page geometry, marks, collisions and font/staff sizes against the
previously pinned Verovio 6.3.0. First rerun the same inputs and options with
[stable Verovio 6.4.1](https://github.com/rism-digital/verovio/releases/tag/version-6.4.1)
and save a separate versioned result. The next implementation work is to raise
the small staff and text sizes without introducing overlaps, then address
remaining page-use and missing-mark cases.
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
reference comes from Acorde. The independent written-onset audit in
[PR #407](https://github.com/kent-tokyo/acorde/pull/407) covers four source
scores: Acorde matched 1,536/1,536 pitch/beat attacks and alphaTab 1.9.0
matched 1,093/1,536; the 443 differences are split-bar timing in one score.
The audit also corrects a whole-measure-rest fermata hold. Expand the source
oracle beyond these simple forms, then recheck note onset/release, tempo,
repeats, pedal, ornaments and instrument changes on sparse diagnostic excerpts
before using an audio onset detector on dense scores. Track import losses
separately.

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
has 82 chorales with explicit labels, equal imported note/chord event counts
and successful parses in both tools: music21 selected 76 correct keys, while
Acorde's legacy first candidate selected 46.
[PR #403](https://github.com/kent-tokyo/acorde/pull/403)
adds an opt-in tonal-profile ranking;
its development-corpus result must not be treated as holdout accuracy.

First land [PR #405](https://github.com/kent-tokyo/acorde/pull/405), which
raises paired parse and note/chord event-count agreement to 98/100 and 98/98
respectively on this development corpus. Unordered pitch/duration event bags
also match on 98/98; audit order, part/staff/voice/measure placement, rests,
spelling and ties before treating this as semantic equality. The separately
frozen [Mozart sonata holdout in PR #406](https://github.com/kent-tokyo/acorde/pull/406)
records 53 primary movements and 52 common
MusicXML inputs, 50 annotated keys selected by Acorde's ranked first choice
versus 51 by music21. MuseScore failed to export K533-3, so first reproduce the
full corpus with a stable converter and inspect the Acorde-only K331-2 error
without tuning on this holdout. Then add independently annotated harmony,
cadence and voice-leading fixtures;
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
