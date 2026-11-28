# External analysis evidence

This document records a bounded music21 comparison protocol. It is not a claim that Acorde and
music21 have equivalent analytical algorithms, engraving, playback, or file-format coverage.

## Projection contract

`scripts/music21_projection.py` parses one fixture with a locally installed music21 and emits
version, fixture SHA-256, options, parsed counts, and music21's inferred key. It does not run in
the normal test suite because music21 is an external optional dependency.

Run it together with Acorde's structural and analysis output:

```bash
python3 scripts/music21_projection.py tests/fixtures/simple.musicxml
cargo run --locked -p acorde-cli -- info tests/fixtures/simple.musicxml
cargo run --locked -p acorde-cli -- analyze tests/fixtures/simple.musicxml
```

Validate a saved report, or stream the observation directly into the generic evidence-contract
validator:

```bash
python3 scripts/music21_projection.py tests/fixtures/simple.musicxml \
  | python3 scripts/validate_external_evidence.py -
```

`validate_external_evidence.py` is intentionally tool-agnostic. MuseScore, Verovio, and alphaTab
measurements must use the same version, fixture hash, options, and projection envelope before they
can be compared or reviewed; validation alone is not a parity result.

The shared projection is deliberately narrow: part count, physical measure count, pitched-note
count, rest count, chord-element count, and one explicitly named key-estimation result. Compare
each field independently. Acorde's key estimates are coverage-based candidates and may be tied;
music21's `score.analyze('key')` selects one result, so a key disagreement is evidence to review,
not a failed semantic round trip.

## Recorded local observation

On 2026-09-22, music21 9.9.1 parsed `tests/fixtures/simple.musicxml` with SHA-256
`55c2d8d2546100437460474b8bc685b173493c3696303cbad2eb85254acbfd7d` as one part, two measures,
five pitched notes, one rest, zero chord elements, and inferred `C major`. Acorde CLI `info`
reported one part, two measures, five notes, and one rest. Its coverage-based key candidates
include C major.

This is one checksum-pinned smoke observation only. It supplies neither a population accuracy
measurement nor a music21 compatibility score. Add more permission-cleared fixtures, pinned
environment details, and explicit expected agreements/disagreements before treating it as a gate.

## Independent global-key corpus (2026-11-07)

The [BCMH Bach chorale baseline](archive/analysis-bcmh-key-baseline.md) compares
Acorde v1.3.10 with music21 10.5.2 against independently annotated keys.
On the 82 mutually parsed scores with matching note/chord event counts and
explicit key labels, music21's single estimate matched 76 labels and Acorde's
first candidate matched 46. Acorde included the label among tied candidates in 78,
which is not single-key accuracy. The result is limited to this corpus and
does not cover harmony or cadence analysis.
The unordered pitch/duration event bag matches on 88 of the 90 paired parses;
this does not check event order, placement, rests or ties.

An [opt-in tonal-profile ranking](archive/analysis-bcmh-key-profile.md) orders
the same tied candidates. Its first choice matched 76 of the same 82 labels,
equal to music21 on this development corpus. This is not an independent
holdout result; parser coverage and other analysis categories remain separate
gates.

The [Mozart sonata observation](archive/analysis-mozart-key-holdout.md) fixed 53
previously unseen annotated movements before scoring. MuseScore 4.7.5 produced
52 valid common MusicXML inputs; one movement failed to export. After
analyst-provided harmony labels were removed, music21 selected 51 annotated
keys and Acorde's opt-in ranking selected 50, both over the original 53-score
denominator. The 52 paired parses had equal pitched-event counts and unordered
pitch/duration bags. Exporter instability and the narrow global-key task keep
this a provisional, task-specific result rather than overall analysis parity.
Its K331-2 result later informed the full-candidate method, so this corpus is
now development evidence for that method.

The [integrated BCMH rerun](archive/analysis-bcmh-integrated-profile.md) combines
the Humdrum field recovery with opt-in tonal-profile ranking. Both tools parse
98/100 scores; all 98 paired parses have matching note/chord counts and
unordered pitch/duration event bags. On the 89 scores with explicit key labels,
Acorde and music21 each select 83 labels. This remains the ranking rule's
development corpus.

The [part and measure audit](archive/analysis-bcmh-structure-audit.md) adds
onset and rest positions to that same corpus. Among 98 paired successful
parses, 97 agree on pitched events by part, measure and onset; 97 agree on
visible-rest positions. One malformed barline yields different measure
boundaries, and 16 further scores differ only when hidden rests are included.
This still leaves absolute measure positions, voices, spelling and ties to audit.

The [Beethoven sonata v2.3 holdout](archive/analysis-beethoven-key-holdout.md)
tests a separate opt-in Aarden-Essen profile against music21 10.5.2 on 64
annotated movements. Both tools parse all 64 common MusicXML inputs; Acorde
selects 57 annotated global keys and music21 selects 56. The one disagreement
is D-flat versus C-sharp major, so this is a spelling-sensitive result, not
proof of better tonal-class recognition. Acorde exposes this ranking through
the Rust `estimate_keys_with_aarden_essen_profile` function, CLI
`analyze --aarden-essen-keys`, and WASM
`estimate_keys_with_aarden_essen_profile`. Default analysis results remain
unchanged. The corpus and comparator do not establish harmony, cadence or
voice-leading parity.

## Later task-specific gates

The [three-style global-key holdout](archive/analysis-global-key-holdout-v1.md)
uses a separately frozen 12-score cohort. One chorale is ineligible under the
preregistered reference rule; Acorde and music21 10.5.2 each match 10/11
remaining keys with equal coverage. Its superiority gate failed.

The [third When in Rome harmony holdout](archive/analysis-wir-harmony-holdout-3-v1.md)
passes a narrower aggregate gate at 559 versus 508 exact Roman labels among
2,341 aligned primary annotation positions. The result does not cover other
harmony positions or styles. The [disjoint local-key holdout](archive/analysis-wir-local-key-holdout-v1.md)
fails at 1,921 versus 1,934 exact labels among 2,903 paired positions. Its
source-beat transitions remain untested. The [Mozart cadence development
audit](archive/analysis-mozart-cadence-sonority-development-v1.md) has no
pinned music21 cadence recipe or scored reserved labels; simple half/deceptive
transition hypotheses made 96 false predictions for 33 correct positions and
were not adopted.

## Located voice-leading quartets (2026-11-15)

The opt-in [quartet comparison](archive/analysis-voice-leading-quartet-development-v1.md)
passes identical written pitches, onsets, and durations from Acorde's located
output to music21 10.5.2's `VoiceLeadingQuartet`. All five checked flags agree
on six selected quartets in two MusicXML fixtures. The fixtures cover a
cross-bar nonadjacent voice pair, antiparallel fifths, and parallel octaves;
separate Rust cases check spelling, rests, unisons, cross-part alignment, and
keyless resolution families.

This is a development semantic check with no independent precision/recall
labels or multi-style holdout. The new Rust, CLI, and native WASM paths passed
local checks; the headless browser WASM test did not run because the pinned
wasm-bindgen CLI could not be installed in that environment. Do not infer a
browser result or overall analysis superiority from the six agreements.

## Roadmap measurement notes (moved 2026-11-28)

These paragraphs were moved verbatim from the [roadmap](../ROADMAP.md) so that it can list
only current status, next steps, and completion conditions. They are dated observations.

### Harmony development candidate and When in Rome cohorts

The [current development candidate](archive/analysis-wir-harmony-development-v5.md)
uses opt-in bar-arpeggio and short forward-completion rules. On the inspected
BCMH corpus it matches 2,193 of 4,627 aligned Roman labels versus music21
10.5.2's 2,073; 208 labeled rows in 75 measures remain excluded. On two
inspected 12-work When in Rome cohorts it matches 366 versus 348 of 1,159,
and 399 versus 394 of 2,246. The second cohort's original
[frozen evaluation](archive/analysis-wir-harmony-holdout-2-v1.md) failed
at 348 versus 394; it became development data before further tuning.

The disjoint [third 12-work holdout](archive/analysis-wir-harmony-holdout-3-v1.md)
was fixed before its gold labels were scored. Both programs parsed every
answer-free input. At 2,341 aligned primary RomanText positions, Acorde
matched 559 figures and music21 508, with exact-position precision of 39.5%
versus 21.8%. It passes the preregistered aggregate harmony gate for this
specific task and candidate. Six of the 12 works favor Acorde. Other analysis
tasks, local-key handling, and broader styles still need independent evidence
before a claim that the library surpasses music21 in music analysis overall.

### Detailed analysis gate history

Next analysis gates, in order:

1. Freeze a new global-key holdout across chorales, keyboard music, and chamber
   works, with common input hashes and independent annotations. The Beethoven
   57/64 versus 56/64 result is one genre-specific observation; record
   per-genre accuracy and parsing exclusions before changing the key ranker.
   A [12-score source selection](archive/analysis-global-key-holdout-selection-v1.md)
   now pins four scores per style without inspecting their key labels. The
   [score-only preflight](archive/analysis-global-key-holdout-preflight-v1.md)
   confirms 12/12 paired parses and equal pitched events in all 496 physical
   measures. The [scoring protocol](archive/analysis-global-key-holdout-protocol-v1.md)
   fixed the chorale transposition rule, both methods, exclusions, and
   success thresholds before opening the labels. Its [fixed result](archive/analysis-global-key-holdout-v1.md)
   **fails** the superiority gate: both tools select 10 exact keys on 11
   eligible scores, with equal prediction coverage. A new disjoint holdout is
   required after any key-ranker change.
2. Add local-key and modulation positions to the harmony contract. Preserve
   absolute score addresses, compare both tools against human annotations,
   and measure coverage as well as exact labels. The current Roman-position
   win does not test local-key transitions. An opt-in [development diagnostic](archive/analysis-wir-local-key-development-v1.md)
   now estimates one key per physical measure from a centered window. At the
   radius selected on 24 inspected works, it matches 2,325/3,405 annotated
   local keys. The [paired development comparison](archive/analysis-wir-local-key-paired-development-v1.md)
   scores music21's best radius from the same grid at 2,157/3,405, with
   12 Acorde wins, 10 music21 wins and two ties by work. This is not an
   untouched holdout. A new [12-work label holdout](archive/analysis-wir-local-key-holdout-preflight-v1.md)
   is selected from disjoint works; both parsers accept all prepared inputs,
   their 1,462 pitched measure timelines agree, and the candidate/scorer
   protocol was frozen before opening its annotations. Its [fixed result](archive/analysis-wir-local-key-holdout-v1.md)
   **fails**: Acorde has 1,921 versus music21's 1,934 exact labels on 2,903
   paired positions and wins only two of twelve works. Do not tune and
   rescore this cohort; a new disjoint label holdout is required after any
   local-key change. Source-beat positions, within-measure transitions,
   transposition and alternate-bar handling remain capability gaps.
3. Build a cadence task from independently annotated scores. The
   [Mozart cadence input preflight](archive/analysis-mozart-cadence-preflight-v1.md)
   freezes 36 MusicXML/cadence-CSV movement pairs from the
   [score-aligned Batik corpus](https://github.com/huispaty/batik_plays_mozart),
   split by complete sonata into 18 development and 18 reserved movements.
   Both parsers accept all 36 answer-free inputs; their pitched timelines
   disagree in six of 5,142 measures, which are recorded for exclusion before
   reserved labels are read. Those cadence CSVs remain unread. A development
   CSV confirms that performance repeats duplicate
   annotated note IDs, so the evaluator must fold those positions explicitly.
   Define how annotated cadence classes map to Acorde candidates and a pinned
   music21 recipe; count false positives at eligible positions. The current
   legacy `analyze_cadences` only recognizes adjacent Roman transitions
   within a voice. On the inspected 18-movement development split it matches
   7/333 cadence positions with 122 false positives. The opt-in
   `analyze-cadences` bass/outer-voice rule reaches 30/333 with 29 false
   positives, but misses 303 annotated cadences. Improve half, imperfect,
   deceptive and evaded cadence handling before a held-out comparison. A
   [development onset audit](archive/analysis-mozart-cadence-sonority-development-v1.md)
   finds a vertical sonority at only 160/333 cadence onsets. Simple `I64 → V`
   and `V → vi/VI` rules find 33 cadences but make 96 false predictions, so
   neither is adopted. Improve source-note placement and phrase evidence
   before adding these cadence types.
4. For voice leading, define a shared set of two-voice note quartets with exact
   pitches, onset, and duration. Compare located parallel fifths/octaves,
   contrary/similar motion, and resolution checks with pinned music21
   [`VoiceLeadingQuartet`](https://music21.org/music21docs/moduleReference/moduleVoiceLeading.html)
   semantics on the same quartets. Include cross-bar and nonadjacent voice
   pairs. The default SATB-facing pass still covers adjacent voices within one
   measure; the opt-in located-quartet Rust, CLI, and WASM APIs now include
   cross-bar, nonadjacent, and cross-part pairs. A [pinned development comparison](archive/analysis-voice-leading-quartet-development-v1.md)
   matches music21 10.5.2 on six selected fixture quartets, with separately
   reviewed synthetic cases. Freeze independently labeled multi-style cases,
   define eligibility and false-positive positions, then score each work and
   rule family. Browser WASM execution remains unverified locally.

### Time-axis work and earlier key and harmony corpora

The first shared written-onset traversal now feeds vertical sonorities,
adjacent-voice motion and SATB crossing/spacing within each physical measure.
It fixes index-paired observations when voices have different note durations.
Vertical-sonority reports now flag missing or unequal physical bars, invalid
note durations and overfull voices, and omit affected measures from inference.
Ties with a matching start at the same absolute beat and sounding pitch now
suppress duplicate harmonic attacks across voices and staves within a part;
explicit predecessor locations are checked. New members of a partly tied
chord still trigger an attack.
Vertical harmony now converts common transposing instruments to concert pitch,
including measure-local instrument changes. On equal physical bars, explicit
written key signatures on transposing staves are converted to concert key;
Roman numerals are emitted only when contributing staves agree or the caller
supplies a concert key.
Equal-total scores with different physical bar partitions now align on
absolute written beats; source addresses retain their own measure indices.
The absolute-timeline pass also resolves concert keys from active staves at
each onset, and suppresses Roman numerals when their keys conflict. Remaining
time-axis work includes incomplete or contradictory part timelines, ambiguous
or repeat/jump partial ties, authored chord-symbol transposition and
diagnostics for additional timing ambiguities. This is a partial Stage 2
implementation.

The [BCMH global-key baseline](archive/analysis-bcmh-key-baseline.md)
has 82 chorales with explicit labels, equal imported note/chord event counts
and successful parses in both tools: music21 selected 76 correct keys, while
Acorde's legacy first candidate selected 46.
[The integrated rerun](archive/analysis-bcmh-integrated-profile.md) combines
the opt-in tonal-profile ranking and Humdrum field recovery. Both tools parse
98/100, and all 98 paired imports have equal note/chord counts and unordered
pitch/duration event bags. On 89 explicitly labeled scores, each tool selects
83 correct keys. A [part/measure audit](archive/analysis-bcmh-structure-audit.md)
finds 97/98 paired scores with matching pitched onsets within the same part
and measure; the remaining score has a malformed barline. Visible-rest positions
also agree for 97/98. After removing synthesized trailing rests from Humdrum
imports, [all-rest onsets](archive/analysis-bcmh-rest-preservation.md)
agree for 97/98, up from 81/98; the remaining source has a malformed barline.
[A spelling and tie audit](archive/analysis-bcmh-spelling-tie-audit.md)
also agrees within part and measure for 97/98 paired parses; the sole difference
is the same malformed barline. Audit absolute measure positions and voice
placement, especially on polyphonic scores, before treating the event bags as
semantic equality. This development-corpus tie is not holdout accuracy. A new
[vertical-harmony screening](archive/analysis-bcmh-vertical-harmony.md)
uses an opt-in simultaneous-note pass and held-aside BCMH Roman labels. On
equal-row-count measures it matches 1,916/3,943 labels with a separately
inferred global key, versus 1,498/3,943 with the written key; 266 measures
remain unaligned. A [source-beat-aligned comparison with music21 10.5.2](archive/analysis-bcmh-harmony-music21.md)
uses the same cleaned `**kern` input in both programs. Of 97 scores with equal
pitched timelines, 1,305 measures align directly to source-derived beats and
75 remain excluded. On 4,627 annotated rows in aligned measures, Acorde
matches 2,155 narrowly normalized Roman labels versus music21's 2,073, but
Acorde produces no label on 382 rows. A
[harmonic-minor leading-tone follow-up](archive/analysis-bcmh-harmonic-minor.md)
raises Acorde's matches to 2,189/4,627 and labels to 4,291/4,627; 210 rows
still have no detected sonority and 126 have a sonority without a Roman label.
These are development-corpus results, not an untouched harmony holdout or
general analysis parity. The earlier
[Mozart sonata observation](archive/analysis-mozart-key-holdout.md) records
53 primary movements and 52 common MusicXML inputs, 50 annotated keys
selected by Acorde's ranked first choice
versus 51 by music21. MuseScore failed to export K533-3. The K331-2 error
informed a new full-candidate route, so Mozart is now development evidence.
The [Beethoven v2.3 key holdout](archive/analysis-beethoven-key-holdout.md)
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
