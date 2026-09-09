# Parser fuzz smoke evidence

The standalone [`fuzz/`](../../fuzz/) package exercises the public bounded report boundaries for
MusicXML, MIDI, ABC, MEI, and MSCZ. It is separate from the production workspace and is not
published.

On 2026-09-09, each target completed 100 libFuzzer runs on macOS arm64 with cargo-fuzz 0.13.1
and Rust nightly 1.98.0 (2026-05-22):

```text
musicxml: 100 runs, no crash or hang
midi:     100 runs, no crash or hang
abc:      100 runs, no crash or hang
mei:      100 runs, no crash or hang
mscz:     100 runs, no crash or hang
```

This is a bounded smoke result, not population-level coverage or a substitute for long-running
CI fuzzing. CI repeats the same target set with an explicit job timeout and the checked-in
`fuzz/Cargo.lock`; corpus growth and sanitizer/property coverage remain separate gates.
