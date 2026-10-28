# acorde-layout

Logical, pixel-free layout computation for acorde-core scores.

compute_layout(&Score, &LayoutConfig) returns visual measure slots, rows, resolved spans, typed
stable-ID spanners, beam groups, tuplet groups, concert-pitch key overrides, and mandatory/courtesy accidental marks. It
does not render pixels and has no browser or filesystem dependency.

`compute_print_layout(&Score, &PrintConfig)` adds a host-neutral page/system plan in physical
millimetres. It preserves explicit breaks and returns stable page/system addresses, physical
measure spans (including multirests), typed break reasons, span continuation ownership, measure
marks, publication metadata, and glyph-resource diagnostics. A4/Letter score and part presets,
pickup/final-page, keep-together, and repeat/volta policies are explicit configuration.

`PrintLayoutResult::validate` and `export_page_artifacts` reject invalid addresses, non-finite
geometry, over-capacity ranges, and conflicting explicit breaks before host reuse. Glyph helpers
provide deterministic placement, collision separation, spacing, and content bounds; font loading,
shaping, PDF output, printers, and filesystem access remain host responsibilities.

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
