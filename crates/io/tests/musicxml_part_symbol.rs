use acorde_core::PartGroupSymbol;
use acorde_io::{parse_musicxml, parse_musicxml_with_report, serialize_musicxml};

fn score(symbol: &str, staves: usize) -> String {
    format!(
        r#"<score-partwise version="4.0"><part-list><score-part id="P1"><part-name>Organ</part-name></score-part></part-list><part id="P1"><measure number="1"><attributes><divisions>1</divisions><staves>{staves}</staves>{symbol}</attributes><note><pitch><step>C</step><octave>4</octave></pitch><duration>1</duration><type>quarter</type></note></measure></part></score-partwise>"#
    )
}

#[test]
fn part_symbol_range_and_default_brace_survive_roundtrip() {
    for (xml, expected, range) in [
        (score("", 3), PartGroupSymbol::Brace, (0, 2)),
        (
            score(
                r#"<part-symbol top-staff="1" bottom-staff="2">square</part-symbol>"#,
                3,
            ),
            PartGroupSymbol::Square,
            (0, 1),
        ),
        (
            score("<part-symbol>none</part-symbol>", 2),
            PartGroupSymbol::None,
            (0, 1),
        ),
    ] {
        let parsed = parse_musicxml(&xml).unwrap();
        let groups = &parsed.parts[0].staff_groups;
        assert_eq!(groups.len(), 1);
        assert_eq!(groups[0].symbol, expected);
        assert_eq!((groups[0].first_staff, groups[0].last_staff), range);
        let restored = parse_musicxml(&serialize_musicxml(&parsed).unwrap()).unwrap();
        assert_eq!(restored.parts[0].staff_groups, *groups);
    }
}

#[test]
fn changed_or_invalid_part_symbol_is_not_silently_accepted() {
    let xml = score("<part-symbol>brace</part-symbol>", 2).replace("</measure>", "</measure><measure number=\"2\"><attributes><part-symbol>none</part-symbol></attributes><note><rest/><duration>1</duration><type>quarter</type></note></measure>");
    let report = parse_musicxml_with_report(&xml).unwrap();
    assert_eq!(
        report.score.parts[0].staff_groups[0].symbol,
        PartGroupSymbol::Brace
    );
    assert!(
        report
            .diagnostics
            .iter()
            .any(|d| d.code == "musicxml.part-symbol-change-not-preserved")
    );
    assert!(
        parse_musicxml(&score(
            r#"<part-symbol bottom-staff="3">brace</part-symbol>"#,
            2
        ))
        .is_err()
    );
    assert!(parse_musicxml(&score("<part-symbol/>", 2)).is_err());
}

#[test]
fn square_part_group_survives_musicxml_roundtrip() {
    let xml = score("", 2).replace("<score-part id=\"P1\">", "<part-group type=\"start\"><group-symbol>square</group-symbol></part-group><score-part id=\"P1\">").replace("</score-part></part-list>", "</score-part><part-group type=\"stop\"/></part-list>");
    let parsed = parse_musicxml(&xml).unwrap();
    assert_eq!(parsed.part_groups[0].symbol, PartGroupSymbol::Square);
    let restored = parse_musicxml(&serialize_musicxml(&parsed).unwrap()).unwrap();
    assert_eq!(restored.part_groups[0].symbol, PartGroupSymbol::Square);
}

#[test]
fn unsupported_nested_staff_groups_are_export_diagnosed() {
    let mut score = parse_musicxml(&score("", 3)).unwrap();
    score.parts[0].staff_groups.push(acorde_core::StaffGroup {
        first_staff: 0,
        last_staff: 1,
        symbol: PartGroupSymbol::Bracket,
        barlines_connect: false,
    });
    let report = acorde_io::serialize_musicxml_with_report(&score).unwrap();
    assert!(
        report
            .diagnostics
            .iter()
            .any(|d| d.code == "musicxml.export-part-staff-groups-incomplete")
    );
}

#[cfg(feature = "mei")]
#[test]
fn square_staff_connector_survives_mei_roundtrip() {
    let parsed = parse_musicxml(&score("<part-symbol>square</part-symbol>", 2)).unwrap();
    let mei = acorde_io::serialize_mei(&parsed).unwrap();
    assert!(mei.contains("symbol=\"bracketsq\""));
    let restored = acorde_io::parse_mei(&mei).unwrap();
    assert!(
        restored
            .parts
            .iter()
            .flat_map(|p| &p.staff_groups)
            .any(|g| g.symbol == PartGroupSymbol::Square)
    );
}

#[test]
fn part_barline_is_assigned_to_the_current_measure_on_empty_staves() {
    let xml = r#"<score-partwise><part-list><score-part id="P1"><part-name>Piano</part-name></score-part></part-list><part id="P1"><measure number="1"><attributes><divisions>1</divisions><staves>2</staves></attributes><note><pitch><step>C</step><octave>4</octave></pitch><duration>1</duration><voice>1</voice><type>quarter</type><staff>1</staff></note><backup><duration>1</duration></backup><note><pitch><step>C</step><octave>3</octave></pitch><duration>1</duration><voice>2</voice><type>quarter</type><staff>2</staff></note></measure><measure number="2"><note><pitch><step>D</step><octave>4</octave></pitch><duration>1</duration><voice>1</voice><type>quarter</type><staff>1</staff></note><barline location="right"><repeat direction="backward"/></barline></measure></part></score-partwise>"#;
    let parsed = parse_musicxml(xml).unwrap();
    for staff in &parsed.parts[0].staves {
        assert_eq!(
            staff.measures[0].barline_right,
            acorde_core::Barline::Normal
        );
        assert_eq!(
            staff.measures[1].barline_right,
            acorde_core::Barline::RepeatEnd
        );
    }
    let restored = parse_musicxml(&serialize_musicxml(&parsed).unwrap()).unwrap();
    for staff in &restored.parts[0].staves {
        assert_eq!(
            staff.measures[0].barline_right,
            acorde_core::Barline::Normal
        );
        assert_eq!(
            staff.measures[1].barline_right,
            acorde_core::Barline::RepeatEnd
        );
    }
}
