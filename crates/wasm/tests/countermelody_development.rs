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
fn source_rhythm_development_matches_native_json_and_replays() {
    let source = fixture::source();
    let source_json = serde_json::to_string(&source).unwrap();
    for kind in [
        acorde_core::CountermelodyDevelopmentKind::Imitation,
        acorde_core::CountermelodyDevelopmentKind::Inversion,
        acorde_core::CountermelodyDevelopmentKind::Augmentation,
        acorde_core::CountermelodyDevelopmentKind::Diminution,
        acorde_core::CountermelodyDevelopmentKind::Answer,
    ] {
        let request = fixture::request(kind);
        let native = acorde_core::arrange_countermelody_development(&source, &request).unwrap();
        let browser: acorde_core::CountermelodyDevelopmentResult = serde_json::from_str(
            &acorde_wasm::arrange_countermelody_development(
                &source_json,
                &serde_json::to_string(&request).unwrap(),
            )
            .unwrap(),
        )
        .unwrap();
        assert_eq!(browser.source_rhythm, native.source_rhythm);
        assert_eq!(browser.transformed_rhythm, native.transformed_rhythm);
        assert_eq!(
            fixture::events(&browser.score, 0),
            fixture::events(&native.score, 0)
        );
        let replay: acorde_core::Score = serde_json::from_str(
            &acorde_wasm::replay_countermelody_development(
                &source_json,
                &serde_json::to_string(&browser.resolved).unwrap(),
            )
            .unwrap(),
        )
        .unwrap();
        assert_eq!(
            fixture::events(&replay, 0),
            fixture::events(&browser.score, 0)
        );
    }
}
