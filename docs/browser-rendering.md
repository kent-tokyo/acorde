# Browser rendering contract

The browser integration keeps the same pipeline as native callers:

```text
parse_musicxml(xml) -> score JSON -> compute_layout_ex(score, config) -> render_score_svg_with_layout
```

The WASM boundary enforces 16 MiB for score JSON, 32 MiB for precomputed layout JSON, and 64 KiB
for layout/playback/render options JSON. Other small JSON arguments are bounded at 256 KiB. An invalid score,
layout, options object, or row index returns a rejected `Result`/`JsValue`; no filesystem or
async runtime is involved.

For optional SoundFont playback, `acorde-soundfont` validates SF2/SF3 metadata
and manages note/voice lifecycle actions. Sample decoding and audio output stay
outside the core/WASM contract and remain application-owned.

## Stable calls

- `compute_layout_ex(score_json, config_json)` returns the serialized `LayoutResult`.
- `render_score_svg_with_layout(score_json, layout_json, options_json)` renders the complete
  score using that layout.
- `render_preflight(score_json)` returns source-located JSON issues for renderer capability
  boundaries before SVG emission (unsupported staff/measure clefs, accidentals, and tablature
  positions).
- `render_score_svg_row(score_json, layout_json, row, options_json)` renders one zero-based
  system, which is the unit a virtualized viewport can cache.
- `render_score_metadata(score_json, layout_json, options_json)` returns a versioned metadata
  object with `contract_version`, `width`, `height`, `part_count`, `staff_count`, `measure_count`,
  `note_count`, `accessible_text`, `address_bounds`, and `text_annotations`. Each bound contains `part`, `staff`,
  `measure`, `voice`, and `note`, so a host can map hit testing and playback highlighting back to
  `NoteAddr` without parsing SVG. Each text annotation contains `part`, `staff`, `measure`,
  `style`, `text`, `placement`, `offset_x`, `offset_y`, `relative_x`, and `relative_y`, so measure-level
  styled text and its source coordinate hints remain available to host views. Use
  `accessible_text` as the text alternative when the host
  cannot expose SVG semantics; check `contract_version` before consuming newer fields.

`SvgRenderOptions` defaults are `width: 900`, `staff_size: 24`, `measures_per_system: 4`, and
`interactive: true`. Interactive SVG groups carry `data-note-addr="part:staff:measure:voice:note"`.
The host owns selection state: it may apply a CSS class or overlay after selecting an address;
the Rust renderer remains stateless.

For repeated browser analysis, the WASM `AnalysisCache` class mirrors the deterministic Rust cache:
it supports bounded single-score and batch analysis, editor replacement, explicit invalidation,
and JSON hit/miss statistics. Cache capacity is caller-owned and zero capacity is rejected.
`capacity()`, `len()`, and `is_empty()` expose bounded cache occupancy without inspecting result
payloads.
`diff_analysis(previous_result_json, current_result_json)` reports changed analysis categories
without requiring the browser host to compare result object graphs itself.
`AnalysisCache.analyze_after_edit_with_diff(previous_score_json, previous_result_json,
current_json)` combines replacement analysis with the same category diff, so an editor can update
its result and decide which views to refresh from one deterministic response.
`AnalysisCache.analyze_selected_after_edit(previous_score_json, previous_result_json, current_json,
categories_json)` recomputes only the requested categories and returns a complete merged result;
the caller must supply the immediately preceding result and categories from the refresh planner.
`AnalysisCache.analyze_after_edit_with_hint(previous_score_json, previous_result_json,
current_json, change_hint_json)` connects that planner directly to the editor command result and
returns the edited analysis with its category diff.
`analysis_refresh_plan(change_hint_json)` separates local and global dependencies and provides
one-measure boundary context for host-side scheduling.
The Rust analysis layer now applies the supplied region to chord-pass traversal and merges the
refreshed region with preserved outside results.
`analysis_provenance(analysis_json, address_json)` provides a deterministic “why” lookup for a
selected note address, including the rule ID, confidence, and source evidence for each finding.
Interval analysis also accepts the region and includes observations touching the edited range,
so boundary notes are not silently omitted.
Voice-leading analysis now traverses only the selected region and preserves outside observations
when merged into an incremental result.
SATB diagnostics use the same bounded voice-leading input and replace only diagnostics whose
evidence touches the selected region, preserving findings elsewhere in the score.

## Incremental updates

`ScoreEngine.apply`, `undo`, and `redo` return a serialized `ChangeHint`. Use `scope` to identify
the affected part or measure range, `layout_dirty` to decide whether to recompute
`LayoutResult`, and `playback_dirty` to decide whether to regenerate playback events. A host can
re-render only affected rows with `render_score_svg_row`; no full-score DOM replacement is
required.
