#![cfg(feature = "musicxml")]

#[path = "../../../tests/fixtures/arrangement/integrated_arrangement.rs"]
mod fixture;

fn sound(score: &acorde_core::Score) -> Vec<(usize, u64, u64, i32, u8)> {
    acorde_core::to_playback_events(score, &Default::default())
        .into_iter()
        .map(|event| {
            (
                event.part_index,
                (event.time_beats * 480.0).round() as u64,
                (event.duration_beats * 480.0).round() as u64,
                event.pitch_midi_cents,
                event.velocity,
            )
        })
        .collect()
}

#[test]
fn integrated_arrangements_survive_musicxml() {
    let source = fixture::source();
    let result =
        acorde_core::suggest_integrated_arrangements(&source, &fixture::request()).unwrap();
    for candidate in &result.candidates {
        let parsed =
            acorde_io::parse_musicxml(&acorde_io::serialize_musicxml(&candidate.score).unwrap())
                .unwrap();
        assert_eq!(sound(&parsed), sound(&candidate.score));
    }
}

#[cfg(feature = "mscz")]
#[test]
fn integrated_arrangements_survive_mscz() {
    let source = fixture::source();
    let result =
        acorde_core::suggest_integrated_arrangements(&source, &fixture::request()).unwrap();
    for candidate in &result.candidates {
        let parsed =
            acorde_io::parse_mscz(&acorde_io::serialize_mscz(&candidate.score).unwrap()).unwrap();
        assert_eq!(sound(&parsed), sound(&candidate.score));
    }
}
