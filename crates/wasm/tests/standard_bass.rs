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

#[cfg_attr(target_arch = "wasm32", wasm_bindgen_test::wasm_bindgen_test)]
#[cfg_attr(not(target_arch = "wasm32"), test)]
fn located_candidates_return_real_rows_and_seventh_omission() {
    let profile = serde_json::json!({"profile":{"bass_buttons":[{"id":"G","position":0,"midi":43}],"chord_buttons":[{"id":"G7","position":0,"midi":[67,71,77]}],"max_distance":0},"locations":[{"id":"G","row":"fundamental","column":4},{"id":"G7","row":"seventh","column":4}]});
    let chord = serde_json::json!({"root":"G","kind":"dominant"});
    let result: serde_json::Value = serde_json::from_str(
        &acorde_wasm::located_standard_bass_candidates(
            &profile.to_string(),
            &chord.to_string(),
            "null",
            true,
            false,
        )
        .unwrap(),
    )
    .unwrap();
    assert_eq!(result[0]["direct_seventh"], true);
    assert_eq!(result[0]["chord_location"]["row"], "seventh");
    assert_eq!(
        result[0]["candidate"]["omitted_pitch_classes"],
        serde_json::json!([2])
    );
}

#[cfg_attr(target_arch = "wasm32", wasm_bindgen_test::wasm_bindgen_test)]
#[cfg_attr(not(target_arch = "wasm32"), test)]
fn located_arrangement_exports_actual_seventh_button_sound() {
    use acorde_core::model::standard_bass_arrangement::StandardBassArrangementResult;
    use acorde_core::{Score, to_playback_events};
    let source = acorde_wasm::parse_abc("X:1\nM:4/4\nL:1/4\nK:C\nC D E F|]\n").unwrap();
    let profile = serde_json::json!({"bass_buttons":[{"id":"G","position":0,"midi":43}],"chord_buttons":[{"id":"G7","position":0,"midi":[67,71,77]},{"id":"Bdim","position":0,"midi":[71,74,77]}],"max_distance":0});
    let instrument = serde_json::json!({"profile":profile,"locations":[{"id":"G","row":"fundamental","column":1},{"id":"G7","row":"seventh","column":1},{"id":"Bdim","row":"diminished","column":2}]});
    let options = serde_json::json!({"melody_part":0,"melody_staff":0,"melody_voice":0,"allow_source_omissions":false,"chords":[{"measure_index":0,"offset_beats":0,"chord":{"root":"G","kind":"dominant"}}],"profile":profile,"allow_chord_omissions":true,"allow_chord_additions":false});
    let result: StandardBassArrangementResult = serde_json::from_str(
        &acorde_wasm::arrange_standard_bass_located(
            &source,
            &options.to_string(),
            &instrument.to_string(),
        )
        .unwrap(),
    )
    .unwrap();
    assert!(result.diagnostics.is_empty(), "{:?}", result.diagnostics);
    assert_eq!(result.actions[0].candidate.chord_button_id, "G7");
    assert_eq!(result.actions[0].candidate.omitted_pitch_classes, [2]);
    let policy = serde_json::json!({"max_position_delta":0,"min_attack_interval_ms":0});
    let screened: serde_json::Value = serde_json::from_str(
        &acorde_wasm::arrange_standard_bass_located_screened(
            &source,
            &options.to_string(),
            &instrument.to_string(),
            &policy.to_string(),
        )
        .unwrap(),
    )
    .unwrap();
    assert_eq!(screened["transitions"][0]["chord_button_id"], "G7");
    let score = result.score.unwrap();
    let signature = |s: &Score| {
        to_playback_events(s, &Default::default())
            .into_iter()
            .map(|e| (e.pitch_midi, e.time_beats, e.duration_beats))
            .collect::<Vec<_>>()
    };
    let archive = acorde_wasm::serialize_mscz(&serde_json::to_string(&score).unwrap()).unwrap();
    let restored: Score =
        serde_json::from_str(&acorde_wasm::parse_mscz(&archive).unwrap()).unwrap();
    assert_eq!(signature(&score), signature(&restored));
}

#[cfg_attr(target_arch = "wasm32", wasm_bindgen_test::wasm_bindgen_test)]
#[cfg_attr(not(target_arch = "wasm32"), test)]
fn selected_melody_uses_located_buttons_and_survives_mscz() {
    use acorde_core::{Score, to_playback_events};
    let source = acorde_wasm::parse_abc("X:1\nM:4/4\nL:1/4\nK:C\n[CE]4|]\n").unwrap();
    let profile = serde_json::json!({"bass_buttons":[{"id":"G","position":0,"midi":43}],"chord_buttons":[{"id":"G7","position":0,"midi":[67,71,77]}],"max_distance":0});
    let instrument = serde_json::json!({"profile":profile,"locations":[{"id":"G","row":"fundamental","column":1},{"id":"G7","row":"seventh","column":1}]});
    let options = serde_json::json!({"melody_part":0,"melody_staff":0,"melody_voice":0,"allow_source_omissions":true,"chords":[{"measure_index":0,"offset_beats":0,"chord":{"root":"G","kind":"dominant"}}],"profile":profile,"allow_chord_omissions":true,"allow_chord_additions":false});
    let selection = serde_json::json!([{"source":{"part":0,"staff":0,"measure":0,"voice":0,"note":0},"pitch_index":1}]);
    let policy = serde_json::json!({"max_position_delta":0,"min_attack_interval_ms":0});
    let result: serde_json::Value = serde_json::from_str(
        &acorde_wasm::arrange_standard_bass_located_selected_screened(
            &source,
            &options.to_string(),
            &selection.to_string(),
            &instrument.to_string(),
            &policy.to_string(),
        )
        .unwrap(),
    )
    .unwrap();
    assert_eq!(result["transitions"][0]["chord_button_id"], "G7");
    let mappings = result["arrangement"]["arrangement"]["source_pitches"]
        .as_array()
        .unwrap();
    assert!(
        mappings
            .iter()
            .any(|p| p["source_pitch"] == 0 && p["target"].is_null())
    );
    let score: Score =
        serde_json::from_value(result["arrangement"]["arrangement"]["score"].clone()).unwrap();
    let archive = acorde_wasm::serialize_mscz(&serde_json::to_string(&score).unwrap()).unwrap();
    let decoded: Score = serde_json::from_str(&acorde_wasm::parse_mscz(&archive).unwrap()).unwrap();
    let fingerprint = |s: &Score| {
        to_playback_events(s, &Default::default())
            .into_iter()
            .map(|e| (e.pitch_midi, e.time_beats, e.duration_beats))
            .collect::<Vec<_>>()
    };
    assert_eq!(fingerprint(&score), fingerprint(&decoded));
}

#[cfg_attr(target_arch = "wasm32", wasm_bindgen_test::wasm_bindgen_test)]
#[cfg_attr(not(target_arch = "wasm32"), test)]
fn located_path_search_preserves_selected_melody_through_repeat_and_mscz() {
    use acorde_core::{Score, to_playback_events};
    let source =
        acorde_wasm::parse_abc("X:1\nM:4/4\nL:1/4\nQ:1/4=120\nK:C\n|:[Cc]4|d4:|\n").unwrap();
    let profile = serde_json::json!({"bass_buttons":[{"id":"C","position":0,"midi":36},{"id":"G far","position":5,"midi":43},{"id":"G near","position":1,"midi":43}],"chord_buttons":[{"id":"C major","position":0,"midi":[60,64,67]},{"id":"G far major","position":5,"midi":[67,71,74]},{"id":"G near major","position":1,"midi":[67,71,74]}],"max_distance":0});
    let instrument = serde_json::json!({"profile":profile,"locations":[{"id":"C","row":"fundamental","column":0},{"id":"G far","row":"fundamental","column":1},{"id":"G near","row":"fundamental","column":8},{"id":"C major","row":"major","column":0},{"id":"G far major","row":"major","column":1},{"id":"G near major","row":"major","column":8}]});
    let options = serde_json::json!({"melody_part":0,"melody_staff":0,"melody_voice":0,"allow_source_omissions":true,"chords":[{"measure_index":0,"offset_beats":0,"chord":{"root":"C","kind":"major"}},{"measure_index":1,"offset_beats":0,"chord":{"root":"G","kind":"major"}}],"profile":profile,"allow_chord_omissions":false,"allow_chord_additions":false});
    let selection = serde_json::json!([{"source":{"part":0,"staff":0,"measure":0,"voice":0,"note":0},"pitch_index":1}]);
    let policy = serde_json::json!({"max_position_delta":1,"min_attack_interval_ms":2000});
    let result: serde_json::Value = serde_json::from_str(
        &acorde_wasm::find_located_standard_bass_path(
            &source,
            &options.to_string(),
            &selection.to_string(),
            &instrument.to_string(),
            &policy.to_string(),
            false,
        )
        .unwrap(),
    )
    .unwrap();
    let actions = result["transitions"].as_array().unwrap();
    assert_eq!(
        actions
            .iter()
            .map(|t| t["bass_button_id"].as_str().unwrap())
            .collect::<Vec<_>>(),
        ["C", "G near", "C", "G near"]
    );
    let score: Score =
        serde_json::from_value(result["arrangement"]["arrangement"]["score"].clone()).unwrap();
    let archive = acorde_wasm::serialize_mscz(&serde_json::to_string(&score).unwrap()).unwrap();
    let decoded: Score = serde_json::from_str(&acorde_wasm::parse_mscz(&archive).unwrap()).unwrap();
    let fingerprint = |s: &Score| {
        to_playback_events(s, &Default::default())
            .into_iter()
            .map(|e| (e.pitch_midi, e.time_beats, e.duration_beats))
            .collect::<Vec<_>>()
    };
    assert_eq!(fingerprint(&score), fingerprint(&decoded));
}
