# Performance checks

The renderer is synchronous and stateless. A reproducible smoke benchmark covers layout and
rendering together for small, medium, and large scores:

```bash
cargo run --release -p acorde-render-svg --example benchmark
```

The command prints one machine-readable key/value line per case and fails if any case exceeds its
budget: small (8 measures) allows 25 ms layout, 50 ms render, and 600 KiB SVG; medium (32 measures)
allows 50 ms, 100 ms, and 2 MiB; large (128 measures) allows 200 ms, 400 ms, and 8 MiB. Use the
same host and release profile when comparing revisions; absolute timings are hardware dependent.
The row renderer is the intended unit for host-side viewport caching and incremental updates, while
`ChangeHint` identifies whether layout or playback invalidation is required.
