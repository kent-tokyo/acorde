# Performance checks

The renderer is synchronous and stateless. A reproducible smoke benchmark covers layout and
rendering together for fixed small, medium, and large in-memory scores. It performs one warm-up
and ten measured iterations per case, then reports the lower median together with the OS,
architecture, and an optional CPU label:

```bash
ACORDE_BENCH_CPU="Apple_M4" \\
  cargo run --release --locked -p acorde-render-svg --example benchmark
```

The command prints one machine-readable header and one line per case. It fails if a median exceeds
its budget: small (8 measures) allows 25 ms layout, 50 ms render, and 600 KiB SVG; medium (32
measures) allows 50 ms, 100 ms, and 2 MiB; large (128 measures) allows 200 ms, 400 ms, and 8 MiB.
Use the same fixed inputs, CPU label, release profile, and repetition count when comparing
revisions; absolute timings are hardware dependent. The row renderer is the intended unit for
host-side viewport caching and incremental updates, while `ChangeHint` identifies whether layout
or playback invalidation is required.

The analysis corpus is deterministic but its latency is host-measured. Record repeated timings with
its pinned manifest, fingerprint, and an explicit CPU label:

```bash
cargo run --release --locked -p acorde-cli -- benchmark benchmarks/analysis.json \\
  --fail-on-mismatch --expected-fingerprint fnv1a64-b3b4b222fa7349e8 \\
  --timing-repetitions 10 --cpu-label "Apple M4"
```

This adds `timing.repetitions`, host OS/architecture, CPU label, and min/median/max microseconds to
the JSON report. Timing starts after the corpus has been parsed, so it measures analysis rather
than file I/O. The normal command remains a deterministic correctness gate and omits timing.

## Current evidence

The old single-run renderer values are intentionally not used for revision comparisons. Regenerate
them with the commands above before making a speed claim, and retain the command output with the
commit ID. The analysis corpus currently contains two synthetic repository fixtures; a passing
precision/recall result proves that pinned corpus only, not general analysis accuracy. Revisit SIMD
or parallel rendering/analysis only after a larger corpus or host workflow shows a reproducible
bottleneck.
