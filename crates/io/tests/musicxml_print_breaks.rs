#![cfg(feature = "musicxml")]

use acorde_io::{parse_musicxml, serialize_musicxml_with_report};

#[test]
fn musicxml_print_breaks_apply_before_the_containing_measure() {
    let xml = r#"<score-partwise version="4.0"><part-list><score-part id="P1"><part-name>Piano</part-name></score-part></part-list><part id="P1"><measure number="1"><print new-page="yes"/><attributes><divisions>1</divisions></attributes><note><pitch><step>C</step><octave>4</octave></pitch><duration>1</duration><type>quarter</type></note></measure><measure number="2"><print new-system="yes"><system-layout/></print><note><pitch><step>D</step><octave>4</octave></pitch><duration>1</duration><type>quarter</type></note></measure><measure number="3"><print new-page="yes"/><note><pitch><step>E</step><octave>4</octave></pitch><duration>1</duration><type>quarter</type></note></measure></part></score-partwise>"#;
    let score = parse_musicxml(xml).unwrap();
    let bars = &score.parts[0].staves[0].measures;
    assert_eq!(
        bars.iter()
            .map(|m| (m.system_break, m.page_break))
            .collect::<Vec<_>>(),
        [(true, false), (false, true), (false, false)]
    );
    let exported = serialize_musicxml_with_report(&score).unwrap();
    let first = exported.output.find("<measure number=\"1\"").unwrap();
    let second = exported.output.find("<measure number=\"2\"").unwrap();
    let third = exported.output.find("<measure number=\"3\"").unwrap();
    assert!(!exported.output[first..second].contains("<print new-"));
    assert!(exported.output[second..third].contains("<print new-system=\"yes\""));
    assert!(exported.output[third..].contains("<print new-page=\"yes\""));
    let restored = parse_musicxml(&exported.output).unwrap();
    let again = &restored.parts[0].staves[0].measures;
    assert_eq!(
        again
            .iter()
            .map(|m| (m.system_break, m.page_break))
            .collect::<Vec<_>>(),
        [(true, false), (false, true), (false, false)]
    );
}

#[test]
fn trailing_break_is_reported_as_unrepresentable() {
    let xml = r#"<score-partwise><part-list><score-part id="P1"><part-name>Piano</part-name></score-part></part-list><part id="P1"><measure number="1"><attributes><divisions>1</divisions></attributes><note><pitch><step>C</step><octave>4</octave></pitch><duration>1</duration><type>quarter</type></note></measure></part></score-partwise>"#;
    let mut score = parse_musicxml(xml).unwrap();
    score.parts[0].staves[0].measures[0].page_break = true;
    let report = serialize_musicxml_with_report(&score).unwrap();
    assert!(
        report
            .diagnostics
            .iter()
            .any(|d| d.code == "musicxml.export-trailing-print-break" && d.is_loss())
    );
    assert!(!report.output.contains("<print new-page=\"yes\""));
}

#[test]
fn unsupported_no_break_and_page_number_are_reported() {
    let xml = r#"<score-partwise><part-list><score-part id="P1"><part-name>Piano</part-name></score-part></part-list><part id="P1"><measure number="1"><attributes><divisions>1</divisions></attributes><note><pitch><step>C</step><octave>4</octave></pitch><duration>1</duration><type>quarter</type></note></measure><measure number="2"><print new-system="no" new-page="yes" page-number="9"/><note><pitch><step>D</step><octave>4</octave></pitch><duration>1</duration><type>quarter</type></note></measure></part></score-partwise>"#;
    let report = acorde_io::parse_musicxml_with_report(xml).unwrap();
    assert!(report.score.parts[0].staves[0].measures[0].page_break);
    let losses: Vec<_> = report
        .diagnostics
        .iter()
        .filter(|d| d.code == "musicxml.unsupported-print-pagination-attribute")
        .collect();
    assert_eq!(losses.len(), 2);
    assert!(losses.iter().all(|d| d.is_loss()));
}
