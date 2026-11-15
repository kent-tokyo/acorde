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
fn form_contract_matches_native_and_replays_through_json() {
    let source = fixture::source();
    let request = fixture::request();
    let native = acorde_core::arrange_ensemble_form(&source, &request);
    assert!(native.diagnostics.is_empty(), "{:?}", native.diagnostics);
    let source_json = serde_json::to_string(&source).unwrap();
    let result: acorde_core::EnsembleFormResult = serde_json::from_str(
        &acorde_wasm::arrange_ensemble_form(
            &source_json,
            &serde_json::to_string(&request).unwrap(),
        )
        .unwrap(),
    )
    .unwrap();
    assert!(result.diagnostics.is_empty());
    assert_eq!(native.measures, result.measures);
    assert_eq!(
        fixture::events(native.score.as_ref().unwrap()),
        fixture::events(result.score.as_ref().unwrap())
    );
    let replay: acorde_core::EnsembleFormResult = serde_json::from_str(
        &acorde_wasm::replay_ensemble_form(
            &source_json,
            &serde_json::to_string(result.resolved.as_ref().unwrap()).unwrap(),
        )
        .unwrap(),
    )
    .unwrap();
    assert_eq!(
        fixture::events(result.score.as_ref().unwrap()),
        fixture::events(replay.score.as_ref().unwrap())
    );
    let mut invalid = request;
    invalid.sections[1].start_measure = 0;
    let rejected: acorde_core::EnsembleFormResult = serde_json::from_str(
        &acorde_wasm::arrange_ensemble_form(
            &source_json,
            &serde_json::to_string(&invalid).unwrap(),
        )
        .unwrap(),
    )
    .unwrap();
    assert!(rejected.score.is_none());
    assert!(rejected.resolved.is_none());
}

#[cfg_attr(target_arch = "wasm32", wasm_bindgen_test::wasm_bindgen_test)]
#[cfg_attr(not(target_arch = "wasm32"), test)]
fn second_repeat_visit_has_a_separate_replayable_performance() {
    let signature = |events: &[acorde_core::PlaybackEvent]| {
        events
            .iter()
            .map(|event| {
                (
                    event.source.clone(),
                    event.part_index,
                    (event.time_beats * 1_000_000.0).round() as i64,
                    (event.time_secs * 1_000_000.0).round() as i64,
                    (event.duration_beats * 1_000_000.0).round() as i64,
                    (event.duration_secs * 1_000_000.0).round() as i64,
                    event.pitch_midi_cents,
                    event.velocity,
                    event.channel,
                    event.program,
                )
            })
            .collect::<Vec<_>>()
    };
    let mut source = fixture::source();
    source.parts[0].staves[0].measures[0].barline_left = acorde_core::Barline::RepeatStart;
    source.parts[0].staves[0].measures[7].barline_right = acorde_core::Barline::RepeatEnd;
    let base = fixture::request();
    let mut alternative = base.clone();
    alternative.sections[0].harmony.as_mut().unwrap().pattern =
        acorde_core::ChordalPadPattern::ArpeggioUpDown;
    let request = acorde_core::EnsembleFormVisitsRequest {
        base,
        variations: vec![acorde_core::FormVisitVariation {
            visit_index: 8,
            alternative,
        }],
    };
    let source_json = serde_json::to_string(&source).unwrap();
    let native = acorde_core::arrange_ensemble_form_visits(&source, &request);
    let browser: acorde_core::EnsembleFormVisitsResult = serde_json::from_str(
        &acorde_wasm::arrange_ensemble_form_visits(
            &source_json,
            &serde_json::to_string(&request).unwrap(),
        )
        .unwrap(),
    )
    .unwrap();
    assert!(browser.diagnostics.is_empty(), "{:?}", browser.diagnostics);
    assert_eq!(signature(&native.events), signature(&browser.events));
    let replay: acorde_core::EnsembleFormVisitsResult = serde_json::from_str(
        &acorde_wasm::replay_ensemble_form_visits(
            &source_json,
            &serde_json::to_string(browser.resolved.as_ref().unwrap()).unwrap(),
        )
        .unwrap(),
    )
    .unwrap();
    assert!(replay.diagnostics.is_empty());
    assert_eq!(signature(&browser.events), signature(&replay.events));
}
