#![cfg(not(all(
    feature = "playback-only",
    not(any(feature = "full", feature = "viewer"))
)))]

#[cfg(target_arch = "wasm32")]
wasm_bindgen_test::wasm_bindgen_test_configure!(run_in_browser);

#[cfg_attr(target_arch = "wasm32", wasm_bindgen_test::wasm_bindgen_test)]
#[cfg_attr(not(target_arch = "wasm32"), test)]
fn score_tempo_survives_mscz_in_wasm() {
    let abc = "X:1\nT:Tempo test\nM:4/4\nL:1/4\nQ:1/4=100\nK:C\n\"C\"C4|";
    let source = acorde_wasm::parse_abc(abc).unwrap();
    for score in [
        source.clone(),
        acorde_wasm::transpose_score(&source, 2).unwrap(),
    ] {
        let restored =
            acorde_wasm::parse_mscz(&acorde_wasm::serialize_mscz(&score).unwrap()).unwrap();
        let parsed: acorde_core::Score = serde_json::from_str(&restored).unwrap();
        assert_eq!(parsed.settings.tempo_bpm, 100);
        let events = |json: &str| -> Vec<(u8, f64, f64)> {
            let events: Vec<acorde_core::PlaybackEvent> =
                serde_json::from_str(&acorde_wasm::to_playback_events_ex(json, "{}").unwrap())
                    .unwrap();
            events
                .into_iter()
                .map(|e| (e.pitch_midi, e.time_secs, e.duration_secs))
                .collect()
        };
        assert_eq!(events(&restored), events(&score));
        assert_eq!(events(&restored)[0].2, 2.4);
    }
}
