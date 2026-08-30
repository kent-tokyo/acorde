# acorde-layout

Logical, pixel-free layout computation for acorde-core scores.

compute_layout(&Score, &LayoutConfig) returns visual measure slots, rows, resolved spans, beam
groups, tuplet groups, concert-pitch key overrides, and mandatory/courtesy accidental marks. It
does not render pixels and has no browser or filesystem dependency.

~~~rust
use acorde_core::Score;
use acorde_layout::{compute_layout, LayoutConfig};

let score = Score::default();
let layout = compute_layout(&score, &LayoutConfig::default());
~~~

LayoutConfig supports measures_per_row, first_row_measures, and concert_pitch. Consumers such as
SVG or Canvas renderers own pixel coordinates.

[API documentation](https://docs.rs/acorde-layout) · [Repository](https://github.com/kent-tokyo/acorde)
