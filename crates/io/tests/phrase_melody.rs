#![cfg(feature = "musicxml")]

#[path = "../../../tests/fixtures/arrangement/countermelody_development.rs"]
mod fixture;

fn arranged() -> acorde_core::PhraseMelodyResult {
    use acorde_core::{MelodyAnticipationEdit, NoteAddr, PhraseMelodyRequest};
    let request = PhraseMelodyRequest {
        countermelody: fixture::request(acorde_core::CountermelodyDevelopmentKind::Imitation),
        countermelody_active_measures: vec![true, false, true, false],
        phrase_entry_max_leap_semitones: Some(12),
        start_measure: 0,
        end_measure_exclusive: 1,
        locked_measures: vec![1, 3],
        anticipations: vec![MelodyAnticipationEdit {
            note: NoteAddr {
                part: 0,
                staff: 0,
                measure: 0,
                voice: 0,
                note: 2,
            },
            preceding_rest: NoteAddr {
                part: 0,
                staff: 0,
                measure: 0,
                voice: 0,
                note: 1,
            },
            early_ticks: 8,
        }],
    };
    acorde_core::arrange_phrase_melody(&fixture::source(), &request).unwrap()
}

#[test]
fn anticipated_melody_survives_musicxml() {
    let result = arranged();
    let reparsed =
        acorde_io::parse_musicxml(&acorde_io::serialize_musicxml(&result.score).unwrap()).unwrap();
    assert_eq!(
        fixture::events(&result.score, 0),
        fixture::events(&reparsed, 0)
    );
}

#[cfg(feature = "mscz")]
#[test]
fn anticipated_melody_survives_mscz() {
    let result = arranged();
    let reparsed =
        acorde_io::parse_mscz(&acorde_io::serialize_mscz(&result.score).unwrap()).unwrap();
    assert_eq!(
        fixture::events(&result.score, 0),
        fixture::events(&reparsed, 0)
    );
}
