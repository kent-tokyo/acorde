# acorde-analysis

Deterministic, explainable analysis primitives for [`acorde-core`](https://docs.rs/acorde-core).
The crate has no I/O, renderer, or host dependencies.

The capability slice labels chord-shaped pitch collections, records adjacent melodic intervals,
and estimates major/minor keys from diatonic pitch coverage. It also reports explicit cadence
transitions and aligned voice-leading observations, including parallel-perfect flags. Every
result contains stable `NoteAddr` evidence and a rule identifier. SATB diagnostics classify voice
crossing, wide spacing, and parallel-perfect motion with typed severity. Key estimation returns all tied
best candidates, so relative-major/minor ambiguity is preserved instead of inventing a single key.
Repeated three-note interval motifs and explicit rest-terminated phrase boundaries are also
reported with source spans.

```rust
use acorde_analysis::analyze_score;
use acorde_core::Score;

let result = analyze_score(&Score::default());
assert!(result.chords.is_empty());
```

`analyze_batch` preserves input order for finite collections, while `analyze_stream` returns a
lazy iterator for host-side streaming. Both use the same deterministic result contract.

Offline benchmark consumers can use `BenchmarkCase` and `run_benchmark` with hand-verified
category counts. The report includes predicted counts, precision, recall, explanation
completeness, and category-level `BenchmarkFailure` records with missing or excess predictions.
`run_benchmark_suite` additionally aggregates case status and metrics; an empty suite is reported
as zero for each aggregate metric. Latency should be measured by the host benchmark runner and is
intentionally not embedded in the deterministic analysis result.

Applications can register deterministic extensions with `AnalysisPass` and execute them through
`run_analysis_passes`. Pass IDs are validated and sorted before execution, so results do not depend
on registration order; empty or duplicate IDs return `AnalysisPassError`.
