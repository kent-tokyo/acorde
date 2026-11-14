# acorde-analysis

Deterministic, explainable analysis primitives for [`acorde-core`](https://docs.rs/acorde-core).
The crate has no I/O, renderer, or host dependencies.

The capability slice labels chord-shaped pitch collections, records adjacent melodic intervals
(including exact signed microtonal cents), does not bridge rests or pitchless events, and estimates
major/minor keys from duration-weighted diatonic pitch coverage. It also reports explicit cadence
transitions and aligned voice-leading observations, including parallel-perfect flags. Every
note-level result contains stable `NoteAddr` evidence and a rule identifier. Voice-leading observations
retain exact signed cents for microtonal motion. SATB diagnostics classify voice crossing, wide
spacing, and parallel-perfect motion with typed severity. Key estimation returns all tied best
candidates, so relative-major/minor ambiguity is preserved instead of inventing a single key.
Vertical sonorities, adjacent-voice motion, and SATB spacing/crossing share written onset
positions within each physical measure. A held note is paired with the note actually sounding
at that beat.
`analyze_vertical_sonorities_report` and its in-key variant report physical-bar
alignment and malformed-duration issues. Affected measures are omitted from
inferred sonorities; callers that need to distinguish absent harmony from
invalid timing should use these report APIs.
When all staves have the same validated total length, the pass also aligns
different physical bar partitions on absolute written beats. Its `measure` and
`beat` use the first available staff as the reference; `label.address` and
`label.evidence` retain each note's own physical measure. The default API now
resolves a concert key from the active contributing staves at each absolute
onset, including their different local measure numbers. It emits a Roman
numeral only when those keys agree. The in-key API still accepts a caller's
concert key when the score does not supply an unambiguous one.
Ties with a matching start in the same part at the same absolute beat and
sounding pitch, including ties across voices or staves, sustain their pitches
without creating an additional harmonic attack. Explicit predecessor
locations must match. A new pitch in a partially tied chord does create an
attack, with the held pitches included in the sonority.
Vertical harmony combines written notes at their concert pitches using staff,
part-instrument and measure-instrument semitone offsets. A concert key supplied
to `analyze_vertical_sonorities_in_key` supports Roman numerals for transposing
parts. With equal physical bars, the default pass also converts an explicitly
stored written key on each transposing staff to concert key and emits Roman
numerals when all contributing staves agree. Missing or conflicting concert
keys leave the chord label but omit its Roman numeral.
Unrepresentable concert pitches are reported as timeline issues and suppress
inference for the affected measure.
Authored chord symbols remain in their written spelling. When attached to a
transposing staff, they do not receive a concert-key Roman numeral until their
source spelling can be reconciled.
Repeated three-note interval motifs and explicit rest-terminated phrase boundaries are also
reported with source spans.

```rust
use acorde_analysis::analyze_score;
use acorde_core::Score;

let result = analyze_score(&Score::default());
assert!(result.chords.is_empty());
```

`analyze_vertical_sonorities(score)` is an opt-in harmony pass that groups
simultaneously sounding notes across parts and voices instead of treating a
melodic sequence as a chord. It reports measure/beat and source addresses,
and preserves authored chord symbols. `analyze_vertical_sonorities_in_key`
accepts a separately chosen concert key; the default pass reconciles written
key changes across aligned staves when possible. Its Roman numerals use
common figured-bass inversion suffixes.
The [BCMH development screening](../../docs/archive/analysis-bcmh-vertical-harmony.md)
records current coverage and errors. The legacy `analyze_score` chord result
is unchanged for existing callers.

`analyze_arpeggiated_bars_report` and its in-key variant add an opt-in
harmonic-completion pass. If no vertical chord exists at a bar start, the pass
may infer an exact tertian chord from notes in the bar's first half. It may
also complete an onset from notes up to the next detected sonority within half
a beat, provided the combined notes form an exact tertian template. Each
extra label has a lower confidence, a distinct rule ID, and addresses of the
later notes used as evidence. Invalid bars, different physical bar partitions,
three-note bar-start melodies, and fully tied continuations remain guarded.
The CLI exposes this pass through `analyze-vertical --arpeggiated-bars`.

`analyze_vertical_cadences_report` and its in-key variant add opt-in
authentic-cadence candidates from those located vertical chords. A candidate
requires a plain dominant followed by a tonic, sounding bass motion of five
semitones modulo 12, and outer voices ending in the same
pitch class. The report keeps timeline issues and source-note evidence; it
does not assert a phrase ending or distinguish perfect from imperfect
authentic cadences. The CLI exposes it through `analyze-cadences [--infer-key]`.
On the inspected Mozart cadence development split, it finds 30 of 333
annotated cadences with 29 false positives at 18,467 eligible positions. This
is not a music21 comparison or held-out accuracy result.

`estimate_local_keys_with_aarden_essen_profile(score, radius)` reports one
opt-in local key per physical measure using duration-weighted pitch classes
from a centered measure window. The first staff's written key signature only
breaks ties between enharmonic spellings. Each result gives the source window,
correlation and rule ID. The CLI exposes `analyze-local-keys --radius 4`.
This cannot distinguish changes within a measure, may lack an estimate for a
silent or uniform window, and uses written pitches. On 24 inspected When in
Rome development works, a radius of four matched 2,325 of 3,405 annotated
local-key positions. The radius was selected on those same works, so this is
development evidence rather than a held-out or paired music21 result.

`analyze_batch` preserves input order for finite collections, while `analyze_stream` returns a
lazy iterator for host-side streaming. Both use the same deterministic result contract.

`AnalysisCache` provides a bounded deterministic in-memory cache keyed by the schema-versioned
score fingerprint. Editing any canonical score content produces a cache miss automatically;
eviction is insertion-order based and does not depend on hash-map iteration.
Its `analyze_batch` method preserves input order while reusing duplicate or previously cached
scores.
Editors can call `invalidate(score)` after dropping an old score snapshot to reclaim that entry
without disturbing other cached results.
`stats()` exposes deterministic hit/miss counters, and `reset_stats()` clears only those counters
for host-side measurement windows.
For an editor replacement flow, `analyze_after_edit(previous, current)` reclaims the previous
snapshot and returns the analysis for the current score in one operation.
When both snapshots have the same canonical fingerprint, the helper keeps and reuses the existing
entry instead of forcing an unnecessary recomputation.

`diff_analysis(previous, current)` reports changed result categories in stable order while keeping
score fingerprint changes separate from category changes. This lets hosts update explanations or
views selectively without treating metadata-only edits as analysis-content changes.
`analysis_provenance(result, address)` returns the deterministic rule ID, confidence, and source
evidence for every finding that contains the selected `NoteAddr`.
`explain_analysis_change(previous, current, address)` combines before/after provenance with the
deterministic category diff for an explainable editor update.

Offline benchmark consumers can use `BenchmarkCase` and `run_benchmark` with hand-verified
category counts. The report includes predicted counts, precision, recall, explanation
completeness, and category-level `BenchmarkFailure` records with missing or excess predictions.
`run_benchmark_suite` additionally aggregates case status and metrics; an empty suite is reported
as zero for each aggregate metric. Latency should be measured by the host benchmark runner and is
intentionally not embedded in the deterministic analysis result.

Applications can register deterministic extensions with `AnalysisPass` and execute them through
`run_analysis_passes`. Pass IDs are validated and sorted before execution, so results do not depend
on registration order; empty or duplicate IDs return `AnalysisPassError`.

Analysis resource-budget ownership is documented in the [security contract](../../docs/security/threat-model.md).
