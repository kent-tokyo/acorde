//! End-to-end pipeline test: MusicXML -> acorde-io -> Score -> acorde-render-svg -> SVG.
//!
//! Exercises the exact path `examples/render_musicxml.rs` demonstrates, as a durable
//! regression test rather than something only verified by manually running the example.

use acorde_render_svg::{SvgRenderOptions, render_svg};

fn opts() -> SvgRenderOptions {
    SvgRenderOptions {
        width: 900.0,
        staff_size: 24.0,
        measures_per_system: 4,
        interactive: true,
    }
}

const SIMPLE_FIXTURE: &str = concat!(
    env!("CARGO_MANIFEST_DIR"),
    "/../../tests/fixtures/simple.musicxml"
);

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

#[test]
fn adjacent_numbered_musicxml_slurs_survive_round_trip_and_render() {
    let source = r#"<score-partwise version="4.0"><part-list><score-part id="P1"><part-name>Piano</part-name></score-part></part-list><part id="P1"><measure number="1"><attributes><divisions>480</divisions><time><beats>3</beats><beat-type>4</beat-type></time><clef><sign>G</sign><line>2</line></clef></attributes><note><pitch><step>C</step><octave>4</octave></pitch><duration>480</duration><type>quarter</type><notations><slur number="1" type="start"/></notations></note><note><pitch><step>D</step><octave>4</octave></pitch><duration>480</duration><type>quarter</type><notations><slur number="1" type="stop"/><slur number="2" type="start"/></notations></note><note><pitch><step>E</step><octave>4</octave></pitch><duration>480</duration><type>quarter</type><notations><slur number="2" type="stop"/></notations></note></measure></part></score-partwise>"#;
    let parsed = acorde_io::parse_musicxml(source).expect("parse numbered slurs");
    let serialized = acorde_io::serialize_musicxml(&parsed).expect("serialize numbered slurs");
    let restored = acorde_io::parse_musicxml(&serialized).expect("reparse numbered slurs");
    let pairs = acorde_layout::compute_layout(&restored, &Default::default())
        .spans
        .iter()
        .filter_map(|span| match span {
            acorde_layout::SpanMark::Slur { start, end } => Some((start.note, end.note)),
            _ => None,
        })
        .collect::<Vec<_>>();
    assert_eq!(pairs, vec![(0, 1), (1, 2)]);
    let svg = render_svg(&restored, &opts()).expect("render numbered slurs");
    assert_eq!(svg.matches("class=\"acorde-slur\"").count(), 2);
}
