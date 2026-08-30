# Migrating to acorde 0.3

acorde 0.3 keeps the `Score → LayoutResult → SVG` pipeline and the stable
`part:staff:measure:voice:note` address format introduced in 0.2.

The release adds reviewed browser screenshot baselines and makes the browser fixture's visual
contract part of CI. Hosts should continue to own selection, hover, and playback state; the Rust
renderer remains synchronous and stateless. If a host has its own Playwright suite, use the same
browser-specific baselines and the same 1% pixel-difference budget.

The publish workflow is manual and reads `CARGO_REGISTRY_TOKEN` from GitHub Actions secrets. A
release should be made from a clean `v0.3.x` tag after all workspace crates have the same semver
version. No generated `crates/wasm/pkg` files are required in the source repository.
