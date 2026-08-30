# Performance checks

The renderer is synchronous and stateless. A reproducible smoke benchmark covers layout and
rendering together for a 32-measure, 128-note score:

```bash
cargo run --release -p acorde-render-svg --example benchmark
```

The command prints layout time, render time, and SVG size in machine-readable key/value form and
fails if the fixed case exceeds 50 ms layout, 100 ms render, or 2 MB SVG output. Use the same host
and release profile when comparing revisions; absolute timings are hardware dependent. The row
renderer is the intended unit for host-side viewport caching and incremental updates, while
`ChangeHint` identifies whether layout or playback invalidation is required.
