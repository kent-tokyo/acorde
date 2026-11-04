#![cfg(not(all(
    feature = "playback-only",
    not(any(feature = "full", feature = "viewer"))
)))]
#[cfg(target_arch = "wasm32")]
wasm_bindgen_test::wasm_bindgen_test_configure!(run_in_browser);
#[cfg_attr(target_arch = "wasm32", wasm_bindgen_test::wasm_bindgen_test)]
#[cfg_attr(not(target_arch = "wasm32"), test)]
fn viewer_passing_note_matches_native() {
    let source = acorde_wasm::parse_abc("X:1\nM:4/4\nL:1/4\nK:C\nc2 e2|]\n").unwrap();
    let score: acorde_core::Score = serde_json::from_str(&source).unwrap();
    let opts = r#"{"part":0,"staff":0,"voice":0,"start_measure":0,"end_measure":1,"key":{"fifths":0,"mode":"major"},"kind":"passing","source":"shorten-note","max_insertions":1,"min_midi":48,"max_midi":96,"max_leap_semitones":2}"#;
    let options: acorde_core::MelodicInsertionOptions = serde_json::from_str(opts).unwrap();
    let native = acorde_core::insert_melodic_notes(&score, options);
    let web: acorde_core::MelodicInsertionResult =
        serde_json::from_str(&acorde_wasm::insert_melodic_notes(&source, opts).unwrap()).unwrap();
    assert!(web.diagnostics.is_empty(), "{:?}", web.diagnostics);
    assert_eq!(
        serde_json::to_value(web.score).unwrap(),
        serde_json::to_value(native.score).unwrap()
    );
}
