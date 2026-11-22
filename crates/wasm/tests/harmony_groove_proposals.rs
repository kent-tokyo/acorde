#![cfg(not(all(
    feature = "playback-only",
    not(any(feature = "full", feature = "viewer"))
)))]
#[path = "../../../tests/fixtures/arrangement/harmony_groove_source.rs"]
mod fixture;

#[cfg_attr(target_arch = "wasm32", wasm_bindgen_test::wasm_bindgen_test)]
#[cfg_attr(not(target_arch = "wasm32"), test)]
fn standard_bass_button_screen_matches_native_and_rechecks_source() {
    use acorde_core::model::standard_bass::{
        StandardBassChordButton, StandardBassNoteButton, StandardBassProfile,
    };
    let source = fixture::source();
    let candidate = acorde_core::suggest_harmony_groove_arrangements(&source, &fixture::request())
        .unwrap()
        .candidates
        .remove(0);
    let profile = StandardBassProfile {
        bass_buttons: vec![StandardBassNoteButton {
            id: "D counterbass".into(),
            position: 0,
            midi: 38,
        }],
        chord_buttons: vec![StandardBassChordButton {
            id: "D minor".into(),
            position: 0,
            midi: vec![62, 65, 69],
        }],
        max_distance: 2,
    };
    let native =
        acorde_core::screen_harmony_groove_standard_bass(&source, &candidate.resolved, &profile)
            .unwrap();
    let source_json = serde_json::to_string(&source).unwrap();
    let resolved_json = serde_json::to_string(&candidate.resolved).unwrap();
    let profile_json = serde_json::to_string(&profile).unwrap();
    let wasm: acorde_core::HarmonyGrooveStandardBassReport = serde_json::from_str(
        &acorde_wasm::screen_harmony_groove_standard_bass(
            &source_json,
            &resolved_json,
            &profile_json,
        )
        .unwrap(),
    )
    .unwrap();
    assert_eq!(wasm, native);
    #[cfg(target_arch = "wasm32")]
    {
        let mut changed = source;
        changed.parts[0].staves[0].measures[0].voices[0][0].pitches[0].alter = 1;
        assert!(
            acorde_wasm::screen_harmony_groove_standard_bass(
                &serde_json::to_string(&changed).unwrap(),
                &resolved_json,
                &profile_json
            )
            .is_err()
        );
    }
}
#[cfg(target_arch = "wasm32")]
wasm_bindgen_test::wasm_bindgen_test_configure!(run_in_browser);

#[cfg_attr(target_arch = "wasm32", wasm_bindgen_test::wasm_bindgen_test)]
#[cfg_attr(not(target_arch = "wasm32"), test)]
fn groove_profiles_diagnostics_and_replay_match_native() {
    let source = fixture::grace_source();
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

#[cfg_attr(target_arch = "wasm32", wasm_bindgen_test::wasm_bindgen_test)]
#[cfg_attr(not(target_arch = "wasm32"), test)]
fn source_provenance_that_fits_the_request_can_replay_a_larger_resolved_contract() {
    let source = fixture::source();
    let mut request = fixture::request();
    for entry in &mut request.base.chords.entries {
        entry.sources = vec![acorde_core::ChordPlanSource {
            note: None,
            reference: Some("source-ref".repeat(2_000)),
        }];
    }
    let source_json = serde_json::to_string(&source).unwrap();
    let request_json = serde_json::to_string(&request).unwrap();
    assert!(request_json.len() < 64 * 1024);
    let result: acorde_core::HarmonyGrooveResult = serde_json::from_str(
        &acorde_wasm::suggest_harmony_groove_arrangements(&source_json, &request_json).unwrap(),
    )
    .unwrap();
    assert!(result.complete);
    for candidate in result.candidates {
        let resolved = serde_json::to_string(&candidate.resolved).unwrap();
        assert!(resolved.len() > 64 * 1024);
        let replay: acorde_core::HarmonyGrooveCandidate = serde_json::from_str(
            &acorde_wasm::replay_harmony_groove_arrangement(&source_json, &resolved).unwrap(),
        )
        .unwrap();
        assert_eq!(candidate.resolved, replay.resolved);
        assert_eq!(
            fixture::sound(&candidate.arrangement.score),
            fixture::sound(&replay.arrangement.score)
        );
        let direct = serde_json::to_string(&candidate.arrangement.resolved).unwrap();
        acorde_wasm::replay_harmony_arrangement(&source_json, &direct).unwrap();
    }
    for entry in &mut request.base.chords.entries {
        if let Some(reference) = &mut entry.sources[0].reference {
            reference.push_str(&"x".repeat(3_000));
        }
    }
    let direct_request = serde_json::to_string(&request.base).unwrap();
    assert!(direct_request.len() > 64 * 1024);
    let direct: acorde_core::HarmonyArrangementResult = serde_json::from_str(
        &acorde_wasm::arrange_harmony_voicings(&source_json, &direct_request).unwrap(),
    )
    .unwrap();
    let direct_resolved = serde_json::to_string(&direct.resolved).unwrap();
    assert!(direct_resolved.len() > 64 * 1024);
    let replay: acorde_core::HarmonyArrangementResult = serde_json::from_str(
        &acorde_wasm::replay_harmony_arrangement(&source_json, &direct_resolved).unwrap(),
    )
    .unwrap();
    assert_eq!(fixture::sound(&direct.score), fixture::sound(&replay.score));
}

#[cfg(target_arch = "wasm32")]
#[wasm_bindgen_test::wasm_bindgen_test]
fn larger_harmony_limits_still_reject_oversized_input() {
    let source_json = serde_json::to_string(&fixture::source()).unwrap();
    assert!(
        acorde_wasm::suggest_harmony_groove_arrangements(
            &source_json,
            &" ".repeat(2 * 1024 * 1024 + 1)
        )
        .is_err()
    );
    assert!(
        acorde_wasm::replay_harmony_groove_arrangement(
            &source_json,
            &" ".repeat(8 * 1024 * 1024 + 1)
        )
        .is_err()
    );
    assert!(
        acorde_wasm::arrange_harmony_voicings(&source_json, &" ".repeat(2 * 1024 * 1024 + 1))
            .is_err()
    );
    assert!(
        acorde_wasm::replay_harmony_arrangement(&source_json, &" ".repeat(8 * 1024 * 1024 + 1))
            .is_err()
    );
}
