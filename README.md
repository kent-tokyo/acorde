# acorde

[![CI](https://github.com/kent-tokyo/acorde/actions/workflows/ci.yml/badge.svg?branch=main)](https://github.com/kent-tokyo/acorde/actions/workflows/ci.yml)
[![Codecov](https://codecov.io/gh/kent-tokyo/acorde/graph/badge.svg?branch=main)](https://app.codecov.io/gh/kent-tokyo/acorde)
[![docs.rs](https://docs.rs/acorde/badge.svg)](https://docs.rs/acorde)

Platform-neutral Rust and WebAssembly score infrastructure.

> **Release notice:** v1.2.19 and v1.2.20 added a public `SvgRenderOptions` field and can break
> Rust code that uses a complete struct literal. Upgrade to the compatible v1.3.0 release before
> adopting the renderer configuration API. See the [migration notes](docs/migrations.md).

acorde provides a serializable score model, undoable edits, bounded notation I/O, logical layout,
deterministic SVG, playback-event projection, analysis primitives, and WASM bindings. Library
crates are synchronous, UI-free, and do not read or write files.

The workspace minimum supported Rust version (MSRV) is **Rust 1.88**.

```text
input bytes/text → acorde-io → Score → acorde-layout → LayoutResult
                                      └──────────────→ acorde-render-svg → SVG
                                                               └────────→ acorde-pdf → PDF
```

## Crates

| Crate | Responsibility |
|---|---|
| `acorde-core` | Score model, commands, validation, playback, theory helpers |
| `acorde-io` | MusicXML/MXL and MIDI; optional ABC, MEI, MSCZ/MSCX, Guitar Pro 3–8 import |
| `acorde-layout` | Pixel-free logical layout and print-page metadata |
| `acorde-render-svg` | Deterministic Rust/WASM SVG renderer |
| `acorde-pdf` | Optional vector PDF export through Harumi (Rust 1.88+) |
| `acorde-omr` | Optional bounded PNG analysis that produces reviewable OMR proposals |
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
acorde = "=1.3.3"
acorde-render-svg = "=1.3.3"
```

Default I/O features are MusicXML and MIDI. Enable optional formats explicitly:

```toml
acorde = { version = "=1.3.3", features = ["abc", "mei", "mscz", "gp"] }
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

MusicXML/MXL is the broadest supported interchange path. MIDI, ABC, MEI, MSCZ/MSCX, and Guitar
Pro 3–8 (`.gp3`/`.gp4`/`.gp5`/`.gpx`/`.gp`, import only) are documented subsets, not lossless-compatibility promises. Use typed `ImportReport`, `ExportReport`,
or `compatibility-report` when a workflow must inspect omission, normalization, or semantic
difference.

acorde owns score semantics, logical geometry, deterministic SVG, and versioned browser/WASM
contracts. `acorde-pdf` is an optional in-memory Harumi adapter; hosts own fonts, UI, printing,
and end-to-end browser presentation. SoundFont synthesis and asset licensing are also host work.

Use the [coverage matrix](docs/notation-coverage.md) for the tested boundary, the
[scorecard](docs/scorecard.md) for the current inventory, and
[external evidence](docs/external-interoperability-evidence.md) for bounded observations.

## CLI

```bash
acorde convert input.mid output.musicxml
acorde render input.musicxml output.svg
acorde render-report input.musicxml output.svg --fail-on-issues
acorde print-report input.musicxml --preset a4-score --fail-on-issues
acorde validate input.musicxml
acorde compatibility-report source.musicxml candidate.musicxml --fail-on-differences
acorde playback-report input.musicxml --bpm 120
acorde omr printed-score.png draft.mscz --report draft.omr.json
acorde omr-evaluate printed-score.png reference.mscz --report evaluation.json
acorde omr-apply-report provider-result.json draft.mscz --report application.json
```

Additional commands cover inspection, analysis, conversion reports, tablature, and playback;
run `acorde --help` for the versioned surface. The CLI owns file access. `omr` is deliberately a
review-first, high-contrast PNG subset; its limits and no-silent-write rule are in the
[proposal contract](docs/omr-proposal-contract.md) and [validation boundary](docs/omr-validation.md).

## Documentation

Start with the [documentation guide](docs/README.md). It groups contracts, migration notes, and
bounded evidence by task. The most common references are the [notation coverage matrix](docs/notation-coverage.md),
[migration notes](docs/migrations.md), [browser contract](docs/browser-rendering.md), and
[PDF export boundary](docs/pdf-export.md).

The [Japanese overview](README_ja.md), [security contract](docs/security/threat-model.md), and
[contribution guide](CONTRIBUTING.md) remain separate entry points.

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
