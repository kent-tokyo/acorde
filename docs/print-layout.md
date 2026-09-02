# Print layout contract

`acorde` owns reusable score semantics and deterministic logical placement. The current
`acorde-layout::compute_print_layout` API is the neutral boundary between a `Score` and a
print-capable host:

```text
Score → PrintConfig → PrintLayoutResult (pages/systems in mm) → SVG/PDF/print host
```

`PrintConfig` defines paper size, orientation, margins, system height, and measures per system.
`PrintLayoutResult` records page dimensions and each system's physical measure indices. Layout
honors existing `system_break` and `page_break` decisions and produces stable output for the
same score and configuration.

This API deliberately does not select or embed fonts, draw glyphs, generate PDF, open files,
invoke OS printer APIs, or provide a preview UI. Those responsibilities belong to
`acorde-render-svg`, a future `acorde-print` crate, or the consuming application/backend.

The current contract is a foundation, not full engraving parity: keep-together groups, balanced
final systems, extracted-part policies, headers/footers, collision-aware spacing, and print SVG
export metadata remain roadmap work. Unsupported or incomplete notation must continue to be
reported through the format capability boundaries rather than treated as lossless.
