#![cfg(not(all(
    feature = "playback-only",
    not(any(feature = "full", feature = "viewer"))
)))]

#[path = "../../../tests/fixtures/arrangement/form_development_v2.rs"]
mod fixture;

use acorde_core::{
    Barline, InstrumentDefinition, InstrumentPartAssignment, InstrumentPlanRequest,
    InstrumentPlanResult, InstrumentRange, PartTechnique, arrange_form_inner_voices_performance,
    plan_instruments,
};

#[cfg(target_arch = "wasm32")]
wasm_bindgen_test::wasm_bindgen_test_configure!(run_in_browser);

#[cfg_attr(target_arch = "wasm32", wasm_bindgen_test::wasm_bindgen_test)]
#[cfg_attr(not(target_arch = "wasm32"), test)]
fn instrument_plan_replays_the_same_repeat_aware_result_in_json() {
    let mut source = fixture::source();
    source.parts[0].staves[0].measures[0].barline_left = Barline::RepeatStart;
    source.parts[0].staves[0].measures[7].barline_right = Barline::RepeatEnd;
    let score =
        arrange_form_inner_voices_performance(&source, &fixture::form_inner_voice_request())
            .unwrap()
            .performance_score;
    let mut wind = InstrumentDefinition::new("wind-melody", "Wind melody");
    wind.midi_program = 73;
    wind.sounding_range = Some(InstrumentRange {
        lowest: 84,
        highest: 91,
    });
    let request = InstrumentPlanRequest {
        assignments: vec![InstrumentPartAssignment {
            part_index: 0,
            instrument: wind,
            sounding_range: InstrumentRange {
                lowest: 84,
                highest: 91,
            },
            technique: PartTechnique::WindLine {
                max_continuous_beats: 16.0,
                min_breath_rest_beats: 1.0,
            },
        }],
    };
    let native = plan_instruments(&score, &request).unwrap();
    let score_json = serde_json::to_string(&score).unwrap();
    let browser: InstrumentPlanResult = serde_json::from_str(
        &acorde_wasm::plan_instruments(&score_json, &serde_json::to_string(&request).unwrap())
            .unwrap(),
    )
    .unwrap();
    assert_eq!(native.reports, browser.reports);
    assert_eq!(
        fixture::events(&native.score),
        fixture::events(&browser.score)
    );
    let replay: InstrumentPlanResult = serde_json::from_str(
        &acorde_wasm::replay_instrument_plan(
            &score_json,
            &serde_json::to_string(&browser.resolved).unwrap(),
        )
        .unwrap(),
    )
    .unwrap();
    assert_eq!(browser.reports, replay.reports);
    let mut too_short = request;
    too_short.assignments[0].technique = PartTechnique::WindLine {
        max_continuous_beats: 2.0,
        min_breath_rest_beats: 1.0,
    };
    assert!(plan_instruments(&score, &too_short).is_err());
    #[cfg(target_arch = "wasm32")]
    assert!(
        acorde_wasm::plan_instruments(&score_json, &serde_json::to_string(&too_short).unwrap())
            .is_err()
    );
}
