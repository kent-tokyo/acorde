#![cfg(not(all(
    feature = "playback-only",
    not(any(feature = "full", feature = "viewer"))
)))]
#[path = "../../../tests/fixtures/arrangement/harmony_arrangement_source.rs"]
mod fixture;

#[cfg(target_arch = "wasm32")]
wasm_bindgen_test::wasm_bindgen_test_configure!(run_in_browser);

#[cfg_attr(target_arch = "wasm32", wasm_bindgen_test::wasm_bindgen_test)]
#[cfg_attr(not(target_arch = "wasm32"), test)]
fn score_context_notes_and_replay_match_native() {
    let source = fixture::source();
    let request = fixture::request();
    let native = acorde_core::arrange_harmony_voicings(&source, &request).unwrap();
    let source_json = serde_json::to_string(&source).unwrap();
    let web: acorde_core::HarmonyArrangementResult = serde_json::from_str(
        &acorde_wasm::arrange_harmony_voicings(
            &source_json,
            &serde_json::to_string(&request).unwrap(),
        )
        .unwrap(),
    )
    .unwrap();
    assert_eq!(web.context, native.context);
    assert_eq!(web.voicings, native.voicings);
    assert_eq!(web.spans, native.spans);
    assert_eq!(web.generated_notes, native.generated_notes);
    assert_eq!(web.resolved, native.resolved);
    assert_eq!(fixture::sound(&web.score), fixture::sound(&native.score));
    let replay: acorde_core::HarmonyArrangementResult = serde_json::from_str(
        &acorde_wasm::replay_harmony_arrangement(
            &source_json,
            &serde_json::to_string(&web.resolved).unwrap(),
        )
        .unwrap(),
    )
    .unwrap();
    assert_eq!(fixture::sound(&replay.score), fixture::sound(&web.score));
}

#[cfg_attr(target_arch = "wasm32", wasm_bindgen_test::wasm_bindgen_test)]
#[cfg_attr(not(target_arch = "wasm32"), test)]
fn anticipated_harmony_uses_the_same_actual_intervals_and_ties_as_native() {
    let source = fixture::source();
    let mut request = fixture::request();
    request.comping.pattern = acorde_core::HarmonyCompingPattern::OffbeatPulse;
    request.comping.accent = acorde_core::HarmonyCompingAccent::Anticipations;
    request.comping.anticipations = vec![acorde_core::HarmonyAnticipation {
        chord_entry: 1,
        lead: acorde_core::HarmonyAnticipationLead::Eighth,
        join: acorde_core::HarmonyAnticipationJoin::Tie,
    }];
    let native = acorde_core::arrange_harmony_voicings(&source, &request).unwrap();
    let source_json = serde_json::to_string(&source).unwrap();
    let web: acorde_core::HarmonyArrangementResult = serde_json::from_str(
        &acorde_wasm::arrange_harmony_voicings(
            &source_json,
            &serde_json::to_string(&request).unwrap(),
        )
        .unwrap(),
    )
    .unwrap();
    assert_eq!(web.context, native.context);
    assert_eq!(web.spans, native.spans);
    assert_eq!(fixture::sound(&web.score), fixture::sound(&native.score));
    assert!(web.spans.iter().any(|span| span.tie_to_next));
    let replay: acorde_core::HarmonyArrangementResult = serde_json::from_str(
        &acorde_wasm::replay_harmony_arrangement(
            &source_json,
            &serde_json::to_string(&web.resolved).unwrap(),
        )
        .unwrap(),
    )
    .unwrap();
    assert_eq!(fixture::sound(&replay.score), fixture::sound(&native.score));
}

#[cfg(target_arch = "wasm32")]
#[wasm_bindgen_test::wasm_bindgen_test]
fn malformed_requests_and_changed_source_fail_atomically() {
    let source = fixture::source();
    let source_json = serde_json::to_string(&source).unwrap();
    assert!(acorde_wasm::arrange_harmony_voicings(&source_json, "{}").is_err());
    let native = acorde_core::arrange_harmony_voicings(&source, &fixture::request()).unwrap();
    let mut changed = source;
    changed.parts[0].staves[0].measures[0].voices[0][0].pitches[0].alter = 1;
    assert!(
        acorde_wasm::replay_harmony_arrangement(
            &serde_json::to_string(&changed).unwrap(),
            &serde_json::to_string(&native.resolved).unwrap()
        )
        .is_err()
    );
}
