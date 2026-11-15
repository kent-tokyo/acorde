#![cfg(not(all(
    feature = "playback-only",
    not(any(feature = "full", feature = "viewer"))
)))]

use acorde_core::{
    AccordionPattern, AccordionPlanOptions, AccordionPlanSectionChoices,
    AccordionPlanShuffleRequest, AccordionPlanShuffleResult, Barline, Score,
};

#[cfg(target_arch = "wasm32")]
wasm_bindgen_test::wasm_bindgen_test_configure!(run_in_browser);

const SOURCE: &str = include_str!("../../../tests/fixtures/arrangement_plan_eight_bars.abc");
const PLAN: &str = include_str!("../../../tests/fixtures/arrangement_plan_eight_bars.json");

fn request() -> AccordionPlanShuffleRequest {
    let plan: AccordionPlanOptions = serde_json::from_str(PLAN).unwrap();
    let mut sections: Vec<_> = plan
        .sections
        .into_iter()
        .map(|s| AccordionPlanSectionChoices {
            start_measure: s.start_measure,
            end_measure: s.end_measure,
            allowed_accompaniments: vec![s.accompaniment],
            allowed_melodic_voices: vec![s.melodic_voice],
        })
        .collect();
    sections[0]
        .allowed_accompaniments
        .push(Some(AccordionPattern::HeldBassChord));
    sections[1]
        .allowed_accompaniments
        .push(Some(AccordionPattern::HeldBassChord));
    AccordionPlanShuffleRequest {
        chords: plan.chords,
        bass_mode: plan.bass_mode,
        sections,
        seed: "1234567890123456789".into(),
        max_attempts: 64,
    }
}

fn musical_events(score: &Score) -> Vec<(u8, f64, f64, f64, f64)> {
    acorde_core::to_playback_events(score, &Default::default())
        .into_iter()
        .map(|e| {
            (
                e.pitch_midi,
                e.time_beats,
                e.duration_beats,
                e.time_secs,
                e.duration_secs,
            )
        })
        .collect()
}

#[cfg_attr(target_arch = "wasm32", wasm_bindgen_test::wasm_bindgen_test)]
#[cfg_attr(not(target_arch = "wasm32"), test)]
fn seeded_web_plan_matches_rust_and_roundtrips() {
    let input = acorde_wasm::parse_abc(SOURCE).unwrap();
    let score: Score = serde_json::from_str(&input).unwrap();
    let req = request();
    let web: AccordionPlanShuffleResult = serde_json::from_str(
        &acorde_wasm::shuffle_accordion_plan(&input, &serde_json::to_string(&req).unwrap())
            .unwrap(),
    )
    .unwrap();
    let native = acorde_core::shuffle_accordion_plan(&score, req);
    assert!(web.plan.score.is_some(), "{:?}", web.plan.diagnostics);
    assert_eq!(web.selected_pool_index, native.selected_pool_index);
    assert_eq!(web.plan.applied_sections, native.plan.applied_sections);
    assert_eq!(web.plan.generated_notes, native.plan.generated_notes);
    let output = web.plan.score.as_ref().unwrap();
    assert_eq!(
        musical_events(output),
        musical_events(native.plan.score.as_ref().unwrap())
    );
    let replay: acorde_core::AccordionPlanResult = serde_json::from_str(
        &acorde_wasm::arrange_from_resolved_plan(
            &acorde_wasm::parse_abc(SOURCE).unwrap(),
            &serde_json::to_string(web.plan.resolved_config.as_ref().unwrap()).unwrap(),
        )
        .unwrap(),
    )
    .unwrap();
    assert_eq!(
        musical_events(output),
        musical_events(replay.score.as_ref().unwrap())
    );
    assert_eq!(web.plan.generated_notes, replay.generated_notes);
    let output_json = serde_json::to_string(output).unwrap();
    let svg = acorde_wasm::render_score_svg(&output_json, "{\"interactive\":true}").unwrap();
    assert!(svg.contains("data-note-addr=\"0:0:4:1:0\""));
    let browser_events: serde_json::Value =
        serde_json::from_str(&acorde_wasm::to_playback_events_ex(&output_json, "{}").unwrap())
            .unwrap();
    assert_eq!(
        browser_events,
        serde_json::to_value(acorde_core::to_playback_events(output, &Default::default())).unwrap()
    );
    let restored: Score = serde_json::from_str(
        &acorde_wasm::parse_mscz(&acorde_wasm::serialize_mscz(&output_json).unwrap()).unwrap(),
    )
    .unwrap();
    assert_eq!(musical_events(output), musical_events(&restored));
}

#[cfg_attr(target_arch = "wasm32", wasm_bindgen_test::wasm_bindgen_test)]
#[cfg_attr(not(target_arch = "wasm32"), test)]
fn repeats_and_endings_use_written_choices() {
    use acorde_core::model::score::VoltaBracket;
    let mut score: Score = serde_json::from_str(&acorde_wasm::parse_abc(SOURCE).unwrap()).unwrap();
    let bars = &mut score.parts[0].staves[0].measures;
    bars[0].barline_left = Barline::RepeatStart;
    bars[6].barline_right = Barline::RepeatEnd;
    bars[6].volta = Some(VoltaBracket {
        number: 1,
        kind: "begin_end".into(),
        numbers: Vec::new(),
    });
    bars[7].volta = Some(VoltaBracket {
        number: 2,
        kind: "begin_end".into(),
        numbers: Vec::new(),
    });
    bars[4].tempo = Some(150);
    let input = serde_json::to_string(&score).unwrap();
    let result: AccordionPlanShuffleResult = serde_json::from_str(
        &acorde_wasm::shuffle_accordion_plan(&input, &serde_json::to_string(&request()).unwrap())
            .unwrap(),
    )
    .unwrap();
    assert!(result.plan.score.is_some(), "{:?}", result.plan.diagnostics);
    let output = result.plan.score.unwrap();
    assert_eq!(
        acorde_core::model::repeat::measure_sequence(&output),
        vec![0, 1, 2, 3, 4, 5, 6, 0, 1, 2, 3, 4, 5, 7]
    );
    let generated_in_bar_four: Vec<_> =
        acorde_core::to_playback_events(&output, &Default::default())
            .into_iter()
            .filter(|e| {
                e.source
                    .as_ref()
                    .is_some_and(|a| a.measure == 4 && a.staff == 1)
            })
            .map(|e| e.time_beats)
            .collect();
    assert!(!generated_in_bar_four.is_empty());
    assert!(
        generated_in_bar_four
            .iter()
            .any(|beat| *beat >= 16.0 && *beat < 20.0)
    );
    assert!(
        generated_in_bar_four
            .iter()
            .any(|beat| *beat >= 44.0 && *beat < 48.0)
    );
}
