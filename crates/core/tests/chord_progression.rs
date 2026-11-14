use acorde_core::{
    ChordProposalRequest, ChordProposalResult, Duration, Note, Pitch, Score,
    arrange_from_chord_proposal, suggest_chord_plans, suggest_chord_plans_at_positions,
    suggest_chord_plans_with_locks,
};

fn melody(bars: &[&[u8]]) -> Score {
    let mut score = Score::new(
        "original harmony diagnostic",
        120,
        4,
        4,
        0,
        bars.len() as u32,
    );
    for (measure, pitches) in score.parts[0].staves[0].measures.iter_mut().zip(bars) {
        let duration = match pitches.len() {
            1 => Duration::Whole,
            2 => Duration::Half,
            _ => Duration::Quarter,
        };
        measure.voices[0] = pitches
            .iter()
            .map(|pitch| Note::new(Pitch::from_midi(*pitch, false), duration.clone()))
            .collect();
    }
    score
}

fn harmony(result: &ChordProposalResult, style: &str) -> Vec<(String, String)> {
    result
        .candidates
        .iter()
        .find(|candidate| candidate.id.starts_with(style))
        .expect("requested style")
        .plan
        .entries
        .iter()
        .map(|entry| (entry.chord.root.clone(), entry.chord.kind.clone()))
        .collect()
}

#[test]
fn jazz_dominant_resolves_to_tonic_without_a_major_seventh_on_v() {
    let score = melody(&[&[67, 71, 74, 77], &[72]]);
    let before = serde_json::to_value(&score).unwrap();
    let result = suggest_chord_plans(&score, &ChordProposalRequest::default());
    assert_eq!(
        harmony(&result, "jazz-color"),
        [
            ("G".into(), "dominant".into()),
            ("C".into(), "major-seventh".into()),
        ]
    );
    let candidate = result
        .candidates
        .iter()
        .find(|candidate| candidate.id.starts_with("jazz-color"))
        .unwrap();
    for preset in ["held-chord", "swing-pulse"] {
        let replay = arrange_from_chord_proposal(&score, &candidate.resolved, preset);
        assert!(replay.diagnostics.is_empty(), "{:?}", replay.diagnostics);
        let arranged = replay.arrangement.unwrap().score.unwrap();
        assert_eq!(
            serde_json::to_value(&arranged.parts[0].staves[0]).unwrap(),
            before["parts"][0]["staves"][0]
        );
    }
    assert_eq!(serde_json::to_value(&score).unwrap(), before);
}

#[test]
fn jazz_can_prepare_a_dominant_with_its_own_dominant() {
    let score = melody(&[&[66, 72], &[67, 71, 74, 77], &[72]]);
    let result = suggest_chord_plans(&score, &ChordProposalRequest::default());
    assert_eq!(
        harmony(&result, "jazz-color"),
        [
            ("D".into(), "dominant".into()),
            ("G".into(), "dominant".into()),
            ("C".into(), "major-seventh".into()),
        ]
    );
}

#[test]
fn later_tonic_changes_the_choice_for_an_ambiguous_opening() {
    let isolated = suggest_chord_plans(&melody(&[&[71, 74]]), &ChordProposalRequest::default());
    assert_eq!(
        harmony(&isolated, "conservative")[0],
        ("B".into(), "diminished".into())
    );
    let contextual = suggest_chord_plans(
        &melody(&[&[71, 74], &[72]]),
        &ChordProposalRequest::default(),
    );
    assert_eq!(
        harmony(&contextual, "conservative"),
        [("G".into(), "major".into()), ("C".into(), "major".into()),]
    );
}

#[test]
fn brief_repeated_chromatic_notes_do_not_outvote_a_long_tonic() {
    let mut unsplit = melody(&[&[72]]);
    let mut long = Note::new(Pitch::from_midi(72, false), Duration::Half);
    long.dot_count = 1;
    unsplit.parts[0].staves[0].measures[0].voices[0] = vec![
        long.clone(),
        Note::new(Pitch::from_midi(66, false), Duration::Quarter),
    ];
    let mut repeated = unsplit.clone();
    repeated.parts[0].staves[0].measures[0].voices[0] = std::iter::once(long)
        .chain((0..8).map(|_| Note::new(Pitch::from_midi(66, false), Duration::ThirtySecond)))
        .collect();
    let first = suggest_chord_plans(&unsplit, &ChordProposalRequest::default());
    let second = suggest_chord_plans(&repeated, &ChordProposalRequest::default());
    assert_eq!(
        harmony(&first, "conservative")[0],
        ("C".into(), "major".into())
    );
    for style in ["conservative", "jazz-color"] {
        assert_eq!(harmony(&first, style), harmony(&second, style));
    }
}

#[test]
fn contextual_nonchord_tones_are_opt_in_and_persist_in_resolved_proposals() {
    let score = melody(&[&[72, 74, 76, 72]]);
    let request = ChordProposalRequest {
        contextual_nonchord_tones: true,
        ..Default::default()
    };
    assert!(
        serde_json::to_value(ChordProposalRequest::default())
            .unwrap()
            .get("contextual_nonchord_tones")
            .is_none()
    );
    let result = suggest_chord_plans(&score, &request);
    assert!(result.diagnostics.is_empty(), "{:?}", result.diagnostics);
    assert!(!result.candidates.is_empty());
    for candidate in &result.candidates {
        assert!(candidate.resolved.contextual_nonchord_tones);
        assert!(
            candidate
                .reason_codes
                .contains(&"contextual-nonchord-tones".into())
        );
        let replay = arrange_from_chord_proposal(&score, &candidate.resolved, "held-chord");
        assert!(replay.diagnostics.is_empty(), "{:?}", replay.diagnostics);
    }
}

#[test]
fn local_key_changes_set_the_dominant_seventh_quality() {
    let mut score = melody(&[&[72], &[69, 73, 76, 79], &[74]]);
    score.parts[0].staves[0].measures[1].key_sig = Some(acorde_core::KeySignature {
        fifths: 2,
        mode: "major".into(),
    });
    let result = suggest_chord_plans(&score, &ChordProposalRequest::default());
    assert_eq!(
        &harmony(&result, "jazz-color")[1..],
        [
            ("A".into(), "dominant".into()),
            ("D".into(), "major-seventh".into()),
        ]
    );
}

#[test]
fn standard_bass_search_excludes_unavailable_diminished_chords() {
    let score = melody(&[&[71, 74]]);
    let request = ChordProposalRequest {
        bass_mode: acorde_core::AccordionBassMode::StandardBass,
        allow_jazz_color: false,
        ..Default::default()
    };
    let result = suggest_chord_plans(&score, &request);
    assert!(result.diagnostics.is_empty(), "{:?}", result.diagnostics);
    assert_eq!(
        harmony(&result, "conservative")[0],
        ("G".into(), "major".into())
    );
    for candidate in result.candidates {
        assert!(
            candidate
                .plan
                .entries
                .iter()
                .all(|entry| matches!(entry.chord.kind.as_str(), "major" | "minor" | "dominant"))
        );
        assert!(
            arrange_from_chord_proposal(&score, &candidate.resolved, "held-chord")
                .diagnostics
                .is_empty()
        );
    }
}

#[test]
fn explicit_mid_measure_boundaries_clip_the_melody_evidence() {
    let mut score = melody(&[&[72, 67], &[72]]);
    let chord = |root: &str| {
        serde_json::from_value(serde_json::json!({"root":root,"kind":"major"})).unwrap()
    };
    score.parts[0].staves[0].measures[0].voices[0][0].chord_symbol = Some(chord("C"));
    score.parts[0].staves[0].measures[0].voices[0][1].chord_symbol = Some(chord("G"));
    score.parts[0].staves[0].measures[1].voices[0][0].chord_symbol = Some(chord("C"));
    let plan = acorde_core::extract_chord_plan(&score, &Default::default())
        .plan
        .unwrap();
    let result = suggest_chord_plans(
        &score,
        &ChordProposalRequest {
            source_plan: Some(plan.clone()),
            ..Default::default()
        },
    );
    assert_eq!(result.candidates[0].plan, plan);
    let jazz = result
        .candidates
        .iter()
        .find(|candidate| candidate.id.starts_with("jazz-color"))
        .unwrap();
    assert_eq!(
        harmony(&result, "jazz-color"),
        [
            ("C".into(), "major-seventh".into()),
            ("G".into(), "dominant".into()),
            ("C".into(), "major-seventh".into()),
        ]
    );
    assert_eq!(jazz.plan.entries[1].position.offset.beats(), 2.0);
}

#[test]
fn stored_v1_plans_replay_without_regenerating_their_harmony() {
    let score = melody(&[&[67, 71, 74, 77], &[72]]);
    let result = suggest_chord_plans(&score, &ChordProposalRequest::default());
    let mut legacy = result
        .candidates
        .iter()
        .find(|candidate| candidate.id.starts_with("jazz-color"))
        .unwrap()
        .resolved
        .clone();
    legacy.candidate_id = "jazz-color-v1".into();
    legacy.plan.entries[0].chord.kind = "major-seventh".into();
    let replay = arrange_from_chord_proposal(&score, &legacy, "held-chord");
    assert!(replay.diagnostics.is_empty());
    assert_eq!(replay.plan, legacy.plan);
    assert_eq!(replay.candidate_id, "jazz-color-v1");
}

fn plan(entries: &[(usize, u32, &str, &str)]) -> acorde_core::ChordPlan {
    serde_json::from_value(serde_json::json!({
        "contract_version":1,
        "entries":entries.iter().map(|(measure, beat, root, kind)| serde_json::json!({
            "position":{"measure_index":measure,"offset":{"numerator":beat,"denominator":1}},
            "offset_beats":beat, "chord":{"root":root,"kind":kind},
            "origin":"host-authored", "sources":[{"note":null,"reference":format!("editor:{measure}:{beat}")}]
        })).collect::<Vec<_>>()
    })).unwrap()
}

#[test]
fn empty_locks_preserve_the_existing_proposal_contract() {
    let score = melody(&[&[66, 72], &[67, 71, 74, 77], &[72]]);
    for source_plan in [
        None,
        Some(plan(&[
            (0, 0, "C", "major"),
            (1, 0, "G", "major"),
            (2, 0, "C", "major"),
        ])),
    ] {
        let request = ChordProposalRequest {
            source_plan,
            ..Default::default()
        };
        assert_eq!(
            serde_json::to_value(suggest_chord_plans(&score, &request)).unwrap(),
            serde_json::to_value(suggest_chord_plans_with_locks(&score, &request, &[])).unwrap()
        );
    }
}

#[test]
fn locked_mid_bar_harmony_keeps_slash_bass_boundaries_and_provenance() {
    let score = melody(&[&[72, 67], &[72]]);
    let mut source = plan(&[
        (0, 0, "C", "major"),
        (0, 2, "G", "major"),
        (1, 0, "C", "major"),
    ]);
    source.entries[0].chord.bass = Some("E".into());
    source.entries[0].chord.harmony_function = Some("opening".into());
    let request = ChordProposalRequest {
        source_plan: Some(source.clone()),
        ..Default::default()
    };
    let before = serde_json::to_value((&score, &request)).unwrap();
    let result = suggest_chord_plans_with_locks(&score, &request, &[2, 0, 2]);
    assert!(result.diagnostics.is_empty(), "{:?}", result.diagnostics);
    let jazz = result
        .candidates
        .iter()
        .find(|c| c.id == "jazz-color-v2")
        .unwrap();
    assert_eq!(jazz.plan.entries[0], source.entries[0]);
    assert_eq!(jazz.plan.entries[2], source.entries[2]);
    assert_eq!(jazz.plan.entries[1].chord.kind, "dominant");
    assert_eq!(
        jazz.plan.entries[1].origin,
        acorde_core::ChordPlanOrigin::GeneratedReharmonization
    );
    assert!(jazz.plan.entries[1].sources.is_empty());
    assert_eq!(jazz.changed_spans.len(), 1);
    assert_eq!(jazz.changed_spans[0].start, source.entries[1].position);
    assert_eq!(jazz.changed_spans[0].end, source.entries[2].position);
    for candidate in &result.candidates {
        assert!(
            candidate
                .reason_codes
                .iter()
                .any(|code| code == "locked-source-harmony")
        );
        for index in [0, 2] {
            assert!(candidate.plan.entries.contains(&source.entries[index]));
        }
        let replay = arrange_from_chord_proposal(&score, &candidate.resolved, "held-chord");
        assert!(replay.diagnostics.is_empty(), "{:?}", replay.diagnostics);
        assert_eq!(replay.plan, candidate.plan);
        assert_eq!(
            serde_json::to_value(&replay.arrangement.unwrap().score.unwrap().parts[0].staves[0])
                .unwrap(),
            serde_json::to_value(&score.parts[0].staves[0]).unwrap()
        );
    }
    assert_eq!(serde_json::to_value((&score, &request)).unwrap(), before);
    assert_eq!(
        serde_json::to_value(&result).unwrap(),
        serde_json::to_value(suggest_chord_plans_with_locks(&score, &request, &[0, 2])).unwrap()
    );
}

#[test]
fn a_locked_destination_changes_the_preceding_unlocked_choice() {
    let score = melody(&[&[71, 74], &[72]]);
    let request = ChordProposalRequest {
        source_plan: Some(plan(&[(0, 0, "C", "major"), (1, 0, "E", "minor")])),
        ..Default::default()
    };
    let free = suggest_chord_plans(&score, &request);
    let fixed = suggest_chord_plans_with_locks(&score, &request, &[1]);
    assert_eq!(
        harmony(&free, "conservative")[0],
        ("G".into(), "major".into())
    );
    assert_eq!(
        harmony(&fixed, "conservative"),
        [
            ("B".into(), "diminished".into()),
            ("E".into(), "minor".into())
        ]
    );
}

#[test]
fn generated_applied_dominants_must_resolve_to_the_locked_destination() {
    let score = melody(&[&[66, 72], &[67, 71, 74, 77], &[72]]);
    let request = ChordProposalRequest {
        source_plan: Some(plan(&[
            (0, 0, "C", "major"),
            (1, 0, "A", "minor"),
            (2, 0, "C", "major"),
        ])),
        ..Default::default()
    };
    assert_eq!(
        harmony(&suggest_chord_plans(&score, &request), "jazz-color")[0],
        ("D".into(), "dominant".into())
    );
    let fixed = suggest_chord_plans_with_locks(&score, &request, &[1]);
    let chords = harmony(&fixed, "jazz-color");
    assert_ne!(chords[0], ("D".into(), "dominant".into()));
    assert_eq!(chords[1], ("A".into(), "minor".into()));
}

#[test]
fn simplified_harmony_retains_a_redundant_locked_boundary() {
    let score = melody(&[&[72], &[72], &[72]]);
    let source = plan(&[
        (0, 0, "C", "major"),
        (1, 0, "C", "major"),
        (2, 0, "F", "major"),
    ]);
    let request = ChordProposalRequest {
        source_plan: Some(source.clone()),
        ..Default::default()
    };
    let result = suggest_chord_plans_with_locks(&score, &request, &[1]);
    let simplified = result
        .candidates
        .iter()
        .find(|c| c.id == "simplified-v2")
        .unwrap();
    assert_eq!(simplified.plan.entries.len(), 2);
    assert_eq!(simplified.plan.entries[1], source.entries[1]);
    assert_eq!(
        simplified.plan.entries[0].chord,
        simplified.plan.entries[1].chord
    );
}

#[test]
fn locking_every_entry_preserves_chromatic_spelling_and_returns_one_plan() {
    let mut score = melody(&[&[72], &[72], &[72]]);
    score.parts[0].staves[0].measures[1].key_sig = Some(acorde_core::KeySignature {
        fifths: 2,
        mode: "major".into(),
    });
    let mut source = plan(&[
        (0, 0, "Bb", "major"),
        (1, 0, "Ebb", "minor"),
        (2, 0, "C##", "dominant"),
    ]);
    source.entries[1].chord.bass = Some("F#".into());
    let request = ChordProposalRequest {
        source_plan: Some(source.clone()),
        ..Default::default()
    };
    let result = suggest_chord_plans_with_locks(&score, &request, &[0, 1, 2]);
    assert!(result.diagnostics.is_empty(), "{:?}", result.diagnostics);
    assert_eq!(result.candidates.len(), 1);
    assert_eq!(result.candidates[0].id, "provided-v1");
    assert_eq!(result.candidates[0].plan, source);
    assert!(result.candidates[0].changed_spans.is_empty());
}

#[test]
fn added_suspended_and_half_diminished_source_chords_can_be_locked_and_replayed() {
    let score = melody(&[&[72]]);
    for kind in [
        "suspended-second",
        "suspended-fourth",
        "major-add9",
        "minor-add9",
        "half-diminished",
    ] {
        let source = plan(&[(0, 0, "C", kind)]);
        let request = ChordProposalRequest {
            source_plan: Some(source.clone()),
            ..Default::default()
        };
        let result = suggest_chord_plans_with_locks(&score, &request, &[0]);
        assert!(
            result.diagnostics.is_empty(),
            "{kind}: {:?}",
            result.diagnostics
        );
        assert_eq!(result.candidates.len(), 1, "{kind}");
        assert_eq!(result.candidates[0].plan, source);
        let replay =
            arrange_from_chord_proposal(&score, &result.candidates[0].resolved, "held-chord");
        assert!(
            replay.diagnostics.is_empty(),
            "{kind}: {:?}",
            replay.diagnostics
        );
        assert!(replay.arrangement.unwrap().score.is_some(), "{kind}");
    }
}

#[test]
fn change_budget_limits_source_regions_and_preserves_unchanged_provenance() {
    let score = melody(&[&[67, 71, 74, 77], &[72]]);
    let source = plan(&[(0, 0, "C", "major"), (1, 0, "C", "major")]);
    let request = |budget| ChordProposalRequest {
        source_plan: Some(source.clone()),
        max_changed_spans: Some(budget),
        ..Default::default()
    };
    let zero = suggest_chord_plans(&score, &request(0));
    assert!(zero.diagnostics.is_empty(), "{:?}", zero.diagnostics);
    assert_eq!(zero.candidates[0].plan, source);
    assert!(
        zero.candidates
            .iter()
            .all(|candidate| candidate.changed_spans.is_empty())
    );

    let one = suggest_chord_plans_with_locks(&score, &request(1), &[1]);
    assert!(one.diagnostics.is_empty(), "{:?}", one.diagnostics);
    assert!(
        one.candidates
            .iter()
            .any(|candidate| candidate.changed_spans.len() == 1)
    );
    for candidate in &one.candidates {
        assert!(candidate.changed_spans.len() <= 1);
        assert_eq!(candidate.plan.entries.last(), source.entries.last());
        assert_eq!(candidate.resolved.max_changed_spans, Some(1));
        assert!(
            candidate
                .reason_codes
                .contains(&"bounded-harmony-change".into())
        );
        for entry in &candidate.plan.entries {
            if let Some(original) = source
                .entries
                .iter()
                .find(|original| original.position == entry.position)
                && original.chord == entry.chord
            {
                assert_eq!(entry.origin, original.origin);
                assert_eq!(entry.sources, original.sources);
            }
        }
        let replay = arrange_from_chord_proposal(&score, &candidate.resolved, "held-chord");
        assert!(replay.diagnostics.is_empty(), "{:?}", replay.diagnostics);
    }
    assert_eq!(
        serde_json::to_value(&one).unwrap(),
        serde_json::to_value(suggest_chord_plans_with_locks(&score, &request(1), &[1])).unwrap()
    );

    let two = suggest_chord_plans(&score, &request(2));
    assert!(
        two.candidates
            .iter()
            .any(|candidate| candidate.changed_spans.len() == 2)
    );
}

#[test]
fn change_budget_requires_a_supported_source_and_a_small_limit() {
    let score = melody(&[&[72]]);
    let missing = suggest_chord_plans(
        &score,
        &ChordProposalRequest {
            max_changed_spans: Some(0),
            ..Default::default()
        },
    );
    assert!(missing.candidates.is_empty());
    assert_eq!(
        missing.diagnostics[0].code,
        "change-budget-requires-source-plan"
    );
    let oversized = suggest_chord_plans(
        &score,
        &ChordProposalRequest {
            source_plan: Some(plan(&[(0, 0, "C", "major")])),
            max_changed_spans: Some(17),
            ..Default::default()
        },
    );
    assert!(oversized.candidates.is_empty());
    assert_eq!(oversized.diagnostics[0].code, "invalid-change-budget");
    let unsupported = suggest_chord_plans(
        &score,
        &ChordProposalRequest {
            source_plan: Some(plan(&[(0, 0, "C", "power")])),
            max_changed_spans: Some(0),
            ..Default::default()
        },
    );
    assert!(unsupported.candidates.is_empty());
    assert_eq!(
        unsupported.diagnostics[0].code,
        "change-budget-source-unsupported"
    );
}

#[test]
fn invalid_locks_fail_before_even_a_provided_candidate_is_returned() {
    let score = melody(&[&[72], &[72]]);
    let request = ChordProposalRequest {
        source_plan: Some(plan(&[(0, 0, "C", "major"), (1, 0, "G", "major")])),
        max_candidates: 1,
        ..Default::default()
    };
    let mut cases = vec![
        (
            ChordProposalRequest::default(),
            vec![0],
            "locks-require-source-plan",
            None,
        ),
        (request.clone(), vec![2], "invalid-chord-lock", None),
        (
            request.clone(),
            vec![usize::MAX],
            "invalid-chord-lock",
            None,
        ),
        (request.clone(), vec![0; 513], "chord-lock-limit", None),
    ];
    let mut unsupported = request.clone();
    unsupported.source_plan.as_mut().unwrap().entries[1]
        .chord
        .kind = "major-ninth".into();
    cases.push((unsupported, vec![1], "locked-chord-unsupported", Some(1)));
    let mut standard = request.clone();
    standard.bass_mode = acorde_core::AccordionBassMode::StandardBass;
    standard.source_plan.as_mut().unwrap().entries[1].chord.kind = "major-seventh".into();
    cases.push((standard, vec![1], "locked-chord-unsupported", Some(1)));
    let mut off_grid = request.clone();
    let entry = &mut off_grid.source_plan.as_mut().unwrap().entries[1];
    entry.position.offset = acorde_core::BeatFraction {
        numerator: 1,
        denominator: 3,
    };
    entry.offset_beats = 1.0 / 3.0;
    cases.push((off_grid, vec![1], "locked-chord-grid-unsupported", Some(1)));
    let mut malformed = request;
    malformed.source_plan.as_mut().unwrap().entries[1]
        .position
        .offset
        .denominator = 0;
    cases.push((malformed, vec![1], "invalid-source-plan", None));
    for (request, locks, code, measure) in cases {
        let result = suggest_chord_plans_with_locks(&score, &request, &locks);
        assert!(result.candidates.is_empty(), "{code}");
        assert_eq!(result.diagnostics[0].code, code);
        assert_eq!(result.diagnostics[0].measure_index, measure);
    }
}

#[test]
fn a_lock_covers_the_whole_source_region_across_measure_boundaries() {
    let score = melody(&[&[72], &[67, 71, 74, 77], &[72]]);
    let source = plan(&[(0, 0, "C", "major"), (2, 0, "F", "major")]);
    let request = ChordProposalRequest {
        source_plan: Some(source.clone()),
        ..Default::default()
    };
    let result = suggest_chord_plans_with_locks(&score, &request, &[0]);
    assert!(result.candidates.len() > 1);
    for candidate in result.candidates {
        assert_eq!(candidate.plan.entries[0], source.entries[0]);
        assert!(
            candidate
                .plan
                .entries
                .iter()
                .skip(1)
                .all(|entry| entry.position.measure_index >= 2)
        );
        assert!(
            candidate
                .changed_spans
                .iter()
                .all(|span| span.start.measure_index >= 2)
        );
    }
}

#[test]
fn standard_bass_locks_keep_a_supported_dominant_while_other_chords_change() {
    let score = melody(&[&[67, 71, 74, 77], &[72]]);
    let source = plan(&[(0, 0, "G", "dominant"), (1, 0, "F", "major")]);
    let request = ChordProposalRequest {
        source_plan: Some(source.clone()),
        bass_mode: acorde_core::AccordionBassMode::StandardBass,
        allow_jazz_color: false,
        ..Default::default()
    };
    let result = suggest_chord_plans_with_locks(&score, &request, &[0]);
    assert!(result.diagnostics.is_empty(), "{:?}", result.diagnostics);
    assert_eq!(
        harmony(&result, "conservative"),
        [
            ("G".into(), "dominant".into()),
            ("C".into(), "major".into())
        ]
    );
    for candidate in result.candidates {
        assert_eq!(candidate.plan.entries[0], source.entries[0]);
        assert!(
            arrange_from_chord_proposal(&score, &candidate.resolved, "held-chord")
                .diagnostics
                .is_empty()
        );
    }
}

fn position(measure: usize, numerator: u32, denominator: u32) -> acorde_core::ChordPlanPosition {
    acorde_core::ChordPlanPosition {
        measure_index: measure,
        offset: acorde_core::BeatFraction {
            numerator,
            denominator,
        },
    }
}

#[test]
fn two_chords_in_one_bar_follow_the_two_melody_regions_and_replay() {
    let score = melody(&[&[72, 76, 67, 71]]);
    let before = serde_json::to_value(&score).unwrap();
    let result = suggest_chord_plans_at_positions(
        &score,
        &Default::default(),
        &[position(0, 0, 1), position(0, 2, 1)],
    );
    assert!(result.diagnostics.is_empty(), "{:?}", result.diagnostics);
    let conservative = result
        .candidates
        .iter()
        .find(|c| c.id == "conservative-v2")
        .unwrap();
    assert_eq!(conservative.plan.entries.len(), 2);
    assert_eq!(conservative.plan.entries[1].position.offset.beats(), 2.0);
    assert_eq!(conservative.plan.entries[0].chord.root, "C");
    assert_eq!(conservative.plan.entries[1].chord.root, "G");
    assert!(
        conservative
            .reason_codes
            .iter()
            .any(|code| code == "caller-supplied-harmonic-rhythm")
    );
    let replay = arrange_from_chord_proposal(&score, &conservative.resolved, "held-chord");
    assert!(replay.diagnostics.is_empty(), "{:?}", replay.diagnostics);
    assert_eq!(
        serde_json::to_value(&replay.arrangement.unwrap().score.unwrap().parts[0].staves[0])
            .unwrap(),
        serde_json::to_value(&score.parts[0].staves[0]).unwrap()
    );
    assert_eq!(serde_json::to_value(&score).unwrap(), before);
}

#[test]
fn barline_positions_match_old_proposals_and_compound_or_pickup_positions_work() {
    let score = melody(&[&[72], &[67], &[72]]);
    let positions = (0..3).map(|bar| position(bar, 0, 1)).collect::<Vec<_>>();
    let ordinary = suggest_chord_plans(&score, &Default::default());
    let selected = suggest_chord_plans_at_positions(&score, &Default::default(), &positions);
    assert_eq!(ordinary.candidates.len(), selected.candidates.len());
    for (old, new) in ordinary.candidates.iter().zip(&selected.candidates) {
        assert_eq!(old.plan, new.plan);
        assert_eq!(old.preset_selection, new.preset_selection);
        assert_eq!(old.resolved, new.resolved);
    }
    let mut compound = Score::new("pickup and compound meter", 120, 6, 8, 0, 2);
    compound.parts[0].staves[0].measures[0].actual_length =
        acorde_core::MeasureLength::from_beats(1.0);
    compound.parts[0].staves[0].measures[0].voices[0] =
        vec![Note::new(Pitch::from_midi(72, false), Duration::Quarter)];
    compound.parts[0].staves[0].measures[1].voices[0] = vec![
        Note::new(Pitch::from_midi(67, false), Duration::Quarter),
        Note::new(Pitch::from_midi(71, false), Duration::Eighth),
        Note::new(Pitch::from_midi(72, false), Duration::Quarter),
        Note::new(Pitch::from_midi(76, false), Duration::Eighth),
    ];
    let result = suggest_chord_plans_at_positions(
        &compound,
        &Default::default(),
        &[position(0, 0, 1), position(1, 0, 1), position(1, 3, 2)],
    );
    assert!(result.diagnostics.is_empty(), "{:?}", result.diagnostics);
    assert!(result.candidates.iter().any(|candidate| {
        candidate
            .plan
            .entries
            .iter()
            .any(|entry| entry.position == position(1, 3, 2))
    }));
}

#[test]
fn invalid_positions_return_no_candidates() {
    let score = melody(&[&[72], &[67]]);
    let request = ChordProposalRequest::default();
    for positions in [
        vec![],
        vec![position(1, 0, 1)],
        vec![position(0, 0, 1), position(0, 0, 1)],
        vec![position(0, 0, 1), position(0, 1, 3)],
        vec![position(0, 0, 1), position(2, 0, 1)],
        vec![position(0, 0, 1), position(0, 4, 1)],
        vec![position(0, 0, 1); 513],
    ] {
        let result = suggest_chord_plans_at_positions(&score, &request, &positions);
        assert!(result.candidates.is_empty());
        assert_eq!(result.diagnostics[0].code, "invalid-harmonic-positions");
    }
    let with_source = ChordProposalRequest {
        source_plan: Some(plan(&[(0, 0, "C", "major"), (1, 0, "G", "major")])),
        ..Default::default()
    };
    let rejected = suggest_chord_plans_at_positions(&score, &with_source, &[position(0, 0, 1)]);
    assert!(rejected.candidates.is_empty());
    assert_eq!(
        rejected.diagnostics[0].code,
        "positions-conflict-source-plan"
    );
}
