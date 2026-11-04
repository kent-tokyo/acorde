#![cfg(not(all(
    feature = "playback-only",
    not(any(feature = "full", feature = "viewer"))
)))]
#[cfg(target_arch = "wasm32")]
wasm_bindgen_test::wasm_bindgen_test_configure!(run_in_browser);

#[cfg_attr(target_arch = "wasm32", wasm_bindgen_test::wasm_bindgen_test)]
#[cfg_attr(not(target_arch = "wasm32"), test)]
fn selected_melody_and_button_notes_survive_mscz_roundtrip() {
    use acorde_core::model::standard_bass_arrangement::StandardBassArrangementResult;
    use acorde_core::{Duration, Note, Pitch, Score, Step, to_playback_events};
    let mut score = Score::new("selected melody", 100, 4, 4, 0, 2);
    for (i, measure) in score.parts[0].staves[0].measures.iter_mut().enumerate() {
        measure.voices[0] = vec![Note::new(Pitch::new(Step::C, 4), Duration::Whole)];
        let mut melody = Note::new(Pitch::new(Step::E, 5), Duration::Whole);
        melody.tie_start = i == 0;
        melody.tie_end = i == 1;
        measure.voices[1] = vec![melody];
    }
    let options = r#"{"melody_part":0,"melody_staff":0,"melody_voice":1,"allow_source_omissions":true,"chords":[{"measure_index":0,"offset_beats":0,"chord":{"root":"C","kind":"major-seventh"}}],"profile":{"bass_buttons":[{"id":"C","position":0,"midi":36}],"chord_buttons":[{"id":"Em","position":1,"midi":[64,67,71]}],"max_distance":1},"allow_chord_omissions":false,"allow_chord_additions":false}"#;
    let result: StandardBassArrangementResult = serde_json::from_str(
        &acorde_wasm::arrange_standard_bass(&serde_json::to_string(&score).unwrap(), options)
            .unwrap(),
    )
    .unwrap();
    assert!(result.diagnostics.is_empty(), "{:?}", result.diagnostics);
    let output = result.score.unwrap();
    let output_json = serde_json::to_string(&output).unwrap();
    let svg = acorde_wasm::render_score_svg(&output_json, "{}").unwrap();
    assert!(svg.contains("<svg"));
    let archive = acorde_wasm::serialize_mscz(&output_json).unwrap();
    let decoded: Score = serde_json::from_str(&acorde_wasm::parse_mscz(&archive).unwrap()).unwrap();
    let signature = |s: &Score, staff: usize, voice: usize| {
        to_playback_events(s, &Default::default())
            .into_iter()
            .filter(|e| {
                e.source
                    .as_ref()
                    .is_some_and(|a| a.staff == staff && a.voice == voice)
            })
            .map(|e| (e.pitch_midi, e.time_beats, e.duration_beats))
            .collect::<Vec<_>>()
    };
    assert_eq!(signature(&score, 0, 1), signature(&output, 0, 0));
    assert_eq!(signature(&output, 0, 0), signature(&decoded, 0, 0));
    assert_eq!(signature(&output, 1, 0), signature(&decoded, 1, 0));
    assert_eq!(
        result
            .source_pitches
            .iter()
            .filter(|p| p.target.is_none())
            .count(),
        2
    );
}

#[cfg_attr(target_arch = "wasm32", wasm_bindgen_test::wasm_bindgen_test)]
#[cfg_attr(not(target_arch = "wasm32"), test)]
fn selected_chord_pitches_and_partial_ties_survive_mscz() {
    use acorde_core::model::standard_bass_pitch_selection::SelectedPitchArrangementResult;
    use acorde_core::{Duration, Note, Pitch, Score, Step, to_playback_events};
    let mut score = Score::new("selected chord", 100, 4, 4, 0, 2);
    for (i, measure) in score.parts[0].staves[0].measures.iter_mut().enumerate() {
        let mut chord = Note::new(Pitch::new(Step::C, 5), Duration::Whole);
        chord.pitches.push(Pitch::new(Step::E, 5));
        chord.set_pitch_ties(&[false, i == 0], &[false, i == 1]);
        measure.voices[0] = vec![chord];
    }
    let options = r#"{"melody_part":0,"melody_staff":0,"melody_voice":0,"allow_source_omissions":true,"chords":[{"measure_index":0,"offset_beats":0,"chord":{"root":"C","kind":"major-seventh"}}],"profile":{"bass_buttons":[{"id":"C","position":0,"midi":36}],"chord_buttons":[{"id":"Em","position":1,"midi":[64,67,71]}],"max_distance":1},"allow_chord_omissions":false,"allow_chord_additions":false}"#;
    let selections = r#"[{"source":{"part":0,"staff":0,"measure":0,"voice":0,"note":0},"pitch_index":1},{"source":{"part":0,"staff":0,"measure":1,"voice":0,"note":0},"pitch_index":1}]"#;
    let source = serde_json::to_string(&score).unwrap();
    let result: SelectedPitchArrangementResult = serde_json::from_str(
        &acorde_wasm::arrange_standard_bass_with_pitch_selection(&source, options, selections)
            .unwrap(),
    )
    .unwrap();
    assert!(
        result.arrangement.diagnostics.is_empty(),
        "{:?}",
        result.arrangement.diagnostics
    );
    assert_eq!(
        result
            .arrangement
            .source_pitches
            .iter()
            .filter(|p| p.target.is_none())
            .count(),
        2
    );
    let output = result.arrangement.score.unwrap();
    let encoded = acorde_wasm::serialize_mscz(&serde_json::to_string(&output).unwrap()).unwrap();
    let decoded: Score = serde_json::from_str(&acorde_wasm::parse_mscz(&encoded).unwrap()).unwrap();
    let signature = |s: &Score| {
        to_playback_events(s, &Default::default())
            .into_iter()
            .filter(|e| e.source.as_ref().is_some_and(|a| a.staff == 0))
            .map(|e| (e.pitch_midi, e.time_beats, e.duration_beats))
            .collect::<Vec<_>>()
    };
    let original_selected = signature(&score)
        .into_iter()
        .filter(|e| e.0 == 76)
        .collect::<Vec<_>>();
    assert_eq!(original_selected, signature(&output));
    assert_eq!(signature(&output), signature(&decoded));
    assert!(decoded.parts[0].staves[0].measures[0].voices[0][0].tie_start);
    assert!(decoded.parts[0].staves[0].measures[1].voices[0][0].tie_end);
}
