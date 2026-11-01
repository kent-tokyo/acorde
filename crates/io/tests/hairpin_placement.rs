use acorde_io::{parse_musicxml, serialize_musicxml};

#[test]
fn hairpin_placement_survives_supported_interchange_formats() {
    for (attr, expected) in [
        (" placement=\"above\"", Some(true)),
        (" placement=\"below\"", Some(false)),
        ("", None),
    ] {
        let xml = format!(
            r#"<score-partwise><part-list><score-part id="P1"><part-name>Piano</part-name></score-part></part-list><part id="P1"><measure number="1"><attributes><divisions>1</divisions><time><beats>2</beats><beat-type>4</beat-type></time><clef><sign>G</sign><line>2</line></clef></attributes><direction{attr}><direction-type><wedge type="crescendo"/></direction-type></direction><note><pitch><step>C</step><octave>4</octave></pitch><duration>1</duration><type>quarter</type></note><note><pitch><step>D</step><octave>4</octave></pitch><duration>1</duration><type>quarter</type></note><direction><direction-type><wedge type="stop"/></direction-type></direction></measure></part></score-partwise>"#
        );
        let score = parse_musicxml(&xml).unwrap();
        let placement =
            |s: &acorde_core::Score| s.parts[0].staves[0].measures[0].voices[0][0].hairpin_above;
        assert_eq!(placement(&score), expected);
        #[cfg(feature = "abc")]
        assert_eq!(
            acorde_io::serialize_abc_with_report(&score)
                .unwrap()
                .diagnostics
                .iter()
                .any(|d| d
                    .source_location
                    .as_deref()
                    .is_some_and(|p| p.ends_with("/hairpin-placement"))),
            expected.is_some()
        );
        #[cfg(feature = "midi")]
        assert_eq!(
            acorde_io::serialize_midi_with_report(&score)
                .unwrap()
                .diagnostics
                .iter()
                .any(|d| d
                    .source_location
                    .as_deref()
                    .is_some_and(|p| p.ends_with("/hairpin-placement"))),
            expected.is_some()
        );
        assert_eq!(
            placement(&parse_musicxml(&serialize_musicxml(&score).unwrap()).unwrap()),
            expected
        );
        #[cfg(feature = "mscz")]
        assert_eq!(
            placement(&acorde_io::parse_mscx(&acorde_io::serialize_mscx(&score).unwrap()).unwrap()),
            expected
        );
        #[cfg(feature = "mei")]
        assert_eq!(
            placement(&acorde_io::parse_mei(&acorde_io::serialize_mei(&score).unwrap()).unwrap()),
            expected
        );
    }
}
