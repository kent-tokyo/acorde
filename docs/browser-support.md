# Browser support and verification matrix

The reusable renderer has no browser or DOM dependency. The WASM package is built for
`wasm32-unknown-unknown` and the checked-in fixture is served as a plain ES module page.
The current render metadata contract is version 22 and includes score-level `score_texts`, note-level MusicXML placement
offsets and typed guitar techniques in addition to position-aware measure-level
`text_annotations` with source placement and offset fields, plus deterministic `tablature_positions`,
in addition to note `address_bounds`. Note semantics also expose ordered stable articulation names,
canonical beat durations, and exact pitch values in MIDI cents.
Typed `tablature_technique_connections` expose stable
start/end note objects, string numbers, and whether a connection crosses a measure boundary.

| Surface | Verification | Status |
|---|---|---|
| Native Rust renderer | `cargo test --all-features --locked`, deterministic SVG goldens | supported |
| WASM package | `wasm-pack build crates/wasm --target web` | supported |
| Chromium / Chrome | Playwright browser-contract smoke + reviewed screenshot baseline | verified |
| Firefox | Playwright browser-contract smoke + reviewed screenshot baseline | verified |
| WebKit | Playwright browser-contract smoke + reviewed screenshot baseline | verified |

The Chromium smoke page also exercises host-owned keyboard selection and hover state through the
stable `data-note-addr` hooks. The CI matrix compares Chromium, Firefox, and WebKit against the
checked-in baselines under `examples/browser/smoke.spec.mjs-snapshots/`. Baselines are reviewed
artifacts, not a substitute for the native SVG structural tests. The local verification on
2026-09-12 used Playwright 1.55.0 with Chromium 140, Firefox 141, and WebKit 26; all three
browser-contract smoke runs passed after the typed metadata contract update, and the Chromium
HiDPI profile remains covered by the checked-in matrix.

The legacy `wasm-pack test --headless --chrome` path uses a separately downloaded WebDriver.
The CI job pins `wasm-pack` 0.15.0, installs the current Chrome, and has an explicit timeout so
driver startup failures cannot hang the workflow. Earlier local runs on 2026-09-06 and 2026-09-12
reached the runner but did not complete successfully; those are retained as infrastructure history,
not browser-test evidence. On 2026-09-22, the locally installed `wasm-pack` 0.13.1 and Chromium
completed all 15 `acorde-wasm` headless Chrome tests. This is a local browser smoke result; the
Playwright matrix remains the cross-browser visual-contract evidence.
