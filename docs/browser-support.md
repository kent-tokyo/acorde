# Browser support and verification matrix

The reusable renderer has no browser or DOM dependency. The WASM package is built for
`wasm32-unknown-unknown` and the checked-in fixture is served as a plain ES module page.

| Surface | Verification | Status |
|---|---|---|
| Native Rust renderer | `cargo test --all`, deterministic SVG goldens | supported |
| WASM package | `wasm-pack build crates/wasm --target web` | supported |
| Chromium / Chrome | Playwright browser-contract smoke + reviewed screenshot baseline | verified |
| Firefox | Playwright browser-contract smoke + reviewed screenshot baseline | verified |
| WebKit | Playwright browser-contract smoke + reviewed screenshot baseline | verified |

The Chromium smoke page also exercises host-owned keyboard selection and hover state through the
stable `data-note-addr` hooks. The CI matrix compares Chromium, Firefox, and WebKit against the
checked-in baselines under `examples/browser/smoke.spec.mjs-snapshots/`. Baselines are reviewed
artifacts, not a substitute for the native SVG structural tests. The local verification used
Playwright 1.55.0 with Chromium 140, Firefox 141, and WebKit 26.

The legacy `wasm-pack test --headless --chrome` path uses a separately downloaded WebDriver.
The CI job pins `wasm-pack` 0.15.0, installs the current Chrome, and has an explicit timeout so
driver startup failures cannot hang the workflow. On the development Mac the same test passed
with the Chrome 151-matched driver; the Playwright matrix remains the cross-browser contract.
