# Print layout contract

`acorde` owns reusable score semantics and deterministic logical placement. The current
`acorde-layout::compute_print_layout` API is the neutral boundary between a `Score` and a
print-capable host:

```text
Score → PrintConfig → PrintLayoutResult (pages/systems in mm) → SVG/PDF/print host
```

`PrintConfig` defines paper size, orientation, margins, bleed/safe areas, system height, scale,
page-numbering, color, crop-mark, and glyph-resource policies, measures per system, and optional
`keep_together` ranges, an optional `first_system_measures` capacity for pickup/title systems,
an automatic or explicit `pickup_policy` that detects a partial first measure, and a `final_page_policy` that
can deterministically balance automatic pagination.
`PrintPreset` provides versioned A4/Letter score and extracted-part starting configurations;
the preset data does not read host preferences or installed resources. Use
`PrintPreset::config_with_title_page` to opt into a title page without changing the default
preset configuration.
A keep-together range uses zero-based inclusive physical measure indices
and is placed in one system when it fits the configured capacity. Invalid ranges, ranges larger
than a system, and ranges containing an explicit system/page break return typed errors. Scale applies to system content geometry while
paper dimensions remain the selected physical page size. Safe-area values reduce the usable
content rectangle; bleed values, the optional one-based page number, color intent, and crop-mark
intent, and glyph-resource policy are carried as explicit page metadata for a host exporter.
`SystemLayout::measure_spans` records the physical inclusive interval represented by each visual
measure slot, including the hidden extent of multirests. `SystemLayout::span_segments` identifies
cross-system span intersections and whether each segment starts or ends on that system.
Multirests consume their full visual width when systems are broken and are never split between
systems; a multirest wider than the configured capacity occupies one system by itself.
`SystemLayout::measure_marks` carries repeat barlines, volta endings, navigation marks, and
rehearsal labels for each physical measure in the system; playback expansion remains in core.
`NotationBreakPolicy::KeepVoltaTogether` is an opt-in system-breaking policy that keeps a
contiguous volta begin/end range in one system when it fits; the default `Preserve` policy
does not infer notation-aware breaks. `NotationBreakPolicy::KeepRepeatsTogether` is an opt-in
page policy that starts a repeat section on a fresh page and keeps it together when it fits the
configured systems-per-page capacity; repeat sections that exceed that capacity return a typed
error. It disables final-page balancing so the repeat boundary remains deterministic.
`PageLayout::span_segments` aggregates cross-system span ownership at page boundaries, so a host
can emit continuation marks without reconstructing spans from adjacent systems.
`PrintConfig::publication` carries an optional running title, header/footer text, and policies
for part labels and measure numbers, plus an opt-in metadata-only title page. Each
`PageLayout::publication` contains copied score metadata, deterministic
part labels, and only the measure numbers belonging to that page, so a host can render headers
and labels without reconstructing page ownership. `PartLayoutPolicy::ExtractedPart` is an
explicit opt-in that scopes pagination and notation spans to one selected part; the default
`FullScore` policy preserves existing score-level behavior. Publication text is exposed as
`PublicationTextBlock` values with semantic roles and physical x/y/width in millimetres.
With `page_number_in_footer`, the final logical page number is emitted as a footer block.
`PublicationTextAlignment` records left, center, or right alignment within each block's width.
`PublicationConfig::line_height_mm` supplies the validated line-box height carried by every text
block; non-finite or non-positive values return a typed layout error.
Publication page metadata is serde-defaulted so older serialized page objects remain readable.
Full-score pages also carry `PartGroupMark` bracket/brace metadata; extracted-part pages omit
cross-part connectors.
Title pages additionally expose title, subtitle, credit, and copyright blocks; ordinary pages do not receive
the running-title header unless configured.
`PrintLayoutResult` records page
dimensions, stable page/system addresses, physical measure indices, and typed break reasons (`MeasureCapacity`, `ExplicitSystemBreak`,
`ExplicitPageBreak`, `PageCapacity`, `TitlePage`, or `EndOfScore`). Layout honors existing `system_break` and
`page_break` decisions and produces stable output for the same score and configuration. Its
`contract_version` is `25` for this address/diagnostic, publication, title-page, part-group, page-number footer, alignment, line-box height, copyright block, bleed/safe-area, scale, page-numbering,
color, crop-mark, and glyph-resource shape. `GlyphResourcePolicy::HostProvided` is only a stable
resource key; resource lookup, font loading, and glyph metrics remain host/provider work.
`PRINT_LAYOUT_CONTRACT_VERSION` identifies this serialized page contract, and `validate()` rejects
results from another contract version before host reuse.
An empty `GlyphResourcePolicy::HostProvided` key is rejected before page layout is produced.
Hosts can retrieve a page artifact with `PrintLayoutResult::page(PageAddress)`, inspect its
physical range with `PageLayout::measure_span()`, and detect cross-page continuations with
`PageLayout::has_span_continuation()`. Page lookup verifies both the vector index and the
serialized page address, returning no artifact for a mismatched or corrupted address.
For page-oriented export, `PrintLayoutResult::export_page_artifacts()` validates the complete
result and returns one `PageArtifact` per page in physical order. Each artifact contains
millimetre dimensions, its physical measure span, the copied `PageLayout`, and typed
`PageArtifactDiagnostic::SpanContinuation` and `GlyphResourceRequired` entries. Hosts can call
`PageLayout::artifact_diagnostics(Some(extents))` to add a `GlyphOverflow` entry with the exact
overflow directions from host-computed glyph bounds. The resource entry marks pages whose
`GlyphResourcePolicy::HostProvided` key still requires host/provider resolution. It emits no file bytes and has no PDF, font,
filesystem, or printer dependency; those concerns remain in the host exporter.
Hosts that persist or transport a complete result can call `PrintLayoutResult::validate()` to
check page indices, global system indices, and page-local system positions before reuse.
It also rejects mixed numbered/unnumbered pages, zero page numbers, non-monotonic numbering,
and numbered pages that do not match the current `page_index + 1` policy.
It also requires `TitlePage` to be page zero with no systems and rejects title metadata on
ordinary pages.
Physical page and system dimensions are checked for finite, positive values (with non-negative
bleed and top offsets) before a persisted layout is reused; content dimensions must not exceed
the physical page.
Hosts that supply glyph geometry should call `validate_glyph_placements` before
`resolve_glyph_collisions`; non-finite coordinates, empty resource keys, negative advances, and
negative bounding-box extents are rejected with a typed error instead of being allowed into
collision math.
The `resolve_glyph_collisions_checked` and `resolve_glyph_horizontal_collisions_checked` variants
combine that validation with mutation and are preferred for host preflight paths. They reject
non-finite collision gaps and arithmetic overflow before mutation; unchecked variants sanitize
non-finite gaps to zero.
After computing remaining system width, `distribute_glyph_spacing` can spread it evenly between
ordered glyph placements without changing the first placement's anchor.
`glyph_extents` aggregates validated placement bounds and returns `None` for empty content, allowing
hosts to derive safe margins without fixed renderer assumptions. Its `GlyphExtents` result exposes
`width_mm()` and `height_mm()` for the derived content spans.
The SVG renderer exposes `glyph_coverage()` for its built-in vector resource and rejects notation
outside the reported clef/accidental coverage with a typed error; it never silently emits a blank
critical glyph.

This API deliberately does not select or embed fonts, draw glyphs, generate PDF, open files,
invoke OS printer APIs, or provide a preview UI. Those responsibilities belong to
`acorde-render-svg`, a future `acorde-print` crate, or the consuming application/backend.

The `Balance` final-page policy redistributes automatically paginated systems as evenly as
possible and is disabled when explicit page breaks are present. The current contract is a
foundation, not full engraving parity: extracted-part policies, headers/footers, collision-aware spacing, and print SVG export metadata
remain roadmap work. Unsupported or incomplete notation must continue to be
reported through the format capability boundaries rather than treated as lossless.
