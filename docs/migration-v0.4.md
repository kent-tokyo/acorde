# Migrating to acorde v0.4

The v0.4 release adds a versioned browser metadata contract to `acorde-render-svg` and its WASM
bindings. Existing SVG and layout calls are unchanged.

`render_score_metadata` now returns `contract_version: 1`, score counts, and `accessible_text` in
addition to dimensions and `address_bounds`. Browser hosts should check `contract_version` before
using fields introduced by a newer renderer. The text can be assigned to an accessible element
referenced by the score container's `aria-describedby` attribute when the host cannot expose SVG
semantics directly.

The existing `address_bounds` entries retain their stable `part`, `staff`, `measure`, `voice`, and
`note` addressing. No filesystem, DOM, or async-runtime dependency was added to the renderer.
