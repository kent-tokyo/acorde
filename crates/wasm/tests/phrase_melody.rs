#![cfg(not(all(
    feature = "playback-only",
    not(any(feature = "full", feature = "viewer"))
)))]

#[path = "../../../tests/fixtures/arrangement/countermelody_development.rs"]
mod fixture;

#[cfg(target_arch = "wasm32")]
wasm_bindgen_test::wasm_bindgen_test_configure!(run_in_browser);

#[cfg_attr(target_arch = "wasm32", wasm_bindgen_test::wasm_bindgen_test)]
#[cfg_attr(not(target_arch = "wasm32"), test)]
fn phrase_melody_matches_native_and_replays() {
    use acorde_core::{MelodyAnticipationEdit, NoteAddr, PhraseMelodyRequest, PhraseMelodyResult};
    let source = fixture::source();
    let score_json = serde_json::to_string(&source).unwrap();
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
    let native = acorde_core::arrange_phrase_melody(&source, &request).unwrap();
    let browser: PhraseMelodyResult = serde_json::from_str(
        &acorde_wasm::arrange_phrase_melody(&score_json, &serde_json::to_string(&request).unwrap())
            .unwrap(),
    )
    .unwrap();
    assert_eq!(browser.resolved, native.resolved);
    assert_eq!(
        fixture::events(&browser.score, 0),
        fixture::events(&native.score, 0)
    );
    let replay: PhraseMelodyResult = serde_json::from_str(
        &acorde_wasm::replay_phrase_melody(
            &score_json,
            &serde_json::to_string(&browser.resolved).unwrap(),
        )
        .unwrap(),
    )
    .unwrap();
    assert_eq!(replay.resolved, browser.resolved);
    assert_eq!(
        fixture::events(&replay.score, 0),
        fixture::events(&native.score, 0)
    );
}
