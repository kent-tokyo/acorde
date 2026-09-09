# Acorde fuzz targets

These standalone `cargo-fuzz` targets exercise the public bounded parser/report boundaries for
MusicXML, MIDI, ABC, MEI, and MSCZ. The fuzz package is intentionally outside the main workspace;
it is not published and does not change runtime dependencies of the library crates.

Install `cargo-fuzz`, then run a bounded smoke pass from this directory:

```text
cargo fuzz run musicxml -- -runs=1000
cargo fuzz run midi -- -runs=1000
cargo fuzz run abc -- -runs=1000
cargo fuzz run mei -- -runs=1000
cargo fuzz run mscz -- -runs=1000
```

Corpus and crash artifacts stay local under `fuzz/corpus/` and `fuzz/artifacts/`. CI should run
each target with an explicit time and memory budget before treating the fuzz gate as complete.
