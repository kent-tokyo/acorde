#![cfg(not(all(
    feature = "playback-only",
    not(any(feature = "full", feature = "viewer"))
)))]

#[path = "../../../tests/fixtures/arrangement/form_development_v2.rs"]
mod fixture;

#[cfg(target_arch = "wasm32")]
wasm_bindgen_test::wasm_bindgen_test_configure!(run_in_browser);

#[cfg_attr(target_arch = "wasm32", wasm_bindgen_test::wasm_bindgen_test)]
#[cfg_attr(not(target_arch = "wasm32"), test)]
fn selected_developed_forms_match_native_json_and_replay() {
    let source = fixture::source();
    let form = fixture::request();
    let request = acorde_core::EnsembleFormProposalRequest {
        ensemble: form.ensemble,
        sections: form
            .sections
            .into_iter()
            .map(|section| acorde_core::FormProposalBoundary {
                label: section.label,
                start_measure: section.start_measure,
                end_measure_exclusive: section.end_measure_exclusive,
            })
            .collect(),
        phrase_end_measures: form.phrase_end_measures,
        count: 3,
    };
    let native = acorde_core::suggest_developed_form_proposals(&source, &request).unwrap();
    let source_json = serde_json::to_string(&source).unwrap();
    let browser: acorde_core::DevelopedFormProposalResult = serde_json::from_str(
        &acorde_wasm::suggest_developed_form_proposals(
            &source_json,
            &serde_json::to_string(&request).unwrap(),
        )
        .unwrap(),
    )
    .unwrap();
    assert_eq!(browser.candidates.len(), 3);
    assert_eq!(browser.rejections.len(), native.rejections.len());
    for (expected, actual) in native.candidates.iter().zip(&browser.candidates) {
        assert_eq!(expected.id, actual.id);
        assert_eq!(
            fixture::events(&expected.score),
            fixture::events(&actual.score)
        );
        let replay: acorde_core::Score = serde_json::from_str(
            &acorde_wasm::replay_developed_form_proposal(
                &source_json,
                &serde_json::to_string(&actual.resolved).unwrap(),
            )
            .unwrap(),
        )
        .unwrap();
        assert_eq!(fixture::events(&replay), fixture::events(&actual.score));
    }
}
