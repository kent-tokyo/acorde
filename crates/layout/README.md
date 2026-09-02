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
`NotationBreakPolicy::KeepVoltaTogether` can preserve contiguous volta endings during system
breaking when explicitly enabled; `NotationBreakPolicy::KeepRepeatsTogether` can start repeat
sections on a fresh page and keep them together when they fit the page capacity;
invalid, over-capacity, or explicit-break-conflicting ranges return typed errors.
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
