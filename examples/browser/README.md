# Browser contract fixture

This page is a dependency-free browser harness for the documented WASM pipeline. Build the
package, serve the repository root, and open `/examples/browser/`:

```bash
wasm-pack build crates/wasm --target web
python3 -m http.server 8000
```

The page verifies `parse_musicxml` → `compute_layout_ex` →
`render_score_svg_with_layout` / `render_score_svg_row` / `render_score_metadata`, including
stable `address_bounds` coverage. It is intentionally framework-free so it can also serve as a
smoke test for future browser integrations.

After loading, each `data-note-addr` group is made keyboard-focusable by the host fixture. Click
or focus a note and press Enter/Space to exercise address-based selection; hover state is kept in
the host as `data-hover`. The Play button uses a small Web Audio oscillator for the selected note,
showing that playback also remains a host concern. This deliberately demonstrates that
selection/highlighting/playback belong to the browser host rather than the stateless Rust renderer.

`acorde-adapter.ts` is a dependency-free, framework-neutral starting point for a browser
workspace. Inject the generated WASM module into `AcordeWorkspace`; it keeps score, layout,
metadata, and analysis transport consistent while `SelectionStore` synchronizes stable note
addresses across notation and analysis views. Results are cached by score revision plus layout or
render configuration, so repeated view updates do not rerun WASM analysis or rendering.
`renderRowSvg(rowIndex)` exposes the same contract one logical row at a time, which lets a host
virtualize long scores without moving row/index arithmetic into the UI.
Parse, layout, render, metadata, and analysis failures are raised as `AcordeWorkspaceError` with
a typed operation field, allowing a host to show an actionable diagnostic without matching error
message text. `replaceScoreJson`, `undo`, and `redo` provide a small host-facing edit history;
failed layout preparation leaves the current score unchanged.
`loadMidi` adds binary MIDI loading with the same transactional replacement behavior, while
`exportMusicXml` provides offline MusicXML export. `playbackEvents`, `playbackPosition`, and
`durationSeconds` expose deterministic scheduling data to a host audio backend without coupling
the adapter to Web Audio or another playback framework.
`loadMusicXmlWithReport` and `exportMusicXmlWithReport` additionally return structured
interchange diagnostics, including preserved values and loss reasons when the format adapter
has such information, so hosts can present repair guidance without parsing error strings.
Each sounding playback event carries the stable source address used by `data-note-addr`, and
`selectPlaybackEvent` sends that address through the shared `SelectionStore` for notation and
analysis highlighting.
