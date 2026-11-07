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

### Execution order and release gates

1. Freeze the input hashes, competitor tags, A4/audio settings, annotations and
   per-score reports. Add untouched piano/chamber, expressive-playback and
   analysis holdouts before changing algorithms. Check parsed score semantics
   first, so an import loss is not counted as an engraving, playback or
   analysis error.
2. Fix print readability and missing marks on the ten-score corpus, then run
   the blind A/B print review and the piano/chamber holdout. The print gate is
   no confirmed clipping or unexplained missing marks, no systematic Verovio
   preference, and measured staff/text sizes and collisions reported per score.
3. Expand the independent written-event oracle to releases, repeats, pedal,
   ornaments, tempo and instrument changes. Repair source-event differences
   before comparing level-matched complete audio. The playback gate is no
   missing or wrongly timed source events, no clipped/truncated/stuck output,
   and no systematic alphaTab preference in the blinded listening review and
   expressive holdout.
4. Audit imported musical order, voice placement, spelling, rests and ties;
   then improve global key, harmony, cadence and voice-leading as separate
   tasks. The analysis gate is at least music21's result on each task's frozen
   labeled holdout with no lower parse coverage. Keep development-corpus
   scores separate from holdout results.

Track each failure as a small reproducible score and an issue linked to its
per-score report. If a holdout is inspected to design a fix, treat it as
development data and freeze a new untouched holdout for the final gate. The
three gates are independent; an average score cannot substitute for any one.

### 1. Engraving and print: Verovio 6.4.1

The [ten-score 6.3.0 corpus](docs/archive/print-corpus-verovio.md) and
[6.4.1 rerun](docs/archive/print-corpus-verovio-6-4-1.md) measure the same
licensed inputs on A4. In the 6.4.1 screening, Acorde uses 18 pages and 57
systems versus Verovio's 16 pages and 40 systems; Acorde's smallest measured
staff space was 1.29 mm versus Verovio's 1.8 mm. A
[staff-size follow-up](docs/archive/print-staff-size-verovio-6-4-1.md) improves
seven scores without changing the 18 pages or 57 systems; Acorde's smallest
measured staff space was 1.50 mm. An
[opt-in eight-measure density run](docs/archive/print-density-ensemble-verovio-6-4-1.md)
uses 18 pages and 54 systems, with a smallest staff space of 1.65 mm; OMR 1036
reaches Verovio's two pages, four systems and 1.8 mm staff space. The public
four-measure default is unchanged. A
[direction-text follow-up](docs/archive/print-direction-text-verovio-6-4-1.md)
preserves two real-score `other-dynamics` prose directions in SVG and PDF,
and corrects the comparison's empty-hairpin count. A
[lyric legibility follow-up](docs/archive/print-lyric-legibility-verovio-6-4-1.md)
raises measured lyrics on four scores to 3.08–3.42 mm without adding pages,
but Verovio still measures 4.05 mm and two Acorde staff spaces shrink slightly.
The next work is to raise the remaining small staff and text sizes without
introducing overlaps, then address page-use
and missing-mark cases. Aloha Oe's final page remains sparsely used.
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
The audit also corrects a whole-measure-rest fermata hold. The
[written note-span follow-up](docs/archive/playback-alpha19-source-onsets.md)
checks source-derived note ends for the same scores. After excluding ambiguous
same-pitch overlaps and expressive markings, Acorde matches 838/838 written
ends, and alphaTab matches 597/623 onset-matched written ends; all 26
differences are in the split-bar score. Expand the source
oracle beyond these simple forms. The
[local-pedal follow-up](docs/archive/playback-alpha19-local-pedal.md) narrows
pedal release caveats to written active intervals and propagates chord-wide
articulations: Acorde matches 824/824 strict written ends and alphaTab
586/612 under the revised eligibility rule. These totals cannot be compared
as a trend with the preceding rule. A
[lined-pedal repair](docs/archive/playback-lined-pedal-controls.md) keeps
MusicXML offsets, barline releases and the Webern fixture's `resume` in
offline playback and MusicXML roundtrips. Next recheck note onset/release, tempo,
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

The [BCMH global-key baseline](docs/archive/analysis-bcmh-key-baseline.md)
has 82 chorales with explicit labels, equal imported note/chord event counts
and successful parses in both tools: music21 selected 76 correct keys, while
Acorde's legacy first candidate selected 46.
[The integrated rerun](docs/archive/analysis-bcmh-integrated-profile.md) combines
the opt-in tonal-profile ranking and Humdrum field recovery. Both tools parse
98/100, and all 98 paired imports have equal note/chord counts and unordered
pitch/duration event bags. On 89 explicitly labeled scores, each tool selects
83 correct keys. A [part/measure audit](docs/archive/analysis-bcmh-structure-audit.md)
finds 97/98 paired scores with matching pitched onsets within the same part
and measure; the remaining score has a malformed barline. Visible-rest positions
also agree for 97/98. After removing synthesized trailing rests from Humdrum
imports, [all-rest onsets](docs/archive/analysis-bcmh-rest-preservation.md)
agree for 97/98, up from 81/98; the remaining source has a malformed barline.
[A spelling and tie audit](docs/archive/analysis-bcmh-spelling-tie-audit.md)
also agrees within part and measure for 97/98 paired parses; the sole difference
is the same malformed barline. Audit absolute measure positions and voice
placement, especially on polyphonic scores, before treating the event bags as
semantic equality. This development-corpus tie is not holdout accuracy. A new
[vertical-harmony screening](docs/archive/analysis-bcmh-vertical-harmony.md)
uses an opt-in simultaneous-note pass and held-aside BCMH Roman labels. On
equal-row-count measures it matches 1,916/3,943 labels with a separately
inferred global key, versus 1,498/3,943 with the written key; 266 measures
remain unaligned. A [source-beat-aligned comparison with music21 10.5.2](docs/archive/analysis-bcmh-harmony-music21.md)
uses the same cleaned `**kern` input in both programs. Of 97 scores with equal
pitched timelines, 1,305 measures align directly to source-derived beats and
75 remain excluded. On 4,627 annotated rows in aligned measures, Acorde
matches 2,155 narrowly normalized Roman labels versus music21's 2,073, but
Acorde produces no label on 382 rows. A
[harmonic-minor leading-tone follow-up](docs/archive/analysis-bcmh-harmonic-minor.md)
raises Acorde's matches to 2,189/4,627 and labels to 4,291/4,627; 210 rows
still have no detected sonority and 126 have a sonority without a Roman label.
These are development-corpus results, not an untouched harmony holdout or
general analysis parity. The earlier
[Mozart sonata observation](docs/archive/analysis-mozart-key-holdout.md) records
53 primary movements and 52 common MusicXML inputs, 50 annotated keys
selected by Acorde's ranked first choice
versus 51 by music21. MuseScore failed to export K533-3. The K331-2 error
informed a new full-candidate route, so Mozart is now development evidence.
The [Beethoven v2.3 key holdout](docs/archive/analysis-beethoven-key-holdout.md)
uses 64 independently annotated movements: both tools parse all 64 common
MusicXML files, and Acorde matches 57 global-key labels versus music21's 56.
Unordered pitched pitch/duration bags agree on all 64, but order, voice
placement, rests and source-export fidelity are not proved. Reproduce the
missing Mozart K533-3 export with a stable converter, then add independently
annotated harmony,
cadence and voice-leading fixtures. Report precision, recall and error cases
per task against music21 under the
same input notes. Task-scoped parity requires Acorde to match or exceed
music21 on each frozen holdout metric without lower parse coverage. A global
key result on these corpora is not overall analysis parity.

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
