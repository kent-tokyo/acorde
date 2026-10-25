# Browser support and verification

`acorde-render-svg` has no browser or DOM dependency. `acorde-wasm` targets
`wasm32-unknown-unknown`; `examples/browser/` is a plain ES-module fixture that checks the
browser/WASM contract rather than providing a production UI.

| Surface | Required evidence | Current status |
|---|---|---|
| Native renderer | Rust tests and deterministic SVG goldens | supported |
| WASM build | `wasm-pack build crates/wasm --target web` | supported |
| Chromium, Firefox, WebKit | Playwright contract smoke and reviewed PNG baselines | verified fixture path |
| HiDPI Chromium | Playwright fixture baseline | verified fixture path |

The fixture checks safe SVG mounting, note selection/hover hooks, transport controls, accessibility
names, and source-address highlighting. Baselines under
`examples/browser/smoke.spec.mjs-snapshots/` are review artifacts, not a substitute for structural
SVG tests or a cross-browser audio-quality claim.

The most recently recorded local browser observation is 2026-09-12 with Playwright 1.55.0,
Chromium 140, Firefox 141, and WebKit 26. It passed the checked fixture matrix. This dated local
result does not prove compatibility with newer browser releases; CI and a fresh browser run are
required for a release claim.

`wasm-pack test --headless --chrome` is a separate smoke path with a WebDriver dependency. The CI
job pins `wasm-pack` 0.15.0 and uses an explicit timeout. A local 2026-09-22 run completed 15
`acorde-wasm` headless-Chrome tests with wasm-pack 0.13.1; earlier unsuccessful starts are retained
as infrastructure history, not test evidence.

See the [browser contract](browser-rendering.md) for integration rules and
[`examples/browser/README.md`](../examples/browser/README.md) for fixture commands.
