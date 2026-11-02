//! Cross-format edge cases that must not silently change a score.

#[cfg(feature = "abc")]
#[test]
fn abc_major_and_minor_key_signatures_round_trip_across_circle_of_fifths() {
    let keys = [
        ("Cb", -7, "major"),
        ("Gb", -6, "major"),
        ("Db", -5, "major"),
        ("Ab", -4, "major"),
        ("Eb", -3, "major"),
        ("Bb", -2, "major"),
        ("F", -1, "major"),
        ("C", 0, "major"),
        ("G", 1, "major"),
        ("D", 2, "major"),
        ("A", 3, "major"),
        ("E", 4, "major"),
        ("B", 5, "major"),
        ("F#", 6, "major"),
        ("C#", 7, "major"),
        ("Abm", -7, "minor"),
        ("Ebm", -6, "minor"),
        ("Bbm", -5, "minor"),
        ("Fm", -4, "minor"),
        ("Cm", -3, "minor"),
        ("Gm", -2, "minor"),
        ("Dm", -1, "minor"),
        ("Am", 0, "minor"),
        ("Em", 1, "minor"),
        ("Bm", 2, "minor"),
        ("F#m", 3, "minor"),
        ("C#m", 4, "minor"),
        ("G#m", 5, "minor"),
        ("D#m", 6, "minor"),
        ("A#m", 7, "minor"),
    ];
    for (key, fifths, mode) in keys {
        let input = format!("X:1\nT:Key test\nM:4/4\nL:1/4\nK:{key}\nC D E F |\n");
        let score = acorde_io::parse_abc(&input).unwrap_or_else(|e| panic!("{key}: {e}"));
        assert_eq!(
            (
                score.settings.key_signature.fifths,
                score.settings.key_signature.mode.as_str()
            ),
            (fifths, mode),
            "input {key}"
        );
        let output = acorde_io::serialize_abc(&score).expect("ABC serialization succeeds");
        assert!(
            output.lines().any(|line| line == format!("K:{key}")),
            "{key}: {output}"
        );
        let restored = acorde_io::parse_abc(&output).expect("serialized ABC parses");
        assert_eq!(
            restored.settings.key_signature, score.settings.key_signature,
            "round trip {key}"
        );
    }
}

#[cfg(feature = "mei")]
#[test]
fn mei_invalid_values_and_unattached_harmony_report_source_and_value() {
    let input = r##"<mei><music><body><mdiv><score><section><measure n="1"><harm deg="V7" chordref="#missing" func="D" tstamp2="invalid">G7</harm><staff n="1"><layer n="1"><multiRest num="0"/><note pname="c" oct="4" dur="4"/></layer></staff></measure></section></score></mdiv></body></music></mei>"##;
    let report = acorde_io::parse_mei_with_report(input).expect("MEI remains importable");
    for (code, value) in [
        ("mei.unsupported-attribute.harm.deg", "V7"),
        ("mei.unsupported-attribute.harm.chordref", "#missing"),
        ("mei.unsupported-attribute.harm.func", "D"),
        ("mei.invalid-value.harm.tstamp2", "invalid"),
        ("mei.invalid-value.multiRest.num", "0"),
    ] {
        let item = report
            .diagnostics
            .iter()
            .find(|d| d.code == code)
            .unwrap_or_else(|| panic!("missing {code}: {:?}", report.diagnostics));
        assert_eq!(item.preserved_value.as_deref(), Some(value), "{code}");
        assert!(item.source_location.is_some(), "{code}");
        assert!(item.is_loss(), "{code}");
    }
    assert!(!report.score.parts.is_empty());
}

#[cfg(feature = "abc")]
#[test]
fn abc_lyrics_preserve_skips_and_melismas_in_multiple_verses() {
    let input = "X:1\nT:Verses\nM:4/4\nL:1/4\nK:C\nC D E F |\nw: one _ * four\nw: two _ * end\n";
    let score = acorde_io::parse_abc(input).expect("ABC parses");
    let notes = &score.parts[0].staves[0].measures[0].voices[0];
    let first = notes[0].lyric.as_ref().expect("first verse");
    assert_eq!(first.text, "one");
    assert!(first.extend);
    let second = &notes[0].additional_lyrics[0];
    assert_eq!(second.verse, 2);
    assert_eq!(second.lyric.text, "two");
    assert!(second.lyric.extend);
    assert!(notes[1].lyric.is_none());
    assert!(notes[2].lyric.is_none());
    assert_eq!(notes[3].lyric.as_ref().unwrap().text, "four");
    assert_eq!(notes[3].additional_lyrics[0].lyric.text, "end");
}

#[cfg(feature = "mei")]
#[test]
fn mei_chord_definition_accidentals_round_trip_with_sounding_pitch() {
    let accidentals = [
        ("3qf", 0, -75),
        ("qf", 0, -50),
        ("1qf", 0, -25),
        ("1qs", 0, 25),
        ("qs", 0, 50),
        ("3qs", 0, 75),
        ("ff", -2, 0),
        ("f", -1, 0),
        ("s", 1, 0),
        ("ss", 2, 0),
    ];
    for (accid, alter, cents) in accidentals {
        let input = format!(
            r#"<mei><music><body><mdiv><score><section><chordTable><chordDef xml:id="c"><chordMember pname="c" oct="4" accid.ges="{accid}"/></chordDef></chordTable><measure n="1"><staff n="1"><layer n="1"><note pname="c" oct="4" dur="4"/></layer></staff></measure></section></score></mdiv></body></music></mei>"#
        );
        let score = acorde_io::parse_mei(&input).expect("MEI parses");
        let pitch = score.chord_definitions[0].members[0]
            .pitch
            .as_ref()
            .unwrap();
        assert_eq!(
            (pitch.alter, pitch.microtone_cents),
            (alter, cents),
            "{accid}"
        );
        let output = acorde_io::serialize_mei(&score).expect("MEI serializes");
        assert!(
            output.contains(&format!("accid.ges=\"{accid}\"")),
            "{output}"
        );
        let restored = acorde_io::parse_mei(&output).expect("serialized MEI parses");
        assert_eq!(
            restored.chord_definitions, score.chord_definitions,
            "{accid}"
        );
    }
}

#[cfg(all(feature = "musicxml", feature = "mei"))]
#[test]
fn openscore_musicxml_to_mei_keeps_written_note_count() {
    let fixtures: &[(&str, &[u8])] = &[
        (
            "Aloha Oe",
            include_bytes!("../../../tests/fixtures/openscore_lieder_aloha_oe.mxl"),
        ),
        (
            "Quilter Prelude",
            include_bytes!("../../../tests/fixtures/openscore_lieder_quilter_prelude.mxl"),
        ),
        (
            "Just for Today",
            include_bytes!("../../../tests/fixtures/openscore_lieder_just_for_today.mxl"),
        ),
        (
            "Webern",
            include_bytes!(
                "../../../tests/fixtures/openscore_lieder_webern_so_ich_traurig_bin.mxl"
            ),
        ),
    ];
    for (name, data) in fixtures {
        let original = acorde_io::parse_mxl(data).unwrap_or_else(|e| panic!("{name}: {e}"));
        let output = acorde_io::serialize_mei_with_report(&original)
            .unwrap_or_else(|e| panic!("{name}: {e}"));
        let restored =
            acorde_io::parse_mei(&output.output).unwrap_or_else(|e| panic!("{name}: {e}"));
        assert_eq!(
            restored.statistics().note_count,
            original.statistics().note_count,
            "{name}"
        );
        let pitches = |score: &acorde_core::Score| {
            let mut values: Vec<i32> = score
                .parts
                .iter()
                .flat_map(|part| &part.staves)
                .flat_map(|staff| &staff.measures)
                .flat_map(|measure| &measure.voices)
                .flat_map(|voice| voice.iter())
                .flat_map(|note| &note.pitches)
                .map(|pitch| pitch.to_midi_cents())
                .collect();
            values.sort_unstable();
            values
        };
        assert_eq!(
            pitches(&restored),
            pitches(&original),
            "written pitches: {name}"
        );
    }
}
