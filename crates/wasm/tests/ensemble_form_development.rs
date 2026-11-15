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
fn developed_form_matches_native_json_and_replays() {
    let source = fixture::source();
    let request = acorde_core::EnsembleFormDevelopmentRequest {
        form: fixture::request(),
        countermelody_kind: acorde_core::CountermelodyDevelopmentKind::Imitation,
    };
    let native = acorde_core::arrange_ensemble_form_development(&source, &request).unwrap();
    let source_json = serde_json::to_string(&source).unwrap();
    let browser: acorde_core::EnsembleFormDevelopmentResult = serde_json::from_str(
        &acorde_wasm::arrange_ensemble_form_development(
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
    let replay: acorde_core::Score = serde_json::from_str(
        &acorde_wasm::replay_ensemble_form_development(
            &source_json,
            &serde_json::to_string(&browser.resolved).unwrap(),
        )
        .unwrap(),
    )
    .unwrap();
    assert_eq!(fixture::events(&replay), fixture::events(&browser.score));

    let native_led =
        acorde_core::arrange_ensemble_form_development_with_phrase_joins(&source, &request, 0)
            .unwrap();
    let browser_led: acorde_core::EnsembleFormDevelopmentResult = serde_json::from_str(
        &acorde_wasm::arrange_ensemble_form_development_with_phrase_joins(
            &source_json,
            &serde_json::to_string(&request).unwrap(),
            0,
        )
        .unwrap(),
    )
    .unwrap();
    assert_eq!(
        browser_led.resolved.phrase_entry_max_leap_semitones,
        Some(0)
    );
    assert_eq!(
        fixture::events(&native_led.score),
        fixture::events(&browser_led.score)
    );
    let led_replay: acorde_core::Score = serde_json::from_str(
        &acorde_wasm::replay_ensemble_form_development(
            &source_json,
            &serde_json::to_string(&browser_led.resolved).unwrap(),
        )
        .unwrap(),
    )
    .unwrap();
    assert_eq!(
        fixture::events(&led_replay),
        fixture::events(&browser_led.score)
    );
}
