# Performance checks

The renderer is synchronous and stateless. A reproducible smoke benchmark covers layout and
rendering together for small, medium, and large scores:

```bash
cargo run --release --locked -p acorde-render-svg --example benchmark
```

The command prints one machine-readable key/value line per case and fails if any case exceeds its
budget: small (8 measures) allows 25 ms layout, 50 ms render, and 600 KiB SVG; medium (32 measures)
allows 50 ms, 100 ms, and 2 MiB; large (128 measures) allows 200 ms, 400 ms, and 8 MiB. Use the
same host and release profile when comparing revisions; absolute timings are hardware dependent.
The row renderer is the intended unit for host-side viewport caching and incremental updates, while
`ChangeHint` identifies whether layout or playback invalidation is required.

## Latest local measurement

Recorded 2026-09-23 from the v1.2.3 release candidate with the release renderer benchmark:

| Case | Layout | Render | SVG | Result |
|---|---:|---:|---:|---|
| small (8 measures) | 24 µs | 635 µs | 20,593 bytes | pass |
| medium (32 measures) | 7 µs | 923 µs | 79,946 bytes | pass |
| large (128 measures) | 19 µs | 2,973 µs | 318,467 bytes | pass |

The analysis benchmark also passed its pinned local gate: 2/2 cases, 100% precision, 100% recall,
and 100% explanation completeness. These are same-host local measurements, not competitor speed
claims. The current evidence does not justify SIMD or parallel rendering/analysis; revisit that
Phase 11 candidate only after a larger corpus or a host workflow shows a reproducible bottleneck.
