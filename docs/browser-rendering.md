# Browser rendering contract

The browser integration keeps the same pipeline as native callers:

```text
parse_musicxml(xml) -> score JSON -> compute_layout_ex(score, config) -> render_score_svg_with_layout
```

For the documented MEI subset, `parse_mei_render_svg(xml, options_json)` provides a bounded
one-call parse-to-SVG path. It uses the same canonical score and renderer as the expanded
pipeline; call `parse_mei_report` when source-located import diagnostics are required.
The ABC parser has the equivalent `parse_abc_render_svg(text, options_json)` convenience path;
call `parse_abc_report` when ABC import diagnostics are required.
MusicXML has the equivalent `parse_musicxml_render_svg(xml, options_json)` convenience path;
call `parse_musicxml_report` when MusicXML import diagnostics are required.
Compressed MusicXML has `parse_mxl_render_svg(data, options_json)`; use `parse_mxl_report` when
archive and import diagnostics are required.
MIDI bytes have the equivalent `parse_midi_render_svg(data, options_json)` path; MIDI's bounded
performance-to-score projection is preserved and `parse_midi_report` remains the diagnostic path.
MuseScore XML and archive inputs have equivalent `parse_mscx_render_svg(xml, options_json)` and
`parse_mscz_render_svg(data, options_json)` paths; use the corresponding report APIs for import
diagnostics and archive safety details.

The WASM boundary enforces 16 MiB for score JSON, 32 MiB for precomputed layout JSON, and 64 KiB
for layout/playback/render options JSON. Other small JSON arguments are bounded at 256 KiB. An invalid score,
layout, options object, or row index returns a rejected `Result`/`JsValue`; no filesystem or
async runtime is involved.

For optional SoundFont playback, `acorde-soundfont` validates SF2/SF3 metadata
and manages note/voice lifecycle actions. Sample decoding and audio output stay
outside the core/WASM contract and remain application-owned.

Playback events retain the legacy string `address` and also expose an optional typed `source`
`NoteAddr`; metronome events set both source fields to `null`. Browser hosts should prefer the
typed field and use the string only for compatibility with older snapshots.

## Stable calls

- `parse_mei_render_svg(xml, options_json)` parses the documented MEI subset and renders its
  canonical score directly to SVG.
- `parse_abc_render_svg(text, options_json)` parses ABC Notation and renders its canonical score
  directly to SVG.
- `parse_musicxml_render_svg(xml, options_json)` parses MusicXML and renders its canonical score
  directly to SVG.
- `parse_mxl_render_svg(data, options_json)` parses compressed MusicXML and renders its canonical
  score directly to SVG.
- `parse_midi_render_svg(data, options_json)` parses MIDI bytes and renders the bounded canonical
  score projection directly to SVG.
- `parse_mscx_render_svg(xml, options_json)` parses MuseScore XML and renders its bounded
  canonical score directly to SVG.
- `parse_mscz_render_svg(data, options_json)` parses a MuseScore archive and renders its bounded
  canonical score directly to SVG.
- `serialize_mscx(score_json)` and `serialize_mscz(score_json)` emit the deterministic canonical
  MuseScore subset for text or binary browser downloads. These serializers do not claim full
  MuseScore feature or byte-for-byte compatibility.
- `compute_layout_ex(score_json, config_json)` returns the serialized `LayoutResult`.
- `compute_print_layout(score_json, print_config_json)` returns the serialized physical
  `PrintLayoutResult` for page-aware hosts. It carries millimetre geometry and publication
  metadata; PDF generation, font resolution, and printer APIs remain host-owned.
- `render_score_svg_with_layout(score_json, layout_json, options_json)` renders the complete
  score using that layout.
- `render_preflight(score_json)` returns source-located JSON issues for renderer capability and
  text-input boundaries before SVG emission (unsupported staff/measure clefs, accidentals,
  tablature positions, oversized/non-finite text inputs, and XML-incompatible text characters).
- `render_score_svg_row(score_json, layout_json, row, options_json)` renders one zero-based
  system, which is the unit a virtualized viewport can cache.
- `render_score_metadata(score_json, layout_json, options_json)` returns a versioned metadata
  object with `contract_version`, `width`, `height`, `part_count`, `staff_count`, `measure_count`,
  `note_count`, `accessible_text`, `address_bounds`, `score_texts`, `text_annotations`, `tablature_positions`,
  `tablature_staves`, `tablature_technique_connections`, and `note_semantics`. Each bound contains `part`, `staff`,
  `measure`, `voice`, and `note`, so a host can map hit testing and playback highlighting back to
  `NoteAddr` without parsing SVG. Each text annotation contains `part`, `staff`, `measure`,
  `style`, `text`, `placement`, `offset_x`, `offset_y`, `relative_x`, and `relative_y`, so measure-level
styled text and its source coordinate hints remain available to host views. Use
`accessible_text` as the text alternative when the host
cannot expose SVG semantics; check `contract_version` before consuming newer fields.
Each `score_texts` entry contains the score-level style, text, and optional placement offsets
imported from title-page structures such as MuseScore `VBox`, so browser editors can retain those
annotations without reparsing the source format.
Each `tablature_positions` entry contains `part`, `staff`, `measure`, `voice`, `note`,
`position`, `string`, and `fret`, so a host can address one position inside a tab chord without
parsing SVG. Each `tablature_staves` entry contains `part`, `staff`, `lines`, `tuning_midi`, and
`capo`, so playback and editing hosts can resolve authored positions without reconstructing staff
tuning from SVG. Analysis chord results similarly include a canonical `name` beside structured chord
data; the analysis result schema is version 13. Each `note_semantics` entry contains the source
address, `is_unpitched`, `tie_start`, `tie_end`, `duration_beats`, `pitch_midi_cents`, optional `dynamic`, `lyric`, `chord_label`, `technique_text`, ordered
`articulations` names (including `tremolo-N`), `fingerings`, and `instrument_id`, plus one
`microtone_cents` value per source pitch and optional
MusicXML placement offsets (`offset_x`, `offset_y`, `relative_x`, `relative_y`), typed guitar
techniques and bend cents, so playback
and editor hosts can consume note identity and common annotations without parsing SVG or inferring
chord pitch order. Interactive note groups additionally expose
`data-acorde-instrument-id` when the source supplied `instrument@id`; for unpitched notes this
allows a host to resolve percussion sound identity from an allowlisted instrument catalog.
Microtone markers expose
`data-acorde-microtone-cents` and `data-acorde-pitch-index`; these are exact score values and must
be treated as opaque data, not as markup or URLs.

- `svg_contract_version()` returns the renderer metadata contract version so browser fixtures and
  generated WASM hosts can compare returned metadata without duplicating a version constant.
- `glyph_resource_contract_version()` returns the print glyph-resource metadata contract version;
  `validate_glyph_resource_descriptor(descriptor_json)` validates a host resource descriptor before
  publication export. It does not load fonts or verify licenses.

`AcordeWorkspace.validateGlyphResourceDescriptor()` and the Worker
`validate-glyph-resource` operation provide the same check without requiring a score to be loaded.
The adapter first compares the descriptor version with the WASM contract version, then delegates
the complete validation to Rust.

`SvgRenderOptions` defaults are `width: 900`, `staff_size: 24`, `measures_per_system: 4`, and
`interactive: true`. Interactive SVG groups carry `data-note-addr="part:staff:measure:voice:note"`.
The host owns selection state: it may apply a CSS class or overlay after selecting an address;
the Rust renderer remains stateless.
The adapter also exposes `formatNoteAddress`, `parseNoteAddress`, and the Worker
`select-note` request so typed metadata addresses can be selected without reconstructing the
legacy string form. `selectedNoteSemantic()` maps the current selection back to the typed
`note_semantics` entry.

The reference fixture demonstrates a safe DOM insertion boundary: it parses the returned SVG as
`image/svg+xml`, requires an SVG root in the SVG namespace, rejects script elements, event-handler
attributes, and `javascript:` links, then imports the validated tree with `importNode`. Production
hosts should keep the same validation step, use a restrictive CSP (`script-src 'self'` and no
untrusted inline execution), enforce Trusted Types where available, and prefer a Worker for WASM
parsing/rendering. Treat every `data-*` value as an opaque untrusted identifier; never turn it into
HTML or a URL without a separate allowlist.

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
selected note address, including the rule ID, confidence, source evidence, and canonical chord
label (when the finding is a chord) for each finding.
`explain_analysis_change(previous_json, current_json, address_json)` combines the before/after
provenance with the category diff in one response for explainable editor updates.
`compatibility_report(source_json, candidate_json)` returns deterministic score and analysis gate
booleans for two canonical score JSON values. It does not infer import/export losses; use the
format-specific report APIs when crossing a file-format boundary.
The browser contract includes a wasm-bindgen regression covering both explanation endpoints;
headless Chrome execution remains an environment-dependent gate.
Interval analysis also accepts the region and includes observations touching the edited range;
each interval observation retains a signed exact `cents` distance in addition to the legacy semitone value,
so microtonal analysis is not silently rounded away. Rests and pitchless events remain hard
melodic boundaries rather than being bridged by the interval pass. Voice-leading observations
likewise retain exact signed upper/lower motion in cents.
so boundary notes are not silently omitted.
Voice-leading analysis now traverses only the selected region and preserves outside observations
when merged into an incremental result.
SATB diagnostics use the same bounded voice-leading input and replace only diagnostics whose
evidence touches the selected region, preserving findings elsewhere in the score.

## Playback comparison contract

`to_playback_events_ex` produces the expected event schedule. A browser or Composer host can
submit its scheduled event trace to `compare_playback_timing(expected_json, actual_json,
tolerance_json)`. The response is a deterministic `PlaybackTimingReport` with contract version,
matched count, maximum start/duration error, and typed mismatches. The default tolerance is 5 ms
for both start time and duration. This checks event identity and timing only; Web Audio clock
behavior, SoundFont decoding, device latency, and rendered PCM remain host/provider-owned.

For fixture preparation outside WASM, the CLI command `acorde playback-report input.musicxml
--bpm 120 --loop-start 0 --loop-end 3` emits the same expected `PlaybackEvent` shape as JSON.
The optional range selects inclusive physical measures. The CLI is an input-side contract helper;
it does not claim to observe browser scheduling or audio output.

After a host writes its scheduled trace as JSON, `acorde playback-compare expected.json actual.json
--fail-on-mismatch` runs the same typed timing comparison locally. This remains event/timing
evidence only; it is not evidence of equivalent audio rendering.

The framework-neutral `examples/browser/acorde-adapter.ts` wraps these bindings into a
transactional workspace. It supports loading MusicXML/MXL/MEI/ABC/MIDI/MSCX/MSCZ, preserving
format import reports, and exporting MusicXML/MEI/ABC/MIDI/MSCX/MSCZ, including bounded export
reports for the MSCX/MSCZ subset. Its playback timing, tablature
projection, and canonical tab round-trip methods are host hand-off contracts; they do not add
audio synthesis, font selection, PDF generation, or browser UI to `acorde`.

For tablature-aware hosts, `project_tablature_performance(score_json, options_json)` returns
playback events paired with authored string/fret positions. It validates tuning and capo against
the sounding pitch, reports missing or invalid positions and microtonal pitch differences, and
never guesses a position. Each positioned event also carries the authored `GuitarTechnique` when
present (`bend`, `slide`, `hammer-on`, or `pull-off`) so hosts can select their own articulation.
For a bend, `bend_alter_cents` preserves the authored alteration amount when available.
The technique-connection metadata additionally identifies paired note addresses, string numbers,
and cross-measure boundaries without requiring SVG parsing. Harmony range endpoints and technique
connection endpoints use the same typed `{part, staff, measure, voice, note}` address object.
Automatic assignment remains an explicit caller step.

## Incremental updates

`ScoreEngine.apply`, `undo`, and `redo` return a serialized `ChangeHint`. Use `scope` to identify
the affected part or measure range, `layout_dirty` to decide whether to recompute
`LayoutResult`, and `playback_dirty` to decide whether to regenerate playback events. A host can
re-render only affected rows with `render_score_svg_row`; no full-score DOM replacement is
required.
