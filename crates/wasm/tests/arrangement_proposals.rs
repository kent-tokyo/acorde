#![cfg(not(all(
    feature = "playback-only",
    not(any(feature = "full", feature = "viewer"))
)))]

#[cfg(target_arch = "wasm32")]
wasm_bindgen_test::wasm_bindgen_test_configure!(run_in_browser);

#[cfg_attr(target_arch = "wasm32", wasm_bindgen_test::wasm_bindgen_test)]
#[cfg_attr(not(target_arch = "wasm32"), test)]
fn proposals_replay_and_regenerate_through_json() {
    let mut source = acorde_core::Score::new("WASM proposals", 120, 4, 4, 0, 4);
    for bar in &mut source.parts[0].staves[0].measures {
        bar.voices[0] = [84, 86, 88, 84]
            .into_iter()
            .map(|midi| {
                acorde_core::Note::new(
                    acorde_core::Pitch::from_midi(midi, false),
                    acorde_core::Duration::Quarter,
                )
            })
            .collect();
    }
    let score_json = serde_json::to_string(&source).unwrap();
    let request = serde_json::json!({
        "harmony": {},
        "ensemble_template": {
            "harmony": {
                "chords":[], "pattern":"held", "voicing":"smooth",
                "midi_program":48, "low_midi":48, "center_midi":62,
                "high_midi":76, "melody_part_index":0,
                "min_melody_gap_semitones":3
            },
            "bass": {
                "chords":[], "pattern":"root-pulse", "midi_program":32,
                "low_midi":28, "center_midi":40, "high_midi":52
            },
            "min_bass_harmony_gap_semitones":3
        },
        "count":3
    });
    let result: acorde_core::ArrangementProposalResult = serde_json::from_str(
        &acorde_wasm::suggest_arrangement_proposals(&score_json, &request.to_string()).unwrap(),
    )
    .unwrap();
    assert!(result.diagnostics.is_empty(), "{:?}", result.diagnostics);
    assert_eq!(result.candidates.len(), 3);
    let selected = &result.candidates[1];
    let replay: acorde_core::ArrangementProposalReplayResult = serde_json::from_str(
        &acorde_wasm::replay_arrangement_proposal(
            &score_json,
            &serde_json::to_string(&selected.resolved).unwrap(),
        )
        .unwrap(),
    )
    .unwrap();
    assert!(replay.diagnostics.is_empty(), "{:?}", replay.diagnostics);
    let events = |score: &acorde_core::Score| {
        acorde_core::to_playback_events(score, &Default::default())
            .into_iter()
            .map(|event| {
                (
                    event.source.unwrap().part,
                    event.time_beats,
                    event.duration_beats,
                    event.pitch_midi,
                )
            })
            .collect::<Vec<_>>()
    };
    assert_eq!(events(&selected.score), events(&replay.score.unwrap()));

    let region = serde_json::json!({
        "original":selected.resolved,
        "replacement":result.candidates[2].resolved,
        "start_measure":1,
        "end_measure_exclusive":3,
        "locked_roles":["bass"],
        "locked_measures":[2]
    });
    let regenerated: acorde_core::ArrangementRegionResult = serde_json::from_str(
        &acorde_wasm::regenerate_arrangement_region(&score_json, &region.to_string()).unwrap(),
    )
    .unwrap();
    assert!(
        regenerated.diagnostics.is_empty(),
        "{:?}",
        regenerated.diagnostics
    );
    assert_eq!(regenerated.changed_measures, vec![1]);
    assert!(regenerated.score.is_some());

    let mut changed_level = region;
    changed_level["replacement"]["ensemble_options"]["harmony_dynamic"] = serde_json::json!("Pp");
    let rejected: acorde_core::ArrangementRegionResult = serde_json::from_str(
        &acorde_wasm::regenerate_arrangement_region(&score_json, &changed_level.to_string())
            .unwrap(),
    )
    .unwrap();
    assert_eq!(rejected.diagnostics[0].code, "incompatible-dynamics");
    assert!(rejected.score.is_none());
}
