#![cfg(not(all(
    feature = "playback-only",
    not(any(feature = "full", feature = "viewer"))
)))]

#[cfg(target_arch = "wasm32")]
wasm_bindgen_test::wasm_bindgen_test_configure!(run_in_browser);

#[cfg_attr(target_arch = "wasm32", wasm_bindgen_test::wasm_bindgen_test)]
#[cfg_attr(not(target_arch = "wasm32"), test)]
fn phrase_harmony_json_matches_native_and_rejects_invalid_schedule() {
    let score_json =
        acorde_wasm::parse_abc("X:1\nM:4/4\nL:1/4\nK:C\nC E G B | C E G B | C E G B | C E G B |\n")
            .unwrap();
    let request = acorde_core::PhraseHarmonyRequest {
        phrases: vec![
            acorde_core::PhraseHarmonySection {
                start_measure: 0,
                end_measure_exclusive: 2,
                pace: acorde_core::PhraseHarmonicPace::EverySecondBar,
                meter_groups: vec![],
            },
            acorde_core::PhraseHarmonySection {
                start_measure: 2,
                end_measure_exclusive: 4,
                pace: acorde_core::PhraseHarmonicPace::TwoGroupsPerBar,
                meter_groups: vec![],
            },
        ],
        proposal: Default::default(),
        locked_chord_entries: vec![],
    };
    let native =
        acorde_core::suggest_phrase_harmony(&serde_json::from_str(&score_json).unwrap(), &request);
    let wasm: acorde_core::PhraseHarmonyResult = serde_json::from_str(
        &acorde_wasm::suggest_phrase_harmony(
            &score_json,
            &serde_json::to_string(&request).unwrap(),
        )
        .unwrap(),
    )
    .unwrap();
    assert_eq!(
        serde_json::to_value(&native).unwrap(),
        serde_json::to_value(&wasm).unwrap()
    );
    assert!(wasm.diagnostics.is_empty(), "{:?}", wasm.diagnostics);
    assert_eq!(wasm.positions.len(), 5);
    assert_eq!(wasm.positions[2].offset.beats(), 2.0);
    assert!(!wasm.proposals.candidates.is_empty());
    let mut invalid = request;
    invalid.phrases[1].start_measure = 1;
    let rejected: acorde_core::PhraseHarmonyResult = serde_json::from_str(
        &acorde_wasm::suggest_phrase_harmony(
            &score_json,
            &serde_json::to_string(&invalid).unwrap(),
        )
        .unwrap(),
    )
    .unwrap();
    assert!(rejected.positions.is_empty());
    assert!(rejected.proposals.candidates.is_empty());
    assert_eq!(rejected.diagnostics[0].code, "invalid-harmonic-phrase");
}
