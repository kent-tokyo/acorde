use acorde_io::{parse_musicxml, parse_musicxml_with_report, serialize_musicxml};

#[test]
fn work_metadata_and_printed_credit_remain_distinct_after_roundtrip() {
    let xml = r#"<score-partwise version="4.0"><work><work-number>Wq.117/11</work-number><work-title>Fantasia</work-title></work><movement-title>Movement</movement-title><identification><creator type="composer">Bach</creator></identification><credit page="1"><credit-type>title</credit-type><credit-words>Fantasia,
</credit-words><credit-words> Wq.117/11</credit-words></credit><credit page="1"><credit-type>subtitle</credit-type><credit-words>Allegro</credit-words></credit><credit page="2"><credit-type>title</credit-type><credit-words>Running header</credit-words></credit><part-list><score-part id="P1"><part-name>Piano</part-name></score-part></part-list><part id="P1"><measure number="1"><note><pitch><step>C</step><octave>4</octave></pitch><duration>1</duration><type>quarter</type></note></measure></part></score-partwise>"#;
    let report = parse_musicxml_with_report(xml).unwrap();
    assert!(
        report
            .diagnostics
            .iter()
            .any(|d| d.code == "musicxml.credit-formatting-not-preserved")
    );
    assert!(
        report
            .diagnostics
            .iter()
            .any(|d| d.code == "musicxml.additional-title-credit-not-preserved")
    );
    let score = report.score;
    assert_eq!(score.metadata.title, "Fantasia");
    assert_eq!(score.metadata.work_number, "Wq.117/11");
    assert_eq!(score.metadata.movement_title, "Movement");
    assert_eq!(score.metadata.print_title, "Fantasia,\n Wq.117/11");
    assert_eq!(score.metadata.print_subtitle, "Allegro");
    let output = serialize_musicxml(&score).unwrap();
    let restored = parse_musicxml(&output).unwrap();
    assert_eq!(restored.metadata.title, score.metadata.title);
    assert_eq!(restored.metadata.work_number, score.metadata.work_number);
    assert_eq!(restored.metadata.print_title, score.metadata.print_title);
    assert_eq!(
        restored.metadata.print_subtitle,
        score.metadata.print_subtitle
    );
}

#[test]
fn standalone_subtitle_credit_overrides_movement_title_for_printing() {
    let xml = r#"<score-partwise><work><work-title>Work</work-title></work><movement-title>Movement</movement-title><credit><credit-type>subtitle</credit-type><credit-words>Printed subtitle</credit-words></credit><part-list><score-part id="P1"><part-name>Piano</part-name></score-part></part-list><part id="P1"><measure number="1"><note><rest/><duration>1</duration><type>quarter</type></note></measure></part></score-partwise>"#;
    let score = parse_musicxml(xml).unwrap();
    assert_eq!(score.metadata.display_title(), "Work");
    assert_eq!(score.metadata.display_subtitle(), "Printed subtitle");
    let restored = parse_musicxml(&serialize_musicxml(&score).unwrap()).unwrap();
    assert_eq!(restored.metadata.display_subtitle(), "Printed subtitle");
}
