//! Compare browser bindings with recorded native release executions, not with
//! another call to core compiled inside the same WASM module.
#![cfg(not(all(
    feature = "playback-only",
    not(any(feature = "full", feature = "viewer"))
)))]

use serde_json::{Value, json};

mod support;

#[cfg(target_arch = "wasm32")]
wasm_bindgen_test::wasm_bindgen_test_configure!(run_in_browser);

fn corpus() -> Value {
    let value: Value = serde_json::from_str(include_str!(
        "../../../tests/fixtures/arrangement/stage45-native-parity.json"
    ))
    .unwrap();
    assert_eq!(value["schema_version"], 1);
    assert_eq!(value["cases"].as_array().unwrap().len(), 14);
    value
}

fn decoded(result: Result<String, wasm_bindgen::JsValue>) -> Value {
    serde_json::from_str(&result.unwrap()).unwrap()
}

fn compare(expected: &Value, actual: &Value, id: &str, field: &str) {
    // Avoid printing megabytes of serialized scores on a failing assertion.
    assert!(expected == actual, "{id}: native {field} mismatch");
}

fn check_score(source: &Value, score: &Value, expected: &Value, id: &str) {
    let source_parts = source["parts"].as_array().unwrap();
    let parts = score["parts"].as_array().unwrap();
    assert!(parts.len() >= source_parts.len());
    compare(
        &json!(source_parts),
        &json!(&parts[..source_parts.len()]),
        id,
        "original parts",
    );
    let events = decoded(acorde_wasm::to_playback_events_ex(&score.to_string(), "{}"));
    compare(expected, &events, id, "playback events");
}

fn target_contract(candidate: &Value) -> Value {
    json!({"resolved":candidate["resolved"],"resolutions":candidate["resolutions"],
        "quality":candidate["quality"],"changed_chord_entries":candidate["changed_chord_entries"]})
}

#[cfg_attr(target_arch = "wasm32", wasm_bindgen_test::wasm_bindgen_test)]
#[cfg_attr(not(target_arch = "wasm32"), test)]
fn target_eight_sixteen_match_recorded_native_events_and_choices() {
    let data = corpus();
    let mut count = 0;
    for case in data["cases"].as_array().unwrap() {
        let id = case["id"].as_str().unwrap();
        if !id.starts_with("target-") {
            continue;
        }
        count += 1;
        let source = &data["sources"][case["source"].as_str().unwrap()];
        let source_json = source.to_string();
        let result = decoded(acorde_wasm::suggest_phrase_harmony_targets(
            &source_json,
            &case["request"].to_string(),
        ));
        assert_eq!(result["complete"], true, "{id}");
        let candidates = result["candidates"].as_array().unwrap();
        assert_eq!(
            candidates.len(),
            case["contracts"].as_array().unwrap().len()
        );
        for (i, candidate) in candidates.iter().enumerate() {
            let mut expected_contract = case["contracts"][i].clone();
            assert!(support::retarget_engine_versions(&mut expected_contract) > 0);
            compare(
                &expected_contract,
                &target_contract(candidate),
                id,
                "choice",
            );
            check_score(
                source,
                &candidate["arrangement"]["score"],
                &case["playback"][i],
                id,
            );
            let replay = decoded(acorde_wasm::replay_phrase_harmony_target(
                &source_json,
                &expected_contract["resolved"].to_string(),
            ));
            compare(
                &expected_contract,
                &target_contract(&replay),
                id,
                "replay choice",
            );
            check_score(
                source,
                &replay["arrangement"]["score"],
                &case["playback"][i],
                id,
            );
        }
    }
    assert_eq!(count, 6);
}

fn rhythm_contract(result: &Value) -> Value {
    json!({"resolved":result["resolved"],"attacks":result["attacks"],"diagnostics":result["diagnostics"]})
}

#[cfg_attr(target_arch = "wasm32", wasm_bindgen_test::wasm_bindgen_test)]
#[cfg_attr(not(target_arch = "wasm32"), test)]
fn rhythm_eight_sixteen_match_recorded_native_events_and_choices() {
    let data = corpus();
    let mut count = 0;
    for case in data["cases"].as_array().unwrap() {
        let id = case["id"].as_str().unwrap();
        if !id.starts_with("rhythm-") {
            continue;
        }
        count += 1;
        let source = &data["sources"][case["source"].as_str().unwrap()];
        let source_json = source.to_string();
        let result = decoded(acorde_wasm::arrange_rhythm_section(
            &source_json,
            &case["request"].to_string(),
        ));
        let mut expected_contract = case["contracts"].clone();
        assert!(support::retarget_engine_versions(&mut expected_contract) > 0);
        compare(&expected_contract, &rhythm_contract(&result), id, "choice");
        check_score(source, &result["score"], &case["playback"], id);
        let replay = decoded(acorde_wasm::replay_rhythm_section(
            &source_json,
            &expected_contract["resolved"].to_string(),
        ));
        compare(
            &expected_contract,
            &rhythm_contract(&replay),
            id,
            "replay choice",
        );
        check_score(source, &replay["score"], &case["playback"], id);
    }
    assert_eq!(count, 8);
}
