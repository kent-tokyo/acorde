# Browser support and verification matrix

The reusable renderer has no browser or DOM dependency. The WASM package is built for
`wasm32-unknown-unknown` and the checked-in fixture is served as a plain ES module page.

| Surface | Verification | Status |
|---|---|---|
| Native Rust renderer | `cargo test --all`, deterministic SVG goldens | supported |
| WASM package | `wasm-pack build crates/wasm --target web` | supported |
| Chromium / Chrome | Playwright browser-contract smoke | verified locally |
| Firefox | Playwright browser-contract smoke | verified locally |
| WebKit | Playwright browser-contract smoke | verified locally |

The Chromium smoke page also exercises host-owned keyboard selection and hover state through the
stable `data-note-addr` hooks. The CI matrix captures Chromium, Firefox, and WebKit screenshots
as review artifacts. A reviewed pixel-diff baseline is still deferred until browser images are
pinned; SVG structural and deterministic tests remain the cross-platform assertion in the
meantime. The local verification used Playwright 1.55.0 with Chromium 140, Firefox 141, and
WebKit 26.

The legacy `wasm-pack test --headless --chrome` path uses a separately downloaded WebDriver.
The CI job pins `wasm-pack` 0.15.0, installs the current Chrome, and has an explicit timeout so
driver startup failures cannot hang the workflow. On the development Mac the same test passed
with the Chrome 151-matched driver; the Playwright matrix remains the cross-browser contract.
