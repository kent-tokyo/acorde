# Performance checks

The renderer is synchronous and stateless. A reproducible smoke benchmark covers layout and
rendering together for fixed small, medium, and large in-memory scores. It performs one warm-up
and ten measured iterations per case, then reports the lower median together with the OS,
architecture, and an optional CPU label:

```bash
ACORDE_BENCH_CPU="Apple_M4" \
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
cargo run --release --locked -p acorde-cli -- benchmark benchmarks/analysis.json \
  --fail-on-mismatch --expected-fingerprint fnv1a64-b3b4b222fa7349e8 \
  --timing-repetitions 10 --cpu-label "Apple M4"
```

This adds `timing.repetitions`, host OS/architecture, CPU label, and min/median/max microseconds to
the JSON report. Timing starts after the corpus has been parsed, so it measures analysis rather
than file I/O. The normal command remains a deterministic correctness gate and omits timing.

## Release print comparison with Verovio

[`scripts/bench_release_print.py`](../scripts/bench_release_print.py) builds the release WASM
package from a git tag and installs a pinned Verovio npm package. It runs
[`bench_print_parity.mjs`](../scripts/bench_print_parity.mjs) in one Node process per engine and
input, alternating the engine order by input, with 2 warmups and 9 measured rounds. Each round
parses the same plain MusicXML and renders every A4 page. Acorde's round also serializes the
canonical score JSON and outlines text with the supplied Tuffy font. The report records the tag,
commit, toolchain, CPU label and artifact hashes. Run it once per release on a fixed host:

```bash
python3 -B scripts/bench_release_print.py --tag vX.Y.Z --cpu-label "<CPU>" \
  --output docs/benchmarks/print-release-vX.Y.Z-<host>.json
```

The first recorded result is [v2.0.1 against Verovio 6.4.1](benchmarks/print-release-v2.0.1-xeon.json).
It ran on a shared cloud VM (Intel Xeon @ 2.10GHz, Node 22, wasm-opt 117), on the four
OpenScore lieder fixtures, with at most four measures per system:

| Score | Acorde median / p95 (ms) | Verovio median / p95 (ms) | Peak RSS A/V (MiB) | Pages A/V |
| --- | ---: | ---: | ---: | ---: |
| Aloha Oe | 94.2 / 127.3 | 147.3 / 153.3 | 454 / 217 | 4 / 3 |
| Just for Today | 208.4 / 253.7 | 126.2 / 152.7 | 436 / 210 | 4 / 3 |
| Quilter Prelude | 37.8 / 105.2 | 53.8 / 59.6 | 385 / 217 | 1 / 1 |
| Webern | 108.8 / 161.4 | 77.2 / 100.1 | 461 / 219 | 3 / 2 |

The result is mixed. Acorde is faster on two scores and slower on two, uses about twice the peak
memory, and produces more pages under these settings. Acorde initializes in about 40 ms, against
about 210 ms for Verovio. The VM had uncontrolled neighbours, so this is a release baseline for the
same host, not a speed claim. Compare later releases only with runs from the same host and
settings.

## Current evidence

The old single-run renderer values are intentionally not used for revision comparisons. Regenerate
them with the commands above before making a speed claim, and retain the command output with the
commit ID. The analysis corpus currently contains two synthetic repository fixtures; a passing
precision/recall result proves that pinned corpus only, not general analysis accuracy. Revisit SIMD
or parallel rendering/analysis only after a larger corpus or host workflow shows a reproducible
bottleneck.
