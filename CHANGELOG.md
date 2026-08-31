# Changelog

All notable changes to acorde are documented here.
Score JSON schema changes are marked **[schema]** — consumers must handle
`#[serde(default)]` fields added in those versions.

The format follows [Keep a Changelog](https://keepachangelog.com/en/1.1.0/).

---

## [Unreleased]

## [0.76.0] - 2026-08-31

### Security contract and threat model

- Added the S0 threat model, resource-limit ownership table, error/disclosure contract, and
  per-crate security documentation links without overstating unfinished parser hardening.

## [0.75.0] - 2026-08-31

### Versioned public scorecard

- Added a machine-readable capability scorecard with explicit known gaps, evidence references,
  and security-gate policy aligned with the notation coverage matrix.

## [0.74.0] - 2026-08-31

### Headless export compatibility reports

- Added `acorde export-report` to export MusicXML or MIDI while emitting deterministic,
  machine-readable format, size, and loss-diagnostic metadata.

## [0.73.0] - 2026-08-31

### Headless normalization automation

- Added `acorde normalize` to parse, structurally validate, and canonically rewrite a score
  through the CLI boundary before exporting it as MusicXML or MIDI.

## [0.72.0] - 2026-08-31

### Headless transpose automation

- Added `acorde transpose --semitones` to expose the existing typed core transposition operation
  through the filesystem-owning CLI boundary, preserving the reusable crates' separation.

## [0.71.0] - 2026-08-31

### Deterministic render annotation extension points

- Added `RenderAnnotation` and `render_svg_with_annotations` for host-defined, XML-escaped SVG
  text marks with validated IDs, deterministic ordering, and no domain logic in the renderer.

## [0.70.0] - 2026-08-31

### Deterministic analysis extension points

- Added `AnalysisPass` and `run_analysis_passes` for external deterministic analysis, with stable
  ID ordering and typed validation for empty or duplicate IDs.

## [0.69.0] - 2026-08-31

### Benchmark fingerprint verification

- Added `--expected-fingerprint` to the local benchmark CLI so CI or offline scripts can reject
  manifest and fixture drift before accepting analysis results.

## [0.68.0] - 2026-08-31

### Benchmark corpus fingerprint

- Added a deterministic FNV-1a fingerprint of each benchmark manifest and its referenced fixture
  bytes to the CLI corpus metadata, making fixture drift visible in saved reports.

## [0.67.0] - 2026-08-31

### Benchmark provenance output

- Extended the local benchmark command to emit corpus identity, license, coverage, and provenance
  metadata alongside its deterministic analysis report.

## [0.66.0] - 2026-08-31

### Versioned analysis benchmark corpus

- Added a checked-in synthetic MusicXML benchmark corpus manifest with provenance, license, and
  hand-verified analysis category expectations for the local CLI benchmark command.

## [0.65.0] - 2026-08-31

### WASM duration editing

- Exposed `ScoreEngine.set_duration` for browser hosts, accepting JSON note addresses and duration
  values while returning the standard undoable `ChangeHint` contract.

## [0.64.0] - 2026-08-31

### Note duration editing

- Completed the `SetDuration` score command with a public `ScoreEngine::set_duration` helper,
  including dotted durations and measure-capacity preservation.

## [0.63.0] - 2026-08-31

### Browser snapshot restoration

- Added validated `restoreSnapshot()` and the `restore-snapshot` Worker request; persisted scores
  are reparsed into a fresh layout and stable selection addresses are restored.

## [0.62.0] - 2026-08-31

### Versioned browser snapshots

- Added `WorkspaceSnapshot.schemaVersion` and `WORKSPACE_SNAPSHOT_SCHEMA_VERSION` so persisted
  browser workspace state can explicitly detect future contract changes.

## [0.61.0] - 2026-08-31

### Browser snapshot cache identity

- Added `WorkspaceSnapshot.analysisCacheKey` so snapshot persistence and Worker synchronization
  can retain the same schema-versioned analysis identity as the result payload.

## [0.60.0] - 2026-08-31

### Browser analysis cache contract

- Exposed the schema-versioned analysis cache key through `AcordeWorkspace` and its Worker
  request boundary, allowing host-level caches to share the same score identity as WASM.

## [0.59.0] - 2026-08-31

### Browser analysis cache identity

- Updated the browser workspace adapter to use the WASM-provided, schema-versioned analysis cache
  key, allowing equivalent score revisions to reuse analysis results safely.

## [0.58.0] - 2026-08-31

### WASM analysis cache key

- Added a WASM `analysis_cache_key` export for cache invalidation without serializing a full
  analysis result.
- Added a native helper that computes the current schema-versioned key without running analysis.

## [0.57.0] - 2026-08-31

### Analysis score consistency

- Added `AnalysisResult::matches_score()` to reject stale analysis results after score changes.

## [0.56.0] - 2026-08-31

### Versioned analysis cache keys

- Added `AnalysisResult::cache_key()` combining the analysis schema and deterministic score
  fingerprint.
- Prevented cache reuse across analysis-result schema revisions.

## [0.55.0] - 2026-08-31

### Analysis fingerprint correction

- Corrected the deterministic score fingerprint to use the documented FNV-1a byte order.
- Added regression coverage ensuring changed score content produces a different fingerprint.

## [0.54.0] - 2026-08-31

### Deterministic analysis identity

- Added a deterministic score fingerprint to `AnalysisResult` for cache keys and reproducibility
  checks.
- Excluded generated score identifiers from the fingerprint so equivalent newly constructed
  scores produce the same key.
- Bumped the analysis result schema with backwards-compatible deserialization defaults.

## [0.53.0] - 2026-08-31

### Interchange report aggregation

- Added stable warning, error, and loss counters to `ImportReport` and `ExportReport`.
- Added `Diagnostic::is_loss()` for host-side filtering without matching serialized fields.

## [0.52.0] - 2026-08-31

### MEI score definition attribute diagnostics

- Added source-valued diagnostics for unsupported MEI score-definition attributes, including
  meter, key signature, and clef settings.

## [0.51.0] - 2026-08-31

### MEI score definition attribute diagnostics

- Added source-valued diagnostics for unsupported MEI score-definition attributes, including
  meter, key signature, and clef settings.

## [0.50.0] - 2026-08-31

### MEI score definition diagnostics

- Added explicit loss diagnostics for unsupported MEI `scoreDef` and `staffDef` elements.
- Prevented score-definition fallback from being mistaken for lossless MEI import.

## [0.49.0] - 2026-08-31

### MEI attribute loss diagnostics

- Added warnings for MEI `meter.count` and `meter.unit` attributes that are not represented by
  the current canonical score model.
- Preserved source attribute values and locations in import reports.

## [0.48.0] - 2026-08-31

### MEI staff and layer diagnostics

- Added loss diagnostics when non-primary MEI staff or layer numbers are flattened into the
  canonical single-staff/single-layer score model.
- Preserved the source `n` value in diagnostics for host-side repair guidance.

## [0.47.0] - 2026-08-31

### Bounded MEI loss diagnostics

- Added hierarchical source paths to MEI unsupported-element diagnostics.
- Bounded repeated MEI loss diagnostics to keep reports predictable for large inputs.

## [0.46.0] - 2026-08-31

### MEI loss diagnostics

- Added parser-backed MEI import diagnostics for known unsupported notation elements.
- Updated the versioned notation coverage matrix for the v0.46.x interchange boundary.

## [0.45.0] - 2026-08-31

### Security contract and CI gates

- Added a repository security contract covering trust boundaries, resource limits, browser
  integration, reporting, and release verification.
- Added cargo-deny license/source policy and CI advisory, license, and source checks.

## [0.44.0] - 2026-08-31

### Parser and dependency hardening

- Updated `quick-xml` to 0.41 and `crossbeam-epoch` to 0.9.20 to address known advisories.
- Added bounded MusicXML, MEI, MSCX, MXL, and MSCZ input and archive entry handling.
- Rejected DOCTYPE events across XML parser boundaries and made oversized archive reads fail
  instead of silently truncating input.

## [0.43.0] - 2026-08-31

### Browser document boundary

- Kept the reference editor on canonical MusicXML instead of exposing internal score JSON.
- Added browser workflow coverage for edit, undo, and redo round trips.

## [0.42.0] - 2026-08-31

### Offline browser workflow

- Expanded the dependency-free browser reference app with local MusicXML load, source editing,
  undo/redo, analysis, note playback, and MusicXML export controls.
- Extended the browser smoke contract to exercise the analysis action and workflow controls.

## [0.41.0] - 2026-08-31

### Worker selection synchronization

- Added Worker requests for updating and reading the shared stable note selection.
- Added a lightweight selection-state response with the current score revision.

## [0.40.0] - 2026-08-31

### Worker history synchronization

- Made Worker undo/redo responses include the changed flag, updated snapshot, and history state.
- Added a lightweight history-state request for UI controls that only need revision and
  undo/redo availability.

## [0.39.0] - 2026-08-31

### Compact browser score transport

- Added UTF-8 score JSON encode/decode helpers and byte-based workspace replacement for Worker
  structured-clone transport.
- Added a byte-oriented score request while preserving the existing string-based API.

## [0.38.0] - 2026-08-31

### Worker-friendly browser workspace protocol

- Added a framework-neutral workspace request/response boundary suitable for Worker message
  handlers, covering loading, editing, rendering, analysis, playback, and export.
- Preserved correlated request IDs and structured workspace errors across the message boundary.

## [0.37.0] - 2026-08-31

### Playback cursor synchronization

- Added browser adapter helpers to resolve and select the active sounding event at a playback
  time, while ignoring metronome events.
- Included the current stable selection address in `WorkspaceSnapshot`.

## [0.36.0] - 2026-08-31

### Playback selection synchronization

- Added stable source note addresses to sounding playback events, with `None` reserved for
  metronome events and chord pitches sharing their source note address.
- Added browser adapter support for forwarding playback-event selection through `SelectionStore`.

## [0.35.0] - 2026-08-31

### Multi-scale renderer budgets

- Expanded the reproducible renderer benchmark to small, medium, and large score cases with
  per-case layout latency, render latency, and SVG-size budgets.

## [0.34.0] - 2026-08-31

### Browser interchange diagnostics

- Exposed structured MusicXML import and export reports through the browser workspace adapter,
  including source-grounded diagnostic fields for host-provided repair and loss reporting.

## [0.33.0] - 2026-08-31

### Browser playback and export

- Added MIDI loading, MusicXML export, playback-event generation, playback-position lookup,
  and duration access to the browser adapter.

## [0.32.0] - 2026-08-31

### Browser workspace edit history

- Added score JSON replacement with transactional layout preparation and undo/redo history to the
  browser adapter.
- Added `canUndo` and `canRedo` state queries for host controls.

## [0.31.0] - 2026-08-31

### Browser workspace diagnostics

- Added typed `AcordeWorkspaceError` operation labels for parse, layout, render, metadata, and
  analysis failures.
- Made failed loads transactional so an invalid replacement does not discard the current score.

## [0.30.0] - 2026-08-31

### Virtualized browser rows

- Added row-level SVG rendering to the browser adapter with revision/configuration-keyed caching
  for virtualized long-score hosts.

## [0.29.0] - 2026-08-31

### Browser workspace caches

- Added revision- and configuration-keyed adapter caches for layout, SVG, metadata, and analysis
  results.
- Exposed the workspace revision in snapshots for host-side invalidation and synchronization.

## [0.28.0] - 2026-08-31

### Browser workspace adapter

- Added a dependency-free TypeScript adapter for the WASM parse, layout, render, metadata, and
  analysis pipeline.
- Added stable-address selection state synchronization for browser hosts.

## [0.27.0] - 2026-08-31

### Benchmark quality gate

- Added `acorde benchmark --fail-on-mismatch` for using category-level benchmark mismatches as a
  local or CI exit-status gate while preserving the JSON report.

## [0.26.0] - 2026-08-31

### Local benchmark CLI

- Added `acorde benchmark` for running a local JSON manifest of score files against the analysis
  benchmark suite and emitting machine-readable aggregate and failure reports.

## [0.25.0] - 2026-08-31

### Analysis benchmark suites

- Added deterministic suite-level aggregation for case status, precision, recall, and explanation
  completeness while preserving each case's failure details.

## [0.24.0] - 2026-08-31

### Analysis benchmark failure reports

- Added typed category-level benchmark failures with expected, predicted, missing, and excess
  counts, so benchmark reports identify the analysis categories requiring review.

## [0.23.0] - 2026-08-31

### Analysis benchmark contract

- Added offline benchmark cases with hand-verified category expectations.
- Added deterministic predicted counts plus precision, recall, and explanation-completeness
  percentages for comparing analysis fixtures without filesystem or network access.

## [0.22.0] - 2026-08-31

### Explainable phrase analysis

- Added deterministic repeated three-note interval motif detection with source spans.
- Added conservative phrase-boundary results for measures ending in explicit rests.
- The analysis result schema is now version `6`; new fields default to empty for older JSON.

## [0.21.0] - 2026-08-31

### Explainable SATB diagnostics

- Added typed SATB diagnostics for voice crossing, wide spacing, and parallel perfect intervals.
- Diagnostics include severity, confidence, stable note addresses, evidence, and rule IDs for
  browser-side inspection and selection.
- The analysis result schema is now version `5`; the new diagnostic field defaults to empty.

## [0.20.0] - 2026-08-31

### Explainable harmony analysis

- Added typed cadence candidates for authentic, plagal, deceptive, and half-cadence transitions.
- Added aligned voice-leading observations with explicit parallel-perfect detection and evidence.
- The analysis result schema is now version `4`; new fields default to empty for older JSON.

## [0.19.0] - 2026-08-31

### Explainable analysis delivery

- Added the `acorde analyze` CLI command and the WASM `analyze_score` entry point.
- Added finite-batch and lazy streaming analysis APIs while keeping the result contract
  deterministic and host-independent.

## [0.18.0] - 2026-08-31

### Explainable music analysis

- Added deterministic major/minor key estimation to `acorde-analysis`.
- Key results expose pitch coverage, confidence, rule IDs, and source evidence, while returning
  all tied best candidates to preserve tonal ambiguity. The analysis schema is now version `3`.

## [0.17.1] - 2026-08-31

### Release metadata

- Corrected all published workspace dependency constraints to reference the matching `0.17.1`
  crate versions.

## [0.17.0] - 2026-08-31

### Explainable music analysis

- Added deterministic adjacent melodic interval observations to `acorde-analysis`.
- Interval results preserve source addresses, semitone and diatonic distances, rule IDs, and
  evidence addresses, with analysis schema version `2`.

## [0.16.0] - 2026-08-31

### Explainable music analysis

- Added the optional `acorde-analysis` crate with deterministic chord-template labels.
- Analysis results include stable `NoteAddr` evidence, a rule identifier, confidence, and an
  optional Roman numeral in the active key; unmatched pitch collections remain unlabeled.

## [0.15.0] - 2026-08-31

### Interchange report contract

- Added `schema_version` to import and export reports so CLI, WASM, and library consumers can
  safely identify the serialized report shape.
- Legacy report JSON without `schema_version` defaults to the current schema version.

## [0.14.0] - 2026-08-31

### Interchange report contract

- Added an explicit lowercase format identifier to import and export reports, including legacy
  deserialization defaults for report JSON.
- Report wrappers now identify MusicXML, MXL, MIDI, ABC, MEI, MSCX, and MSCZ outputs.

## [0.13.0] - 2026-08-31

### CLI interchange diagnostics

- Added `acorde report <input>` to emit structured import reports as JSON for all supported input
  formats, including the documented MEI subset.

## [0.12.0] - 2026-08-31

### Browser interchange diagnostics

- Exposed MusicXML and MEI import/export reports through WASM JSON APIs.
- Added WASM import/export for the documented MEI subset.

## [0.11.0] - 2026-08-31

### MEI interoperability

- Added an explicit, documented MEI subset boundary for importing and exporting the canonical
  `Score` model.
- Added round-trip coverage for MEI title, measures, notes, rests, accidentals, dots, and basic
  power-of-two durations.

## [0.10.0] - 2026-08-31

### Interchange diagnostics

- Added typed `ImportReport` and `ExportReport` foundations with structured diagnostics for
  source location, severity, preserved values, and loss reasons.
- Added report-returning wrappers for MusicXML, MIDI, ABC, and MuseScore interchange APIs.

## [0.9.2] - 2026-08-31

### Interchange coverage

- Added the versioned notation coverage matrix for MusicXML, MIDI, ABC, MSCZ/MSCX, and JSON.
- Documented preservation, rendering, export scope, and known partial or unsupported slices as
  the baseline for future import/export diagnostics.

## [0.9.1] - 2026-08-31

### Playback and documentation

- Fixed playback timing for sparse voices so events remain aligned to notated measure boundaries.
- Refreshed the public README set to document the v0.9 workspace, crate responsibilities, and
  supported APIs without exposing internal development material.

### Documentation

- Rewrote the root and crate README files to match the v0.9.0 workspace, public APIs, feature
  flags, CLI commands, and the fact that `acorde-render-svg` is not re-exported by `acorde`.
- Kept development instructions and roadmap material out of the public documentation set.

## [0.9.0] - 2026-08-30

### Complete score patches

- Extended `ScorePatch` with measure time-signature, barline, rehearsal-mark, and volta updates,
  including clearing optional values.
- Added indexed note insertion so patches preserve note order when inserting into a voice.
- Added a deterministic `ReplaceScore` fallback for structural or otherwise uncovered score changes;
  patch application no longer silently drops parts, staves, or notation attributes.
- Added native coverage tests and the v0.9 migration guide; the Score JSON schema is unchanged.

## [0.8.0] - 2026-08-30

### Portable score patches

- Exposed `score_patch` and `apply_score_patch` through WebAssembly so browser hosts can compute,
  persist, transmit, and apply deterministic score deltas without reimplementing score traversal.
- Added native and browser contract tests for patch JSON round-trips and malformed input handling.
- Added the v0.8 migration guide; the Score JSON schema is unchanged.

## [0.7.0] - 2026-08-30

### Validation hardening

- Expanded `validate` with structural diagnostics for empty scores, parts without staves,
  staves without measures, mismatched staff measure counts, and invalid time signatures.
- Exposed the same diagnostics through the CLI and `validate_score` WASM JSON report.
- Added the v0.7 migration guide; existing valid scores and JSON schemas remain compatible.

## [0.6.0] - 2026-08-30

### MusicXML correctness

- Fixed first-measure `attributes` capture so non-default clefs, keys, and time signatures are
  preserved on the measure itself.
- Added a regression test covering the first measure of a multi-part score.

## [0.5.0] - 2026-08-30

### Production release hardening

- Added reviewed Chromium HiDPI (device scale factor 2) visual regression coverage.
- Added a CI WASM artifact size budget and release verification gate.
- Added the v0.5 migration notes; renderer and WASM JSON contracts remain compatible with v0.4.

## [0.4.0] - 2026-08-30

### Browser contract and accessibility

- Added versioned `RenderMetadata` with score counts and `accessible_text` for accessible
  fallbacks when SVG semantics are unavailable.
- Added a browser fixture fallback connected with `aria-describedby`, plus structural and browser
  contract assertions for the metadata version.
- Added the v0.4 migration guide and completed Phase 5 visual-regression/browser hardening.

## [0.3.0] - 2026-08-30

### Production browser hardening

- Added reviewed Playwright screenshot baselines for Chromium, Firefox, and WebKit.
- Added a visual-regression policy and v0.3 migration guide.
- Made the browser screenshot contract enforceable in CI with a bounded pixel-diff budget.
- Kept the manual crates.io workflow aligned with the complete workspace release.

## [0.2.0] - 2026-08-30

This release consolidates the v0.2 notation pipeline: expanded score editing and theory APIs,
MusicXML/MIDI/ABC/MuseScore I/O, renderer-facing layout data, deterministic SVG rendering, and
the corresponding WebAssembly bindings and CLI support. See the entries below for the detailed
feature history.

### Added

#### Phase 2 initial slice — span engraving
- `acorde-render-svg` renders local ties and layout-resolved hairpin, slur, pedal, ottava, and
  trill-line spans as deterministic SVG geometry, including cross-system continuation segments.
- Cross-measure ties are split at system boundaries and retain stable SVG classes for host-side
  interaction.
- Note-attached dynamics, chord symbols, lyrics (including XML escaping and syllable hyphens),
  and common staccato/accent/tenuto articulations are emitted with stable SVG classes.
- Sixteenth-, thirty-second-, and sixty-fourth-note rests, custom noteheads, grace/cue-note
  scaling, and optional part-group connector/label hooks are also emitted by the SVG renderer.
- Added browser-facing WASM entry points for precomputed-layout rendering, single-system
  rendering, and deterministic SVG dimension/NoteAddr-bound metadata.
- Added semantic interactive hooks for staff groups, spans, and span endpoint addresses; SVG
  output now includes accessible `role="img"`, `<title>`, and `<desc>` metadata.
- Invalid precomputed layout indices now return `InvalidLayout` instead of reaching renderer
  indexing paths and panicking.
- Added structural coverage for local and cross-system span mark families.
- Added malformed-input guards covering empty input and 64 MiB garbage, plus a CI Chrome
  browser-contract job; the browser example demonstrates keyboard selection and host-owned
  hover state.
- Added large-score, many-staff, and pathological-voice renderer soak guards, together with a
  reproducible release-profile layout/render benchmark.
- Verified the browser contract smoke fixture on Chromium, Firefox, and WebKit; CI stores the
  rendered screenshots as review artifacts.
- Pinned CI to `wasm-pack 0.15.0`; the WASM browser contract now passes with a Chrome-matched
  WebDriver in the release verification environment.
- Added enforced release-profile benchmark budgets for layout latency, render latency, and SVG
  size on the representative 32-measure / 128-note case.

#### Phase 17 — acorde-render-svg: SVG score renderer
- New crate `crates/render-svg` (`acorde-render-svg`) — pure-Rust/WASM SVG renderer. Dependency direction `acorde-core → acorde-layout → acorde-render-svg`; no reverse dependency, no browser/DOM dependency, no `acorde` umbrella re-export (kept out of the published dependency graph for now)
- `render_svg(score: &Score, options: &SvgRenderOptions) -> Result<String, RenderError>` — computes layout internally and renders
- `render_svg_with_layout(score, layout: &LayoutResult, options) -> Result<String, RenderError>` — renders from an already-computed `LayoutResult`
- `SvgRenderOptions { width, staff_size, measures_per_system, interactive }` — all fields have defaults, `#[serde(default)]` for partial-JSON deserialization
- `RenderError { EmptyScore, UnsupportedClef, UnsupportedAccidental { alter } }` — system-boundary validation; percussion clef and `|alter| > 2` are rejected, never silently dropped or approximated
- Supports: 5-line staves, treble/bass/alto/tenor clefs, grand-staff (multi-staff) systems, system breaks, key signatures (major/minor, any fifths), time signatures (any numerator/denominator, rendered as original 7-segment-style digit glyphs — no font), whole/half/quarter/eighth notes + dotted variants, matching rests, natural/sharp/flat/double-sharp/double-flat accidentals, ledger lines (derived purely from `Step`+octave, never from MIDI number), barlines (normal/double/final/dashed/dotted/repeat), two-voice-per-staff rendering (voice 0 stems up / voice 1 down unless `Note.stem_up` overrides)
- All glyphs (clefs, noteheads, accidentals, rests, digits) are original hand-authored SVG paths generated from parametric math — no vendored font, no system-font dependency; see `crates/render-svg/README.md` for the rationale
- Stable `data-acorde-kind` / `data-part` / `data-staff` / `data-measure` / `data-voice` / `data-note` / `data-note-addr` attributes on every note/rest/measure group when `interactive: true` — positional addressing only, no embedded UUIDs, deterministic across re-renders of a structurally-identical score
- `crates/wasm` — `render_score_svg(score_json, options_json) -> Result<String, JsValue>` — thin wrapper calling the identical native `render_svg` code path
- `examples/render_satb.rs` in `crates/render-svg` — runnable SATB chorale demo (`cargo run -p acorde-render-svg --example render_satb`)
- `crates/layout` — `AccidentalMark { part, staff, measure, voice, note_index, pitch_index, alter }` + `LayoutResult.accidentals: Vec<AccidentalMark>` **[non-breaking, `#[serde(default)]`]** — mandatory (non-courtesy) accidentals: the first chromatic alteration of a step+octave within a measure, scoped per staff across all voices, reset every barline. Fills a real gap found while building the renderer: `courtesy_accidentals` only covered cross-measure reminders, not the far more common in-measure case. Renderers should prefer a mandatory mark over a courtesy mark at the same address (plain accidental wins over parenthesized)
- 38 new tests in `acorde-render-svg` (unit + structural SVG + determinism + one small golden fixture) and 9 new tests in `acorde-layout` for `AccidentalMark`; `cargo test --all` all green, `cargo clippy --all -- -D warnings` clean, `wasm-pack build crates/wasm --target web` green

#### Phase 16 — WASM: major function expansion
- `crates/wasm` — `serialize_midi_region(score_json, start, end)` — per-region MIDI export
- `crates/wasm` — `to_playback_events_ex(score_json, options_json)` — full `PlaybackOptions` variant
- `crates/wasm` — `compute_playback_position(score_json, time_secs)` — reverse lookup: time → measure/beat
- `crates/wasm` — `compute_layout_ex(score_json, config_json)` — full `LayoutConfig` variant
- `crates/wasm` — `diff_scores` · `score_statistics` · `score_duration_secs` · `score_duration_secs_region`
- `crates/wasm` — `respell_score(score_json, prefer_flat)` · `respell_score_to_key`
- `crates/wasm` — `measure_beats_remaining` · `pitch_from_midi` · `pitch_from_str` · `interval_between`
- `crates/wasm` — `key_alter_for_step` · `key_contains_pitch` · `key_display_name`
- `crates/wasm` — `clef_middle_line_midi` · `suggested_stem_up(pitches_json, clef_json)` · `compute_beams(notes_json, time_sig_json)` · `command_key_from_json`
- `crates/wasm` — `ScoreEngine.apply_batch_labeled(cmds_json, label)` — labeled batch for undo UI
- `crates/wasm` — `ScoreEngine.get_undo_label()` / `get_redo_label()` / `get_undo_key()` / `get_redo_key()`

#### Phase 15 — layout expansion: TupletGroup · CourtesyAccidental · first_row_measures
- `crates/layout` — `TupletGroup { part, staff, measure, voice, note_indices, actual_notes, normal_notes }` — tuplet grouping for renderers
- `crates/layout` — `CourtesyAccidental { part, staff, measure, voice, note_index, pitch_index, alter }` — accidentals that must be shown as courtesy
- `crates/layout` — `LayoutResult.tuplet_groups: Vec<TupletGroup>` and `courtesy_accidentals: Vec<CourtesyAccidental>` fields added
- `crates/layout` — `LayoutConfig.first_row_measures: Option<usize>` — allows overriding the measure count for the first system

#### Phase 14 — Interval · Respell · core theory API expansion
- `crates/core` — `Interval { semitones, quality: IntervalQuality, diatonic_steps }` and `IntervalQuality` enum; `interval_between(p1: &Pitch, p2: &Pitch) -> Interval`
- `crates/core` — `RespellScoreCmd` / `RespellScoreToKeyCmd` — enharmonic respelling (all notes / conform to key signature); `Command::RespellScore` / `Command::RespellScoreToKey`
- `crates/core` — `ValidationWarning` and `ValidationReport { errors, warnings }` — `validate()` now returns `ValidationReport` with separated error / warning levels
- `crates/core` — `Score::diff(a: &Score, b: &Score) -> Vec<ScoreChange>` — structural diff producing `ScoreChange` enum variants
- `crates/core` — `ScoreTemplate` enum + `Score::template(kind: ScoreTemplate) -> Score` — quick-start constructors for common ensembles

#### Phase 13 — Performance-mark details · staff management · layout breaks · part groups
- `crates/core` — `SetSystemBreakCmd` / `SetPageBreakCmd` — `Measure.system_break` / `Measure.page_break` **[schema: Measure]**
- `crates/core` — `ToggleSlurCmd` — toggle `Note.slur_start` / `Note.slur_end`
- `crates/core` — `AddStaffCmd` / `DeleteStaffCmd` — add/remove staves from a part (undo/redo-able)
- `crates/core` — `SetTupletCmd` — set `Note.tuplet: Option<TupletInfo>` via command
- `crates/core` — `SetStemCmd` — `Note.stem_up: Option<bool>` **[schema: Note]**
- `crates/core` — `SetArpeggioCmd` — `Note.arpeggiate: Option<bool>` **[schema: Note]**
- `crates/core` — `SetTechniqueTextCmd` — `Note.technique_text: Option<String>` **[schema: Note]**
- `crates/core` — `SetFingeringCmd` — `Note.fingering: Option<u8>` **[schema: Note]**
- `crates/core` — `SetStringNumberCmd` — `Note.string_number: Option<u8>` **[schema: Note]**
- `crates/core` — `NoteHead` enum + `SetNoteHeadCmd` — `Note.note_head: NoteHead` **[schema: Note]**
- `crates/core` — `SetCueCmd` — `Note.is_cue: bool` **[schema: Note]**
- `crates/core` — `GuitarTechnique` enum + `SetGuitarTechniqueCmd` — `Note.guitar_technique: Option<GuitarTechnique>` **[schema: Note]**
- `crates/core` — `SetExpressionTextCmd` — `Measure.expression_text: Option<String>` **[schema: Measure]**
- `crates/core` — `ToggleTrillLineCmd` — `Note.trill_line_start` / `Note.trill_line_end` **[schema: Note]**
- `crates/core` — `PartGroup { first_part, last_part, symbol: PartGroupSymbol, barlines_connect }` + `SetPartGroupCmd` — `Score.part_groups: Vec<PartGroup>` **[schema: Score]**
- `crates/core` — `ScoreTemplate` enum + `Score::template()` constructor for common ensembles
- Command count reaches 53 variants (including `Batch`)

#### Phase 26 — MSCZ expansion: repeat marks · Dynamic/Lyric/Slur · MuseScore 4.x support
- `crates/io` (feature = "mscz") — Repeat barlines: `<startRepeat/>` → `Barline::RepeatStart`, `<endRepeat>` → `Barline::RepeatEnd`; both set on `Measure.barline_left` / `barline_right`
- `crates/io` (feature = "mscz") — Volta brackets: `<Spanner type="Volta">` parsed from Measure-level spanners; `<beginText>1.</beginText>` → `VoltaBracket { number: 1, kind: "begin_end" | "begin" }`
- `crates/io` (feature = "mscz") — Slur import: `<Spanner type="Slur">` at Chord level sets `Note.slur_start = true` when a `<next>` element is present (start-of-slur detection)
- `crates/io` (feature = "mscz") — Dynamic import: `<Dynamic><subtype>p</subtype></Dynamic>` at Measure level; stored in `pending_dynamic` and applied to the next Chord via `.take()` — `parse_dynamic_str()` helper maps 14 dynamic labels
- `crates/io` (feature = "mscz") — Lyric import: `<Lyrics><text>…</text><syllabic>begin</syllabic></Lyrics>` at Chord level → `Note.lyric = Some(Lyric { text, syllabic })`; defaults to `"single"` when syllabic is absent
- `crates/io` (feature = "mscz") — MuseScore 4.x voice wrapper: `<voice>` as a Measure-level container element (4.x) distinguished from `<voice>` text child of `<Chord>` (3.x) by context guard `in_chord=false`; voice index increments per wrapper; both 3.x and 4.x files parse correctly
- `parse_volta_number()` helper strips trailing non-digit characters from volta text like `"1."` → `1`
- 7 new unit tests in `crates/io/src/mscz/mod.rs`; `cargo test -p acorde-io --features mscz` 69 (all green), `cargo test --all` 459 (all green), `cargo clippy --all -- -D warnings` clean

#### Phase 25 — Music theory analysis: Scale::best_fit · roman_numeral · MSCZ basic import
- `crates/core` — `Scale { tonic: String, mode: String }` struct with `Scale::best_fit(pitches: &[Pitch]) -> Option<Scale>` — scores candidate scales by coverage; returns the best-matching major or natural-minor scale from a set of pitches
- `crates/core` — `detect_chord(pitches: &[Pitch]) -> Option<ChordSymbol>` — template-matching chord detection across 13 chord types; inversion-aware (tries every pitch class as root); sets `ChordSymbol.bass` for slash chords
- `crates/core` — `roman_numeral(chord: &ChordSymbol, key: &KeySignature) -> Option<String>` — Roman numeral analysis with case (upper = major quality, lower = minor), suffix (`o`, `o7`, `ø7`, `+`, `7`, `maj7`), and slash notation (`I/V`)
- `crates/io` (feature = "mscz") — `parse_mscz(data: &[u8]) -> Result<Score, Error>` — decompresses `.mscz` zip archive, extracts embedded `.mscx`, delegates to `parse_mscx`
- `crates/io` (feature = "mscz") — `parse_mscx(xml: &str) -> Result<Score, Error>` — event-driven quick-xml parser for MuseScore 3.x XML: pitches via TPC (Tonal Pitch Class), durations, rests, ties, grace notes, tuplets, key/time/clef signatures, tempo, hairpins, ottava, pedal, rehearsal/navigation marks, chord symbols
- `crates/wasm` — `detect_chord(pitches_json)`, `roman_numeral(chord_json, key_json)`, `best_fit_scale(pitches_json)` exposed to JavaScript

#### Batch 8 — undo/redo ChangeHint · batch_apply · WASM completions · schema_version · ABC multi-voice
- `crates/core` — `ChangeHint::merge(self, other) -> ChangeHint` + `merge_scope()` helper — merges scope (broadest wins) and OR-s dirty flags
- `crates/core` — `CommandStack::undo()` / `redo()` now return `Result<ChangeHint, Error>` (was `Result<(), Error>`) — consumers get the same hint as the undone/redone command
- `crates/core` — `ScoreEngine::undo()` / `redo()` return type updated accordingly
- `crates/core` — `BatchCmd { commands: Vec<Command> }` — 34th command variant; applied and undone as a single entry
- `crates/core` — `CommandStack::batch_execute(cmds, score)` — rollback-safe: restores snapshot on first error
- `crates/core` — `ScoreEngine::batch_apply(cmds: Vec<Command>) -> Result<ChangeHint, Error>` — merges hints from all sub-commands
- `crates/core` — `Score.schema_version: u32` **[schema]** — `#[serde(default)]`; set to `1` on all new/constructed scores; deserializes to `0` from pre-existing files
- `crates/core` — `SCORE_SCHEMA_VERSION: u32 = 1` public constant
- `crates/io` — `serialize_abc`: multi-part support — emits `V:N` inline voice tags when `score.parts.len() > 1`; single-part output unchanged
- `crates/wasm` — `acorde-io` dependency now includes `features = ["abc"]`
- `crates/wasm` — `parse_abc(text) -> Result<String, JsValue>` and `serialize_abc(score_json) -> Result<String, JsValue>` exposed to JS
- `crates/wasm` — `validate_score(score_json) -> Result<String, JsValue>` — JSON array of `ValidationError` objects
- `crates/wasm` — `transpose_score(score_json, semitones: i8) -> Result<String, JsValue>`
- `crates/wasm` — `extract_part(score_json, part_index: usize) -> Result<String, JsValue>`
- `crates/wasm` — `merge_scores(score_a_json, score_b_json) -> Result<String, JsValue>`
- `crates/wasm` — `ScoreEngine.apply_batch(cmds_json) -> Result<String, JsValue>` — JSON array of commands
- `crates/wasm` — `ScoreEngine.undo()` / `redo()` / `paste_voice()` now return `Result<String, JsValue>` (ChangeHint JSON, was `void`)
- 34 new tests; `cargo test --all` 206 (all green), `cargo test -p acorde-io --features abc` (all green), `cargo clippy --all -- -D warnings` clean

#### Batch 7 — WASM GM lookup · ChangeHint · Percussion channel 10 · Concert pitch layout
- `crates/wasm` — `gm_program_name(program: u8) -> String` / `gm_drum_name(note: u8) -> String` JS functions
- `crates/core` — `ChangeHint { scope: ChangeScope, layout_dirty: bool, playback_dirty: bool }` — returned by `ScoreEngine::apply()` and `paste_voice()`; consumers skip redundant recomputes
- `crates/core` — `ChangeScope` enum: `Global` · `Part(usize)` · `Measures { part, staff, start, end }`
- `crates/core` — `command_hint(cmd: &Command) -> ChangeHint` — classifies all 33 commands without executing them
- `crates/wasm` — `ScoreEngine.apply()` now returns a `ChangeHint` JSON string (was `void`)
- `crates/core` — `drum_name(note: u8) -> &'static str` — GM percussion name lookup (notes 35–81)
- `crates/core` / `crates/io` — Channel 9 (GM percussion) skips `Staff.transpose_semitones` in both `to_playback_events` and `serialize_midi`
- `crates/layout` — `LayoutConfig.concert_pitch: bool` flag; `LayoutResult.concert_key_overrides: Vec<ConcertKeyOverride>` — per-staff key signature fifths adjusted to concert pitch for transposing instruments
- `crates/layout` — `ConcertKeyOverride { part_index, staff_index, fifths }` struct
- `crates/wasm` — `compute_layout` gains `concert_pitch: bool` third argument
- 13 new tests; `cargo test --all` 172 (all green), `cargo test -p acorde-io --features abc` 65 (all green), `cargo clippy --all -- -D warnings` clean

#### Batch 6 — MusicXML transpose · GM names · MIDI program import · CopyVoice/PasteVoice · ABC serializer
- `crates/core` — `program_name(n: u8) -> &'static str` — General MIDI Level 1 program name lookup (128 entries); re-exported from `acorde_core`
- `crates/io` — MusicXML `<transpose><chromatic>` round-trip: serializer emits block when `staff.transpose_semitones != 0`; parser restores value from the element
- `crates/io` — MIDI parser: `extract_program()` captures first `ProgramChange` event per track; `Part.midi_channel` and `Part.midi_program` are now populated on import
- `crates/core` — `ScoreEngine.clipboard: Option<Vec<Note>>` — session clipboard for copy/paste
- `crates/core` — `ScoreEngine::copy_voice(part, staff, measure, voice)` — stores a voice clone in the clipboard (not a Command; does not affect undo history)
- `crates/core` — `ScoreEngine::paste_voice(part, staff, measure, voice)` — constructs a `PasteVoice` command from the clipboard and applies it (undo-able)
- `crates/core` — `Command::PasteVoice(PasteVoiceCmd)` — the 33rd command variant; notes are embedded in the command for deterministic undo/redo
- `crates/wasm` — `ScoreEngine.copy_voice` / `ScoreEngine.paste_voice` JS methods
- `crates/io` — `serialize_abc(score) -> Result<String, Error>` (feature = "abc") — ABC Notation serializer: header (X/T/C/M/L/Q/K), notes/rests/chords/ties/octave markers, L:1/4 unit length, `|` barlines, voice 0 only
- 23 new tests (10 in base suite + 7 ABC-feature-only + 6 ABC roundtrips);
  `cargo test --all` 145 (all green), `cargo test -p acorde-io --features abc` 65 (all green),
  `cargo clippy --all -- -D warnings` clean

#### Batch 5 — MIDI instrument round-trip · transposing instruments · per-measure tempo · Score::merge
- `crates/core` — `SetMidiInstrumentCmd { part_index, midi_channel, midi_program }` — undo/redo for channel & program changes **[schema: Part]**
- `crates/io` — MusicXML `<midi-instrument>` round-trip: serializer writes `<midi-channel>`/`<midi-program>` (1-based); parser reads them via pre-collected HashMap and applies to Part
- `crates/core` — `Staff.transpose_semitones: i8` — written-to-concert-pitch offset for transposing instruments (Bb clarinet = −2, Eb alto sax = −9, etc.) **[schema: Staff]**
- `crates/core` — `SetTransposeCmd { part_index, staff_index, semitones }` — set per-staff transposition
- `crates/core` — `to_playback_events`: applies `staff.transpose_semitones` to MIDI pitch; `time_secs` now accumulates per-note (correct across mid-score tempo changes)
- `crates/io` — `serialize_midi` `build_part_track`: applies `staff.transpose_semitones` to MIDI pitch
- `crates/core` — `SetTempoAtMeasureCmd { measure_index, bpm: Option<u16> }` — command-stack control of `Measure.tempo`
- `crates/io` — `serialize_midi` `build_meta_track`: scans `measure_sequence` and emits Tempo meta events at the correct tick for each measure that carries a tempo override
- `crates/core` — `Score::merge(other: &Score) -> Score` — append `other`'s parts to `self`; shorter score is padded with empty measures; metadata and settings taken from `self`
- 30 new tests; total: 133 (all green), `cargo clippy --all -- -D warnings` clean

#### Batch 5 — Bug fixes (security/correctness audit)
- `crates/io` — `serialize_midi` `build_meta_track`: fixed tick-0 duplicate Tempo event when `Measure.tempo` is set on measure 0 (initial BPM now taken from measure-0 override, skipping the per-measure loop for the first measure)
- `crates/io` — `serialize_midi`: added `clamp_delta()` helper clamping u64 ticks to MIDI VLQ limit (28 bits = 0x0FFF_FFFF) before `u28::from()` — prevents panic on extremely long scores in both `build_meta_track` and `build_part_track`
- `crates/io` — MusicXML parser: `<sound tempo="X"/>` attribute now parsed and stored in `Measure.tempo` — playback-tempo information previously discarded on parse
- `crates/io` — MusicXML serializer: per-measure `Measure.tempo` overrides now emitted as `<sound tempo>` + `<metronome>` `<direction>` blocks for measures after the first
- `crates/io` — ABC parser `unit_to_duration` / `is_dotted`: multiplications use `saturating_mul` to prevent u32 overflow on malformed input
- 2 new roundtrip tests: `musicxml_per_measure_tempo_roundtrip`, `musicxml_measure0_tempo_override_no_duplicate`; total: 135 (all green)

#### Earlier batches
- `crates/core` — Score data model, `Command` enum (32 variants), `ScoreEngine` with undo/redo
- `crates/io` — MusicXML parser + serializer, MXL (compressed) parser, MIDI parser + serializer, ABC parser
- `crates/layout` — `compute_layout` producing `vis_slots`, `RowLayout`, resolved `SpanMark`s
- `crates/cli` — `score convert`, `score info`, `score validate`, `score extract` binary
- `crates/wasm` — wasm-bindgen bindings: `parse_musicxml`, `serialize_musicxml`, `parse_midi`, `serialize_midi`, `to_playback_events`, `compute_layout`, `ScoreEngine` JS class
- `crates/core` — `measure_sequence(score)` — expand repeat barlines, volta brackets, D.C./D.S. navigation
- `crates/core` — `Score::validate()`, `Score::statistics()`, `Score::extract_part()`, `transpose(score, semitones)`
- `crates/core` — `PlaybackEvent { time_beats, time_secs, pitch_midi, velocity, duration_beats, duration_secs }`
- `crates/core` — `Part.midi_channel`, `Part.midi_program` — MIDI channel and program per part
- Fixture files: `tests/fixtures/simple.musicxml`, `tests/fixtures/multipart.musicxml`, `tests/fixtures/sample.abc`
- Integration roundtrip tests in `crates/io/tests/roundtrip.rs`
- GitHub Actions CI: `cargo test --all` + `cargo clippy --all -- -D warnings` on Ubuntu / macOS / Windows + wasm-pack build

### Score JSON schema (v0.1.0 → current)
All fields below are the complete current set. Fields marked **[schema]** carry
`#[serde(default)]` — existing JSON without them deserializes without error.

**`Score`**
```
id: String (uuid v4)
schema_version: u32              [schema]  ← added in batch 8 (phase 11)
metadata: ScoreMetadata { title, composer, lyricist, copyright, work_number, movement_title }
settings: ScoreSettings { tempo_bpm, time_signature, key_signature }
parts: Vec<Part>
part_groups: Vec<PartGroup>      [schema]  ← added in phase 13
```

**`Part`**
```
id: String
name: String
short_name: String
staves: Vec<Staff>
midi_channel: u8    (0–15)   [schema]
midi_program: u8    (0–127)  [schema]
```

**`Staff`**
```
clef: Clef
measures: Vec<Measure>
transpose_semitones: i8      [schema]  ← added in batch 5
```

**`Measure`**
```
number: u32
time_sig: Option<TimeSignature>   [schema]
key_sig:  Option<KeySignature>    [schema]
clef:     Option<Clef>            [schema]
tempo:    Option<u16>             [schema]
barline_left / barline_right: Barline
volta: Option<VoltaBracket>       [schema]
tempo_text: Option<String>        [schema]
rehearsal:  Option<String>        [schema]
navigation: Option<String>        [schema]
expression_text: Option<String>   [schema]  ← added in phase 13
multi_rest_count: Option<u8>      [schema]
system_break: bool                [schema]  ← added in phase 13
page_break:   bool                [schema]  ← added in phase 13
voices: [Vec<Note>; 4]
```

**`Note`**
```
pitches:       Vec<Pitch>
duration:      Duration
dot_count:     u8                 [schema]
is_rest:       bool
is_grace:      bool               [schema]
grace_slash:   bool               [schema]
dynamic:       Option<Dynamic>    [schema]
articulations: Vec<Articulation>  [schema]
beam:          BeamState          [schema]
tuplet:        Option<TupletInfo> [schema]
tie_start / tie_end: bool         [schema]
slur_start / slur_end: bool       [schema]
hairpin_start: Option<HairpinKind>[schema]
hairpin_end:   bool               [schema]
ottava_start:  Option<OttavaKind> [schema]
ottava_end:    bool               [schema]
pedal_start / pedal_end: bool     [schema]
chord_symbol:  Option<ChordSymbol>[schema]
lyric:         Option<Lyric>      [schema]
stem_up:        Option<bool>             [schema]  ← added in phase 13
arpeggiate:     Option<bool>             [schema]  ← added in phase 13
technique_text: Option<String>           [schema]  ← added in phase 13
fingering:      Option<u8>               [schema]  ← added in phase 13
string_number:  Option<u8>               [schema]  ← added in phase 13
note_head:      NoteHead                 [schema]  ← added in phase 13
is_cue:         bool                     [schema]  ← added in phase 13
trill_line_start / trill_line_end: bool  [schema]  ← added in phase 13
guitar_technique: Option<GuitarTechnique>[schema]  ← added in phase 13
```
