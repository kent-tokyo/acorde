#![cfg(not(all(
    feature = "playback-only",
    not(any(feature = "full", feature = "viewer"))
)))]

#[cfg(target_arch = "wasm32")]
wasm_bindgen_test::wasm_bindgen_test_configure!(run_in_browser);

#[cfg_attr(target_arch = "wasm32", wasm_bindgen_test::wasm_bindgen_test)]
#[cfg_attr(not(target_arch = "wasm32"), test)]
fn expansion_json_matches_native_and_pinned_pitches() {
    let corpus: serde_json::Value = serde_json::from_str(include_str!(
        "../../../tests/fixtures/arrangement/harmony-expansion.json"
    ))
    .unwrap();
    for case in corpus["cases"].as_array().unwrap() {
        let chord: acorde_core::ChordSymbol =
            serde_json::from_value(case["chord"].clone()).unwrap();
        let native = acorde_core::expand_harmony(&chord).unwrap();
        let web: acorde_core::ExpandedHarmony =
            serde_json::from_str(&acorde_wasm::expand_harmony(&case["chord"].to_string()).unwrap())
                .unwrap();
        assert_eq!(web, native);
        assert_eq!(
            serde_json::to_value(
                web.tones
                    .iter()
                    .map(|tone| tone.pitch_at_root_octave(3).unwrap().to_midi())
                    .collect::<Vec<_>>()
            )
            .unwrap(),
            case["midi"]
        );
    }
}

#[cfg(target_arch = "wasm32")]
#[wasm_bindgen_test::wasm_bindgen_test]
fn invalid_harmony_and_unbounded_json_are_rejected() {
    for invalid in [
        "",
        "{}",
        r#"{"root":"C","kind":"unknown"}"#,
        r#"{"root":"C","kind":"major","degrees":[{"value":0,"alter":0,"kind":"add"}]}"#,
    ] {
        assert!(acorde_wasm::expand_harmony(invalid).is_err());
    }
    assert!(acorde_wasm::expand_harmony(&" ".repeat(64 * 1024 * 1024)).is_err());
}
