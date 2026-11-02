use acorde_core::ChordSymbol;
use acorde_core::model::standard_bass::{
    StandardBassChordButton, StandardBassNoteButton, StandardBassProfile,
};
use acorde_core::model::standard_bass_button_layout::{
    LocatedStandardBassProfile, PreviousStandardBassButtons, StandardBassButtonLocation,
    StandardBassButtonRow as Row, located_standard_bass_candidates,
};
fn chord(bass: Option<&str>) -> ChordSymbol {
    serde_json::from_value(serde_json::json!({"root":"G","kind":"dominant","bass":bass})).unwrap()
}
fn fixture() -> LocatedStandardBassProfile {
    LocatedStandardBassProfile {
        profile: StandardBassProfile {
            bass_buttons: vec![
                StandardBassNoteButton {
                    id: "G bass".into(),
                    position: 0,
                    midi: 43,
                },
                StandardBassNoteButton {
                    id: "F counterbass".into(),
                    position: 1,
                    midi: 41,
                },
            ],
            chord_buttons: vec![
                StandardBassChordButton {
                    id: "G7".into(),
                    position: 0,
                    midi: vec![67, 71, 77],
                },
                StandardBassChordButton {
                    id: "B diminished".into(),
                    position: 1,
                    midi: vec![71, 74, 77],
                },
                StandardBassChordButton {
                    id: "G major".into(),
                    position: 0,
                    midi: vec![67, 71, 74],
                },
            ],
            max_distance: 2,
        },
        locations: vec![
            StandardBassButtonLocation {
                id: "G bass".into(),
                row: Row::Fundamental,
                column: 2,
            },
            StandardBassButtonLocation {
                id: "F counterbass".into(),
                row: Row::CounterBass,
                column: 1,
            },
            StandardBassButtonLocation {
                id: "G7".into(),
                row: Row::Seventh,
                column: 2,
            },
            StandardBassButtonLocation {
                id: "B diminished".into(),
                row: Row::Diminished,
                column: 3,
            },
            StandardBassButtonLocation {
                id: "G major".into(),
                row: Row::Major,
                column: 2,
            },
        ],
    }
}
#[test]
fn direct_seventh_preferred_only_when_explicit_row_and_policy_allow_it() {
    let profile = fixture();
    let previous = PreviousStandardBassButtons {
        bass_button_id: "F counterbass".into(),
        chord_button_id: "G major".into(),
    };
    let found =
        located_standard_bass_candidates(&profile, &chord(None), true, false, Some(&previous))
            .unwrap();
    assert_eq!(found[0].candidate.chord_button_id, "G7");
    assert!(found[0].direct_seventh);
    assert_eq!(found[0].candidate.omitted_pitch_classes, [2]);
    assert_eq!(found[0].bass_location.row, Row::Fundamental);
    assert_eq!(found[0].chord_location.column, 2);
    assert_eq!(found[0].transition_column_distance, Some(1));
    let exact =
        located_standard_bass_candidates(&profile, &chord(None), false, false, None).unwrap();
    assert_eq!(exact[0].candidate.chord_button_id, "B diminished");
    assert!(!exact[0].direct_seventh);
    let slash =
        located_standard_bass_candidates(&profile, &chord(Some("F")), false, false, None).unwrap();
    assert_eq!(slash[0].candidate.bass_button_id, "F counterbass");
    assert_eq!(slash[0].candidate.chord_button_id, "G major");
    assert!(!slash[0].direct_seventh);
}
#[test]
fn incomplete_or_inconsistent_layout_is_rejected() {
    let mut profile = fixture();
    profile.locations.pop();
    assert!(located_standard_bass_candidates(&profile, &chord(None), true, false, None).is_err());
    let mut profile = fixture();
    profile.locations[0].row = Row::Major;
    assert!(located_standard_bass_candidates(&profile, &chord(None), true, false, None).is_err());
    let mut profile = fixture();
    profile.locations[3].row = Row::Seventh;
    profile.locations[3].column = 2;
    assert!(located_standard_bass_candidates(&profile, &chord(None), true, false, None).is_err());
    let previous = PreviousStandardBassButtons {
        bass_button_id: "missing".into(),
        chord_button_id: "G7".into(),
    };
    assert!(
        located_standard_bass_candidates(&fixture(), &chord(None), true, false, Some(&previous))
            .is_err()
    );
}
