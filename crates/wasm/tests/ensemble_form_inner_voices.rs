#![cfg(not(all(
    feature = "playback-only",
    not(any(feature = "full", feature = "viewer"))
)))]

#[path = "../../../tests/fixtures/arrangement/form_development_v2.rs"]
mod fixture;

#[cfg(target_arch = "wasm32")]
wasm_bindgen_test::wasm_bindgen_test_configure!(run_in_browser);

#[cfg_attr(target_arch = "wasm32", wasm_bindgen_test::wasm_bindgen_test)]
#[cfg_attr(not(target_arch = "wasm32"), test)]
fn form_inner_voice_json_matches_native_and_replays() {
    let source = fixture::source();
    let request = fixture::form_inner_voice_request();
    let native = acorde_core::arrange_form_inner_voices(&source, &request).unwrap();
    let source_json = serde_json::to_string(&source).unwrap();
    let browser: acorde_core::FormInnerVoicesResult = serde_json::from_str(
        &acorde_wasm::arrange_form_inner_voices(
            &source_json,
            &serde_json::to_string(&request).unwrap(),
        )
        .unwrap(),
    )
    .unwrap();
    assert_eq!(native.measures, browser.measures);
    assert_eq!(
        fixture::events(&native.score),
        fixture::events(&browser.score)
    );
    let replay: acorde_core::FormInnerVoicesResult = serde_json::from_str(
        &acorde_wasm::replay_form_inner_voices(
            &source_json,
            &serde_json::to_string(&browser.resolved).unwrap(),
        )
        .unwrap(),
    )
    .unwrap();
    assert_eq!(
        fixture::events(&replay.score),
        fixture::events(&browser.score)
    );
}
