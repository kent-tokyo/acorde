# acorde-layout

Logical, pixel-free layout computation for acorde-core scores.

compute_layout(&Score, &LayoutConfig) returns visual measure slots, rows, resolved spans, beam
groups, tuplet groups, concert-pitch key overrides, and mandatory/courtesy accidental marks. It
does not render pixels and has no browser or filesystem dependency.

`compute_print_layout(&Score, &PrintConfig)` adds a host-neutral page/system plan. Dimensions
are physical millimetres, and explicit `Measure::page_break` decisions are preserved. The
result contains page dimensions, stable page/system addresses, physical measure indices, typed
break reasons, explicit bleed metadata, scaled system geometry, configurable page numbering, and
host-facing color/crop-mark/glyph-resource policies.
Safe areas constrain the content rectangle, but the crate does not choose fonts, emit PDF, access
printers, or perform filesystem I/O.

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
