#![cfg(not(all(
    feature = "playback-only",
    not(any(feature = "full", feature = "viewer"))
)))]

#[allow(dead_code)]
#[path = "../../../tests/fixtures/arrangement/development_v2.rs"]
mod development_v2;
#[allow(dead_code)]
#[path = "../../../tests/fixtures/arrangement/development_v2_request.rs"]
mod development_v2_request;

#[cfg(target_arch = "wasm32")]
wasm_bindgen_test::wasm_bindgen_test_configure!(run_in_browser);

fn events(score: &acorde_core::Score) -> Vec<(usize, u64, u64, i32, u8)> {
    acorde_core::to_playback_events(score, &Default::default())
        .into_iter()
        .filter_map(|event| {
            event.source.map(|source| {
                (
                    source.part,
                    (event.time_beats * 480.0).round() as u64,
                    (event.duration_beats * 480.0).round() as u64,
                    event.pitch_midi_cents,
                    event.velocity,
                )
            })
        })
        .collect()
}

#[cfg_attr(target_arch = "wasm32", wasm_bindgen_test::wasm_bindgen_test)]
#[cfg_attr(not(target_arch = "wasm32"), test)]
fn cross_plan_forms_match_native_json_and_replay() {
    let corpus: development_v2::Corpus = serde_json::from_str(development_v2::FIXTURE).unwrap();
    let case = &corpus.cases[0];
    let source = development_v2::score(case);
    let mut arrangement = development_v2_request::request(case).unwrap();
    arrangement.count = 3;
    let request = acorde_core::CrossPlanFormRequest {
        arrangement,
        sections: case
            .sections
            .iter()
            .map(|section| acorde_core::FormProposalBoundary {
                label: section.label.clone(),
                start_measure: section.start_measure,
                end_measure_exclusive: section.end_measure_exclusive,
            })
            .collect(),
        phrase_end_measures: Vec::new(),
    };
    let native = acorde_core::suggest_cross_plan_forms(&source, &request).unwrap();
    let source_json = serde_json::to_string(&source).unwrap();
    let browser: acorde_core::CrossPlanFormResult = serde_json::from_str(
        &acorde_wasm::suggest_cross_plan_forms(
            &source_json,
            &serde_json::to_string(&request).unwrap(),
        )
        .unwrap(),
    )
    .unwrap();
    assert_eq!(browser.candidates.len(), 3);
    assert_eq!(browser.rejections, native.rejections);
    for (expected, actual) in native.candidates.iter().zip(&browser.candidates) {
        assert_eq!(expected.id, actual.id);
        assert_eq!(expected.plan, actual.plan);
        assert_eq!(
            expected.axis_distance_from_first,
            actual.axis_distance_from_first
        );
        assert_eq!(
            expected.nearest_selected_distance,
            actual.nearest_selected_distance
        );
        assert_eq!(expected.difference_codes, actual.difference_codes);
        assert_eq!(events(&expected.score), events(&actual.score));
        let replay: acorde_core::Score = serde_json::from_str(
            &acorde_wasm::replay_cross_plan_form(
                &source_json,
                &serde_json::to_string(&actual.resolved).unwrap(),
            )
            .unwrap(),
        )
        .unwrap();
        assert_eq!(events(&replay), events(&actual.score));
    }
}
