//! Compare 8/16-bar browser results with a recorded native execution.
#![cfg(not(all(
    feature = "playback-only",
    not(any(feature = "full", feature = "viewer"))
)))]

use acorde_core::{Score, to_playback_events};
use serde_json::{Value, json};

mod support;

#[cfg(target_arch = "wasm32")]
wasm_bindgen_test::wasm_bindgen_test_configure!(run_in_browser);

fn playback_events(score: &Score) -> Value {
    json!(
        to_playback_events(score, &Default::default())
            .into_iter()
            .map(|event| {
                (
                    event.part_index,
                    (event.time_beats * 480.0).round() as u64,
                    (event.duration_beats * 480.0).round() as u64,
                    event.pitch_midi_cents,
                    event.velocity,
                )
            })
            .collect::<Vec<_>>()
    )
}

#[cfg_attr(target_arch = "wasm32", wasm_bindgen_test::wasm_bindgen_test)]
#[cfg_attr(not(target_arch = "wasm32"), test)]
fn integrated_eight_and_sixteen_match_recorded_native_choices_and_events() {
    let receipt: Value = serde_json::from_str(include_str!(
        "../../../tests/fixtures/arrangement/stage678-native-parity.json"
    ))
    .unwrap();
    assert_eq!(receipt["schema_version"], 1);
    let cases = receipt["cases"].as_array().unwrap();
    assert_eq!(cases.len(), 2);
    for case in cases {
        let source_json = case["source"].to_string();
        let request_json = case["request"].to_string();
        let result: acorde_core::IntegratedArrangementResult = serde_json::from_str(
            &acorde_wasm::suggest_integrated_arrangements(&source_json, &request_json).unwrap(),
        )
        .unwrap();
        let expected = case["candidates"].as_array().unwrap();
        assert_eq!(result.candidates.len(), expected.len(), "{}", case["id"]);
        for (candidate, native) in result.candidates.iter().zip(expected) {
            let mut expected_resolved = native["resolved"].clone();
            assert!(support::retarget_engine_versions(&mut expected_resolved) > 0);
            assert!(
                serde_json::to_value(&candidate.resolved).unwrap() == expected_resolved,
                "{}: resolved choice",
                case["id"]
            );
            assert_eq!(
                json!(candidate.difference_codes),
                native["difference_codes"]
            );
            assert_eq!(
                json!(candidate.observed_difference_codes),
                native["observed_difference_codes"],
                "{}: observed role changes",
                case["id"]
            );
            assert_eq!(
                json!(candidate.axis_distance_from_first),
                native["axis_distance_from_first"],
                "{}: observed axes",
                case["id"]
            );
            assert_eq!(
                json!(candidate.nearest_selected_distance),
                native["nearest_selected_distance"],
                "{}: nearest selected distance",
                case["id"]
            );
            assert_eq!(
                playback_events(&candidate.score),
                native["playback_events"],
                "{}: generated playback",
                case["id"]
            );
            let resolved_json = serde_json::to_string(&candidate.resolved).unwrap();
            let _: acorde_core::ResolvedIntegratedArrangement =
                serde_json::from_str(&resolved_json).unwrap();
            let replay: Score = serde_json::from_str(
                &acorde_wasm::replay_integrated_arrangement(&source_json, &resolved_json).unwrap(),
            )
            .unwrap();
            assert_eq!(
                playback_events(&replay),
                native["playback_events"],
                "{}: replay playback",
                case["id"]
            );
        }
    }
}
