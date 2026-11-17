#![cfg(not(all(
    feature = "playback-only",
    not(any(feature = "full", feature = "viewer"))
)))]
#[path = "../../../tests/fixtures/arrangement/phrase_harmony_targets.rs"]
mod fixture;
#[cfg(target_arch = "wasm32")]
wasm_bindgen_test::wasm_bindgen_test_configure!(run_in_browser);
use acorde_core::*;

#[cfg_attr(target_arch = "wasm32", wasm_bindgen_test::wasm_bindgen_test)]
#[cfg_attr(not(target_arch = "wasm32"), test)]
fn target_choices_evidence_and_replay_match_native() {
    let source = fixture::source();
    let json = serde_json::to_string(&source).unwrap();
    for request in [
        fixture::request(),
        fixture::modulation(vec![ModulationApproach::CommonChord]),
        fixture::modulation(vec![ModulationApproach::Dominant]),
    ] {
        let native = suggest_phrase_harmony_targets(&source, &request).unwrap();
        let web: PhraseHarmonyTargetResult = serde_json::from_str(
            &acorde_wasm::suggest_phrase_harmony_targets(
                &json,
                &serde_json::to_string(&request).unwrap(),
            )
            .unwrap(),
        )
        .unwrap();
        assert!(native.complete && web.complete);
        assert_eq!(native.rejections, web.rejections);
        for (a, b) in native.candidates.iter().zip(&web.candidates) {
            assert_eq!(a.resolved, b.resolved);
            assert_eq!(a.resolutions, b.resolutions);
            assert_eq!(a.quality, b.quality);
            assert_eq!(
                fixture::sound(&a.arrangement.score),
                fixture::sound(&b.arrangement.score)
            );
            let replay: PhraseHarmonyTargetCandidate = serde_json::from_str(
                &acorde_wasm::replay_phrase_harmony_target(
                    &json,
                    &serde_json::to_string(&b.resolved).unwrap(),
                )
                .unwrap(),
            )
            .unwrap();
            assert_eq!(a.resolved, replay.resolved);
            assert_eq!(
                fixture::sound(&a.arrangement.score),
                fixture::sound(&replay.arrangement.score)
            );
        }
    }
}
#[cfg_attr(target_arch = "wasm32", wasm_bindgen_test::wasm_bindgen_test)]
#[cfg_attr(not(target_arch = "wasm32"), test)]
fn target_shortages_are_not_hidden_in_wasm() {
    let mut request = fixture::request();
    request.locked_chord_entries = vec![0];
    let result: PhraseHarmonyTargetResult = serde_json::from_str(
        &acorde_wasm::suggest_phrase_harmony_targets(
            &serde_json::to_string(&fixture::source()).unwrap(),
            &serde_json::to_string(&request).unwrap(),
        )
        .unwrap(),
    )
    .unwrap();
    assert!(!result.complete && result.candidates.is_empty());
    assert!(
        result
            .rejections
            .contains(&PhraseHarmonyTargetRejection::LockedChord { entry: 0 })
    );
}
#[cfg(target_arch = "wasm32")]
#[wasm_bindgen_test::wasm_bindgen_test]
fn malformed_target_requests_and_altered_replays_are_errors() {
    let source = fixture::source();
    let json = serde_json::to_string(&source).unwrap();
    assert!(acorde_wasm::suggest_phrase_harmony_targets(&json, "{}").is_err());
    assert!(
        acorde_wasm::suggest_phrase_harmony_targets(&json, &" ".repeat(2 * 1024 * 1024 + 1))
            .is_err()
    );
    assert!(
        acorde_wasm::replay_phrase_harmony_target(&json, &" ".repeat(8 * 1024 * 1024 + 1)).is_err()
    );
    let result = suggest_phrase_harmony_targets(&source, &fixture::request()).unwrap();
    let mut bad = result.candidates[0].resolved.clone();
    bad.request.max_changed_harmonies = 0;
    assert!(
        acorde_wasm::replay_phrase_harmony_target(&json, &serde_json::to_string(&bad).unwrap())
            .is_err()
    );
}
