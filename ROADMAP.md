# Acorde roadmap

This roadmap tracks work still needing evidence or implementation. The
[scorecard](docs/scorecard.md) inventories current contracts, the
[notation coverage matrix](docs/notation-coverage.md) describes tested format support, and the
[changelog](CHANGELOG.md) records releases. Issue acceptance criteria remain authoritative.

## Current baseline

Version 2.0.1 includes a Rust score model with undoable commands, bounded notation I/O,
SVG and print-page rendering, playback scheduling, analysis primitives, WASM bindings,
and the file-based CLI. MusicXML/MXL is the broadest interchange path; Humdrum `**kern` and
Plaine & Easie import are opt-in subsets. Import and export reports identify known losses,
but broad real-score round-trip compatibility remains an evidence question.
The opt-in [standard-bass screen](docs/harmony-groove-standard-bass.md) checks
the exact sounding pitches of a harmony/groove candidate against a supplied
button profile. New standard-bass preset and layer scores separate the treble
accompaniment chords from the bass staff. The screen does not infer fingering,
bellows, or hand transitions, so performer review remains necessary.

[Explicit arrangement plans](docs/arrangement-plans.md) select existing
accompaniment and melodic layers by written-measure range (#433), with source-checked
replay and section provenance. Bounded seeded selection from allowed section
combinations (#435) is included. Opt-in phrase harmony, repeat-visit performance
scores, ensemble forms, inner voices, rhythm sections and integrated proposals
now have source-bound replay contracts and development evidence. Their musical
quality across independent scores remains an open validation task.

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

Measurements for the ten-score A4 corpus and its follow-ups are in the
[Verovio engraving baseline](docs/verovio-engraving-baseline.md#roadmap-measurement-notes-moved-2026-11-28).
On the 6.4.1 corpus Acorde still uses 18 pages to Verovio's 16. After the
staff-size follow-up its smallest measured staff space was 1.50 mm against 1.8 mm,
and the lyric follow-up measured 3.08–3.42 mm lyrics on four scores against 4.05 mm.

The next work is to raise the remaining small staff and text sizes without
introducing overlaps, then address page-use
and missing-mark cases. Aloha Oe's final page remains sparsely used.
Compare each revision on the same A4 inputs and verify suspicious collision
counts in the PDFs, including slurs, beams, lyrics and directions.

The active [print parity protocol](docs/engraving-parity.md) records the user's
2026-11-15 instruction for the implementing assistant to perform every-page,
print-size source and output review ([#390](https://github.com/kent-tokyo/acorde/issues/390)).
Record readability, spacing, typography, collisions, paper use and missing/wrong
marks per score and page. This replaces the earlier two-human-reviewer requirement
for this task; it is not independent review evidence. All four gates remain pending.
The first reserved cohort failed and is now development data; a disjoint frozen
cohort is required before a broader parity claim. Page count alone is not a quality score.

### 2. Playback and audio: alphaTab 1.9.0

Measurements for the eight-score corpus, the written-onset and note-end audits,
and the pedal follow-ups are in [playback evidence](docs/playback-evidence.md#roadmap-measurement-notes-moved-2026-11-28).
On four source scores Acorde matches 1,536/1,536 written attacks and alphaTab 1.9.0
matches 1,093/1,536; every difference is split-bar timing in one score.
Next recheck note onset/release, tempo,
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

The [source-positioned evaluation contract](docs/analysis-evaluation.md) checks
labels and locations rather than category counts, and records parse failures
separately. Global-key and harmony reports have label-level adapters. The
new voice-leading quartet comparator checks five rule outputs on identical
located notes, but has no independent corpus labels yet. Cadence and
voice-leading scoring adapters, new untouched holdouts, and the improvements
below remain necessary before an overall superiority claim.

| Analysis task | Current gate evidence | Status |
| --- | --- | --- |
| Global key | Three-style fixed set: 10/11 exact for each tool, equal coverage | Superiority gate failed |
| Harmony | Third When in Rome fixed set: 559/2,341 versus 508/2,341 at annotated positions | Narrow aggregate gate passed; six of 12 works favor Acorde |
| Local key | Disjoint fixed set: 1,921 versus 1,934 exact at 2,903 paired positions | Gate failed |
| Cadence | Mozart development: 30/333 annotated positions, 29 false positives; no music21 cadence recipe yet | Development only |
| Voice leading | Six selected quartets: five flags agree with music21 10.5.2; no independent accuracy labels | Semantic comparison only |
| Final multi-style suite | Not frozen or scored across all tasks | Pending |

Next analysis gates, in order. Each holdout result is final for its cohort: after
any algorithm change, score a new disjoint holdout. Measurements and the history behind
each item are in [analysis evidence](docs/external-analysis-evidence.md#roadmap-measurement-notes-moved-2026-11-28).

1. **Global key.** The fixed 12-score holdout across chorales, keyboard music, and
   chamber works [fails](docs/archive/analysis-global-key-holdout-v1.md): both tools
   select 10 of 11 eligible keys with equal coverage. Next, change the key ranker on
   development data only and record per-genre accuracy and parse exclusions.
   Done when a new disjoint holdout matches or exceeds music21 with no lower coverage.
2. **Local key and modulation.** The frozen 12-work holdout
   [fails](docs/archive/analysis-wir-local-key-holdout-v1.md): 1,921 versus 1,934
   exact labels on 2,903 positions, two Acorde work wins. Next, add source-beat
   positions, within-measure transitions, transposition and alternate-bar handling.
   Done when a new disjoint label holdout matches or exceeds music21.
3. **Cadence.** 36 Mozart movement pairs are
   [frozen](docs/archive/analysis-mozart-cadence-preflight-v1.md), 18 reserved and
   unread. On development data the opt-in rule matches 30/333 with 29 false positives,
   and only 160/333 cadence onsets have a vertical sonority. Next, improve source-note
   placement and phrase evidence, map cadence classes to Acorde candidates and a
   pinned music21 recipe, and count false positives at eligible positions.
   Done when the reserved movements are scored against music21.
4. **Voice leading.** Located-quartet Rust, CLI and WASM APIs cover cross-bar,
   nonadjacent and cross-part pairs, and agree with music21 10.5.2 on six selected
   [fixture quartets](docs/archive/analysis-voice-leading-quartet-development-v1.md).
   Next, freeze independently labeled multi-style cases and define eligibility and
   false-positive positions; browser WASM execution is still unverified.
   Done when each work and rule family is scored.
5. **Final suite.** Run a final untouched, multi-style suite for each task after algorithm
   changes stop. Require no lower import coverage and report both aggregate
   and per-work results; a single aggregate lead cannot establish broad
   superiority. Keep reproducible Rust, CLI, and WASM outputs with source
   addresses, rule IDs, and enough evidence to audit every prediction.

Open time-axis work: incomplete or contradictory part timelines, ambiguous or
repeat/jump partial ties, authored chord-symbol transposition, and diagnostics for
other timing ambiguities. Reproduce the missing Mozart K533-3 export with a stable
converter. Task-scoped parity requires Acorde to match or exceed music21 on each
frozen holdout metric without lower parse coverage; a global-key result is not
overall analysis parity.

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
