# Browser and WASM contract

A browser host uses the same canonical pipeline as native callers:

```text
source bytes/text -> parse_* -> Score JSON -> compute_layout_ex -> render_score_svg_with_layout
```

`parse_*_render_svg` is the one-call preview path for MusicXML/MXL, MEI, ABC, MIDI, MSCX, and
MSCZ. It uses the same parser and renderer as the expanded pipeline. Use a matching
`parse_*_report` or `serialize_*_report` whenever import diagnostics, archive limits, or output
loss matter.

The WASM boundary accepts bounded JSON: score JSON is limited to 16 MiB, precomputed layout JSON
to 32 MiB, render/layout/playback options to 64 KiB, and other small JSON arguments to 256 KiB.
Invalid input returns `Result`/`JsValue`; it does not access the filesystem or run an async runtime.

## Stable integration surface

Generated wasm-bindgen TypeScript declarations are authoritative for signatures and complete JSON
shapes. The stable groups below are the intended browser boundary.

| Need | Calls |
|---|---|
| Import/export | `parse_*`, `serialize_*`, and their `*_report` variants |
| Layout and SVG | `compute_layout_ex`, `render_score_svg_with_layout`, `render_score_svg_row`, `render_preflight` |
| Host metadata | `render_score_metadata`, `svg_contract_version`, glyph-resource validation |
| Editing | `ScoreEngine`, score patch/diff helpers, structural planning, canonical validation |
| Playback | `to_playback_events_ex`, `compute_playback_position`, `compare_playback_timing` |
| Print planning | `compute_print_layout`, page-render-tree export/validation |
| Optional adapters | OMR proposal recognition, analysis, SoundFont metadata, and tablature projection |

`render_score_metadata` is the typed alternative to parsing SVG geometry. It provides dimensions,
stable note addresses, accessible text, measure/score text, note semantics, and TAB positions and
technique connections. Hosts must check `contract_version` before relying on added fields; opaque
addresses and `data-*` values must never be turned into markup or URLs.

Interactive SVG groups use `data-note-addr="part:staff:measure:voice:note"`. A host owns
selection, hover, CSS, and viewport behavior. `render_score_svg_row` is the cacheable unit for a
virtualized score view. `render_preflight` must be used when a host needs to reject unsupported
clefs, accidentals, TAB positions, invalid text, or oversize inputs before mounting SVG.

## Playback and practice hosts

`SvgRenderOptions` defaults are `width: 900`, `staff_size: 24`,
`measures_per_system: 4`, and `interactive: true`. It deliberately retains these four public
fields so existing Rust struct literals remain source-compatible. Configure the clear vertical
distance between adjacent staves through
`SvgRenderConfig::new(options).with_staff_gap(10.0)`. The gap is measured in staff spaces and
must be finite and positive; see the [migration notes](migrations.md).

`to_playback_events_ex` produces the expected score schedule; it does not synthesize audio.
`compute_playback_position` maps elapsed time to a typed musical position using the same tempo-ramp
calculation. A host can compare its scheduler trace through `compare_playback_timing`; the default
tolerance is 5 ms for event start and duration. This validates event identity/timing only, not Web
Audio clocks, device latency, SoundFont quality, mixing, or rendered PCM.

The checked-in browser fixture demonstrates a deliberately small practice host: transport,
seek/rewind, a 50–200% tempo multiplier, an inclusive measure loop, metronome, 0–8 beat count-in,
musical position, and source-address highlighting. It uses an oscillator preview only. The
multiplier scales the authored tempo map rather than changing score data; seeking rebuilds future
scheduling. Per-part mute/solo, audio providers, follow-scroll policy, asset licences, and output
device selection remain host responsibilities.

## Safe mounting and incremental updates

Treat SVG and every source document as untrusted input. The fixture parses SVG as `image/svg+xml`,
requires an SVG root in the SVG namespace, rejects scripts, event handlers, and `javascript:`
links, then imports the validated tree. Production hosts should retain that boundary, use a
restrictive CSP, enforce Trusted Types where available, and prefer a Worker for parsing/rendering.

`ScoreEngine.apply`, `undo`, and `redo` return a `ChangeHint`. Use `layout_dirty` and
`playback_dirty` to recompute only the affected projection, then re-render rows as needed. Browser
hosts own DOM replacement, animation, focus restoration, reduced-motion policy, and accessibility
announcements.

For support evidence and the checked browsers, see [browser support](browser-support.md). For
notation limits, see the [coverage matrix](notation-coverage.md).
