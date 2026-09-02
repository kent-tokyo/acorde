# Print layout contract

`acorde` owns reusable score semantics and deterministic logical placement. The current
`acorde-layout::compute_print_layout` API is the neutral boundary between a `Score` and a
print-capable host:

```text
Score → PrintConfig → PrintLayoutResult (pages/systems in mm) → SVG/PDF/print host
```

`PrintConfig` defines paper size, orientation, margins, bleed/safe areas, system height, scale,
page-numbering policy, and measures per system. Scale applies to system content geometry while
paper dimensions remain the selected physical page size. Safe-area values reduce the usable
content rectangle; bleed values and the optional one-based page number are carried as explicit
page metadata for a host exporter.
`PrintLayoutResult` records page dimensions, stable page/system addresses, physical measure
indices, and typed break reasons (`MeasureCapacity`, `ExplicitSystemBreak`,
`ExplicitPageBreak`, `PageCapacity`, or `EndOfScore`). Layout honors existing `system_break` and
`page_break` decisions and produces stable output for the same score and configuration. Its
`contract_version` is `5` for this address/diagnostic, bleed/safe-area, scale, and page-numbering
shape.

This API deliberately does not select or embed fonts, draw glyphs, generate PDF, open files,
invoke OS printer APIs, or provide a preview UI. Those responsibilities belong to
`acorde-render-svg`, a future `acorde-print` crate, or the consuming application/backend.

The current contract is a foundation, not full engraving parity: keep-together groups, balanced
final systems, extracted-part policies, headers/footers, collision-aware spacing, and print SVG
export metadata remain roadmap work. Unsupported or incomplete notation must continue to be
reported through the format capability boundaries rather than treated as lossless.
