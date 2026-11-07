#![cfg(not(all(
    feature = "playback-only",
    not(any(feature = "full", feature = "viewer"))
)))]
#[cfg(target_arch = "wasm32")]
wasm_bindgen_test::wasm_bindgen_test_configure!(run_in_browser);

#[cfg_attr(target_arch = "wasm32", wasm_bindgen_test::wasm_bindgen_test)]
#[cfg_attr(not(target_arch = "wasm32"), test)]
fn fujisan_mscz_native_viewer_render_playback_and_roundtrip() {
    let source = include_bytes!("../../io/tests/fixtures/fujisan-1911.mscz");
    let score_json = acorde_wasm::parse_mscz(source).unwrap();
    let score: acorde_core::Score = serde_json::from_str(&score_json).unwrap();
    assert!(score.parts[0].staves[0].measures.len() >= 16);
    let options = acorde_core::MelodicInsertionOptions {
        part: 0,
        staff: 0,
        voice: 0,
        start_measure: 0,
        end_measure: 16,
        key: score.settings.key_signature.clone(),
        kind: acorde_core::MelodicInsertionKind::Passing,
        source: acorde_core::MelodicInsertionSource::ShortenNote,
        max_insertions: 16,
        min_midi: 48,
        max_midi: 96,
        max_leap_semitones: 2,
    };
    let options_json = serde_json::to_string(&options).unwrap();
    let native = acorde_core::insert_melodic_notes(&score, options);
    assert_eq!(native.insertions.len(), 6);
    assert!(
        !native
            .diagnostics
            .iter()
            .any(|item| item.code == "unsupported-measure")
    );
    let web: acorde_core::MelodicInsertionResult = serde_json::from_str(
        &acorde_wasm::insert_melodic_notes(&score_json, &options_json).unwrap(),
    )
    .unwrap();
    assert_eq!(
        serde_json::to_value(&native.insertions).unwrap(),
        serde_json::to_value(&web.insertions).unwrap()
    );
    assert_eq!(
        serde_json::to_value(&native.diagnostics).unwrap(),
        serde_json::to_value(&web.diagnostics).unwrap()
    );
    assert_eq!(
        serde_json::to_value(&native.score).unwrap(),
        serde_json::to_value(&web.score).unwrap()
    );
    let edited = native.score.unwrap();
    let edited_json = serde_json::to_string(&edited).unwrap();
    let svg = acorde_wasm::render_score_svg(&edited_json, "{}").unwrap();
    assert!(svg.contains("<svg"));
    let web_events: serde_json::Value =
        serde_json::from_str(&acorde_wasm::to_playback_events_ex(&edited_json, "{}").unwrap())
            .unwrap();
    let native_events = serde_json::to_value(acorde_core::to_playback_events(
        &edited,
        &Default::default(),
    ))
    .unwrap();
    assert_eq!(web_events, native_events);
    let exported = acorde_wasm::serialize_mscz(&edited_json).unwrap();
    let reparsed: acorde_core::Score =
        serde_json::from_str(&acorde_wasm::parse_mscz(&exported).unwrap()).unwrap();
    let before_pitches: Vec<_> = edited.parts[0].staves[0]
        .measures
        .iter()
        .take(16)
        .map(|measure| {
            measure.voices[0]
                .iter()
                .map(|note| (&note.pitches, note.beats()))
                .collect::<Vec<_>>()
        })
        .collect();
    let after_pitches: Vec<_> = reparsed.parts[0].staves[0]
        .measures
        .iter()
        .take(16)
        .map(|measure| {
            measure.voices[0]
                .iter()
                .map(|note| (&note.pitches, note.beats()))
                .collect::<Vec<_>>()
        })
        .collect();
    assert_eq!(before_pitches, after_pitches);
}

#[cfg_attr(target_arch = "wasm32", wasm_bindgen_test::wasm_bindgen_test)]
#[cfg_attr(not(target_arch = "wasm32"), test)]
fn contextual_api_matches_native_and_viewer() {
    let source = acorde_wasm::parse_abc("X:1\nM:2/4\nL:1/4\nK:C\nc e|]\n").unwrap();
    let score: acorde_core::Score = serde_json::from_str(&source).unwrap();
    let options = serde_json::json!({
        "part":0,"staff":0,"voice":0,"start_measure":0,"end_measure":1,
        "key":{"fifths":0,"mode":"major"},"kind":"passing","source":"shorten-note",
        "max_insertions":1,"min_midi":48,"max_midi":96,"max_leap_semitones":3
    });
    let context = serde_json::json!({"chord_plan":{
        "contract_version":1,"entries":[{
            "position":{"measure_index":0,"offset":{"numerator":0,"denominator":1}},
            "offset_beats":0.0,"chord":{"root":"B","kind":"major","bass":null},
            "origin":"host-authored","sources":[]
        }]
    }});
    let options_json = options.to_string();
    let context_json = context.to_string();
    let native = acorde_core::insert_melodic_notes_with_context(
        &score,
        serde_json::from_value(options).unwrap(),
        &serde_json::from_value(context).unwrap(),
    );
    let web: acorde_core::MelodicInsertionContextResult = serde_json::from_str(
        &acorde_wasm::insert_melodic_notes_with_context(&source, &options_json, &context_json)
            .unwrap(),
    )
    .unwrap();
    assert_eq!(
        serde_json::to_value(native).unwrap(),
        serde_json::to_value(web).unwrap()
    );
}
