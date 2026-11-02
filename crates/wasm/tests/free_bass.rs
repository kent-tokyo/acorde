#![cfg(not(all(
    feature = "playback-only",
    not(any(feature = "full", feature = "viewer"))
)))]
#[cfg(target_arch = "wasm32")]
wasm_bindgen_test::wasm_bindgen_test_configure!(run_in_browser);
#[cfg_attr(target_arch = "wasm32", wasm_bindgen_test::wasm_bindgen_test)]
#[cfg_attr(not(target_arch = "wasm32"), test)]
fn polyphonic_free_bass_preserves_mscz_playback_and_provenance() {
    let source = acorde_wasm::parse_abc("X:1\nM:4/4\nL:1/2\nK:C\n[C,Ee] [D,Ff]|]\n").unwrap();
    let result: acorde_core::FreeBassResult =
        serde_json::from_str(&acorde_wasm::split_for_free_bass(&source, "{}").unwrap()).unwrap();
    assert!(result.diagnostics.is_empty(), "{:?}", result.diagnostics);
    assert_eq!(result.source_map.len(), 6);
    assert!(result.requires_performer_review);
    assert_eq!(
        result.required_bass_mode,
        acorde_core::AccordionBassMode::FreeBass
    );
    let encoded =
        acorde_wasm::serialize_mscz(&serde_json::to_string(&result.score.unwrap()).unwrap())
            .unwrap();
    let decoded = acorde_wasm::parse_mscz(&encoded).unwrap();
    let events = |json: &str| {
        let score: acorde_core::Score = serde_json::from_str(json).unwrap();
        let mut events: Vec<_> = acorde_core::to_playback_events(&score, &Default::default())
            .iter()
            .map(|e| (e.time_beats as u32, e.pitch_midi, e.duration_beats as u32))
            .collect();
        events.sort_unstable();
        events
    };
    assert_eq!(events(&source), events(&decoded));
}
