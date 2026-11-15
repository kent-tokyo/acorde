#![cfg(not(all(
    feature = "playback-only",
    not(any(feature = "full", feature = "viewer"))
)))]

use serde_json::{Value, json};

#[cfg(target_arch = "wasm32")]
wasm_bindgen_test::wasm_bindgen_test_configure!(run_in_browser);

fn playback(score_json: &str) -> Vec<(u8, u64, usize)> {
    let score: acorde_core::Score = serde_json::from_str(score_json).unwrap();
    acorde_core::to_playback_events(&score, &Default::default())
        .into_iter()
        .filter_map(|event| {
            event.source.map(|source| {
                (
                    event.pitch_midi,
                    (event.time_beats * 480.0).round() as u64,
                    source.part,
                )
            })
        })
        .collect()
}

#[cfg_attr(target_arch = "wasm32", wasm_bindgen_test::wasm_bindgen_test)]
#[cfg_attr(not(target_arch = "wasm32"), test)]
fn independent_bass_matches_native_and_survives_formats() {
    let source: acorde_core::Score =
        serde_json::from_str(&acorde_wasm::parse_abc("X:1\nM:4/4\nL:1/4\nK:C\nc4 | d4 |").unwrap())
            .unwrap();
    let input = serde_json::to_string(&source).unwrap();
    let options = json!({
        "chords":[
            {"measure_index":0,"offset_beats":0.0,"chord":{"root":"C","kind":"major"}},
            {"measure_index":1,"offset_beats":0.0,"chord":{"root":"F","kind":"major"}}
        ],
        "pattern":"approach", "midi_program":32,
        "low_midi":28, "high_midi":60, "center_midi":40
    });
    let web: acorde_core::BassLineResult = serde_json::from_str(
        &acorde_wasm::arrange_bass_line(&input, &options.to_string()).unwrap(),
    )
    .unwrap();
    let native =
        acorde_core::arrange_bass_line(&source, serde_json::from_value(options.clone()).unwrap());
    assert!(web.diagnostics.is_empty(), "{:?}", web.diagnostics);
    assert_eq!(web.motion_report, native.motion_report);
    let generated = serde_json::to_string(web.score.as_ref().unwrap()).unwrap();
    let expected = serde_json::to_string(native.score.as_ref().unwrap()).unwrap();
    assert_eq!(playback(&generated), playback(&expected));
    assert_eq!(
        serde_json::to_value(&web.score.as_ref().unwrap().parts[0]).unwrap(),
        serde_json::to_value(&source.parts[0]).unwrap()
    );
    assert!(
        acorde_wasm::render_score_svg(&generated, "{}")
            .unwrap()
            .contains("<svg")
    );
    let mscz = acorde_wasm::serialize_mscz(&generated).unwrap();
    assert_eq!(
        playback(&generated),
        playback(&acorde_wasm::parse_mscz(&mscz).unwrap())
    );
    #[cfg(feature = "musicxml")]
    {
        let xml = acorde_wasm::serialize_musicxml(&generated).unwrap();
        assert_eq!(
            playback(&generated),
            playback(&acorde_wasm::parse_musicxml(&xml).unwrap())
        );
    }
    let mut invalid = options;
    invalid["chords"][0]["offset_beats"] = json!(0.015625);
    let rejected: Value = serde_json::from_str(
        &acorde_wasm::arrange_bass_line(&input, &invalid.to_string()).unwrap(),
    )
    .unwrap();
    assert!(rejected["score"].is_null());
    assert!(rejected["motion_report"].is_null());
    assert_eq!(rejected["diagnostics"][0]["code"], "chord-position");
}
