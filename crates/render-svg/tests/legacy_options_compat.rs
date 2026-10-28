//! Compiles as an external crate and protects the legacy public struct literal.

use acorde_render_svg::{SvgRenderConfig, SvgRenderOptions};

#[test]
fn legacy_four_field_literal_remains_source_compatible() {
    let options = SvgRenderOptions {
        width: 900.0,
        staff_size: 24.0,
        measures_per_system: 4,
        interactive: true,
    };
    let config = SvgRenderConfig::new(options).with_staff_gap(8.0);
    assert_eq!(config.staff_gap(), 8.0);
}
