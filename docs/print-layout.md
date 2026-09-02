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
`PageLayout::span_segments` aggregates cross-system span ownership at page boundaries, so a host
can emit continuation marks without reconstructing spans from adjacent systems.
`PrintLayoutResult` records page
dimensions, stable page/system addresses, physical measure indices, and typed break reasons (`MeasureCapacity`, `ExplicitSystemBreak`,
`ExplicitPageBreak`, `PageCapacity`, or `EndOfScore`). Layout honors existing `system_break` and
`page_break` decisions and produces stable output for the same score and configuration. Its
`contract_version` is `14` for this address/diagnostic, bleed/safe-area, scale, page-numbering,
color, crop-mark, and glyph-resource shape. `GlyphResourcePolicy::HostProvided` is only a stable
resource key; resource lookup, font loading, and glyph metrics remain host/provider work.
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
