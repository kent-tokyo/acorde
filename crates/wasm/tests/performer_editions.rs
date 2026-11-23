#![cfg(not(all(
    feature = "playback-only",
    not(any(feature = "full", feature = "viewer"))
)))]
use serde_json::Value;
#[cfg(target_arch = "wasm32")]
wasm_bindgen_test::wasm_bindgen_test_configure!(run_in_browser);

#[cfg_attr(target_arch = "wasm32", wasm_bindgen_test::wasm_bindgen_test)]
#[cfg_attr(not(target_arch = "wasm32"), test)]
fn editions_and_clean_copies_match_recorded_native_playback() {
    let source = acorde_wasm::parse_mscz(include_bytes!(
        "../../../tests/fixtures/arrangement/editions/lantern-16.mscz"
    ))
    .unwrap();
    let expected: Value = serde_json::from_str(include_str!(
        "../../../tests/fixtures/arrangement/editions/native-playback.json"
    ))
    .unwrap();
    for (mode, config) in [
        (
            "solo",
            include_str!("../../../tests/fixtures/arrangement/editions/solo.json"),
        ),
        (
            "duet",
            include_str!("../../../tests/fixtures/arrangement/editions/duet.json"),
        ),
    ] {
        let result: Value =
            serde_json::from_str(&acorde_wasm::arrange_performer_edition(&source, config).unwrap())
                .unwrap();
        assert!(result["Ok"]["score"].is_object(), "{:?}", result["Err"]);
        let score = result["Ok"]["score"].to_string();
        let engraved = acorde_wasm::engrave_rhythm(&score, false).unwrap();
        for copy in [&score, &engraved] {
            let events: Vec<acorde_core::PlaybackEvent> =
                serde_json::from_str(&acorde_wasm::to_playback_events_ex(copy, "{}").unwrap())
                    .unwrap();
            let mut pitches: Vec<_> = events
                .iter()
                .map(|e| {
                    (
                        e.part_index,
                        (e.time_beats * 480.).round() as u32,
                        (e.duration_beats * 480.).round() as u32,
                        e.pitch_midi_cents,
                        e.velocity,
                    )
                })
                .collect();
            pitches.sort();
            assert_eq!(serde_json::to_value(pitches).unwrap(), expected[mode]);
        }
    }
}

#[allow(dead_code)]
#[path = "../../../tests/fixtures/arrangement/development_v2.rs"]
mod development_v2;

#[cfg_attr(target_arch = "wasm32", wasm_bindgen_test::wasm_bindgen_test)]
#[cfg_attr(not(target_arch = "wasm32"), test)]
fn trio_region_and_replay_match_the_native_32_bar_edition() {
    let corpus: development_v2::Corpus = serde_json::from_str(development_v2::FIXTURE).unwrap();
    let case = corpus.cases.iter().find(|c| c.id == "long-road").unwrap();
    let mut src = development_v2::score(case);
    src.parts[0].name = "Flute".into();
    src.parts[0].short_name = "Fl.".into();
    let bars = &mut src.parts[0].staves[0].measures;
    bars[0].barline_left = acorde_core::Barline::RepeatStart;
    bars[3].barline_right = acorde_core::Barline::RepeatEnd;
    let source = serde_json::to_string(&src).unwrap();
    let data: Value = serde_json::from_str(include_str!(
        "../../../tests/fixtures/arrangement/editions/trio-parity.json"
    ))
    .unwrap();
    let decode = |result: Result<String, wasm_bindgen::JsValue>| {
        serde_json::from_str::<Value>(&result.unwrap()).unwrap()
    };
    let original = decode(acorde_wasm::arrange_ensemble_form(
        &source,
        &data["request"].to_string(),
    ));
    let edited = decode(acorde_wasm::regenerate_ensemble_form_region(
        &source,
        &original["resolved"].to_string(),
        &data["edit"].to_string(),
        &data["locks"].to_string(),
    ));
    for (result, expected) in [
        (&original, &data["original_events"]),
        (&edited, &data["edited_events"]),
    ] {
        assert!(result["score"].is_object(), "{:?}", result["diagnostics"]);
        let replay = decode(acorde_wasm::replay_ensemble_form(
            &source,
            &result["resolved"].to_string(),
        ));
        for score in [&result["score"], &replay["score"]] {
            let events: Vec<acorde_core::PlaybackEvent> = serde_json::from_str(
                &acorde_wasm::to_playback_events_ex(&score.to_string(), "{}").unwrap(),
            )
            .unwrap();
            let mut pitches: Vec<_> = events
                .iter()
                .map(|e| {
                    (
                        e.part_index,
                        (e.time_beats * 480.).round() as u32,
                        (e.duration_beats * 480.).round() as u32,
                        e.pitch_midi_cents,
                        e.velocity,
                    )
                })
                .collect();
            pitches.sort();
            assert_eq!(&serde_json::to_value(pitches).unwrap(), expected);
        }
    }
}
