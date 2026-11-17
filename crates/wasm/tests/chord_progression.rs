#![cfg(not(all(
    feature = "playback-only",
    not(any(feature = "full", feature = "viewer"))
)))]

#[cfg(target_arch = "wasm32")]
wasm_bindgen_test::wasm_bindgen_test_configure!(run_in_browser);

#[cfg_attr(target_arch = "wasm32", wasm_bindgen_test::wasm_bindgen_test)]
#[cfg_attr(not(target_arch = "wasm32"), test)]
fn contextual_harmony_replays_renders_and_plays_through_json() {
    let input =
        acorde_wasm::parse_abc("X:1\nM:4/4\nL:1/4\nK:C\n^F2 c2 | G B d f | c4 |\n").unwrap();
    let source: acorde_core::Score = serde_json::from_str(&input).unwrap();
    let native = acorde_core::suggest_chord_plans(&source, &Default::default());
    let json = acorde_wasm::suggest_chord_plans(&input, "{}").unwrap();
    assert_eq!(
        serde_json::from_str::<serde_json::Value>(&json).unwrap(),
        serde_json::to_value(&native).unwrap()
    );
    let result: acorde_core::ChordProposalResult = serde_json::from_str(&json).unwrap();
    let candidate = result
        .candidates
        .iter()
        .find(|candidate| candidate.id == "jazz-color-v2")
        .unwrap();
    assert_eq!(
        candidate
            .plan
            .entries
            .iter()
            .map(|entry| (entry.chord.root.as_str(), entry.chord.kind.as_str()))
            .collect::<Vec<_>>(),
        [("D", "dominant"), ("G", "dominant"), ("C", "major-seventh"),]
    );
    let resolved = serde_json::to_string(&candidate.resolved).unwrap();
    let replay =
        acorde_wasm::arrange_from_chord_proposal(&input, &resolved, "swing-pulse").unwrap();
    let replay: acorde_core::ChordProposalArrangementResult =
        serde_json::from_str(&replay).unwrap();
    assert!(replay.diagnostics.is_empty(), "{:?}", replay.diagnostics);
    let arranged = replay.arrangement.unwrap().score.unwrap();
    assert_eq!(
        serde_json::to_value(&arranged.parts[0].staves[0]).unwrap(),
        serde_json::to_value(&source.parts[0].staves[0]).unwrap()
    );
    let output = serde_json::to_string(&arranged).unwrap();
    assert!(
        acorde_wasm::render_score_svg(&output, "{}")
            .unwrap()
            .contains("<svg")
    );
    let events: serde_json::Value =
        serde_json::from_str(&acorde_wasm::to_playback_events_ex(&output, "{}").unwrap()).unwrap();
    assert!(!events.as_array().unwrap().is_empty());
    assert!(!acorde_wasm::serialize_mscz(&output).unwrap().is_empty());
}

#[cfg_attr(target_arch = "wasm32", wasm_bindgen_test::wasm_bindgen_test)]
#[cfg_attr(not(target_arch = "wasm32"), test)]
fn contextual_nonchord_option_matches_native_json() {
    let input = acorde_wasm::parse_abc("X:1\nM:4/4\nL:1/4\nK:C\nC D E C |\n").unwrap();
    let request = serde_json::json!({"contextual_nonchord_tones":true});
    let wasm = acorde_wasm::suggest_chord_plans(&input, &request.to_string()).unwrap();
    let native = acorde_core::suggest_chord_plans(
        &serde_json::from_str(&input).unwrap(),
        &serde_json::from_value(request).unwrap(),
    );
    assert_eq!(
        serde_json::from_str::<serde_json::Value>(&wasm).unwrap(),
        serde_json::to_value(&native).unwrap()
    );
    assert!(native.candidates.iter().all(|candidate| {
        candidate.resolved.contextual_nonchord_tones
            && candidate
                .reason_codes
                .contains(&"contextual-nonchord-tones".into())
    }));
}

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
fn locked_harmony_survives_replay_section_voicing_and_export() {
    let input =
        acorde_wasm::parse_abc("X:1\nM:4/4\nL:1/4\nK:C\n\"C/E\"C2 \"G\"G2 | \"C\"c4 |\n").unwrap();
    let extracted: acorde_core::ChordPlanExtractionResult =
        serde_json::from_str(&acorde_wasm::extract_chord_plan(&input, "{}").unwrap()).unwrap();
    let plan = extracted.plan.unwrap();
    assert_eq!(plan.entries.len(), 3);
    let request = serde_json::json!({"source_plan":plan,"max_changed_spans":1});
    let json =
        acorde_wasm::suggest_chord_plans_with_locks(&input, &request.to_string(), "[0,2]").unwrap();
    let native = acorde_core::suggest_chord_plans_with_locks(
        &serde_json::from_str(&input).unwrap(),
        &serde_json::from_value(request.clone()).unwrap(),
        &[0, 2],
    );
    assert_eq!(
        serde_json::from_str::<serde_json::Value>(&json).unwrap(),
        serde_json::to_value(&native).unwrap()
    );
    let result: acorde_core::ChordProposalResult = serde_json::from_str(&json).unwrap();
    assert!(result.diagnostics.is_empty(), "{:?}", result.diagnostics);
    let candidate = result
        .candidates
        .iter()
        .find(|candidate| candidate.id == "jazz-color-v2")
        .unwrap();
    assert_eq!(candidate.plan.entries[0], plan.entries[0]);
    assert_eq!(candidate.plan.entries[2], plan.entries[2]);
    assert_eq!(candidate.plan.entries[1].chord.kind, "dominant");
    assert_eq!(candidate.changed_spans.len(), 1);
    assert_eq!(candidate.resolved.max_changed_spans, Some(1));
    let replay: acorde_core::ChordProposalArrangementResult = serde_json::from_str(
        &acorde_wasm::arrange_from_chord_proposal(
            &input,
            &serde_json::to_string(&candidate.resolved).unwrap(),
            "held-chord",
        )
        .unwrap(),
    )
    .unwrap();
    assert!(replay.diagnostics.is_empty());
    assert_eq!(replay.plan, candidate.plan);
    let sections = serde_json::json!({
        "chords":candidate.preset_selection.chords, "bass_mode":"free-bass",
        "default_pattern":"held-bass-chord",
        "sections":[{"first_measure":1,"last_measure":1,"pattern":"swing-accompaniment"}]
    });
    let voiced: acorde_core::VoiceLedSectionedAccordionResult = serde_json::from_str(
        &acorde_wasm::arrange_sections_with_voice_leading(&input, &sections.to_string(), "{}")
            .unwrap(),
    )
    .unwrap();
    assert!(voiced.arrangement.diagnostics.is_empty());
    let score = voiced.arrangement.score.unwrap();
    assert_eq!(
        score.parts[0].staves[1].measures[0].voices[0][0].pitches[0].to_midi(),
        40
    );
    let output = serde_json::to_string(&score).unwrap();
    assert_eq!(events(&input, Some(0)), events(&output, Some(0)));
    assert!(
        acorde_wasm::render_score_svg(&output, "{}")
            .unwrap()
            .contains("<svg")
    );
    let playback: serde_json::Value =
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
    let bad: acorde_core::ChordProposalResult = serde_json::from_str(
        &acorde_wasm::suggest_chord_plans_with_locks(&input, &request.to_string(), "[3]").unwrap(),
    )
    .unwrap();
    assert!(bad.candidates.is_empty());
    assert_eq!(bad.diagnostics[0].code, "invalid-chord-lock");
    assert_eq!(
        acorde_wasm::suggest_chord_plans_with_locks(&input, &request.to_string(), "[]").unwrap(),
        acorde_wasm::suggest_chord_plans(&input, &request.to_string()).unwrap(),
    );
}

#[cfg_attr(target_arch = "wasm32", wasm_bindgen_test::wasm_bindgen_test)]
#[cfg_attr(not(target_arch = "wasm32"), test)]
fn positioned_harmonic_rhythm_survives_render_playback_and_export() {
    let input = acorde_wasm::parse_abc("X:1\nM:4/4\nL:1/4\nK:C\nC E G B | c4 |\n").unwrap();
    let positions = serde_json::json!([
        {"measure_index":0,"offset":{"numerator":0,"denominator":1}},
        {"measure_index":0,"offset":{"numerator":2,"denominator":1}},
        {"measure_index":1,"offset":{"numerator":0,"denominator":1}}
    ]);
    let json = acorde_wasm::suggest_chord_plans_at_positions(&input, "{}", &positions.to_string())
        .unwrap();
    let native = acorde_core::suggest_chord_plans_at_positions(
        &serde_json::from_str(&input).unwrap(),
        &Default::default(),
        &serde_json::from_value::<Vec<acorde_core::ChordPlanPosition>>(positions.clone()).unwrap(),
    );
    assert_eq!(
        serde_json::from_str::<serde_json::Value>(&json).unwrap(),
        serde_json::to_value(&native).unwrap()
    );
    let result: acorde_core::ChordProposalResult = serde_json::from_str(&json).unwrap();
    let candidate = result
        .candidates
        .iter()
        .find(|candidate| candidate.id == "conservative-v2")
        .unwrap();
    assert_eq!(candidate.plan.entries.len(), 3);
    assert_eq!(candidate.plan.entries[1].position.offset.beats(), 2.0);
    let replay: acorde_core::ChordProposalArrangementResult = serde_json::from_str(
        &acorde_wasm::arrange_from_chord_proposal(
            &input,
            &serde_json::to_string(&candidate.resolved).unwrap(),
            "held-chord",
        )
        .unwrap(),
    )
    .unwrap();
    assert!(replay.diagnostics.is_empty(), "{:?}", replay.diagnostics);
    let output = serde_json::to_string(&replay.arrangement.unwrap().score.unwrap()).unwrap();
    assert_eq!(events(&input, Some(0)), events(&output, Some(0)));
    assert!(
        acorde_wasm::render_score_svg(&output, "{}")
            .unwrap()
            .contains("<svg")
    );
    let imported = acorde_wasm::parse_mscz(&acorde_wasm::serialize_mscz(&output).unwrap()).unwrap();
    assert_eq!(events(&output, None), events(&imported, None));
    #[cfg(feature = "musicxml")]
    {
        let imported =
            acorde_wasm::parse_musicxml(&acorde_wasm::serialize_musicxml(&output).unwrap())
                .unwrap();
        assert_eq!(events(&output, None), events(&imported, None));
    }
    let mut bad = positions;
    bad[1]["offset"]["denominator"] = serde_json::json!(3);
    let rejected: acorde_core::ChordProposalResult = serde_json::from_str(
        &acorde_wasm::suggest_chord_plans_at_positions(&input, "{}", &bad.to_string()).unwrap(),
    )
    .unwrap();
    assert!(rejected.candidates.is_empty());
    assert_eq!(rejected.diagnostics[0].code, "invalid-harmonic-positions");
}
