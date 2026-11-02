#![cfg(not(all(
    feature = "playback-only",
    not(any(feature = "full", feature = "viewer"))
)))]
#[cfg(target_arch = "wasm32")]
wasm_bindgen_test::wasm_bindgen_test_configure!(run_in_browser);

#[cfg_attr(target_arch = "wasm32", wasm_bindgen_test::wasm_bindgen_test)]
#[cfg_attr(not(target_arch = "wasm32"), test)]
fn playback_screen_matches_native_and_survives_mscz() {
    use acorde_core::model::standard_bass_transition::ScreenedStandardBassResult;
    use acorde_core::{Barline, Duration, Note, Pitch, Score, Step, to_playback_events};
    let mut source = Score::new("repeat", 120, 4, 4, 0, 2);
    for (i, measure) in source.parts[0].staves[0].measures.iter_mut().enumerate() {
        measure.voices[0] = vec![Note::new(
            Pitch::new(if i == 0 { Step::C } else { Step::D }, 5),
            Duration::Whole,
        )];
    }
    source.parts[0].staves[0].measures[0].barline_left = Barline::RepeatStart;
    source.parts[0].staves[0].measures[1].barline_right = Barline::RepeatEnd;
    let options = r#"{"melody_part":0,"melody_staff":0,"melody_voice":0,"allow_source_omissions":false,"chords":[{"measure_index":0,"offset_beats":0,"chord":{"root":"C","kind":"major"}},{"measure_index":1,"offset_beats":0,"chord":{"root":"G","kind":"major"}}],"profile":{"bass_buttons":[{"id":"C","position":0,"midi":36},{"id":"G","position":5,"midi":43}],"chord_buttons":[{"id":"Cmaj","position":0,"midi":[60,64,67]},{"id":"Gmaj","position":5,"midi":[67,71,74]}],"max_distance":0},"allow_chord_omissions":false,"allow_chord_additions":false}"#;
    let source_json = serde_json::to_string(&source).unwrap();
    let screened: ScreenedStandardBassResult = serde_json::from_str(
        &acorde_wasm::arrange_standard_bass_screened(
            &source_json,
            options,
            "[]",
            r#"{"max_position_delta":5,"min_attack_interval_ms":2000}"#,
        )
        .unwrap(),
    )
    .unwrap();
    assert!(
        screened.arrangement.arrangement.diagnostics.is_empty(),
        "{:?}",
        screened.arrangement.arrangement.diagnostics
    );
    assert_eq!(
        screened
            .transitions
            .iter()
            .map(|t| t.time_seconds)
            .collect::<Vec<_>>(),
        [0.0, 2.0, 4.0, 6.0]
    );
    let score = screened.arrangement.arrangement.score.unwrap();
    let archive = acorde_wasm::serialize_mscz(&serde_json::to_string(&score).unwrap()).unwrap();
    let decoded: Score = serde_json::from_str(&acorde_wasm::parse_mscz(&archive).unwrap()).unwrap();
    let signature = |score: &Score| {
        to_playback_events(score, &Default::default())
            .into_iter()
            .map(|e| (e.pitch_midi, e.time_beats, e.duration_beats))
            .collect::<Vec<_>>()
    };
    assert_eq!(signature(&score), signature(&decoded));
    let rejected: ScreenedStandardBassResult = serde_json::from_str(
        &acorde_wasm::arrange_standard_bass_screened(
            &source_json,
            options,
            "[]",
            r#"{"max_position_delta":4,"min_attack_interval_ms":0}"#,
        )
        .unwrap(),
    )
    .unwrap();
    assert_eq!(
        rejected.arrangement.arrangement.diagnostics[0].code,
        "transition-too-far"
    );
    assert!(rejected.arrangement.arrangement.score.is_none() && rejected.transitions.is_empty());

    // Same sounding G buttons at a nearer position can satisfy both repeat edges.
    let mut search_options: serde_json::Value = serde_json::from_str(options).unwrap();
    search_options["profile"]["bass_buttons"]
        .as_array_mut()
        .unwrap()
        .push(serde_json::json!({"id":"Z near G","position":1,"midi":43}));
    search_options["profile"]["chord_buttons"]
        .as_array_mut()
        .unwrap()
        .push(serde_json::json!({"id":"Z near Gmaj","position":1,"midi":[67,71,74]}));
    let found: ScreenedStandardBassResult = serde_json::from_str(
        &acorde_wasm::arrange_standard_bass_find_buttons(
            &source_json,
            &search_options.to_string(),
            "[]",
            r#"{"max_position_delta":1,"min_attack_interval_ms":2000}"#,
        )
        .unwrap(),
    )
    .unwrap();
    assert!(
        found.arrangement.arrangement.diagnostics.is_empty(),
        "{:?}",
        found.arrangement.arrangement.diagnostics
    );
    assert_eq!(
        found
            .transitions
            .iter()
            .map(|t| t.bass_button_id.as_str())
            .collect::<Vec<_>>(),
        ["C", "Z near G", "C", "Z near G"]
    );
    assert_eq!(
        signature(&score),
        signature(found.arrangement.arrangement.score.as_ref().unwrap())
    );

    // The screened path also supports explicit pitch selection within chords.
    for (i, measure) in source.parts[0].staves[0].measures.iter_mut().enumerate() {
        measure.voices[0][0]
            .pitches
            .push(Pitch::new(if i == 0 { Step::E } else { Step::F }, 5));
    }
    let choices = r#"[{"source":{"part":0,"staff":0,"measure":0,"voice":0,"note":0},"pitch_index":1},{"source":{"part":0,"staff":0,"measure":1,"voice":0,"note":0},"pitch_index":1}]"#;
    let poly_options = options.replace(
        r#""allow_source_omissions":false"#,
        r#""allow_source_omissions":true"#,
    );
    let poly: ScreenedStandardBassResult = serde_json::from_str(
        &acorde_wasm::arrange_standard_bass_screened(
            &serde_json::to_string(&source).unwrap(),
            &poly_options,
            choices,
            r#"{"max_position_delta":5,"min_attack_interval_ms":2000}"#,
        )
        .unwrap(),
    )
    .unwrap();
    assert!(
        poly.arrangement.arrangement.diagnostics.is_empty(),
        "{:?}",
        poly.arrangement.arrangement.diagnostics
    );
    assert_eq!(
        poly.arrangement
            .arrangement
            .source_pitches
            .iter()
            .filter(|pitch| pitch.target.is_none())
            .count(),
        2
    );
    assert_eq!(poly.transitions.len(), 4);
}
