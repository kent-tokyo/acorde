# Browser rendering contract

The browser integration keeps the same pipeline as native callers:

```text
parse_musicxml(xml) -> score JSON -> compute_layout_ex(score, config) -> render_score_svg_with_layout
```

All JSON arguments are bounded by the host before crossing the WASM boundary. An invalid score,
layout, options object, or row index returns a rejected `Result`/`JsValue`; no filesystem or
async runtime is involved.

## Stable calls

- `compute_layout_ex(score_json, config_json)` returns the serialized `LayoutResult`.
- `render_score_svg_with_layout(score_json, layout_json, options_json)` renders the complete
  score using that layout.
- `render_score_svg_row(score_json, layout_json, row, options_json)` renders one zero-based
  system, which is the unit a virtualized viewport can cache.
- `render_score_metadata(score_json, layout_json, options_json)` returns `{ width, height,
  address_bounds }`. Each bound contains `part`, `staff`, `measure`, `voice`, and `note`, so a
  host can map hit testing and playback highlighting back to `NoteAddr` without parsing SVG.

`SvgRenderOptions` defaults are `width: 900`, `staff_size: 24`, `measures_per_system: 4`, and
`interactive: true`. Interactive SVG groups carry `data-note-addr="part:staff:measure:voice:note"`.
The host owns selection state: it may apply a CSS class or overlay after selecting an address;
the Rust renderer remains stateless.

## Incremental updates

`ScoreEngine.apply`, `undo`, and `redo` return a serialized `ChangeHint`. Use `scope` to identify
the affected part or measure range, `layout_dirty` to decide whether to recompute
`LayoutResult`, and `playback_dirty` to decide whether to regenerate playback events. A host can
re-render only affected rows with `render_score_svg_row`; no full-score DOM replacement is
required.
