#![cfg(not(all(
    feature = "playback-only",
    not(any(feature = "full", feature = "viewer"))
)))]

use serde_json::{Value, json};

#[cfg(target_arch = "wasm32")]
wasm_bindgen_test::wasm_bindgen_test_configure!(run_in_browser);

fn playback(score_json: &str) -> Vec<(u8, u64, u8, usize)> {
    let score: acorde_core::Score = serde_json::from_str(score_json).unwrap();
    acorde_core::to_playback_events(&score, &Default::default())
        .into_iter()
        .filter_map(|event| {
            event.source.map(|source| {
                (
                    event.pitch_midi,
                    (event.time_beats * 480.0).round() as u64,
                    event.velocity,
                    source.staff,
                )
            })
        })
        .collect()
}

#[cfg_attr(target_arch = "wasm32", wasm_bindgen_test::wasm_bindgen_test)]
#[cfg_attr(not(target_arch = "wasm32"), test)]
fn dynamic_sections_match_native_and_roundtrip() {
    let input = acorde_wasm::parse_abc("X:1\nM:4/4\nL:1/4\nK:C\ne4 | e4 | e4 | e4 |").unwrap();
    let accompaniment = json!({
        "chords":[{"measure_index":0,"offset_beats":0.0,"chord":{"root":"C","kind":"major"}}],
        "bass_mode":"free-bass", "default_pattern":"held-bass-chord",
        "sections":[{"first_measure":2,"last_measure":2,"pattern":null}]
    });
    let dynamics = json!({
        "default_dynamic":"Mp",
        "sections":[
            {"first_measure":3,"last_measure":3,"dynamic":"P"},
            {"first_measure":1,"last_measure":1,"dynamic":"F"}
        ]
    });
    let web: acorde_core::DynamicSectionedAccordionResult = serde_json::from_str(
        &acorde_wasm::arrange_sections_with_dynamics(
            &input,
            &accompaniment.to_string(),
            &dynamics.to_string(),
        )
        .unwrap(),
    )
    .unwrap();
    let source: acorde_core::Score = serde_json::from_str(&input).unwrap();
    let native = acorde_core::arrange_sections_with_dynamics(
        &source,
        serde_json::from_value(accompaniment.clone()).unwrap(),
        serde_json::from_value(dynamics).unwrap(),
    );
    assert!(
        web.arrangement.diagnostics.is_empty(),
        "{:?}",
        web.arrangement.diagnostics
    );
    assert_eq!(web.resolved_dynamics, native.resolved_dynamics);
    assert_eq!(web.dynamic_marks, native.dynamic_marks);
    let output = serde_json::to_string(web.arrangement.score.as_ref().unwrap()).unwrap();
    let native_output = serde_json::to_string(native.arrangement.score.as_ref().unwrap()).unwrap();
    assert_eq!(playback(&output), playback(&native_output));
    assert!(
        acorde_wasm::render_score_svg(&output, "{}")
            .unwrap()
            .contains("<svg")
    );
    let mscz = acorde_wasm::serialize_mscz(&output).unwrap();
    let imported = acorde_wasm::parse_mscz(&mscz).unwrap();
    assert_eq!(playback(&output), playback(&imported));
    #[cfg(feature = "musicxml")]
    {
        let xml = acorde_wasm::serialize_musicxml(&output).unwrap();
        let imported = acorde_wasm::parse_musicxml(&xml).unwrap();
        assert_eq!(playback(&output), playback(&imported));
    }
    let invalid = json!({"default_dynamic":"Sfz","sections":[]});
    let rejected: Value = serde_json::from_str(
        &acorde_wasm::arrange_sections_with_dynamics(
            &input,
            &accompaniment.to_string(),
            &invalid.to_string(),
        )
        .unwrap(),
    )
    .unwrap();
    assert!(rejected["arrangement"]["score"].is_null());
    assert_eq!(
        rejected["arrangement"]["diagnostics"][0]["code"],
        "unsupported-dynamic"
    );
}
