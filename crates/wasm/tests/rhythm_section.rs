#![cfg(not(all(
    feature = "playback-only",
    not(any(feature = "full", feature = "viewer"))
)))]
#[path = "../../../tests/fixtures/arrangement/rhythm_section.rs"]
mod fixture;
#[cfg(target_arch = "wasm32")]
wasm_bindgen_test::wasm_bindgen_test_configure!(run_in_browser);
use acorde_core::*;
#[cfg_attr(target_arch = "wasm32", wasm_bindgen_test::wasm_bindgen_test)]
#[cfg_attr(not(target_arch = "wasm32"), test)]
fn shared_rhythm_matches_native_and_replays_both_ghost_routes() {
    let s = fixture::source();
    let json = serde_json::to_string(&s).unwrap();
    for muted in [false, true] {
        let r = fixture::request(muted);
        let native = arrange_rhythm_section(&s, &r).unwrap();
        let web: RhythmSectionResult = serde_json::from_str(
            &acorde_wasm::arrange_rhythm_section(&json, &serde_json::to_string(&r).unwrap())
                .unwrap(),
        )
        .unwrap();
        assert_eq!(native.resolved, web.resolved);
        assert_eq!(native.attacks, web.attacks);
        assert_eq!(native.diagnostics, web.diagnostics);
        assert_eq!(fixture::sound(&native.score), fixture::sound(&web.score));
        let replay: RhythmSectionResult = serde_json::from_str(
            &acorde_wasm::replay_rhythm_section(
                &json,
                &serde_json::to_string(&web.resolved).unwrap(),
            )
            .unwrap(),
        )
        .unwrap();
        assert_eq!(fixture::sound(&native.score), fixture::sound(&replay.score));
    }
}
#[cfg(target_arch = "wasm32")]
#[wasm_bindgen_test::wasm_bindgen_test]
fn malformed_oversize_locked_and_tampered_rhythm_requests_fail() {
    let s = fixture::source();
    let json = serde_json::to_string(&s).unwrap();
    assert!(acorde_wasm::arrange_rhythm_section(&json, "{}").is_err());
    assert!(acorde_wasm::arrange_rhythm_section(&json, &" ".repeat(2 * 1024 * 1024 + 1)).is_err());
    assert!(acorde_wasm::replay_rhythm_section(&json, &" ".repeat(8 * 1024 * 1024 + 1)).is_err());
    let mut r = fixture::request(false);
    r.locked_measures = vec![0];
    assert!(
        acorde_wasm::arrange_rhythm_section(&json, &serde_json::to_string(&r).unwrap()).is_err()
    );
    let mut saved = arrange_rhythm_section(&s, &fixture::request(false))
        .unwrap()
        .resolved;
    saved.bass[0].pitch.octave += 1;
    assert!(
        acorde_wasm::replay_rhythm_section(&json, &serde_json::to_string(&saved).unwrap()).is_err()
    );
}
