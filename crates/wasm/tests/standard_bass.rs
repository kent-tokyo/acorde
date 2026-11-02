#![cfg(not(all(
    feature = "playback-only",
    not(any(feature = "full", feature = "viewer"))
)))]

#[cfg(target_arch = "wasm32")]
wasm_bindgen_test::wasm_bindgen_test_configure!(run_in_browser);

#[cfg_attr(target_arch = "wasm32", wasm_bindgen_test::wasm_bindgen_test)]
#[cfg_attr(not(target_arch = "wasm32"), test)]
fn chord_candidates_keep_profile_bass_and_actual_sounding_pitches() {
    use acorde_core::model::standard_bass::StandardBassCandidate;
    let profile = r#"{"bass_buttons":[{"id":"C","position":0,"midi":36},{"id":"F","position":1,"midi":41}],"chord_buttons":[{"id":"Em","position":1,"midi":[64,67,71]},{"id":"G","position":1,"midi":[67,71,74]}],"max_distance":1}"#;
    for (chord, bass, button, pitches) in [
        (
            r#"{"root":"C","kind":"major-seventh"}"#,
            36,
            "Em",
            &[64, 67, 71][..],
        ),
        (
            r#"{"root":"G","kind":"dominant","bass":"F"}"#,
            41,
            "G",
            &[67, 71, 74][..],
        ),
    ] {
        let json =
            acorde_wasm::standard_bass_chord_candidates(profile, chord, false, false).unwrap();
        let result: Vec<StandardBassCandidate> = serde_json::from_str(&json).unwrap();
        assert_eq!(result.len(), 1);
        assert_eq!(result[0].bass_midi, bass);
        assert_eq!(result[0].chord_button_id, button);
        assert_eq!(result[0].chord_midi, pitches);
        assert!(result[0].omitted_pitch_classes.is_empty());
        assert!(result[0].added_pitch_classes.is_empty());
        assert!(result[0].requires_performer_review);
    }
}
