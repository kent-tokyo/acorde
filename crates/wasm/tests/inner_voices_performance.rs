#![cfg(not(all(
    feature = "playback-only",
    not(any(feature = "full", feature = "viewer"))
)))]

#[path = "../../../tests/fixtures/arrangement/inner_voice_repeat.rs"]
mod fixture;

#[cfg(target_arch = "wasm32")]
wasm_bindgen_test::wasm_bindgen_test_configure!(run_in_browser);

#[cfg_attr(target_arch = "wasm32", wasm_bindgen_test::wasm_bindgen_test)]
#[cfg_attr(not(target_arch = "wasm32"), test)]
fn repeat_visit_inner_voices_match_native_json_and_replay() {
    let source = fixture::source();
    let request = fixture::request();
    let native = acorde_core::arrange_inner_voices_performance(&source, request.clone()).unwrap();
    let source_json = serde_json::to_string(&source).unwrap();
    let browser: acorde_core::InnerVoicesPerformanceResult = serde_json::from_str(
        &acorde_wasm::arrange_inner_voices_performance(
            &source_json,
            &serde_json::to_string(&request).unwrap(),
        )
        .unwrap(),
    )
    .unwrap();
    assert_eq!(
        native.written_measures_by_visit,
        browser.written_measures_by_visit
    );
    assert_eq!(native.repeat_back_joins, browser.repeat_back_joins);
    assert_eq!(
        fixture::events(&native.performance_score),
        fixture::events(&browser.performance_score)
    );
    let replay: acorde_core::InnerVoicesPerformanceResult = serde_json::from_str(
        &acorde_wasm::replay_inner_voices_performance(
            &source_json,
            &serde_json::to_string(&browser.resolved).unwrap(),
        )
        .unwrap(),
    )
    .unwrap();
    assert_eq!(
        fixture::events(&replay.performance_score),
        fixture::events(&browser.performance_score)
    );
}
