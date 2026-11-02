#![cfg(not(all(
    feature = "playback-only",
    not(any(feature = "full", feature = "viewer"))
)))]
#[cfg(target_arch = "wasm32")]
wasm_bindgen_test::wasm_bindgen_test_configure!(run_in_browser);

#[cfg_attr(target_arch = "wasm32", wasm_bindgen_test::wasm_bindgen_test)]
#[cfg_attr(not(target_arch = "wasm32"), test)]
fn dominant_preset_and_explicit_profile_survive_mscz_playback() {
    use acorde_core::model::standard_bass_arrangement::StandardBassArrangementResult;
    use acorde_core::{AccordionPresetResult, Score, to_playback_events};
    let source = acorde_wasm::parse_abc("X:1\nM:4/4\nL:1/4\nK:C\nG A B c|]\n").unwrap();
    let original: Score = serde_json::from_str(&source).unwrap();
    let signature = |score: &Score| {
        let mut events = to_playback_events(score, &Default::default())
            .into_iter()
            .map(|e| (e.pitch_midi, e.time_beats, e.duration_beats))
            .collect::<Vec<_>>();
        events.sort_by(|a, b| a.1.total_cmp(&b.1).then_with(|| a.0.cmp(&b.0)));
        events
    };
    for root in ["G", "D"] {
        let selection=serde_json::json!({"chords":[{"measure_index":0,"offset_beats":0,"chord":{"root":root,"kind":"dominant"}}],"alternative_chords":null,"inversion":null,"bass_mode":"standard-bass"}).to_string();
        let result: AccordionPresetResult = serde_json::from_str(
            &acorde_wasm::arrange_accordion_preset(&source, &selection, "bass-pulse").unwrap(),
        )
        .unwrap();
        assert!(result.diagnostics.is_empty(), "{:?}", result.diagnostics);
        let output = result.score.unwrap();
        assert_eq!(
            output.parts[0].staves[0].measures[0].voices[0]
                .iter()
                .map(|n| n.pitches.clone())
                .collect::<Vec<_>>(),
            original.parts[0].staves[0].measures[0].voices[0]
                .iter()
                .map(|n| n.pitches.clone())
                .collect::<Vec<_>>()
        );
        let encoded =
            acorde_wasm::serialize_mscz(&serde_json::to_string(&output).unwrap()).unwrap();
        let decoded: Score =
            serde_json::from_str(&acorde_wasm::parse_mscz(&encoded).unwrap()).unwrap();
        assert_eq!(signature(&output), signature(&decoded));
    }
    let options = serde_json::json!({"melody_part":0,"melody_staff":0,"melody_voice":0,"allow_source_omissions":false,"chords":[{"measure_index":0,"offset_beats":0,"chord":{"root":"G","kind":"dominant"}}],"profile":{"bass_buttons":[{"id":"G bass","position":0,"midi":43}],"chord_buttons":[{"id":"G7","position":0,"midi":[67,71,77]}],"max_distance":0},"allow_chord_omissions":true,"allow_chord_additions":false});
    let profiled: StandardBassArrangementResult = serde_json::from_str(
        &acorde_wasm::arrange_standard_bass(&source, &options.to_string()).unwrap(),
    )
    .unwrap();
    assert!(
        profiled.diagnostics.is_empty(),
        "{:?}",
        profiled.diagnostics
    );
    assert_eq!(profiled.actions[0].candidate.omitted_pitch_classes, vec![2]);
    assert_eq!(profiled.actions[0].candidate.chord_midi, vec![67, 71, 77]);
    let output = profiled.score.unwrap();
    let encoded = acorde_wasm::serialize_mscz(&serde_json::to_string(&output).unwrap()).unwrap();
    let decoded: Score = serde_json::from_str(&acorde_wasm::parse_mscz(&encoded).unwrap()).unwrap();
    assert_eq!(signature(&output), signature(&decoded));
    let pulse_options = serde_json::json!({
        "selection": {"chords": [{"measure_index":0,"offset_beats":0,"chord":{"root":"G","kind":"dominant"}}],"alternative_chords":null,"inversion":null,"bass_mode":"standard-bass"},
        "profile": options["profile"], "allow_chord_omissions":true, "allow_chord_additions":false
    });
    let pulse: acorde_core::model::standard_bass_preset::ProfiledBassPulseResult =
        serde_json::from_str(
            &acorde_wasm::arrange_profiled_bass_pulse(&source, &pulse_options.to_string()).unwrap(),
        )
        .unwrap();
    assert!(pulse.diagnostics.is_empty(), "{:?}", pulse.diagnostics);
    assert_eq!(pulse.actions[0].sounding_midi, [43]);
    assert_eq!(pulse.actions[1].sounding_midi, [67, 71, 77]);
    assert_eq!(pulse.actions[1].candidate.omitted_pitch_classes, [2]);
    let pulse_score = pulse.score.unwrap();
    let pulse_encoded =
        acorde_wasm::serialize_mscz(&serde_json::to_string(&pulse_score).unwrap()).unwrap();
    let pulse_decoded: Score =
        serde_json::from_str(&acorde_wasm::parse_mscz(&pulse_encoded).unwrap()).unwrap();
    assert_eq!(signature(&pulse_score), signature(&pulse_decoded));
    let mut missing_pulse = pulse_options;
    missing_pulse["profile"]["chord_buttons"] = serde_json::json!([]);
    let missing_result: acorde_core::model::standard_bass_preset::ProfiledBassPulseResult =
        serde_json::from_str(
            &acorde_wasm::arrange_profiled_bass_pulse(&source, &missing_pulse.to_string()).unwrap(),
        )
        .unwrap();
    assert_eq!(
        missing_result.diagnostics[0].code,
        "no-standard-bass-candidate"
    );
    assert!(missing_result.score.is_none() && missing_result.actions.is_empty());

    let mut missing = options;
    missing["profile"]["chord_buttons"] = serde_json::json!([]);
    let rejected: StandardBassArrangementResult = serde_json::from_str(
        &acorde_wasm::arrange_standard_bass(&source, &missing.to_string()).unwrap(),
    )
    .unwrap();
    assert!(rejected.score.is_none());
    assert_eq!(rejected.diagnostics[0].code, "no-standard-bass-candidate");
}
