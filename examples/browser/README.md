# Browser contract fixture

This page is a dependency-free browser harness for the documented WASM pipeline. Build the
package, serve the repository root, and open `/examples/browser/`:

```bash
wasm-pack build crates/wasm --target web
python3 -m http.server 8000
```

The page verifies `parse_musicxml` → `compute_layout_ex` →
`render_score_svg_with_layout` / `render_score_svg_row` / `render_score_metadata`, including
stable `address_bounds` coverage. It also provides an offline reference workflow for loading a
local MusicXML file, editing and applying the source, undoing/redoing edits, running analysis,
selecting and playing notes, and exporting MusicXML. It is intentionally framework-free so it can
also serve as a smoke test for future browser integrations.
The editor always exposes canonical MusicXML, while the WASM score JSON remains an internal
transport representation; applying an edit and undoing or redoing it therefore preserves the
document boundary.

After loading, each `data-note-addr` group is made keyboard-focusable by the host fixture. Click
or focus a note and press Enter/Space to exercise address-based selection; hover state is kept in
the host as `data-hover`. The Play button uses a small Web Audio oscillator for the selected note,
showing that playback also remains a host concern. This deliberately demonstrates that
selection/highlighting/playback belong to the browser host rather than the stateless Rust renderer.

`acorde-adapter.ts` is a dependency-free, framework-neutral starting point for a browser
workspace. Inject the generated WASM module into `AcordeWorkspace`; it keeps score, layout,
metadata, and analysis transport consistent while `SelectionStore` synchronizes stable note
addresses across notation and analysis views. Results are cached by the WASM-provided analysis
cache key plus layout or render configuration, so repeated view updates and equivalent revisions do
not rerun WASM analysis or rendering.
`analysisCacheKey()` and the `analysis-cache-key` Worker request expose the same schema-versioned
score identity to a host-level or persistent cache without duplicating hashing logic in JavaScript.
`WorkspaceSnapshot.analysisCacheKey` carries that identity alongside the analysis result for hosts
that persist or synchronize snapshots as one unit.
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
`playbackEventAt` resolves the active sounding event at a host clock time, and `selectPlaybackAt`
combines that lookup with selection updates; `WorkspaceSnapshot.selectedAddress` makes the same
state available to persistence and view synchronization code.
`WorkspaceRequest` and `handleWorkspaceRequest` provide a serializable message boundary for
Worker hosts; the same handler returns correlated success or structured error responses for load,
edit, render, analysis, playback, and export operations.
`encodeScoreJson`, `decodeScoreJson`, `scoreJsonBytes`, and the `replace-score-bytes` request use
UTF-8 `Uint8Array` transport, preserving the string API while allowing Worker structured-clone
messages to avoid carrying an additional JavaScript string representation.
The `undo` and `redo` requests return `{ changed, snapshot, history }`, and `history-state`
provides the revision and undo/redo availability without requiring a full render or analysis
round trip.
`select-address` updates the shared `SelectionStore` from a Worker host and returns the current
revision plus selected address; `selection-state` reads the same lightweight state without
rendering or analysis.
