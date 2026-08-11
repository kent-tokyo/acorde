//! End-to-end pipeline test: MusicXML -> acorde-io -> Score -> acorde-render-svg -> SVG.
//!
//! Exercises the exact path `examples/render_musicxml.rs` demonstrates, as a durable
//! regression test rather than something only verified by manually running the example.

use acorde_render_svg::{render_svg, SvgRenderOptions};

fn opts() -> SvgRenderOptions {
    SvgRenderOptions { width: 900.0, staff_size: 24.0, measures_per_system: 4, interactive: true }
}

const SIMPLE_FIXTURE: &str =
    concat!(env!("CARGO_MANIFEST_DIR"), "/../../tests/fixtures/simple.musicxml");

#[test]
fn simple_musicxml_renders_to_well_formed_svg() {
    let xml = std::fs::read_to_string(SIMPLE_FIXTURE).unwrap();
    let score = acorde_io::parse_musicxml(&xml).unwrap();
    let svg = render_svg(&score, &opts()).unwrap();

    assert!(svg.starts_with("<svg"));
    assert!(svg.trim_end().ends_with("</svg>"));
    // 4 quarter notes + 1 half note + 1 rest, single voice/staff.
    assert_eq!(svg.matches(r#"data-acorde-kind="note""#).count(), 5);
    assert_eq!(svg.matches(r#"data-acorde-kind="rest""#).count(), 1);
}

#[test]
fn musicxml_pipeline_is_deterministic() {
    let xml = std::fs::read_to_string(SIMPLE_FIXTURE).unwrap();
    let a = render_svg(&acorde_io::parse_musicxml(&xml).unwrap(), &opts()).unwrap();
    let b = render_svg(&acorde_io::parse_musicxml(&xml).unwrap(), &opts()).unwrap();
    assert_eq!(a, b);
}
