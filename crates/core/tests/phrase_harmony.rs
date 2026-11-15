use acorde_core::{
    BeatFraction, ChordCadenceKind, ChordCadenceTarget, ChordKeyRegion, ChordPlan, ChordPlanEntry,
    ChordPlanOrigin, ChordPlanPosition, ChordPlanSource, ChordProposalRequest, ChordSymbol,
    Duration, KeySignature, Note, PhraseHarmonicPace, PhraseHarmonyRequest, PhraseHarmonySection,
    Pitch, Score, arrange_from_chord_proposal, suggest_phrase_harmony,
};

fn score(numerator: u8, denominator: u8, bars: usize) -> Score {
    let mut score = Score::new(
        "phrase-harmony development",
        112,
        numerator,
        denominator,
        0,
        bars as u32,
    );
    let beat_unit = if denominator == 8 {
        Duration::Eighth
    } else {
        Duration::Quarter
    };
    for bar in &mut score.parts[0].staves[0].measures {
        bar.voices[0] = (0..numerator)
            .map(|beat| {
                Note::new(
                    Pitch::from_midi([72, 74, 76, 79][usize::from(beat) % 4], false),
                    beat_unit.clone(),
                )
            })
            .collect();
    }
    score
}

fn request(bars: usize, pace: PhraseHarmonicPace) -> PhraseHarmonyRequest {
    PhraseHarmonyRequest {
        phrases: vec![PhraseHarmonySection {
            start_measure: 0,
            end_measure_exclusive: bars,
            pace,
            meter_groups: vec![],
        }],
        proposal: ChordProposalRequest::default(),
        locked_chord_entries: vec![],
        source_reanchor: None,
        local_keys: vec![],
        cadences: vec![],
    }
}

fn authored_plan(bars: usize) -> ChordPlan {
    let chord: ChordSymbol =
        serde_json::from_value(serde_json::json!({"root":"C","kind":"major"})).unwrap();
    ChordPlan {
        contract_version: 1,
        entries: (0..bars)
            .map(|measure| ChordPlanEntry {
                position: ChordPlanPosition {
                    measure_index: measure,
                    offset: BeatFraction::ZERO,
                },
                offset_beats: 0.0,
                chord: chord.clone(),
                origin: ChordPlanOrigin::HostAuthored,
                sources: vec![ChordPlanSource {
                    note: None,
                    reference: Some(format!("host:bar-{measure}")),
                }],
            })
            .collect(),
    }
}

#[test]
fn sections_vary_harmonic_rhythm_without_changing_melody() {
    let source = score(4, 4, 8);
    let mut input = request(8, PhraseHarmonicPace::EverySecondBar);
    input.phrases = vec![
        PhraseHarmonySection {
            start_measure: 0,
            end_measure_exclusive: 4,
            pace: PhraseHarmonicPace::EverySecondBar,
            meter_groups: vec![],
        },
        PhraseHarmonySection {
            start_measure: 4,
            end_measure_exclusive: 6,
            pace: PhraseHarmonicPace::EveryBar,
            meter_groups: vec![],
        },
        PhraseHarmonySection {
            start_measure: 6,
            end_measure_exclusive: 8,
            pace: PhraseHarmonicPace::TwoGroupsPerBar,
            meter_groups: vec![],
        },
    ];
    let before = serde_json::to_value(&source).unwrap();
    let result = suggest_phrase_harmony(&source, &input);
    assert!(result.diagnostics.is_empty(), "{:?}", result.diagnostics);
    let starts = result
        .positions
        .iter()
        .map(|p| (p.measure_index, p.offset.beats()))
        .collect::<Vec<_>>();
    assert_eq!(
        starts,
        [
            (0, 0.0),
            (2, 0.0),
            (4, 0.0),
            (5, 0.0),
            (6, 0.0),
            (6, 2.0),
            (7, 0.0),
            (7, 2.0)
        ]
    );
    assert!(!result.proposals.candidates.is_empty());
    assert_eq!(serde_json::to_value(&source).unwrap(), before);
    assert_eq!(
        result.proposals.candidates[0].plan.entries[0].position,
        result.positions[0]
    );
}

#[test]
fn compound_and_irregular_groups_place_changes_on_real_group_boundaries() {
    for (numerator, denominator, expected) in [(6, 8, 1.5), (5, 8, 1.5), (7, 8, 2.0)] {
        let source = score(numerator, denominator, 2);
        let result =
            suggest_phrase_harmony(&source, &request(2, PhraseHarmonicPace::TwoGroupsPerBar));
        assert!(
            result.diagnostics.is_empty(),
            "{numerator}/8: {:?}",
            result.diagnostics
        );
        assert_eq!(result.positions.len(), 4);
        assert_eq!(result.positions[1].offset.beats(), expected);
        assert_eq!(result.positions[3].offset.beats(), expected);
    }
}

#[test]
fn original_chord_and_lock_survive_aligned_phrase_boundaries() {
    let source = score(4, 4, 4);
    let mut input = request(4, PhraseHarmonicPace::EveryBar);
    let entries = authored_plan(4).entries;
    input.proposal.source_plan = Some(ChordPlan {
        contract_version: 1,
        entries: entries.clone(),
    });
    input.proposal.max_changed_spans = Some(2);
    input.locked_chord_entries = vec![1];
    let result = suggest_phrase_harmony(&source, &input);
    assert!(result.diagnostics.is_empty(), "{:?}", result.diagnostics);
    assert!(
        result
            .proposals
            .candidates
            .iter()
            .all(|item| item.plan.entries[1] == entries[1])
    );
    input.phrases[0].pace = PhraseHarmonicPace::EverySecondBar;
    let rejected = suggest_phrase_harmony(&source, &input);
    assert_eq!(
        rejected.diagnostics[0].code,
        "source-harmonic-positions-mismatch"
    );
    assert!(rejected.proposals.candidates.is_empty());
}

#[test]
fn explicit_reanchor_splits_and_merges_regions_with_traced_provenance() {
    let source = score(4, 4, 4);
    let original = authored_plan(4);
    let mut split = request(4, PhraseHarmonicPace::TwoGroupsPerBar);
    split.proposal.source_plan = Some(original.clone());
    split.source_reanchor = Some(vec![0, 0, 1, 1, 2, 2, 3, 3]);
    split.locked_chord_entries = vec![1];
    split.local_keys = vec![ChordKeyRegion {
        start_measure: 2,
        end_measure_exclusive: 4,
        key: KeySignature {
            fifths: 1,
            mode: "major".into(),
        },
    }];
    let split_result = suggest_phrase_harmony(&source, &split);
    assert!(
        split_result.diagnostics.is_empty(),
        "{:?}",
        split_result.diagnostics
    );
    let reanchored = split_result.reanchored_source_plan.as_ref().unwrap();
    assert_eq!(reanchored.entries.len(), 8);
    assert_eq!(reanchored.entries[3].position.offset.beats(), 2.0);
    assert_eq!(
        reanchored.entries[3].origin,
        ChordPlanOrigin::GeneratedReharmonization
    );
    assert_eq!(reanchored.entries[3].sources, original.entries[1].sources);
    assert!(split_result.proposals.candidates.iter().all(|candidate| {
        [2, 3].into_iter().all(|target| {
            candidate.plan.entries.iter().any(|entry| {
                entry.position == split_result.positions[target]
                    && entry.chord == original.entries[1].chord
                    && entry.sources == reanchored.entries[target].sources
            })
        })
    }));
    assert!(split_result.proposals.candidates.iter().all(|candidate| {
        candidate
            .reason_codes
            .contains(&"explicit-source-reanchor".into())
    }));
    let chosen = &split_result.proposals.candidates[0];
    let replay = arrange_from_chord_proposal(&source, &chosen.resolved, "held-chord");
    assert!(replay.diagnostics.is_empty(), "{:?}", replay.diagnostics);
    assert!(replay.arrangement.unwrap().score.is_some());

    let mut merge = request(4, PhraseHarmonicPace::EverySecondBar);
    merge.proposal.source_plan = Some(original.clone());
    merge.source_reanchor = Some(vec![0, 2]);
    let merged = suggest_phrase_harmony(&source, &merge);
    assert!(merged.diagnostics.is_empty(), "{:?}", merged.diagnostics);
    assert_eq!(merged.positions.len(), 2);
    assert_eq!(merged.omitted_source_entries, vec![1, 3]);
    assert_eq!(
        merged.reanchored_source_plan.unwrap().entries[1].sources,
        original.entries[2].sources
    );

    let mut contrasting = original;
    contrasting.entries[1].chord =
        serde_json::from_value(serde_json::json!({"root":"G","kind":"major"})).unwrap();
    merge.proposal.source_plan = Some(contrasting);
    merge.proposal.max_changed_spans = Some(0);
    let over_budget = suggest_phrase_harmony(&source, &merge);
    assert!(over_budget.proposals.candidates.is_empty());
    assert!(
        over_budget
            .diagnostics
            .iter()
            .any(|issue| issue.code == "change-budget-exceeded-after-reanchor")
    );
    merge.proposal.max_changed_spans = Some(1);
    let within_budget = suggest_phrase_harmony(&source, &merge);
    assert!(!within_budget.proposals.candidates.is_empty());
    assert!(within_budget.proposals.candidates.iter().all(|candidate| {
        candidate
            .changed_spans
            .iter()
            .any(|span| span.start.measure_index == 1)
    }));
}

#[test]
fn reanchor_rejects_implicit_or_moved_locked_regions_atomically() {
    let source = score(4, 4, 4);
    let mut input = request(4, PhraseHarmonicPace::EverySecondBar);
    input.proposal.source_plan = Some(authored_plan(4));
    input.source_reanchor = Some(vec![0, 2]);
    input.locked_chord_entries = vec![1];
    let omitted_lock = suggest_phrase_harmony(&source, &input);
    assert_eq!(
        omitted_lock.diagnostics[0].code,
        "locked-source-region-moved"
    );
    assert!(omitted_lock.proposals.candidates.is_empty());
    assert!(omitted_lock.reanchored_source_plan.is_none());

    input.locked_chord_entries = vec![2];
    let moved_end = suggest_phrase_harmony(&source, &input);
    assert_eq!(moved_end.diagnostics[0].code, "locked-source-region-moved");

    input.locked_chord_entries.clear();
    input.source_reanchor = Some(vec![2, 0]);
    let invalid_order = suggest_phrase_harmony(&source, &input);
    assert_eq!(invalid_order.diagnostics[0].code, "invalid-source-reanchor");

    input.proposal.source_plan = None;
    let missing_source = suggest_phrase_harmony(&source, &input);
    assert_eq!(
        missing_source.diagnostics[0].code,
        "reanchor-requires-source-plan"
    );
}

#[test]
fn supplied_local_key_changes_harmony_without_rewriting_the_score() {
    let mut source = score(4, 4, 4);
    for measure in &mut source.parts[0].staves[0].measures[2..] {
        measure.voices[0] = [66, 69, 72, 66]
            .into_iter()
            .map(|midi| Note::new(Pitch::from_midi(midi, false), Duration::Quarter))
            .collect();
    }
    let before = serde_json::to_value(&source).unwrap();
    let mut input = request(4, PhraseHarmonicPace::EveryBar);
    let baseline = suggest_phrase_harmony(&source, &input);
    input.local_keys = vec![ChordKeyRegion {
        start_measure: 2,
        end_measure_exclusive: 4,
        key: KeySignature {
            fifths: 1,
            mode: "major".into(),
        },
    }];
    let changed = suggest_phrase_harmony(&source, &input);
    assert!(changed.diagnostics.is_empty(), "{:?}", changed.diagnostics);
    assert_eq!(changed.applied_local_keys, input.local_keys);
    let baseline_plan = &baseline
        .proposals
        .candidates
        .iter()
        .find(|candidate| candidate.id == "conservative-v2")
        .unwrap()
        .plan;
    let local_plan = &changed
        .proposals
        .candidates
        .iter()
        .find(|candidate| candidate.id == "conservative-v2")
        .unwrap()
        .plan;
    assert!(local_plan.entries[2..].iter().any(|entry| {
        !baseline_plan.entries[2..]
            .iter()
            .any(|old| old.position == entry.position && old.chord == entry.chord)
    }));
    assert_eq!(serde_json::to_value(&source).unwrap(), before);
    assert_eq!(
        changed.proposals.input_fingerprint,
        baseline.proposals.input_fingerprint
    );
    let chosen = &changed.proposals.candidates[0];
    let replay = arrange_from_chord_proposal(&source, &chosen.resolved, "held-chord");
    assert!(replay.diagnostics.is_empty(), "{:?}", replay.diagnostics);
    assert!(replay.arrangement.unwrap().score.is_some());
}

#[test]
fn local_key_regions_must_use_harmonic_bar_boundaries_and_valid_keys() {
    let source = score(4, 4, 4);
    let mut input = request(4, PhraseHarmonicPace::EverySecondBar);
    input.local_keys = vec![ChordKeyRegion {
        start_measure: 1,
        end_measure_exclusive: 4,
        key: KeySignature::default(),
    }];
    for case in 0..4 {
        match case {
            0 => {}
            1 => {
                input.local_keys[0].start_measure = 0;
                input.local_keys[0].end_measure_exclusive = 3;
            }
            2 => {
                input.local_keys[0].end_measure_exclusive = 4;
                input.local_keys[0].key.fifths = 8;
            }
            3 => {
                input.local_keys[0].key.fifths = 0;
                input.local_keys[0].key.mode = "dorian".into();
            }
            _ => unreachable!(),
        }
        let rejected = suggest_phrase_harmony(&source, &input);
        assert_eq!(rejected.diagnostics[0].code, "invalid-local-key-region");
        assert!(rejected.positions.is_empty());
        assert!(rejected.proposals.candidates.is_empty());
    }
    input.local_keys = vec![
        ChordKeyRegion {
            start_measure: 0,
            end_measure_exclusive: 4,
            key: KeySignature::default(),
        },
        ChordKeyRegion {
            start_measure: 2,
            end_measure_exclusive: 4,
            key: KeySignature::default(),
        },
    ];
    let overlap = suggest_phrase_harmony(&source, &input);
    assert_eq!(overlap.diagnostics[0].code, "invalid-local-key-region");
}

#[test]
fn cadence_targets_constrain_approach_and_arrival_without_touching_melody() {
    let source = score(4, 4, 4);
    let original = serde_json::to_value(&source).unwrap();
    for (kind, approach, arrival) in [
        (ChordCadenceKind::DominantTonic, "G", "C"),
        (ChordCadenceKind::SubdominantTonic, "F", "C"),
        (ChordCadenceKind::DominantArrival, "", "G"),
    ] {
        let mut input = request(4, PhraseHarmonicPace::EveryBar);
        input.cadences = vec![ChordCadenceTarget {
            arrival_position_index: 3,
            kind,
        }];
        let result = suggest_phrase_harmony(&source, &input);
        assert!(
            result.diagnostics.is_empty(),
            "{kind:?}: {:?}",
            result.diagnostics
        );
        assert!(!result.proposals.candidates.is_empty(), "{kind:?}");
        assert_eq!(result.applied_cadences, input.cadences);
        for candidate in &result.proposals.candidates {
            assert_eq!(candidate.plan.entries.last().unwrap().chord.root, arrival);
            if !approach.is_empty() {
                assert_eq!(
                    candidate.plan.entries[candidate.plan.entries.len() - 2]
                        .chord
                        .root,
                    approach
                );
            }
            assert!(
                candidate
                    .reason_codes
                    .contains(&"caller-supplied-cadence".into())
            );
        }
        let selected = &result.proposals.candidates[0];
        let replay = arrange_from_chord_proposal(&source, &selected.resolved, "held-chord");
        assert!(replay.diagnostics.is_empty(), "{:?}", replay.diagnostics);
        assert!(replay.arrangement.unwrap().score.is_some());
    }
    assert_eq!(serde_json::to_value(&source).unwrap(), original);
}

#[test]
fn cadence_rejects_invalid_targets_and_conflicting_source_locks() {
    let source = score(4, 4, 4);
    let mut input = request(4, PhraseHarmonicPace::EveryBar);
    input.cadences = vec![ChordCadenceTarget {
        arrival_position_index: 0,
        kind: ChordCadenceKind::DominantTonic,
    }];
    let invalid = suggest_phrase_harmony(&source, &input);
    assert_eq!(invalid.diagnostics[0].code, "invalid-cadence-target");
    assert!(invalid.proposals.candidates.is_empty());
    input.cadences[0].arrival_position_index = 3;
    input.cadences.push(ChordCadenceTarget {
        arrival_position_index: 2,
        kind: ChordCadenceKind::DominantArrival,
    });
    let overlap = suggest_phrase_harmony(&source, &input);
    assert_eq!(overlap.diagnostics[0].code, "invalid-cadence-target");
    input.cadences.pop();
    input.proposal.source_plan = Some(authored_plan(4));
    input.locked_chord_entries = vec![2];
    let conflicting = suggest_phrase_harmony(&source, &input);
    assert!(conflicting.proposals.candidates.is_empty());
    assert!(
        conflicting
            .diagnostics
            .iter()
            .any(|issue| issue.code == "cadence-incompatible")
    );

    input.locked_chord_entries.clear();
    input.proposal.max_candidates = 1;
    let one = suggest_phrase_harmony(&source, &input);
    assert!(one.diagnostics.is_empty(), "{:?}", one.diagnostics);
    assert_eq!(one.proposals.candidates.len(), 1);
    assert_eq!(one.proposals.candidates[0].resolved.max_candidates, 1);
    assert_eq!(one.proposals.candidates[0].plan.entries[2].chord.root, "G");
    assert_eq!(one.proposals.candidates[0].plan.entries[3].chord.root, "C");
}

#[test]
fn gaps_invalid_groups_and_unanchored_locks_fail_atomically() {
    let source = score(5, 8, 4);
    for case in 0..4 {
        let mut input = request(4, PhraseHarmonicPace::TwoGroupsPerBar);
        match case {
            0 => input.phrases[0].start_measure = 1,
            1 => input.phrases[0].meter_groups = vec![2, 2],
            2 => input.locked_chord_entries = vec![0],
            3 => input.phrases[0].end_measure_exclusive = 3,
            _ => unreachable!(),
        }
        let result = suggest_phrase_harmony(&source, &input);
        assert!(result.positions.is_empty(), "case {case}");
        assert!(result.proposals.candidates.is_empty());
        assert!(!result.diagnostics.is_empty());
    }
}
