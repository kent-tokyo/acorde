# acorde-layout

Logical, pixel-free layout computation for acorde-core scores.

compute_layout(&Score, &LayoutConfig) returns visual measure slots, rows, resolved spans, beam
groups, tuplet groups, concert-pitch key overrides, and mandatory/courtesy accidental marks. It
does not render pixels and has no browser or filesystem dependency.

`compute_print_layout(&Score, &PrintConfig)` adds a host-neutral page/system plan. Dimensions
are physical millimetres, and explicit `Measure::page_break` decisions are preserved. The
result contains page dimensions, stable page/system addresses, physical measure indices, typed
break reasons, explicit bleed metadata, scaled system geometry, configurable page numbering, and
host-facing color/crop-mark/glyph-resource policies. Hosts can also pass
`PrintConfig::keep_together` ranges to keep contiguous physical measures in one system, and
`first_system_measures` to reserve a shorter first system for pickup/title material;
`final_page_policy` can balance automatically paginated systems across pages;
`pickup_policy` automatically isolates a non-empty partial first measure by default, or can be
set to `Preserve` to opt out;
each system also exposes physical `measure_spans`, including the hidden extent of multirest
slots;
multirests consume their full visual width during system breaking and remain unsplit;
cross-system spans are exposed as per-system `span_segments` with explicit start/end ownership;
repeat barlines, volta endings, navigation marks, and rehearsal labels are exposed as
per-system `measure_marks` without changing playback order;
pages aggregate cross-system span ownership as `PageSpanSegment` values;
`PrintLayoutResult::page` retrieves a stable page artifact without recomputation, with helpers
for its physical measure range and cross-page span continuation;
`PrintLayoutResult::export_page_artifacts` validates the complete result and returns one
host-neutral `PageArtifact` per page with physical dimensions, measure span, copied page layout,
and typed resource/span-continuation diagnostics; `PageLayout::artifact_diagnostics` additionally
turns host-computed `GlyphExtents` into deterministic per-side overflow diagnostics;
`PrintLayoutResult::validate` checks serialized page and system addresses before host reuse;
`PrintConfig::publication` and `PageLayout::publication` carry deterministic score metadata,
part labels, running titles, page-scoped measure numbers, and physical header/footer text blocks
for publication hosts. `PublicationConfig::title_page` adds a metadata-only first page without
consuming music-system capacity, with title/subtitle/credit blocks placed in physical millimetres;
`PrintPreset` supplies versioned A4/Letter score and extracted-part starting configurations;
`PrintPreset::config_with_title_page` opts into a title page while preserving preset defaults;
full-score publication metadata includes bracket/brace part-group marks;
`PublicationConfig::page_number_in_footer` adds the final logical page number as a footer block;
publication text blocks carry explicit left/center/right alignment;
each block also carries the validated physical line-box height;
title pages expose copyright as a dedicated block;
publication page metadata is serde-defaulted so older serialized page objects remain readable;
`NotationBreakPolicy::KeepVoltaTogether` can preserve contiguous volta endings during system
breaking when explicitly enabled; `NotationBreakPolicy::KeepRepeatsTogether` can start repeat
sections on a fresh page and keep them together when they fit the page capacity;
invalid, over-capacity, or explicit-break-conflicting ranges return typed errors.
Safe areas constrain the content rectangle, but the crate does not choose fonts, emit PDF, access
printers, or perform filesystem I/O.

Print consumers can use `GlyphMetrics` and `GlyphPlacement` for font-independent millimetre
geometry. `resolve_glyph_collisions` applies deterministic, priority-aware vertical separation and
`resolve_glyph_horizontal_collisions` applies corresponding horizontal separation for overlapping
glyphs, and `distribute_glyph_spacing` spreads additional system width evenly between ordered
placements. The `*_checked` resolver variants validate geometry before mutating placements and
reject non-finite gaps and placement arithmetic overflow; unchecked variants sanitize non-finite
gaps to zero. Font loading and final typography remain host responsibilities.
`glyph_extents` returns validated content bounds so hosts can derive content-aware margins without
reconstructing glyph geometry; `GlyphExtents::width_mm` and `height_mm` provide the derived spans.

~~~rust
use acorde_core::Score;
use acorde_layout::{compute_layout, LayoutConfig};

let score = Score::default();
let layout = compute_layout(&score, &LayoutConfig::default());
~~~

LayoutConfig supports measures_per_row, first_row_measures, and concert_pitch. Consumers such as
SVG or Canvas renderers own pixel coordinates. See the repository's
[print layout contract](../../docs/print-layout.md) for the boundary with host applications.

Layout input and resource-limit rules are documented in the [security contract](../../docs/security/threat-model.md).

[API documentation](https://docs.rs/acorde-layout) · [Repository](https://github.com/kent-tokyo/acorde)
