#![cfg(feature = "musicxml")]

#[test]
fn lyric_placement_roundtrips_per_verse_and_old_json_defaults_to_unspecified() {
    let xml = r#"<score-partwise><part-list><score-part id="P1"><part-name>Voice</part-name></score-part></part-list><part id="P1"><measure number="1"><attributes><divisions>1</divisions></attributes><note><pitch><step>C</step><octave>4</octave></pitch><duration>1</duration><type>quarter</type><lyric number="1" placement="above"><text>Write</text></lyric><lyric number="2" placement="below"><text>Below</text></lyric></note><note><pitch><step>D</step><octave>4</octave></pitch><duration>1</duration><type>quarter</type><lyric><text>Unspecified</text></lyric></note></measure></part></score-partwise>"#;
    let report = acorde_io::parse_musicxml_with_report(xml).unwrap();
    assert!(
        report
            .diagnostics
            .iter()
            .any(|d| d.code == "musicxml.lyric-above-rendering-unsupported"
                && d.preserved_value.as_deref() == Some("above"))
    );
    let score = report.score;
    let notes = &score.parts[0].staves[0].measures[0].voices[0];
    assert_eq!(notes[0].lyric.as_ref().unwrap().placement_above, Some(true));
    assert_eq!(
        notes[0].additional_lyrics[0].lyric.placement_above,
        Some(false)
    );
    assert_eq!(notes[1].lyric.as_ref().unwrap().placement_above, None);
    let xml = acorde_io::serialize_musicxml(&score).unwrap();
    let restored = acorde_io::parse_musicxml(&xml).unwrap();
    let actual = &restored.parts[0].staves[0].measures[0].voices[0];
    assert_eq!(actual[0].lyric, notes[0].lyric);
    assert_eq!(actual[0].additional_lyrics, notes[0].additional_lyrics);
    assert_eq!(actual[1].lyric, notes[1].lyric);
    let json = serde_json::to_string(&score).unwrap();
    let restored: acorde_core::Score = serde_json::from_str(&json).unwrap();
    let json_notes = &restored.parts[0].staves[0].measures[0].voices[0];
    assert_eq!(json_notes[0].lyric, notes[0].lyric);
    assert_eq!(json_notes[0].additional_lyrics, notes[0].additional_lyrics);
    let legacy: acorde_core::Lyric =
        serde_json::from_str(r#"{"text":"old","syllabic":"single"}"#).unwrap();
    assert_eq!(legacy.placement_above, None);
}

#[test]
#[cfg(all(feature = "abc", feature = "mei", feature = "mscz", feature = "midi"))]
fn other_exporters_report_the_lyric_side_they_cannot_write() {
    let xml = r#"<score-partwise><part-list><score-part id="P1"><part-name>Voice</part-name></score-part></part-list><part id="P1"><measure number="1"><attributes><divisions>1</divisions></attributes><note><pitch><step>C</step><octave>4</octave></pitch><duration>1</duration><type>quarter</type><lyric placement="above"><text>Write</text></lyric></note></measure></part></score-partwise>"#;
    let score = acorde_io::parse_musicxml(xml).unwrap();
    let diagnostics = [
        acorde_io::serialize_abc_with_report(&score)
            .unwrap()
            .diagnostics,
        acorde_io::serialize_mei_with_report(&score)
            .unwrap()
            .diagnostics,
        acorde_io::serialize_mscx_with_report(&score)
            .unwrap()
            .diagnostics,
        acorde_io::serialize_midi_with_report(&score)
            .unwrap()
            .diagnostics,
    ];
    for (format, report) in ["abc", "mei", "mscx", "midi"].into_iter().zip(diagnostics) {
        assert!(report.iter().any(|d| d.code
            == format!("{format}.export-unsupported-lyric-placement")
            && d.is_loss()));
    }
}

#[test]
fn invalid_lyric_placement_is_diagnosed() {
    let xml = r#"<score-partwise><part-list><score-part id="P1"><part-name>Voice</part-name></score-part></part-list><part id="P1"><measure number="1"><attributes><divisions>1</divisions></attributes><note><pitch><step>C</step><octave>4</octave></pitch><duration>1</duration><type>quarter</type><lyric placement="sideways"><text>Write</text></lyric></note></measure></part></score-partwise>"#;
    let report = acorde_io::parse_musicxml_with_report(xml).unwrap();
    assert!(
        report
            .diagnostics
            .iter()
            .any(|d| d.code == "musicxml.invalid-lyric-placement"
                && d.is_loss()
                && d.preserved_value.as_deref() == Some("sideways"))
    );
}
