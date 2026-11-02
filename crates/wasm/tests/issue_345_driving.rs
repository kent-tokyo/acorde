#![cfg(not(all(
    feature = "playback-only",
    not(any(feature = "full", feature = "viewer"))
)))]

#[cfg(target_arch = "wasm32")]
wasm_bindgen_test::wasm_bindgen_test_configure!(run_in_browser);

#[cfg_attr(target_arch = "wasm32", wasm_bindgen_test::wasm_bindgen_test)]
#[cfg_attr(not(target_arch = "wasm32"), test)]
fn driving_viewer_and_native_events_match() {
    let source = acorde_wasm::parse_abc("X:1\nM:4/4\nL:1/4\nQ:1/4=120\nK:C\nc4|]\n").unwrap();
    let score: acorde_core::Score = serde_json::from_str(&source).unwrap();
    let selection = r#"{"chords":[{"measure_index":0,"offset_beats":0,"chord":{"root":"C","kind":"major","bass":null}},{"measure_index":0,"offset_beats":2,"chord":{"root":"G","kind":"major","bass":null}}],"alternative_chords":null,"inversion":null,"bass_mode":"standard-bass"}"#;
    let native_selection: acorde_core::AccordionPresetSelection =
        serde_json::from_str(selection).unwrap();
    for id in ["driving-bass-chord", "driving-bass-chord-light"] {
        let web: acorde_core::AccordionPresetResult = serde_json::from_str(
            &acorde_wasm::arrange_accordion_preset(&source, selection, id).unwrap(),
        )
        .unwrap();
        let native = acorde_core::arrange_accordion_preset(&score, native_selection.clone(), id);
        assert!(web.diagnostics.is_empty(), "{:?}", web.diagnostics);
        let events = |score: &acorde_core::Score| {
            acorde_core::to_playback_events(score, &Default::default())
                .into_iter()
                .map(|e| (e.pitch_midi, e.time_beats, e.duration_beats, e.velocity))
                .collect::<Vec<_>>()
        };
        assert_eq!(
            events(web.score.as_ref().unwrap()),
            events(native.score.as_ref().unwrap())
        );
    }
}
