#![cfg(not(all(
    feature = "playback-only",
    not(any(feature = "full", feature = "viewer"))
)))]
use serde_json::{Value, json};
#[cfg(target_arch = "wasm32")]
wasm_bindgen_test::wasm_bindgen_test_configure!(run_in_browser);
#[cfg_attr(target_arch = "wasm32", wasm_bindgen_test::wasm_bindgen_test)]
#[cfg_attr(not(target_arch = "wasm32"), test)]
fn legacy_voicing_bounds_phase_and_replay_match_native() {
    let source = acorde_wasm::parse_abc("X:1\nM:4/4\nL:1/4\nK:C\nC D E C|]").unwrap();
    let request = json!({"contract_version":1,"pattern":"held","midi_program":21,"regions":[{"measure":0,"start_tick":0,"end_tick":128,"bass_midi":36,"chord_midi":[48,52,55],"chord":{"root":"C","kind":"major"},"phase_beats":0}]});
    let result: Value = serde_json::from_str(
        &acorde_wasm::arrange_legacy_accompaniment(&source, &request.to_string()).unwrap(),
    )
    .unwrap();
    let native = acorde_core::arrange_legacy_accompaniment(
        &serde_json::from_str(&source).unwrap(),
        &serde_json::from_value(request).unwrap(),
    )
    .unwrap();
    let web_score: acorde_core::Score = serde_json::from_value(result["score"].clone()).unwrap();
    let pitches = |score: &acorde_core::Score| {
        let mut v = acorde_core::to_playback_events(score, &Default::default())
            .into_iter()
            .map(|e| (e.pitch_midi, e.time_beats as u32, e.duration_beats as u32))
            .collect::<Vec<_>>();
        v.sort_unstable();
        v
    };
    assert_eq!(pitches(&web_score), pitches(&native.score));
    assert!(pitches(&web_score).contains(&(36, 0, 4)));
    let replay: Value = serde_json::from_str(
        &acorde_wasm::replay_legacy_accompaniment(&source, &result["resolved"].to_string())
            .unwrap(),
    )
    .unwrap();
    let replay_score: acorde_core::Score = serde_json::from_value(replay["score"].clone()).unwrap();
    assert_eq!(pitches(&web_score), pitches(&replay_score));
    let abc = acorde_wasm::serialize_abc(&result["score"].to_string()).unwrap();
    let reparsed: acorde_core::Score =
        serde_json::from_str(&acorde_wasm::parse_abc(&abc).unwrap()).unwrap();
    assert_eq!(pitches(&web_score), pitches(&reparsed));
}
