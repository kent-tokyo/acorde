#![cfg(not(all(
    feature = "playback-only",
    not(any(feature = "full", feature = "viewer"))
)))]

use serde_json::{Value, json};

#[cfg(target_arch = "wasm32")]
wasm_bindgen_test::wasm_bindgen_test_configure!(run_in_browser);

#[cfg_attr(target_arch = "wasm32", wasm_bindgen_test::wasm_bindgen_test)]
#[cfg_attr(not(target_arch = "wasm32"), test)]
fn proposed_harmony_can_be_voiced_rendered_played_and_exported() {
    let input =
        acorde_wasm::parse_abc("X:1\nM:4/4\nL:1/4\nK:C\n^F2 c2 | G B d f | c4 |\n").unwrap();
    let source: acorde_core::Score = serde_json::from_str(&input).unwrap();
    let proposals: acorde_core::ChordProposalResult =
        serde_json::from_str(&acorde_wasm::suggest_chord_plans(&input, "{}").unwrap()).unwrap();
    let candidate = proposals
        .candidates
        .iter()
        .find(|candidate| candidate.id == "jazz-color-v2")
        .unwrap();
    let layers = json!({
        "chords":candidate.preset_selection.chords, "alternative_chords":null,
        "inversion":null, "accompaniment":"swing-accompaniment",
        "melodic_voice":"unchanged", "swing":"notated-accompaniment-two-to-one", "bass_mode":"free-bass"
    });
    let json = acorde_wasm::arrange_with_voice_leading(&input, &layers.to_string(), "{}").unwrap();
    let result: acorde_core::VoiceLedAccordionResult = serde_json::from_str(&json).unwrap();
    let native = acorde_core::arrange_with_voice_leading(
        &source,
        serde_json::from_value(layers.clone()).unwrap(),
        Default::default(),
    );
    assert_eq!(result.voicings, native.voicings);
    assert_eq!(result.motion_cost, native.motion_cost);
    assert!(result.motion_cost <= result.original_motion_cost);
    assert!(result.arrangement.diagnostics.is_empty());
    assert!(result.arrangement.notated_swing);
    let output = result.arrangement.score.unwrap();
    assert_eq!(
        serde_json::to_value(&output.parts[0].staves[0]).unwrap(),
        serde_json::to_value(&source.parts[0].staves[0]).unwrap()
    );
    let output = serde_json::to_string(&output).unwrap();
    assert!(
        acorde_wasm::render_score_svg(&output, "{}")
            .unwrap()
            .contains("<svg")
    );
    let events: Value =
        serde_json::from_str(&acorde_wasm::to_playback_events_ex(&output, "{}").unwrap()).unwrap();
    assert!(!events.as_array().unwrap().is_empty());
    let exported = acorde_wasm::serialize_mscz(&output).unwrap();
    let imported = acorde_wasm::parse_mscz(&exported).unwrap();
    let pitches = |score: &str| {
        let score: acorde_core::Score = serde_json::from_str(score).unwrap();
        acorde_core::to_playback_events(&score, &Default::default())
            .into_iter()
            .map(|event| {
                (
                    event.pitch_midi,
                    (event.time_beats * 480.0).round() as u64,
                    (event.duration_beats * 480.0).round() as u64,
                )
            })
            .collect::<Vec<_>>()
    };
    assert_eq!(pitches(&output), pitches(&imported));
    let failed: Value = serde_json::from_str(
        &acorde_wasm::arrange_with_voice_leading(
            &input,
            &layers.to_string(),
            r#"{"min_midi":60,"max_midi":60}"#,
        )
        .unwrap(),
    )
    .unwrap();
    assert!(failed["arrangement"]["score"].is_null());
    assert!(failed["voicings"].as_array().unwrap().is_empty());
}

fn events(score: &str, staff: Option<usize>) -> Vec<(i32, u64, u64)> {
    let score: acorde_core::Score = serde_json::from_str(score).unwrap();
    acorde_core::to_playback_events(&score, &Default::default())
        .into_iter()
        .filter(|event| {
            staff.is_none_or(|staff| {
                event
                    .source
                    .as_ref()
                    .is_some_and(|addr| addr.staff == staff)
            })
        })
        .map(|event| {
            (
                event.pitch_midi_cents,
                (event.time_beats * 480.0).round() as u64,
                (event.duration_beats * 480.0).round() as u64,
            )
        })
        .collect()
}

#[cfg_attr(target_arch = "wasm32", wasm_bindgen_test::wasm_bindgen_test)]
#[cfg_attr(not(target_arch = "wasm32"), test)]
fn melody_clearance_applies_to_both_voicing_apis_and_survives_export() {
    let input = acorde_wasm::parse_abc("X:1\nM:4/4\nL:1/4\nK:C\nc2 E2 | G B G B | c4 |\n").unwrap();
    let source: acorde_core::Score = serde_json::from_str(&input).unwrap();
    let chords = json!([
        {"measure_index":0,"offset_beats":0,"chord":{"root":"C","kind":"major"}},
        {"measure_index":1,"offset_beats":0,"chord":{"root":"G","kind":"dominant"}},
        {"measure_index":2,"offset_beats":0,"chord":{"root":"C","kind":"major"}}
    ]);
    let layers = json!({"chords":chords,"alternative_chords":null,"inversion":null,
        "accompaniment":"held-bass-chord","melodic_voice":"unchanged","swing":"straight","bass_mode":"free-bass"});
    let options = json!({"min_melody_gap_semitones":3});
    let result: acorde_core::VoiceLedAccordionResult = serde_json::from_str(
        &acorde_wasm::arrange_with_voice_leading(&input, &layers.to_string(), &options.to_string())
            .unwrap(),
    )
    .unwrap();
    let native = acorde_core::arrange_with_voice_leading(
        &source,
        serde_json::from_value(layers.clone()).unwrap(),
        serde_json::from_value(options.clone()).unwrap(),
    );
    assert!(
        result.arrangement.diagnostics.is_empty(),
        "{:?}",
        result.arrangement.diagnostics
    );
    assert_eq!(result.voicings, native.voicings);
    assert_eq!(result.motion_cost, native.motion_cost);
    assert_eq!(result.voicings[0].melody_ceiling_midi, Some(61));
    let sections = json!({"chords":chords,"bass_mode":"free-bass","default_pattern":"held-bass-chord",
        "sections":[{"first_measure":1,"last_measure":1,"pattern":"swing-accompaniment"},
                    {"first_measure":2,"last_measure":2,"pattern":null}]});
    let sectioned: acorde_core::VoiceLedSectionedAccordionResult = serde_json::from_str(
        &acorde_wasm::arrange_sections_with_voice_leading(
            &input,
            &sections.to_string(),
            &options.to_string(),
        )
        .unwrap(),
    )
    .unwrap();
    assert!(sectioned.arrangement.diagnostics.is_empty());
    assert_eq!(sectioned.voicings[0].melody_ceiling_midi, Some(61));
    for score in [
        result.arrangement.score.unwrap(),
        sectioned.arrangement.score.unwrap(),
    ] {
        let output = serde_json::to_string(&score).unwrap();
        assert_eq!(events(&input, Some(0)), events(&output, Some(0)));
        let right = events(&output, Some(0));
        for (pitch, start, duration) in events(&output, Some(1))
            .iter()
            .copied()
            .filter(|event| event.0 >= 4800)
        {
            for &(melody, from, length) in &right {
                if start < from + length && from < start + duration {
                    assert!(pitch + 300 <= melody);
                }
            }
        }
        assert!(
            acorde_wasm::render_score_svg(&output, "{}")
                .unwrap()
                .contains("<svg")
        );
        let imported =
            acorde_wasm::parse_mscz(&acorde_wasm::serialize_mscz(&output).unwrap()).unwrap();
        assert_eq!(events(&output, None), events(&imported, None));
        #[cfg(feature = "musicxml")]
        {
            let imported =
                acorde_wasm::parse_musicxml(&acorde_wasm::serialize_musicxml(&output).unwrap())
                    .unwrap();
            assert_eq!(events(&output, None), events(&imported, None));
        }
    }
    let failed: Value = serde_json::from_str(
        &acorde_wasm::arrange_sections_with_voice_leading(
            &input,
            &sections.to_string(),
            r#"{"min_melody_gap_semitones":24}"#,
        )
        .unwrap(),
    )
    .unwrap();
    assert_eq!(
        failed["arrangement"]["diagnostics"][0]["code"],
        "no-voicing-below-melody"
    );
    assert!(failed["arrangement"]["score"].is_null());
    assert!(failed["voicings"].as_array().unwrap().is_empty());
    let old: acorde_core::VoiceLedAccordionResult = serde_json::from_str(
        &acorde_wasm::arrange_with_voice_leading(&input, &layers.to_string(), "{}").unwrap(),
    )
    .unwrap();
    let disabled: acorde_core::VoiceLedAccordionResult = serde_json::from_str(
        &acorde_wasm::arrange_with_voice_leading(
            &input,
            &layers.to_string(),
            r#"{"min_melody_gap_semitones":null}"#,
        )
        .unwrap(),
    )
    .unwrap();
    assert_eq!(old.voicings, disabled.voicings);
    assert_eq!(old.resolved_voicing, disabled.resolved_voicing);
    assert_eq!(
        events(
            &serde_json::to_string(&old.arrangement.score.unwrap()).unwrap(),
            None
        ),
        events(
            &serde_json::to_string(&disabled.arrangement.score.unwrap()).unwrap(),
            None
        )
    );
}
