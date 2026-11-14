#![cfg(not(all(
    feature = "playback-only",
    not(any(feature = "full", feature = "viewer"))
)))]

#[cfg(target_arch = "wasm32")]
wasm_bindgen_test::wasm_bindgen_test_configure!(run_in_browser);

#[cfg_attr(target_arch = "wasm32", wasm_bindgen_test::wasm_bindgen_test)]
#[cfg_attr(not(target_arch = "wasm32"), test)]
fn scheduled_harmony_bass_and_countermelody_replay_through_json() {
    let mut source = acorde_core::Score::new("ensemble WASM", 120, 4, 4, 0, 4);
    for bar in &mut source.parts[0].staves[0].measures {
        bar.voices[0] = vec![acorde_core::Note::new(
            acorde_core::Pitch::from_midi(84, false),
            acorde_core::Duration::Whole,
        )];
    }
    source.parts[0].staves[0].measures[0].voices[0] = [84, 86, 88, 84]
        .into_iter()
        .map(|midi| {
            acorde_core::Note::new(
                acorde_core::Pitch::from_midi(midi, false),
                acorde_core::Duration::Quarter,
            )
        })
        .collect();
    let score_json = serde_json::to_string(&source).unwrap();
    let chords = serde_json::json!([
        {"measure_index":0,"offset_beats":0.0,"chord":{"root":"C","kind":"major"}},
        {"measure_index":1,"offset_beats":0.0,"chord":{"root":"F","kind":"major"}},
        {"measure_index":2,"offset_beats":0.0,"chord":{"root":"G","kind":"major"}},
        {"measure_index":3,"offset_beats":0.0,"chord":{"root":"C","kind":"major"}}
    ]);
    let request = serde_json::json!({
        "harmony": {
            "chords":chords,"pattern":"held","voicing":"root-position",
            "midi_program":48,"low_midi":48,"high_midi":84,"center_midi":64,
            "min_melody_gap_semitones":3,"melody_part_index":0
        },
        "bass": {
            "chords":chords,"pattern":"root-fifth",
            "midi_program":32,"low_midi":28,"high_midi":60,"center_midi":40
        },
        "countermelody": {
            "chords":chords,"melody_part_index":0,
            "motif_start_measure":0,"motif_end_measure_exclusive":1,
            "midi_program":73,"low_midi":75,"center_midi":77,"high_midi":80,
            "min_melody_gap_semitones":4
        },
        "harmony_dynamic":"Pp",
        "bass_dynamic":"Mp",
        "countermelody_dynamic":"P",
        "sections":[
            {"role":"bass","start_measure":0,"end_measure_exclusive":1,"enabled":false},
            {"role":"harmony","start_measure":1,"end_measure_exclusive":2,"enabled":false},
            {"role":"countermelody","start_measure":2,"end_measure_exclusive":3,"enabled":false}
        ],
        "restart_countermelody_at_section_entry":true,
        "min_bass_harmony_gap_semitones":3,
        "min_harmony_countermelody_gap_semitones":3,
        "max_generated_attacks_per_measure":7
    });
    let result: acorde_core::EnsembleRoleResult = serde_json::from_str(
        &acorde_wasm::arrange_ensemble_roles(&score_json, &request.to_string()).unwrap(),
    )
    .unwrap();
    assert!(result.diagnostics.is_empty(), "{:?}", result.diagnostics);
    assert_eq!(result.role_parts.len(), 3);
    assert!(
        result
            .resolved_options
            .restart_countermelody_at_section_entry
    );
    assert_eq!(
        result.resolved_options.countermelody_dynamic,
        Some(acorde_core::Dynamic::P)
    );
    let output = result.score.unwrap();
    assert_eq!(
        serde_json::to_value(&output.parts[..1]).unwrap(),
        serde_json::to_value(&source.parts).unwrap()
    );
    let replay: acorde_core::EnsembleRoleResult = serde_json::from_str(
        &acorde_wasm::arrange_ensemble_roles(
            &score_json,
            &serde_json::to_string(&result.resolved_options).unwrap(),
        )
        .unwrap(),
    )
    .unwrap();
    let events = |score: &acorde_core::Score| {
        acorde_core::to_playback_events(score, &Default::default())
            .into_iter()
            .map(|event| {
                (
                    event.source.unwrap().part,
                    event.time_beats,
                    event.pitch_midi,
                    event.velocity,
                )
            })
            .collect::<Vec<_>>()
    };
    assert_eq!(events(&output), events(&replay.score.unwrap()));

    let counter: acorde_core::CountermelodyResult = serde_json::from_str(
        &acorde_wasm::arrange_countermelody(
            &score_json,
            &serde_json::to_string(result.resolved_options.countermelody.as_ref().unwrap())
                .unwrap(),
        )
        .unwrap(),
    )
    .unwrap();
    assert!(counter.diagnostics.is_empty(), "{:?}", counter.diagnostics);
    assert_eq!(counter.added_part, Some(1));

    let mut ensemble = request;
    ensemble["sections"] = serde_json::json!([]);
    let handoff = serde_json::json!({
        "ensemble":ensemble,
        "melody_part_index":0,
        "melody_staff_index":0,
        "melody_sections":[
            {"start_measure":2,"end_measure_exclusive":3,"enabled":false}
        ]
    });
    let performance: acorde_core::EnsembleMelodyResult = serde_json::from_str(
        &acorde_wasm::arrange_ensemble_with_melody(&score_json, &handoff.to_string()).unwrap(),
    )
    .unwrap();
    assert!(
        performance.diagnostics.is_empty(),
        "{:?}",
        performance.diagnostics
    );
    assert_eq!(performance.melody_activity, vec![true, true, false, true]);
    assert!(
        performance.score.unwrap().parts[0].staves[0].measures[2].voices[0]
            .iter()
            .all(|note| note.is_rest)
    );

    let mut tied = source;
    for measure in 1..4 {
        tied.parts[0].staves[0].measures[measure].voices[0][0]
            .pitches
            .push(acorde_core::Pitch::from_midi(88, false));
    }
    tied.parts[0].staves[0].measures[1].voices[0][0]
        .set_pitch_ties(&[true, false], &[false, false]);
    tied.parts[0].staves[0].measures[2].voices[0][0].set_pitch_ties(&[true, false], &[true, false]);
    tied.parts[0].staves[0].measures[3].voices[0][0]
        .set_pitch_ties(&[false, false], &[true, false]);
    tied.parts[0].staves[0].measures[3].voices[0][0].pitch_incoming_partial_ties =
        vec![true, false];
    let tied_result: acorde_core::EnsembleMelodyResult = serde_json::from_str(
        &acorde_wasm::arrange_ensemble_with_melody(
            &serde_json::to_string(&tied).unwrap(),
            &handoff.to_string(),
        )
        .unwrap(),
    )
    .unwrap();
    assert!(
        tied_result.diagnostics.is_empty(),
        "{:?}",
        tied_result.diagnostics
    );
    let tied_output = tied_result.score.unwrap();
    assert_eq!(
        tied_output.parts[0].staves[0].measures[1].voices[0][0].pitch_tie_starts_or_uniform(),
        [false, false]
    );
    assert_eq!(
        tied_output.parts[0].staves[0].measures[3].voices[0][0].pitch_tie_ends_or_uniform(),
        [false, false]
    );
    assert_eq!(
        tied_output.parts[0].staves[0].measures[3].voices[0][0].pitch_incoming_partial_ties,
        [true, false]
    );
}

#[cfg_attr(target_arch = "wasm32", wasm_bindgen_test::wasm_bindgen_test)]
#[cfg_attr(not(target_arch = "wasm32"), test)]
fn melody_handoff_keeps_an_active_alternate_ending_tie() {
    let mut source = acorde_core::Score::new("alternate ending WASM", 120, 4, 4, 0, 3);
    for bar in &mut source.parts[0].staves[0].measures {
        bar.voices[0] = vec![acorde_core::Note::new(
            acorde_core::Pitch::from_midi(72, false),
            acorde_core::Duration::Whole,
        )];
    }
    let bars = &mut source.parts[0].staves[0].measures;
    bars[0].barline_left = acorde_core::Barline::RepeatStart;
    bars[0].voices[0][0].tie_start = true;
    bars[1].barline_right = acorde_core::Barline::RepeatEnd;
    bars[1].volta = Some(acorde_core::VoltaBracket {
        number: 1,
        kind: "begin_end".into(),
        numbers: vec![],
    });
    bars[1].voices[0][0].tie_end = true;
    bars[2].volta = Some(acorde_core::VoltaBracket {
        number: 2,
        kind: "begin_end".into(),
        numbers: vec![],
    });
    bars[2].voices[0][0].pitch_incoming_partial_ties = vec![true];
    let request = serde_json::json!({
        "ensemble":{}, "melody_part_index":0, "melody_staff_index":0,
        "melody_sections":[{"start_measure":1,"end_measure_exclusive":2,"enabled":false}]
    });
    let result: acorde_core::EnsembleMelodyResult = serde_json::from_str(
        &acorde_wasm::arrange_ensemble_with_melody(
            &serde_json::to_string(&source).unwrap(),
            &request.to_string(),
        )
        .unwrap(),
    )
    .unwrap();
    assert!(result.diagnostics.is_empty(), "{:?}", result.diagnostics);
    let output = result.score.unwrap();
    assert!(output.parts[0].staves[0].measures[0].voices[0][0].tie_start);
    let events = acorde_core::to_playback_events(&output, &Default::default())
        .into_iter()
        .map(|event| (event.time_beats, event.duration_beats))
        .collect::<Vec<_>>();
    assert_eq!(events, [(0.0, 4.0), (8.0, 8.0)]);
}
