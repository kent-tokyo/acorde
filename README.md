# acorde

Platform-neutral Rust and WebAssembly score infrastructure (v1.2.3).

acorde provides a serializable score model, undoable edits, bounded notation I/O, logical layout,
deterministic SVG, playback-event projection, analysis primitives, and WASM bindings. Library
crates are synchronous, UI-free, and do not read or write files.

```text
input bytes/text → acorde-io → Score → acorde-layout → LayoutResult
                                      └──────────────→ acorde-render-svg → SVG
```

## Crates

| Crate | Responsibility |
|---|---|
| `acorde-core` | Score model, commands, validation, playback, theory helpers |
| `acorde-io` | MusicXML/MXL and MIDI; optional ABC, MEI, MSCZ/MSCX |
| `acorde-layout` | Pixel-free logical layout and print-page metadata |
| `acorde-render-svg` | Deterministic Rust/WASM SVG renderer |
| `acorde-analysis` | Deterministic explainable harmony and SATB analysis |
| `acorde-soundfont` | Optional bounded SF2/SF3/provider integration boundary |
| `acorde-wasm` | JavaScript bindings |
| `acorde-cli` | File-based conversion and inspection |
| `acorde` | Umbrella re-export of core, I/O, and layout |

`acorde-render-svg` is intentionally not re-exported by the umbrella crate; add it directly when
rendering SVG.

## Quick start

```toml
[dependencies]
acorde = "1.2.3"
acorde-render-svg = "1.2.3"
```

Default I/O features are MusicXML and MIDI. Enable optional formats explicitly:

```toml
acorde = { version = "1.2.3", features = ["abc", "mei", "mscz"] }
```

```rust
use acorde_core::{Command, Score, ScoreEngine, SetTempoCmd};

let mut engine = ScoreEngine::new();
engine.apply(Command::SetTempo(SetTempoCmd { bpm: 120 }))?;
engine.undo()?;
let score: &Score = engine.score();
# let _ = score;
```

## Format and host boundaries

MusicXML/MXL is the broadest supported interchange path. MIDI, ABC, MEI, and MSCZ/MSCX are
documented subsets, not lossless-compatibility promises. Use typed `ImportReport`, `ExportReport`,
or `compatibility-report` when a workflow must inspect omission, normalization, or semantic
difference.

acorde owns score semantics, logical geometry, deterministic SVG, and versioned browser/WASM
contracts. Hosts own filesystem/UI, audio rendering, font discovery and embedding, PDF backends,
print dialogs, and browser end-to-end presentation. A SoundFont provider can consume Acorde
playback events, but synthesis and asset licensing remain host concerns.

The detailed boundary and tested slices are in the [notation coverage matrix](docs/notation-coverage.md).
The [scorecard](docs/scorecard.md) is a conservative inventory, and the
[external evidence](docs/external-interoperability-evidence.md) records bounded observations only.

## CLI

```bash
acorde convert input.mid output.musicxml
acorde render input.musicxml output.svg
acorde render-report input.musicxml output.svg --fail-on-issues
acorde print-report input.musicxml --preset a4-score --fail-on-issues
acorde validate input.musicxml
acorde compatibility-report source.musicxml candidate.musicxml --fail-on-differences
acorde playback-report input.musicxml --bpm 120
```

Additional commands cover `info`, `report`, `preflight`, `analyze`, `benchmark`, `extract`,
`transpose`, `normalize`, tablature assignment/performance, playback comparison, and
machine-readable export reports. Run `acorde --help` for the complete, versioned command surface.
CLI owns file access; the underlying library APIs accept in-memory strings or bytes.

## Documentation

- [Japanese overview](README_ja.md)
- [Notation coverage and known losses](docs/notation-coverage.md)
- [Migration notes](docs/migrations.md)
- [Print layout contract](docs/print-layout.md) and [page-SVG contract](docs/print-svg-contract.md)
- [Browser contract](docs/browser-rendering.md) and [browser support checks](docs/browser-support.md)
- [Performance evidence](docs/performance.md), [security contract](docs/security/threat-model.md), and [contribution guide](CONTRIBUTING.md)

## Development

```bash
cargo fmt --all -- --check
cargo test --workspace --all-features --locked
cargo clippy --workspace --all-features --locked --all-targets -- -D warnings
```

Run focused package, fuzz, WASM, browser, or external-tool checks when changing those surfaces.
Fixture provenance and capability boundaries are part of the review contract.

## License

MIT OR Apache-2.0, at your option.
