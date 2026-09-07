# acorde

Platform-agnostic music score library for Rust and WebAssembly (v1.1.5).

acorde provides a serializable score model, undoable commands, format I/O, logical layout,
deterministic SVG rendering, playback events, and WASM bindings. Core libraries are synchronous,
UI-free, and do not access the filesystem.

## Crates

| Crate | Purpose |
|---|---|
| `acorde-core` | Score model, commands, validation, playback, and music-theory helpers |
| `acorde-io` | MusicXML/MXL and MIDI I/O; optional ABC and MuseScore MSCZ/MSCX I/O |
| `acorde-layout` | Pixel-free rows, spans, beams, tuplets, accidental marks, and logical print pages |
| `acorde-render-svg` | Pure Rust/WASM SVG renderer; depends on core and layout |
| `acorde-wasm` | JavaScript bindings for I/O, editing, layout, and SVG |
| `acorde-cli` | File-based conversion and inspection commands |
| `acorde-analysis` | Deterministic, explainable harmony and SATB analysis |
| `acorde-soundfont` | Optional bounded SF2/SF3 metadata and provider-neutral playback boundary |
| `acorde` | Umbrella crate re-exporting core, io, and layout |

```text
input bytes/text → acorde-io → Score → acorde-layout → LayoutResult
                                      └──────────────→ acorde-render-svg → SVG
```

The umbrella crate does not re-export `acorde-render-svg`; depend on that crate directly when
rendering.

See the [notation coverage matrix](docs/notation-coverage.md) for the supported interchange
slices and known information-loss boundaries.
The host-neutral print page contract is described in [print-layout.md](docs/print-layout.md),
and the page-level SVG requirements are in [print-svg-contract.md](docs/print-svg-contract.md);
PDF conversion, font resolution, printer access, and preview UI remain host responsibilities.
The current conservative capability inventory is tracked in the [scorecard](docs/scorecard.md).
Security boundaries and resource-limit ownership are documented in the [security contract](docs/security/threat-model.md).

For deterministic score analysis, add `acorde-analysis` directly. `AnalysisCache` supports bounded
single-score and batch reuse, explicit editor snapshot invalidation, and hit/miss measurements;
all cache keys include the analysis schema version and canonical score fingerprint.

```toml
acorde-analysis = "1.1.5"
```

The optional `soundfont` feature exposes `acorde::soundfont`, a bounded SF2/SF3
metadata and provider-neutral note lifecycle boundary. It consumes unchanged
`PlaybackEvent` values; sample decoding, synthesis, and licensed asset ownership
remain with the application renderer. The boundary also carries sample regions, deterministic
zone selection, voice parameters, and bounded decoded-PCM validation. `SampleDecoder`,
`SampleRenderer`, and the versioned `SoundFontProvider` contract provide the typed provider/host
integration point. Providers advertise codec and synthesis capabilities; unsupported paths are
rejected explicitly without bundling licensed codec code or sample assets.
Malformed SoundFont zones are rejected before scheduling with typed diagnostics.
The soundfont crate includes bounded SF2 PCM16 decoding and deterministic offline sample-action
rendering; SF3 Vorbis is an opt-in feature requiring a separately licensed decoder.
Its `SoundFontPresetZone` mapping API exposes bank/program/key/velocity selection and bounded
sample frame ranges without requiring Composer to duplicate SoundFont parsing.
For the umbrella crate, enable `soundfont-sf3-vorbis` to forward that feature to the SoundFont
adapter.

Interchange APIs also provide typed `ImportReport` and `ExportReport` wrappers for structured
conversion diagnostics. WASM exposes report-returning variants for supported import formats and
MusicXML, MEI, MIDI, and ABC exports.
MIDI pitch-bend events retain their tick, channel, and signed 14-bit value through the score
model and MIDI round-trip.

## Quick start

```toml
[dependencies]
acorde = "1.1.5"
acorde-render-svg = "1.1.5"
```

The default I/O features are `musicxml` and `midi`; enable the optional `abc`, `mscz`, or `mei`
features when needed:

```toml
acorde = { version = "1.1.5", features = ["abc", "mscz", "mei"] }
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
The WASM API also provides `parse_musicxml_render_svg`, `parse_mxl_render_svg`, `parse_mei_render_svg`,
`parse_abc_render_svg`, and `parse_midi_render_svg` as bounded one-call paths from documented
inputs to canonical SVG. MuseScore XML and archive inputs are also covered by
`parse_mscx_render_svg` and `parse_mscz_render_svg`; use the corresponding `*_report` functions
when import diagnostics are needed.
MusicXML voice numbers 1–4 are preserved in `Measure.voices` and through MusicXML round-trips.
Tablature string/fret positions and fractional MusicXML alterations are preserved in the score
model; common ABC (`^/`, `_/`) and MEI (`qs`, `qf`) quarter-accidental subsets are also supported.

## SVG and browser API

`acorde-render-svg` offers `render_svg`, `render_svg_with_layout`, `render_svg_row`, and
`render_svg_metadata`, plus `render_preflight` for source-located capability checks before SVG
emission. It emits deterministic SVG with optional `data-note-addr` hooks and returns errors for
unsupported clefs, accidentals, layouts, rows, or render options.

For a deterministic cross-format comparison, use `acorde compatibility-report source candidate`.
It reports positional semantic changes and import diagnostics for both files without claiming
lossless interchange.
Add `--fail-on-differences` in CI when any semantic change should fail the gate.
Use `--fail-on-loss` when typed conversion losses must also fail the gate.
The report also includes explicit `semantic_equivalent` and `lossless` fields; `lossless` means
that the score is equivalent and no typed conversion loss was reported.
`analysis_changed_categories` reports which deterministic analysis categories changed.
The difference gate covers both score and analysis changes, with the latter exposed as
`analysis_equivalent`.

Playback events include stable source note addresses for synchronizing audio cursors with
notation selection. The WASM package exposes the same pipeline plus `ScoreEngine`, score diff/patch, validation,
playback, theory helpers, deterministic tablature fingering selection, and explainable score analysis through
`analyze_score`; its `AnalysisCache` class provides bounded repeated analysis and hit/miss stats.
The WASM `diff_analysis` call identifies changed analysis categories between two result JSON
payloads, while `AnalysisCache.analyze_after_edit_with_diff` returns the edited result and its
category diff together for incremental editor updates.
`affected_analysis_categories` maps a serialized `ChangeHint` to conservative analysis refresh
categories for host-side incremental update planning.
`AnalysisCache.analyze_after_edit_with_hint` combines that planning step with cached incremental
analysis and the deterministic category diff.
`analysis_refresh_plan` additionally separates measure-local categories from score-global
dependencies and reports one-measure context on either side for boundary-sensitive passes.
`analysis_provenance` returns the deterministic rule, confidence, and source evidence attached
to a selected `NoteAddr` without rerunning analysis.
`explain_analysis_change` combines that lookup for before/after results with the category diff.
`compatibility_report` combines canonical score differences with deterministic analysis-category
changes for browser-side compatibility gates; format diagnostics remain in the `*_report` APIs.
See [the browser contract](docs/browser-rendering.md) and the
[browser fixture](examples/browser/README.md).

## CLI

```bash
acorde convert input.mid output.musicxml
acorde convert input.musicxml output.abc
acorde convert input.musicxml output.mei
acorde info input.musicxml
acorde validate input.musicxml
acorde preflight input.musicxml
acorde preflight input.musicxml --fail-on-issues
acorde validate guitar.musicxml       # includes tablature line/tuning/string checks
acorde extract --part 0 input.musicxml part.musicxml
acorde transpose --semitones 2 input.musicxml transposed.musicxml
acorde normalize input.musicxml normalized.musicxml
acorde export-report input.musicxml exported.musicxml
acorde export-report input.musicxml exported.abc
acorde export-report input.musicxml exported.mei
acorde tab-position guitar.musicxml edited.musicxml --part 0 --measure 0 --note 1 --string 2 --fret 3
acorde auto-tab guitar.musicxml guitar-tabbed.musicxml
acorde auto-tab-report guitar.musicxml guitar-tabbed.musicxml
```

The CLI supports `.musicxml`, `.mxl`, `.mid`/`.midi`, `.abc`, `.mei`, `.mscz`, and `.mscx` input.
Conversion output is MusicXML, MIDI, ABC, or MEI.
`validate` performs the same local structural checks for tablature metadata and explicit string
positions; it does not require a SoundFont or network access.
`preflight` reports SVG renderer capability boundaries before rendering.
With `--fail-on-issues`, it also returns status 1 when any issue is detected.
`tab-position --clear` removes an explicit position; all indices are zero-based except the
one-based `--string` value.
`auto-tab` assigns missing single-note and chord positions while minimizing fret load and
movement between successive notes.
`auto-tab-report` additionally prints deterministic assignment and fret-load metrics as JSON.

## Development

```bash
cargo test --all-features --locked
cargo clippy --all --all-features --locked -- -D warnings
```

For the browser fixture:

```bash
wasm-pack build crates/wasm --target web
python3 -m http.server 8000
```

See [browser support](docs/browser-support.md), [performance](docs/performance.md), and
[visual regression](docs/visual-regression.md) for focused checks. The Score JSON schema is
currently version 1. See [CHANGELOG.md](CHANGELOG.md) for release notes and [README_ja.md](README_ja.md)
for a Japanese overview. Compatibility notes for older releases are in [migrations.md](docs/migrations.md).

## License

MIT OR Apache-2.0, at your option.
