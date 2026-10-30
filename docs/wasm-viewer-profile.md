# Viewer-oriented WASM build

The default `acorde-wasm` feature set remains the complete API. A browser viewer
that imports ABC and MSCZ and displays scores can opt into the smaller profile:

```sh
wasm-pack build crates/wasm --target web --release \
  --out-dir ../../target/wasm-profiles/viewer \
  --no-default-features --features viewer
```

`viewer` retains ABC and MSCZ parsing and export, `ScoreEngine`, layout, SVG
rendering and note addresses, beam computation, chord detection, analysis,
playback events, and MSCZ export. The default `full` feature retains every
previously exported binding. `omr` remains independently opt-in.

| Feature | Viewer | Full |
| --- | --- | --- |
| ABC and MSCZ/MSCX import; ABC and MSCZ/MSCX export | yes | yes |
| Score editing, analysis, beam and chord helpers, layout and SVG | yes | yes |
| Playback events, offline render manifest, timing helpers | yes | yes |
| MusicXML/MXL import and MusicXML export | no | yes |
| MIDI import/export | no | yes |
| Guitar Pro import | no | yes |
| MEI import/export | no | yes |
| SoundFont inspection/materialization | no | yes |
| OMR | opt-in `omr` | opt-in `omr` |

Individual `musicxml`, `midi`, `gp`, `mei`, and `soundfont` features can be
added to `viewer` for hosts that need them. The generated `acorde_wasm.d.ts`
is the exact export manifest for each build. The viewer omits 22 functions
from the default 104 function exports; it keeps the other 82 unchanged.

## Reproduce the measurements

```sh
bash scripts/build-wasm-profiles.sh > target/wasm-profile-sizes.json
PLAYWRIGHT_MODULE=/path/to/playwright/index.mjs \
  node scripts/benchmark-wasm-viewer.mjs > target/wasm-browser-benchmark.json
```

The first script builds the default and viewer profiles and five one-feature
additions with the same installed Rust toolchain, `wasm-pack`, release profile,
and `wasm-opt`. It prints raw, gzip level 9, and Brotli quality 11 bytes plus
the full export list. The second script requires Playwright and an installed
Chrome. It serves the same WASM binaries with Brotli over local HTTP, disables
the browser cache, and emulates 80 ms latency with 1 MB/s download and upload.
It uses a fresh browser context for each of three runs per profile and fixture.
The reported transfer, initialization, parse, and first SVG durations are
separate. The setup phase generates one 4-part, 8-staff, 32-bar MSCZ fixture
from the original tiny ABC tune; both profiles receive identical bytes.

Measured locally on 2026-10-30 with macOS arm64, Rust 1.97.0
(`2d8144b78`), `wasm-pack` 0.15.0, `wasm-opt` 130, Node 24.5.0,
Playwright 1.60.0, and Chrome 156.0.8078.74. The source base is v1.3.0
(`421f26b2e6226343a30e79ddacdab5a93ee5153d`) plus the feature-profile
changes in this PR. Each row used the same release optimization settings.

| Profile | Raw bytes | gzip bytes | Brotli bytes |
| --- | ---: | ---: | ---: |
| Full | 5,496,504 | 1,771,356 | 1,203,214 |
| Viewer | 4,486,312 | 1,421,069 | 961,382 |

The [size and export record](wasm-viewer-sizes.json) lists every measured
profile and generated function export.

The viewer saves 1,010,192 raw bytes, 350,287 gzip bytes, and 241,832
Brotli bytes from this full build. The size contribution of each removable
feature was measured by adding only that feature to the viewer:

| Viewer plus one feature | Raw bytes | gzip bytes | Brotli bytes | Brotli increase |
| --- | ---: | ---: | ---: | ---: |
| MusicXML/MXL | 4,809,988 | 1,527,776 | 1,035,688 | 74,306 |
| MIDI | 4,622,042 | 1,471,091 | 994,586 | 33,204 |
| Guitar Pro | 4,664,888 | 1,493,682 | 1,014,389 | 53,007 |
| MEI | 4,833,623 | 1,541,101 | 1,045,252 | 83,870 |
| SoundFont | 4,548,386 | 1,437,059 | 969,259 | 7,877 |

These increments are not additive: the formats share dependencies and
compression patterns. All five together produce the full-size row above.

The [per-run measurement record](wasm-viewer-benchmark.json) contains the
individual trials and response byte counts.

The medians of three cold Chrome runs (milliseconds) were:

| Profile and input | WASM transfer | Init | Parse | First SVG |
| --- | ---: | ---: | ---: | ---: |
| Full, tiny ABC (2 bars) | 1,300.6 | 6.8 | 5.2 | 27.3 |
| Viewer, tiny ABC (2 bars) | 1,055.6 | 5.5 | 5.2 | 27.3 |
| Full, generated MSCZ (4 parts, 8 staves, 32 bars) | 1,300.8 | 6.5 | 30.5 | 64.7 |
| Viewer, generated MSCZ (4 parts, 8 staves, 32 bars) | 1,056.2 | 5.9 | 31.6 | 64.3 |

The encoded WASM response bodies were 1,203,214 bytes for full and 961,382
bytes for viewer, matching Brotli size. The cold transfer medians differ by
about 245 ms under the specified emulation. These are local observations,
not a cross-device latency guarantee. Browser caching, preloading, CDN
compression, and network policy belong to the host.
