use acorde_core::TimeSymbol;
use acorde_render_svg::{SvgRenderOptions, render_svg};

#[test]
fn musescore_common_time_survives_mscz_and_renders() {
    let mscx = r#"<museScore version="4.20"><Score><Part><Staff id="1"/><trackName>Piano</trackName><Instrument/></Part><Staff id="1"><Measure><voice><TimeSig><subtype>1</subtype><sigN>4</sigN><sigD>4</sigD></TimeSig><Chord><durationType>whole</durationType><Note><pitch>60</pitch><tpc>14</tpc></Note></Chord></voice></Measure></Staff></Score></museScore>"#;
    let score = acorde_io::parse_mscx(mscx).unwrap();
    let data = acorde_io::serialize_mscz(&score).unwrap();
    let score = acorde_io::parse_mscz(&data).unwrap();
    assert_eq!(
        score.parts[0].staves[0].measures[0].time_symbol,
        Some(TimeSymbol::Common)
    );
    let svg = render_svg(&score, &SvgRenderOptions::default()).unwrap();
    assert!(svg.contains("acorde-time-symbol"));
    assert!(svg.contains("data-symbol=\"common\""));
}
