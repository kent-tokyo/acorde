use acorde_core::PartGroupSymbol;
use acorde_io::{parse_musicxml, serialize_musicxml};

#[test]
fn hidden_part_names_and_symbol_free_groups_survive_roundtrip() {
    let xml = r#"<score-partwise version="4.0"><part-list><part-group type="start" number="1"><group-symbol>none</group-symbol><group-barline>yes</group-barline></part-group><score-part id="P1"><part-name print-object="no">Piano</part-name><part-abbreviation print-object="no">Pno.</part-abbreviation></score-part><part-group type="stop" number="1"/><part-group type="start" number="2"/><score-part id="P2"><part-name>Flute</part-name></score-part><part-group type="stop" number="2"/></part-list><part id="P1"><measure number="1"><note><pitch><step>C</step><octave>4</octave></pitch><duration>1</duration><type>quarter</type></note></measure></part><part id="P2"><measure number="1"><note><pitch><step>D</step><octave>4</octave></pitch><duration>1</duration><type>quarter</type></note></measure></part></score-partwise>"#;
    let score = parse_musicxml(xml).unwrap();
    assert_eq!(score.parts[0].name, "Piano");
    assert_eq!(score.parts[0].short_name, "Pno.");
    assert!(score.parts[0].name_hidden && score.parts[0].short_name_hidden);
    assert!(!score.parts[1].name_hidden);
    assert_eq!(score.part_groups.len(), 2);
    assert!(
        score
            .part_groups
            .iter()
            .all(|g| g.symbol == PartGroupSymbol::None)
    );
    assert!(score.part_groups[0].barlines_connect);
    let exported = serialize_musicxml(&score).unwrap();
    assert!(exported.contains("<part-name print-object=\"no\">Piano</part-name>"));
    assert!(exported.contains("<part-abbreviation print-object=\"no\">Pno.</part-abbreviation>"));
    let restored = parse_musicxml(&exported).unwrap();
    assert!(restored.parts[0].name_hidden && restored.parts[0].short_name_hidden);
    assert_eq!(restored.part_groups.len(), 2);
    assert!(
        restored
            .part_groups
            .iter()
            .all(|g| g.symbol == PartGroupSymbol::None)
    );
    assert!(restored.part_groups[0].barlines_connect);
}
