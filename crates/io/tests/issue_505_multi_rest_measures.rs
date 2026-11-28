#![cfg(feature = "musicxml")]
//! Issue #505: `<multiple-rest>` is a display hint, so MusicXML export must still
//! write every measure that the multi-measure rest covers.
use acorde_io::musicxml::{parse_musicxml, serialize_musicxml};

fn rest_measure(number: usize, attributes: &str) -> String {
    format!(
        "<measure number=\"{number}\">{attributes}<note><rest measure=\"yes\"/><duration>4</duration><voice>1</voice></note></measure>"
    )
}

fn fixture() -> String {
    let first = rest_measure(
        1,
        "<attributes><divisions>1</divisions><key><fifths>0</fifths></key><time><beats>4</beats><beat-type>4</beat-type></time><clef><sign>G</sign><line>2</line></clef><measure-style><multiple-rest>3</multiple-rest></measure-style></attributes>",
    );
    let mut measures = vec![first, rest_measure(2, ""), rest_measure(3, "")];
    measures.push(
        "<measure number=\"4\"><note><pitch><step>C</step><octave>5</octave></pitch><duration>4</duration><voice>1</voice><type>whole</type></note></measure>"
            .to_string(),
    );
    format!(
        "<?xml version=\"1.0\" encoding=\"UTF-8\"?><score-partwise version=\"4.0\"><part-list><score-part id=\"P1\"><part-name>Music</part-name></score-part></part-list><part id=\"P1\">{}</part></score-partwise>",
        measures.concat()
    )
}

#[test]
fn export_keeps_measures_covered_by_multi_measure_rest() {
    let score = parse_musicxml(&fixture()).unwrap();
    assert_eq!(score.parts[0].staves[0].measures.len(), 4);

    let xml = serialize_musicxml(&score).unwrap();
    assert_eq!(xml.matches("<measure number=").count(), 4, "{xml}");
    assert_eq!(xml.matches("<multiple-rest>3</multiple-rest>").count(), 1);

    let reopened = parse_musicxml(&xml).unwrap();
    let measures = &reopened.parts[0].staves[0].measures;
    assert_eq!(measures.len(), 4);
    assert_eq!(measures[0].multi_rest_count, Some(3));
    assert!(measures[3].voices[0].iter().any(|note| !note.is_rest));
}
