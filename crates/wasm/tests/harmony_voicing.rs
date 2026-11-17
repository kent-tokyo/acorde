#![cfg(not(all(
    feature = "playback-only",
    not(any(feature = "full", feature = "viewer"))
)))]

#[cfg(target_arch = "wasm32")]
wasm_bindgen_test::wasm_bindgen_test_configure!(run_in_browser);

const REQUEST: &str = include_str!("../../../tests/fixtures/arrangement/harmony-voicing.json");

#[cfg_attr(target_arch = "wasm32", wasm_bindgen_test::wasm_bindgen_test)]
#[cfg_attr(not(target_arch = "wasm32"), test)]
fn voice_identity_root_policy_and_repeat_edges_match_native() {
    let request: acorde_core::HarmonyVoicingRequest = serde_json::from_str(REQUEST).unwrap();
    let native = acorde_core::plan_harmony_voicings(&request).unwrap();
    let web: acorde_core::HarmonyVoicingResult =
        serde_json::from_str(&acorde_wasm::plan_harmony_voicings(REQUEST).unwrap()).unwrap();
    assert_eq!(web, native);
    assert_eq!(web.voicings.len(), 3);
    assert_eq!(web.transitions.len(), 3);
    assert!(web.transitions[2].edge.repeat_boundary);
    let mut letters: Vec<_> = web.voicings[0]
        .tones
        .iter()
        .map(|tone| tone.pitch.step.to_char())
        .collect();
    letters.sort_unstable();
    assert_eq!(letters, ['A', 'C', 'E', 'F']);
    assert_eq!(
        web.voicings[0].root_decision,
        acorde_core::RootVoicingDecision::OmittedWithBassSupport
    );
    assert!(web.transitions.iter().all(|edge| edge.voices.len() == 4));
}

#[cfg(target_arch = "wasm32")]
#[wasm_bindgen_test::wasm_bindgen_test]
fn invalid_edges_and_excessive_harmony_graphs_fail_atomically() {
    let mut request: acorde_core::HarmonyVoicingRequest = serde_json::from_str(REQUEST).unwrap();
    request.edges[0].to = 3;
    assert!(acorde_wasm::plan_harmony_voicings(&serde_json::to_string(&request).unwrap()).is_err());
    request.edges.clear();
    request.nodes = vec![request.nodes[0].clone(); 65];
    assert!(acorde_wasm::plan_harmony_voicings(&serde_json::to_string(&request).unwrap()).is_err());
}
