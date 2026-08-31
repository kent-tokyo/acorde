# acorde benchmark corpus

This directory contains the checked-in, redistributable benchmark manifest for the deterministic
analysis suite. The manifest references the repository's small MusicXML fixtures; it does not
download files or depend on an external corpus.

## Provenance and license

All inputs are synthetic fixtures created for this repository and are distributed under the same
MIT OR Apache-2.0 terms as acorde. They contain no third-party score, performer, or personal data.
The fixture paths, expected category counts, and manifest format are versioned with the library.

## Run

```bash
cargo run --offline -p acorde-cli -- benchmark benchmarks/analysis.json --fail-on-mismatch
```

The command emits a deterministic JSON report. Runtime latency is intentionally measured by the
host runner and is not part of the expected analysis annotations.
