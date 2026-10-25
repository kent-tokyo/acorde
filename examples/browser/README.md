# Browser contract fixture

This dependency-free page is a smoke-test host for the documented WASM pipeline. It is not a
product UI or a MuseScore compatibility layer.

```bash
wasm-pack build crates/wasm --target web
python3 -m http.server 8000
```

Open `/examples/browser/` after serving the repository root. The page exercises the canonical
`parse_musicxml` → layout → SVG path, typed render metadata, source-address hit testing, analysis,
editing with undo/redo, and MusicXML export. Browser smoke tests also cover tablature staff
metadata and the versioned analysis result.

## Practice preview

The fixture is an offline reference implementation for host-owned playback. It provides transport,
seek, a 50–200% tempo multiplier, inclusive measure loops, metronome, a 0–8 beat count-in,
musical position, and source-address highlighting. Seek and option changes rebuild a bounded Web
Audio look-ahead schedule; its trace is for contract review, not audio-quality measurement.

It deliberately uses oscillators. SoundFonts, synthesis quality, output-device access, mixer
controls, asset licensing, and browser latency are application responsibilities. The selected-note
button is only a selection and scheduling diagnostic.

## Adapter

`acorde-adapter.ts` is a framework-neutral starting point for a browser workspace. Supply the
generated WASM module to `AcordeWorkspace`; it maintains score, layout, render metadata, analysis,
selection, and transactional edit history. `WorkspaceRequest` and `handleWorkspaceRequest` expose
the same bounded operations for Worker hosts.

The generated TypeScript declarations are authoritative for method names, request payloads, result
fields, and contract versions. The adapter covers canonical imports and exports, render preflight,
validation, incremental row rendering, deterministic playback events, tablature projection, and
format conversion reports. It never turns SVG metadata into trusted HTML or chooses audio, fonts,
PDF, or UI policy for the host.

Snapshots carry a schema version and stable note addresses. Hosts should reject unsupported
versions, treat every `data-*` value as an opaque identifier, and keep selection outside the
stateless Rust renderer.

## Production integration

Parse returned SVG as `image/svg+xml`, require an SVG root in the SVG namespace, and reject scripts,
event-handler attributes, and `javascript:` links before inserting it into the DOM. Use a restrictive
CSP, Trusted Types where available, and a Worker for expensive parsing or rendering.

See [the browser rendering contract](../../docs/browser-rendering.md) for integration boundaries
and the generated bindings for the complete, versioned surface.
