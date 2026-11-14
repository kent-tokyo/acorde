#![cfg(not(all(
    feature = "playback-only",
    not(any(feature = "full", feature = "viewer"))
)))]

use serde_json::{Value, json};

#[cfg(target_arch = "wasm32")]
wasm_bindgen_test::wasm_bindgen_test_configure!(run_in_browser);

fn events(score: &str, staff: Option<usize>) -> Vec<(u8, u64, u64)> {
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
                event.pitch_midi,
                (event.time_beats * 480.0).round() as u64,
                (event.duration_beats * 480.0).round() as u64,
            )
        })
        .collect()
}

#[cfg_attr(target_arch = "wasm32", wasm_bindgen_test::wasm_bindgen_test)]
#[cfg_attr(not(target_arch = "wasm32"), test)]
fn melodic_sections_are_exported_and_match_native_arrangement() {
    let input = acorde_wasm::parse_abc("X:1\nM:4/4\nL:1/4\nK:C\ne4 | e4 | e4 | e4 |").unwrap();
    let accompaniment = json!({
        "chords":[{"measure_index":0,"offset_beats":0.0,"chord":{"root":"C","kind":"major"}}],
        "bass_mode":"free-bass", "default_pattern":"held-bass-chord",
        "sections":[{"first_measure":3,"last_measure":3,"pattern":null}]
    });
    let melodic = json!({
        "default_voice":"unchanged",
        "sections":[
            {"first_measure":1,"last_measure":1,"voice":"third-below"},
            {"first_measure":2,"last_measure":2,"voice":"sixth-below"},
            {"first_measure":3,"last_measure":3,"voice":"octave-below"}
        ]
    });
    let web: acorde_core::MelodicSectionedAccordionResult = serde_json::from_str(
        &acorde_wasm::arrange_sections_with_melodic_voices(
            &input,
            &accompaniment.to_string(),
            &melodic.to_string(),
        )
        .unwrap(),
    )
    .unwrap();
    let source: acorde_core::Score = serde_json::from_str(&input).unwrap();
    let native = acorde_core::arrange_sections_with_melodic_voices(
        &source,
        serde_json::from_value(accompaniment).unwrap(),
        serde_json::from_value(melodic).unwrap(),
    );
    assert!(
        web.arrangement.diagnostics.is_empty(),
        "{:?}",
        web.arrangement.diagnostics
    );
    assert_eq!(web.resolved_melodic_voices, native.resolved_melodic_voices);
    assert_eq!(
        web.arrangement.generated_notes,
        native.arrangement.generated_notes
    );
    let output = serde_json::to_string(web.arrangement.score.as_ref().unwrap()).unwrap();
    let native_output = serde_json::to_string(native.arrangement.score.as_ref().unwrap()).unwrap();
    assert_eq!(events(&output, None), events(&native_output, None));
    assert_eq!(
        events(&input, Some(0)).len() + 3,
        events(&output, Some(0)).len()
    );
    #[cfg(feature = "musicxml")]
    {
        let xml = acorde_wasm::serialize_musicxml(&output).unwrap();
        let parsed = acorde_wasm::parse_musicxml(&xml).unwrap();
        assert_eq!(events(&output, None), events(&parsed, None));
    }
    let mut invalid = serde_json::to_value(&web.resolved_melodic_schedule).unwrap();
    invalid["sections"][1]["first_measure"] = json!(1);
    let rejected: Value = serde_json::from_str(
        &acorde_wasm::arrange_sections_with_melodic_voices(
            &input,
            &serde_json::to_string(&web.arrangement.resolved_options).unwrap(),
            &invalid.to_string(),
        )
        .unwrap(),
    )
    .unwrap();
    assert!(rejected["arrangement"]["score"].is_null());
    assert_eq!(
        rejected["arrangement"]["diagnostics"][0]["code"],
        "overlapping-melodic-sections"
    );
}

#[cfg_attr(target_arch = "wasm32", wasm_bindgen_test::wasm_bindgen_test)]
#[cfg_attr(not(target_arch = "wasm32"), test)]
fn proposed_harmony_supports_sections_silence_and_export_roundtrip() {
    let input =
        acorde_wasm::parse_abc("X:1\nM:4/4\nL:1/4\nK:C\nC E G2 | F A c2 | G B d f | c4 |\n")
            .unwrap();
    let proposals: acorde_core::ChordProposalResult =
        serde_json::from_str(&acorde_wasm::suggest_chord_plans(&input, "{}").unwrap()).unwrap();
    let candidate = proposals
        .candidates
        .iter()
        .find(|candidate| candidate.id == "jazz-color-v2")
        .unwrap();
    let options = json!({
        "chords": candidate.preset_selection.chords,
        "bass_mode":"free-bass", "default_pattern":"held-bass-chord",
        "sections":[
            {"first_measure":1,"last_measure":1,"pattern":"bass-chord-pulse"},
            {"first_measure":2,"last_measure":2,"pattern":"swing-accompaniment"},
            {"first_measure":3,"last_measure":3,"pattern":null}
        ]
    });
    let json = acorde_wasm::arrange_with_sections(&input, &options.to_string()).unwrap();
    let result: acorde_core::SectionedAccordionResult = serde_json::from_str(&json).unwrap();
    let source: acorde_core::Score = serde_json::from_str(&input).unwrap();
    let native = acorde_core::arrange_with_sections(
        &source,
        serde_json::from_value(options.clone()).unwrap(),
    );
    assert_eq!(result.resolved_patterns, native.resolved_patterns);
    assert_eq!(result.generated_notes, native.generated_notes);
    assert_eq!(result.source_notes, native.source_notes);
    assert!(result.diagnostics.is_empty());
    assert_eq!(result.notated_swing_measures, vec![2]);
    assert!(result.notated_swing);
    let output = serde_json::to_string(result.score.as_ref().unwrap()).unwrap();
    assert_eq!(events(&input, Some(0)), events(&output, Some(0)));
    assert_eq!(
        events(&output, None),
        events(
            &serde_json::to_string(&native.score.unwrap()).unwrap(),
            None
        )
    );
    assert!(
        events(&output, Some(1))
            .iter()
            .all(|(_, time, _)| *time < 12 * 480)
    );
    assert!(
        acorde_wasm::render_score_svg(&output, "{}")
            .unwrap()
            .contains("<svg")
    );
    let playback: Value =
        serde_json::from_str(&acorde_wasm::to_playback_events_ex(&output, "{}").unwrap()).unwrap();
    assert!(!playback.as_array().unwrap().is_empty());
    let exported = acorde_wasm::serialize_mscz(&output).unwrap();
    let imported = acorde_wasm::parse_mscz(&exported).unwrap();
    assert_eq!(events(&output, None), events(&imported, None));
    #[cfg(feature = "musicxml")]
    {
        let xml = acorde_wasm::serialize_musicxml(&output).unwrap();
        let imported = acorde_wasm::parse_musicxml(&xml).unwrap();
        assert_eq!(events(&output, None), events(&imported, None));
    }
    let mut bad = options;
    bad["sections"][1]["first_measure"] = json!(1);
    let rejected: Value = serde_json::from_str(
        &acorde_wasm::arrange_with_sections(&input, &bad.to_string()).unwrap(),
    )
    .unwrap();
    assert!(rejected["score"].is_null());
    assert_eq!(rejected["diagnostics"][0]["code"], "overlapping-sections");
    assert!(rejected["generated_notes"].as_array().unwrap().is_empty());
}

#[cfg_attr(target_arch = "wasm32", wasm_bindgen_test::wasm_bindgen_test)]
#[cfg_attr(not(target_arch = "wasm32"), test)]
fn proposed_harmony_sections_and_voicing_share_the_exported_score() {
    let input = acorde_wasm::parse_abc(
        "X:1\nM:4/4\nL:1/4\nK:C\nC E G2 | F A c2 | G B d f | c4 | F A c2 | G B d f | c4 |\n",
    )
    .unwrap();
    let source: acorde_core::Score = serde_json::from_str(&input).unwrap();
    let proposals: acorde_core::ChordProposalResult =
        serde_json::from_str(&acorde_wasm::suggest_chord_plans(&input, "{}").unwrap()).unwrap();
    let candidate = proposals
        .candidates
        .iter()
        .find(|candidate| candidate.id == "jazz-color-v2")
        .unwrap();
    let options = json!({
        "chords": candidate.preset_selection.chords,
        "bass_mode":"free-bass", "default_pattern":"held-bass-chord",
        "sections":[
            {"first_measure":1,"last_measure":1,"pattern":"arpeggio"},
            {"first_measure":2,"last_measure":2,"pattern":"swing-accompaniment"},
            {"first_measure":3,"last_measure":3,"pattern":null},
            {"first_measure":4,"last_measure":4,"pattern":"bass-chord-pulse"},
            {"first_measure":5,"last_measure":5,"pattern":"walking-bass"}
        ]
    });
    let result: acorde_core::VoiceLedSectionedAccordionResult = serde_json::from_str(
        &acorde_wasm::arrange_sections_with_voice_leading(&input, &options.to_string(), "{}")
            .unwrap(),
    )
    .unwrap();
    let native = acorde_core::arrange_sections_with_voice_leading(
        &source,
        serde_json::from_value(options.clone()).unwrap(),
        Default::default(),
    );
    assert!(
        result.arrangement.diagnostics.is_empty(),
        "{:?}",
        result.arrangement.diagnostics
    );
    assert_eq!(result.voicings, native.voicings);
    assert_eq!(result.motion_cost, native.motion_cost);
    assert!(result.motion_cost <= result.original_motion_cost);
    assert_eq!(
        result.arrangement.resolved_patterns,
        native.arrangement.resolved_patterns
    );
    assert_eq!(result.arrangement.notated_swing_measures, vec![2]);
    assert!(result.voicings.iter().all(|group| {
        group
            .targets
            .iter()
            .all(|addr| [0, 2, 4, 6].contains(&addr.measure))
    }));
    let output = serde_json::to_string(result.arrangement.score.as_ref().unwrap()).unwrap();
    assert_eq!(events(&input, Some(0)), events(&output, Some(0)));
    assert_eq!(
        events(&output, None),
        events(
            &serde_json::to_string(&native.arrangement.score.unwrap()).unwrap(),
            None
        )
    );
    assert!(
        acorde_wasm::render_score_svg(&output, "{}")
            .unwrap()
            .contains("<svg")
    );
    let playback: Value =
        serde_json::from_str(&acorde_wasm::to_playback_events_ex(&output, "{}").unwrap()).unwrap();
    assert!(!playback.as_array().unwrap().is_empty());
    let imported = acorde_wasm::parse_mscz(&acorde_wasm::serialize_mscz(&output).unwrap()).unwrap();
    assert_eq!(events(&output, None), events(&imported, None));
    #[cfg(feature = "musicxml")]
    {
        let imported =
            acorde_wasm::parse_musicxml(&acorde_wasm::serialize_musicxml(&output).unwrap())
                .unwrap();
        assert_eq!(events(&output, None), events(&imported, None));
    }
    let failed: Value = serde_json::from_str(
        &acorde_wasm::arrange_sections_with_voice_leading(
            &input,
            &options.to_string(),
            r#"{"min_midi":60,"max_midi":60}"#,
        )
        .unwrap(),
    )
    .unwrap();
    assert_eq!(
        failed["arrangement"]["diagnostics"][0]["code"],
        "no-voicing-in-range"
    );
    assert!(failed["arrangement"]["score"].is_null());
    assert!(
        failed["arrangement"]["generated_notes"]
            .as_array()
            .unwrap()
            .is_empty()
    );
    assert!(
        failed["arrangement"]["resolved_patterns"]
            .as_array()
            .unwrap()
            .is_empty()
    );
    assert!(failed["voicings"].as_array().unwrap().is_empty());
}
