#![cfg(feature = "musicxml")]

#[test]
fn authored_lyric_placement_is_reported_as_lost() {
    let xml = r#"<score-partwise><part-list><score-part id="P1"><part-name>Voice</part-name></score-part></part-list><part id="P1"><measure number="1"><attributes><divisions>1</divisions></attributes><note><pitch><step>C</step><octave>4</octave></pitch><duration>1</duration><type>quarter</type><lyric placement="above"><text>Write</text></lyric></note></measure></part></score-partwise>"#;
    let report = acorde_io::parse_musicxml_with_report(xml).unwrap();
    assert_eq!(
        report.score.parts[0].staves[0].measures[0].voices[0][0]
            .lyric
            .as_ref()
            .unwrap()
            .text,
        "Write"
    );
    assert!(
        report
            .diagnostics
            .iter()
            .any(|d| d.code == "musicxml.unsupported-lyric-placement"
                && d.is_loss()
                && d.preserved_value.as_deref() == Some("above"))
    );
}
