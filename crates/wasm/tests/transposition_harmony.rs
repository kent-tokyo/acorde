#![cfg(not(all(
    feature = "playback-only",
    not(any(feature = "full", feature = "viewer"))
)))]
#[cfg(target_arch = "wasm32")]
wasm_bindgen_test::wasm_bindgen_test_configure!(run_in_browser);
#[cfg_attr(target_arch = "wasm32", wasm_bindgen_test::wasm_bindgen_test)]
#[cfg_attr(not(target_arch = "wasm32"), test)]
fn transposed_chords_and_playback_survive_mscz_export() {
    let source = acorde_wasm::parse_abc("X:1\nM:4/4\nL:1/4\nK:C\n\"C/E\"C E G c|]\n").unwrap();
    let moved = acorde_wasm::transpose_score(&source, 2).unwrap();
    let encoded = acorde_wasm::serialize_mscz(&moved).unwrap();
    let decoded = acorde_wasm::parse_mscz(&encoded).unwrap();
    let score: acorde_core::Score = serde_json::from_str(&decoded).unwrap();
    let chord = score.parts[0].staves[0].measures[0].voices[0][0]
        .chord_symbol
        .as_ref()
        .unwrap();
    assert_eq!(chord.root, "D");
    assert_eq!(chord.bass.as_deref(), Some("F#"));
    let before: acorde_core::Score = serde_json::from_str(&source).unwrap();
    let old = acorde_core::to_playback_events(&before, &Default::default());
    let new = acorde_core::to_playback_events(&score, &Default::default());
    assert_eq!(old.len(), new.len());
    for (a, b) in old.iter().zip(&new) {
        assert_eq!(b.pitch_midi_cents, a.pitch_midi_cents + 200);
        assert_eq!(a.time_beats, b.time_beats);
        assert_eq!(a.duration_beats, b.duration_beats);
    }
}
