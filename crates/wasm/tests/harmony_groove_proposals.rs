#![cfg(not(all(
    feature = "playback-only",
    not(any(feature = "full", feature = "viewer"))
)))]
#[path = "../../../tests/fixtures/arrangement/harmony_groove_source.rs"]
mod fixture;
#[cfg(target_arch = "wasm32")]
wasm_bindgen_test::wasm_bindgen_test_configure!(run_in_browser);

#[cfg_attr(target_arch = "wasm32", wasm_bindgen_test::wasm_bindgen_test)]
#[cfg_attr(not(target_arch = "wasm32"), test)]
fn groove_profiles_diagnostics_and_replay_match_native() {
    let source = fixture::source();
    let request = fixture::request();
    let source_json = serde_json::to_string(&source).unwrap();
    let native = acorde_core::suggest_harmony_groove_arrangements(&source, &request).unwrap();
    let web: acorde_core::HarmonyGrooveResult = serde_json::from_str(
        &acorde_wasm::suggest_harmony_groove_arrangements(
            &source_json,
            &serde_json::to_string(&request).unwrap(),
        )
        .unwrap(),
    )
    .unwrap();
    assert!(native.complete && web.complete);
    assert_eq!(native.rejections, web.rejections);
    assert_eq!(native.realizations, web.realizations);
    for (a, b) in native.candidates.iter().zip(&web.candidates) {
        assert_eq!(a.resolved, b.resolved);
        assert_eq!(a.quality, b.quality);
        assert_eq!(a.difference, b.difference);
        assert_eq!(
            fixture::sound(&a.arrangement.score),
            fixture::sound(&b.arrangement.score)
        );
        let replay: acorde_core::HarmonyGrooveCandidate = serde_json::from_str(
            &acorde_wasm::replay_harmony_groove_arrangement(
                &source_json,
                &serde_json::to_string(&b.resolved).unwrap(),
            )
            .unwrap(),
        )
        .unwrap();
        assert_eq!(
            fixture::sound(&replay.arrangement.score),
            fixture::sound(&a.arrangement.score)
        );
    }
}

#[cfg_attr(target_arch = "wasm32", wasm_bindgen_test::wasm_bindgen_test)]
#[cfg_attr(not(target_arch = "wasm32"), test)]
fn insufficient_choices_remain_explicit_in_wasm() {
    let mut request = fixture::request();
    request.tensions.clear();
    request.allow_rootless = false;
    request.allow_offbeats = false;
    request.anticipation = None;
    let web: acorde_core::HarmonyGrooveResult = serde_json::from_str(
        &acorde_wasm::suggest_harmony_groove_arrangements(
            &serde_json::to_string(&fixture::source()).unwrap(),
            &serde_json::to_string(&request).unwrap(),
        )
        .unwrap(),
    )
    .unwrap();
    assert!(!web.complete);
    assert_eq!(web.candidates.len(), 1);
    assert_eq!(web.missing_profiles.len(), 2);
}

#[cfg(target_arch = "wasm32")]
#[wasm_bindgen_test::wasm_bindgen_test]
fn malformed_groove_and_disallowed_replay_are_errors() {
    let source = fixture::source();
    let json = serde_json::to_string(&source).unwrap();
    assert!(acorde_wasm::suggest_harmony_groove_arrangements(&json, "{}").is_err());
    let result =
        acorde_core::suggest_harmony_groove_arrangements(&source, &fixture::request()).unwrap();
    let mut resolved = result.candidates[2].resolved.clone();
    resolved.request.allow_offbeats = false;
    resolved.request.anticipation = None;
    assert!(
        acorde_wasm::replay_harmony_groove_arrangement(
            &json,
            &serde_json::to_string(&resolved).unwrap()
        )
        .is_err()
    );
}
