# acorde

Platform-agnostic music score library for Rust and WebAssembly (v0.22.0).

acorde provides a serializable score model, undoable commands, format I/O, logical layout,
deterministic SVG rendering, playback events, and WASM bindings. Core libraries are synchronous,
UI-free, and do not access the filesystem.

## Crates

| Crate | Purpose |
|---|---|
| `acorde-core` | Score model, commands, validation, playback, and music-theory helpers |
| `acorde-io` | MusicXML/MXL and MIDI I/O; optional ABC and MuseScore MSCZ/MSCX I/O |
| `acorde-layout` | Pixel-free rows, spans, beams, tuplets, and accidental marks |
| `acorde-render-svg` | Pure Rust/WASM SVG renderer; depends on core and layout |
| `acorde-wasm` | JavaScript bindings for I/O, editing, layout, and SVG |
| `acorde-cli` | File-based conversion and inspection commands |
| `acorde-analysis` | Deterministic, explainable harmony and SATB analysis |
| `acorde` | Umbrella crate re-exporting core, io, and layout |

```text
input bytes/text → acorde-io → Score → acorde-layout → LayoutResult
                                      └──────────────→ acorde-render-svg → SVG
```

The umbrella crate does not re-export `acorde-render-svg`; depend on that crate directly when
rendering.

See the [notation coverage matrix](docs/notation-coverage.md) for the supported interchange
slices and known information-loss boundaries.

Interchange APIs also provide typed `ImportReport` and `ExportReport` wrappers for structured
conversion diagnostics.

## Quick start

```toml
[dependencies]
acorde = "0.18"
acorde-render-svg = "0.18"
```

Optional I/O features are disabled by default in `acorde` and `acorde-io`:

```toml
acorde = { version = "0.11", features = ["abc", "mscz", "mei"] }
```

```rust
use acorde_core::{Command, Score, ScoreEngine, SetTempoCmd};

let mut engine = ScoreEngine::new();
engine.apply(Command::SetTempo(SetTempoCmd { bpm: 120 }))?;
engine.undo()?;
let score: &Score = engine.score();
# let _ = score;
```

## Format support

`acorde-io` exposes `parse_musicxml`, `parse_mxl`, `serialize_musicxml`, `parse_midi`,
`serialize_midi`, and `serialize_midi_region` with the default `musicxml` and `midi` features.
The `abc` feature adds ABC parse/serialize; `mscz` adds MuseScore `.mscz`/`.mscx` parsing; `mei`
adds the documented MEI subset import/export boundary.
Parsers accept memory buffers and return typed errors. They do not read files.

## SVG and browser API

`acorde-render-svg` offers `render_svg`, `render_svg_with_layout`, `render_svg_row`, and
`render_svg_metadata`. It emits deterministic SVG with optional `data-note-addr` hooks and
returns errors for unsupported clefs, accidentals, layouts, rows, or render options.

The WASM package exposes the same pipeline plus `ScoreEngine`, score diff/patch, validation,
playback, theory helpers, and explainable score analysis through `analyze_score`. See [the browser contract](docs/browser-rendering.md) and the
[browser fixture](examples/browser/README.md).

## CLI

```bash
acorde convert input.mid output.musicxml
acorde info input.musicxml
acorde validate input.musicxml
acorde extract --part 0 input.musicxml part.musicxml
```

The CLI supports `.musicxml`, `.mxl`, `.mid`/`.midi`, `.mscz`, and `.mscx` input. Conversion
output is MusicXML or MIDI.

## Development

```bash
cargo test --all
cargo clippy --all -- -D warnings
```

For the browser fixture:

```bash
wasm-pack build crates/wasm --target web
python3 -m http.server 8000
```

See [browser support](docs/browser-support.md), [performance](docs/performance.md), and
[visual regression](docs/visual-regression.md) for focused checks. The Score JSON schema is
currently version 1. See [CHANGELOG.md](CHANGELOG.md) for release notes and [README_ja.md](README_ja.md)
for a Japanese overview.

## License

MIT OR Apache-2.0, at your option.
