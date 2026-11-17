#![cfg(not(all(
    feature = "playback-only",
    not(any(feature = "full", feature = "viewer"))
)))]

#[path = "../../../tests/fixtures/arrangement/integrated_arrangement.rs"]
mod fixture;

#[cfg(target_arch = "wasm32")]
wasm_bindgen_test::wasm_bindgen_test_configure!(run_in_browser);

#[cfg_attr(target_arch = "wasm32", wasm_bindgen_test::wasm_bindgen_test)]
#[cfg_attr(not(target_arch = "wasm32"), test)]
fn integrated_candidates_match_native_and_replay() {
    let source = fixture::source();
    let request = fixture::request();
    let native = acorde_core::suggest_integrated_arrangements(&source, &request).unwrap();
    let score_json = serde_json::to_string(&source).unwrap();
    let browser: acorde_core::IntegratedArrangementResult = serde_json::from_str(
        &acorde_wasm::suggest_integrated_arrangements(
            &score_json,
            &serde_json::to_string(&request).unwrap(),
        )
        .unwrap(),
    )
    .unwrap();
    assert_eq!(browser.candidates.len(), 3);
    for (expected, actual) in native.candidates.iter().zip(&browser.candidates) {
        assert_eq!(actual.resolved, expected.resolved);
        let events = |score: &acorde_core::Score| {
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
                .collect::<Vec<_>>()
        };
        assert_eq!(events(&actual.score), events(&expected.score));
        let replay: acorde_core::Score = serde_json::from_str(
            &acorde_wasm::replay_integrated_arrangement(
                &score_json,
                &serde_json::to_string(&actual.resolved).unwrap(),
            )
            .unwrap(),
        )
        .unwrap();
        assert_eq!(events(&replay), events(&actual.score));
    }
}
