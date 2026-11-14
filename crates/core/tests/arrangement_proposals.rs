use acorde_core::{
    ArrangementProposalRequest, ArrangementRegionRequest, ChordProposalRequest, Duration,
    EnsembleRole, EnsembleRoleOptions, EnsembleRoleSection, Note, Pitch, Score,
    regenerate_arrangement_region, replay_arrangement_proposal, suggest_arrangement_proposals,
};
use std::collections::BTreeSet;

fn source() -> Score {
    let mut score = Score::new("proposal source", 120, 4, 4, 0, 4);
    for bar in &mut score.parts[0].staves[0].measures {
        bar.voices[0] = [84, 86, 88, 84]
            .into_iter()
            .map(|midi| Note::new(Pitch::from_midi(midi, false), Duration::Quarter))
            .collect();
    }
    score
}

fn request(count: u8) -> ArrangementProposalRequest {
    let ensemble_template: EnsembleRoleOptions = serde_json::from_value(serde_json::json!({
        "harmony": {
            "chords":[], "pattern":"held", "voicing":"smooth",
            "midi_program":48, "low_midi":48, "center_midi":62,
            "high_midi":76, "melody_part_index":0,
            "min_melody_gap_semitones":3
        },
        "bass": {
            "chords":[], "pattern":"root-pulse", "midi_program":32,
            "low_midi":28, "center_midi":40, "high_midi":52
        },
        "min_bass_harmony_gap_semitones":3
    }))
    .unwrap();
    ArrangementProposalRequest {
        harmony: ChordProposalRequest::default(),
        locked_chord_entries: Vec::new(),
        ensemble_template,
        count,
    }
}

fn generated_events(score: &Score) -> Vec<(usize, u64, u64, u8)> {
    let mut events = acorde_core::to_playback_events(score, &Default::default())
        .into_iter()
        .filter_map(|event| {
            let part = event.source?.part;
            (part > 0).then_some((
                part,
                (event.time_beats * 480.0).round() as u64,
                (event.duration_beats * 480.0).round() as u64,
                event.pitch_midi,
            ))
        })
        .collect::<Vec<_>>();
    events.sort();
    events
}

#[test]
fn five_distinct_complete_proposals_replay_without_source_changes() {
    let score = source();
    let proposals = suggest_arrangement_proposals(&score, &request(5));
    assert!(
        proposals.diagnostics.is_empty(),
        "{:?}",
        proposals.diagnostics
    );
    assert_eq!(proposals.candidates.len(), 5);
    let events = proposals
        .candidates
        .iter()
        .map(|candidate| {
            assert_eq!(candidate.score.parts.len(), 3);
            assert_eq!(
                serde_json::to_value(&candidate.score.parts[0]).unwrap(),
                serde_json::to_value(&score.parts[0]).unwrap()
            );
            assert!(candidate.features.harmony_attack_count > 0);
            assert!(candidate.features.bass_attack_count > 0);
            assert!(!candidate.reason_codes.is_empty());
            let replay = replay_arrangement_proposal(&score, &candidate.resolved);
            assert!(replay.diagnostics.is_empty(), "{:?}", replay.diagnostics);
            assert_eq!(
                generated_events(&candidate.score),
                generated_events(&replay.score.unwrap())
            );
            generated_events(&candidate.score)
        })
        .collect::<Vec<_>>();
    for (index, left) in events.iter().enumerate() {
        assert!(events[index + 1..].iter().all(|right| right != left));
    }
    assert!(
        proposals.candidates[1..]
            .iter()
            .all(|candidate| !candidate.difference_codes.is_empty())
    );
}

fn request_with_countermelody() -> ArrangementProposalRequest {
    let mut request = request(5);
    let harmony = request.ensemble_template.harmony.as_mut().unwrap();
    harmony.low_midi = 44;
    harmony.center_midi = 58;
    harmony.high_midi = 71;
    request.ensemble_template.countermelody = Some(
        serde_json::from_value(serde_json::json!({
            "chords":[], "melody_part_index":0,
            "motif_start_measure":0, "motif_end_measure_exclusive":1,
            "midi_program":73, "low_midi":75, "center_midi":77,
            "high_midi":80, "min_melody_gap_semitones":4
        }))
        .unwrap(),
    );
    request
        .ensemble_template
        .min_harmony_countermelody_gap_semitones = 3;
    request
}

#[test]
fn countermelody_changes_orchestration_when_requested() {
    let score = source();
    let request = request_with_countermelody();
    let result = suggest_arrangement_proposals(&score, &request);
    assert!(result.diagnostics.is_empty(), "{:?}", result.diagnostics);
    assert_eq!(result.candidates.len(), 5);
    assert!(result.candidates.iter().any(|candidate| {
        candidate.features.countermelody_attack_count > 0
            && candidate
                .difference_codes
                .contains(&"orchestration-changed".to_string())
    }));
}

#[test]
fn three_proposals_include_a_countermelody_when_requested() {
    let score = source();
    let mut request = request_with_countermelody();
    request.count = 3;
    request.ensemble_template.countermelody_dynamic = Some(acorde_core::Dynamic::P);
    let result = suggest_arrangement_proposals(&score, &request);
    assert!(result.diagnostics.is_empty(), "{:?}", result.diagnostics);
    assert_eq!(result.candidates.len(), 3);
    assert_eq!(result.candidates[0].features.countermelody_attack_count, 0);
    assert!(
        result.candidates[0]
            .resolved
            .ensemble_options
            .countermelody_dynamic
            .is_none()
    );
    assert!(result.candidates.iter().any(|candidate| {
        candidate.features.countermelody_attack_count > 0
            && candidate.resolved.ensemble_options.countermelody_dynamic
                == Some(acorde_core::Dynamic::P)
            && candidate
                .difference_codes
                .contains(&"orchestration-changed".to_string())
    }));
    for candidate in &result.candidates {
        let replay = replay_arrangement_proposal(&score, &candidate.resolved);
        assert!(replay.diagnostics.is_empty(), "{:?}", replay.diagnostics);
        assert_eq!(
            generated_events(&candidate.score),
            generated_events(&replay.score.unwrap())
        );
    }
}

#[test]
fn eight_measure_proposals_keep_role_sections_and_phrase_endings() {
    let fixture: serde_json::Value = serde_json::from_str(include_str!(
        "../../../tests/fixtures/arrangement/phrase-development-v1.json"
    ))
    .unwrap();
    let case = &fixture["cases"][0];
    let melody: Vec<Vec<u8>> = serde_json::from_value(case["melody"].clone()).unwrap();
    let phrase_end_measures: Vec<usize> =
        serde_json::from_value(case["phrase_end_measures"].clone()).unwrap();
    let harmony_meter_groups: Vec<u8> =
        serde_json::from_value(case["harmony_meter_groups"].clone()).unwrap();
    let counter_inactive_ranges: Vec<[usize; 2]> =
        serde_json::from_value(case["counter_inactive_ranges"].clone()).unwrap();
    assert_eq!(melody.len(), 8);
    let mut score = Score::new("long form proposal", 120, 4, 4, 0, melody.len() as u32);
    for (bar, pitches) in score.parts[0].staves[0].measures.iter_mut().zip(melody) {
        bar.voices[0] = pitches
            .into_iter()
            .map(|midi| Note::new(Pitch::from_midi(midi, false), Duration::Quarter))
            .collect();
    }
    let mut request = request_with_countermelody();
    request.count = 5;
    let harmony = request.ensemble_template.harmony.as_mut().unwrap();
    harmony.midi_program = 0;
    harmony.low_midi = 40;
    harmony.center_midi = 45;
    harmony.high_midi = 50;
    harmony.min_melody_gap_semitones = Some(4);
    harmony.phrase_end_measures = phrase_end_measures.clone();
    harmony.meter_group_sections = vec![acorde_core::ChordalPadMeterGroupSection {
        start_measure: 0,
        end_measure_exclusive: 8,
        groups: harmony_meter_groups,
    }];
    let bass = request.ensemble_template.bass.as_mut().unwrap();
    bass.midi_program = 0;
    bass.low_midi = 28;
    bass.center_midi = 33;
    bass.high_midi = 39;
    let counter = request.ensemble_template.countermelody.as_mut().unwrap();
    counter.midi_program = 0;
    counter.low_midi = 53;
    counter.center_midi = 56;
    counter.high_midi = 61;
    counter.min_melody_gap_semitones = 3;
    request
        .ensemble_template
        .min_harmony_countermelody_gap_semitones = 2;
    request
        .ensemble_template
        .countermelody
        .as_mut()
        .unwrap()
        .phrase_end_measures = phrase_end_measures;
    request.ensemble_template.sections = counter_inactive_ranges
        .into_iter()
        .map(
            |[start_measure, end_measure_exclusive]| EnsembleRoleSection {
                role: EnsembleRole::Countermelody,
                start_measure,
                end_measure_exclusive,
                enabled: false,
            },
        )
        .collect();
    request
        .ensemble_template
        .restart_countermelody_at_section_entry = true;
    let proposals = suggest_arrangement_proposals(&score, &request);
    assert!(
        proposals.diagnostics.is_empty(),
        "{:?}",
        proposals.diagnostics
    );
    assert_eq!(proposals.candidates.len(), 5);
    assert_eq!(
        proposals
            .candidates
            .iter()
            .filter(|candidate| candidate.features.countermelody_attack_count > 0)
            .count(),
        2
    );
    assert_eq!(
        proposals
            .candidates
            .iter()
            .map(|candidate| candidate.resolved.harmony_candidate_id.as_str())
            .collect::<BTreeSet<_>>()
            .len(),
        3
    );
    assert_eq!(
        proposals
            .candidates
            .iter()
            .map(|candidate| candidate.resolved.profile_id.as_str())
            .collect::<BTreeSet<_>>()
            .len(),
        5
    );
    let harmony_onsets = proposals
        .candidates
        .iter()
        .map(|candidate| {
            acorde_core::to_playback_events(&candidate.score, &Default::default())
                .into_iter()
                .filter(|event| event.source.as_ref().is_some_and(|source| source.part == 1))
                .map(|event| (event.time_beats * 480.0).round() as u64)
                .collect::<BTreeSet<_>>()
                .into_iter()
                .collect::<Vec<_>>()
        })
        .collect::<BTreeSet<_>>();
    assert_eq!(harmony_onsets.len(), 5);
    let offbeat = proposals
        .candidates
        .iter()
        .find(|candidate| candidate.resolved.profile_id == "offbeat-fifth")
        .unwrap();
    let steady = proposals
        .candidates
        .iter()
        .find(|candidate| candidate.resolved.profile_id == "steady-fifth")
        .unwrap();
    assert!(
        proposals
            .candidates
            .iter()
            .any(|candidate| candidate.resolved.profile_id == "meter-answer")
    );
    assert_eq!(
        offbeat.features.generated_attacks_by_measure,
        steady.features.generated_attacks_by_measure
    );
    assert_ne!(
        generated_events(&offbeat.score),
        generated_events(&steady.score)
    );
    let mut ungrouped = request.clone();
    let harmony = ungrouped.ensemble_template.harmony.as_mut().unwrap();
    harmony.phrase_end_measures.clear();
    harmony.meter_group_sections.clear();
    let fallback = suggest_arrangement_proposals(&score, &ungrouped);
    assert!(
        fallback.diagnostics.is_empty(),
        "{:?}",
        fallback.diagnostics
    );
    assert_eq!(fallback.candidates.len(), 5);
    assert!(
        fallback
            .candidates
            .iter()
            .any(|candidate| candidate.resolved.profile_id == "meter-fifth-answer")
    );
    assert_eq!(
        proposals.candidates[0].features.countermelody_attack_count,
        0
    );
    for candidate in &proposals.candidates {
        assert_eq!(candidate.features.generated_attacks_by_measure.len(), 8);
        assert_eq!(
            serde_json::to_value(&candidate.score.parts[0]).unwrap(),
            serde_json::to_value(&score.parts[0]).unwrap()
        );
        let replay = replay_arrangement_proposal(&score, &candidate.resolved);
        assert!(replay.diagnostics.is_empty(), "{:?}", replay.diagnostics);
        assert_eq!(
            generated_events(&candidate.score),
            generated_events(&replay.score.unwrap())
        );
        if candidate.features.countermelody_attack_count > 0 {
            let bars = &candidate.score.parts[3].staves[0].measures;
            let attacks = |measure: usize| {
                bars[measure].voices[0]
                    .iter()
                    .filter(|note| !note.is_rest && !note.tie_end)
                    .count()
            };
            assert!(
                [0, 1, 4, 5]
                    .into_iter()
                    .all(|measure| attacks(measure) == 0)
            );
            assert!(attacks(2) > attacks(3));
            assert!(attacks(6) > attacks(7));
        }
    }
    let region = regenerate_arrangement_region(
        &score,
        ArrangementRegionRequest {
            original: proposals.candidates[0].resolved.clone(),
            replacement: proposals.candidates[1].resolved.clone(),
            start_measure: 2,
            end_measure_exclusive: 4,
            locked_roles: vec![EnsembleRole::Harmony, EnsembleRole::Bass],
            locked_measures: Vec::new(),
            locked_chord_entries: Vec::new(),
        },
    );
    assert!(region.diagnostics.is_empty(), "{:?}", region.diagnostics);
    assert_eq!(region.changed_roles, vec![EnsembleRole::Countermelody]);
    assert_eq!(region.changed_measures, vec![2, 3]);
    let output = region.score.unwrap();
    assert_eq!(output.parts.len(), 4);
    assert!([0, 1, 4, 5, 6, 7].into_iter().all(|measure| {
        output.parts[3].staves[0].measures[measure].voices[0]
            .iter()
            .all(|note| note.is_rest)
    }));
    assert!(acorde_core::model::validate::validate(&output).is_valid());
}

#[test]
fn compound_eight_measure_proposals_preserve_grouped_phrases() {
    let fixture: serde_json::Value = serde_json::from_str(include_str!(
        "../../../tests/fixtures/arrangement/phrase-compound-development-v1.json"
    ))
    .unwrap();
    let case = &fixture["cases"][0];
    assert_eq!(case["meter"], serde_json::json!([6, 8]));
    assert_eq!(case["melody_eighth_notes"], true);
    let melody: Vec<Vec<u8>> = serde_json::from_value(case["melody"].clone()).unwrap();
    let phrase_ends: Vec<usize> =
        serde_json::from_value(case["phrase_end_measures"].clone()).unwrap();
    let groups: Vec<u8> = serde_json::from_value(case["harmony_meter_groups"].clone()).unwrap();
    let inactive: Vec<[usize; 2]> =
        serde_json::from_value(case["counter_inactive_ranges"].clone()).unwrap();
    let mut score = Score::new("compound phrase", 120, 6, 8, 0, melody.len() as u32);
    for (bar, pitches) in score.parts[0].staves[0].measures.iter_mut().zip(melody) {
        bar.voices[0] = pitches
            .into_iter()
            .map(|midi| Note::new(Pitch::from_midi(midi, false), Duration::Eighth))
            .collect();
    }
    assert!(acorde_core::model::validate::validate(&score).is_valid());
    let mut request = request_with_countermelody();
    let harmony = request.ensemble_template.harmony.as_mut().unwrap();
    harmony.midi_program = 0;
    harmony.low_midi = 40;
    harmony.center_midi = 45;
    harmony.high_midi = 50;
    harmony.min_melody_gap_semitones = Some(4);
    harmony.phrase_end_measures = phrase_ends.clone();
    harmony.meter_group_sections = vec![acorde_core::ChordalPadMeterGroupSection {
        start_measure: 0,
        end_measure_exclusive: 8,
        groups,
    }];
    let bass = request.ensemble_template.bass.as_mut().unwrap();
    bass.midi_program = 0;
    bass.low_midi = 28;
    bass.center_midi = 33;
    bass.high_midi = 39;
    let counter = request.ensemble_template.countermelody.as_mut().unwrap();
    counter.midi_program = 0;
    counter.low_midi = 53;
    counter.center_midi = 56;
    counter.high_midi = 61;
    counter.min_melody_gap_semitones = 3;
    counter.phrase_end_measures = phrase_ends;
    request
        .ensemble_template
        .min_harmony_countermelody_gap_semitones = 2;
    request.ensemble_template.sections = inactive
        .into_iter()
        .map(
            |[start_measure, end_measure_exclusive]| EnsembleRoleSection {
                role: EnsembleRole::Countermelody,
                start_measure,
                end_measure_exclusive,
                enabled: false,
            },
        )
        .collect();
    request
        .ensemble_template
        .restart_countermelody_at_section_entry = true;

    let proposals = suggest_arrangement_proposals(&score, &request);
    assert!(
        proposals.diagnostics.is_empty(),
        "{:?}",
        proposals.diagnostics
    );
    assert_eq!(proposals.candidates.len(), 5);
    assert_eq!(
        proposals
            .candidates
            .iter()
            .map(|candidate| candidate.resolved.profile_id.as_str())
            .collect::<BTreeSet<_>>()
            .len(),
        5
    );
    assert_eq!(
        proposals
            .candidates
            .iter()
            .map(|candidate| candidate.resolved.harmony_candidate_id.as_str())
            .collect::<BTreeSet<_>>()
            .len(),
        3
    );
    assert_eq!(
        proposals
            .candidates
            .iter()
            .filter(|candidate| candidate.features.countermelody_attack_count > 0)
            .count(),
        2
    );
    let metered = proposals
        .candidates
        .iter()
        .find(|candidate| candidate.resolved.profile_id == "meter-answer")
        .unwrap();
    let harmony_attacks = |measure: usize| {
        metered.score.parts[1].staves[0].measures[measure].voices[0]
            .iter()
            .filter(|note| !note.is_rest && !note.tie_end)
            .count()
    };
    assert!(harmony_attacks(2) > harmony_attacks(3));
    assert!(harmony_attacks(6) > harmony_attacks(7));
    for candidate in &proposals.candidates {
        assert_eq!(candidate.features.generated_attacks_by_measure.len(), 8);
        assert_eq!(
            serde_json::to_value(&candidate.score.parts[0]).unwrap(),
            serde_json::to_value(&score.parts[0]).unwrap()
        );
        let replay = replay_arrangement_proposal(&score, &candidate.resolved);
        assert!(replay.diagnostics.is_empty(), "{:?}", replay.diagnostics);
        assert_eq!(
            generated_events(&candidate.score),
            generated_events(&replay.score.unwrap())
        );
        if candidate.features.countermelody_attack_count > 0 {
            let bars = &candidate.score.parts[3].staves[0].measures;
            let attacks = |measure: usize| {
                bars[measure].voices[0]
                    .iter()
                    .filter(|note| !note.is_rest && !note.tie_end)
                    .count()
            };
            assert!(
                [0, 1, 3, 4, 5, 7]
                    .into_iter()
                    .all(|measure| attacks(measure) == 0)
            );
            assert!(attacks(2) > 0);
            assert!(attacks(6) > 0);
        }
    }
}

#[test]
fn region_can_add_and_rest_countermelody_without_changing_other_roles() {
    let score = source();
    let mut request = request_with_countermelody();
    request.ensemble_template.countermelody_dynamic = Some(acorde_core::Dynamic::P);
    let proposals = suggest_arrangement_proposals(&score, &request);
    assert!(
        proposals.diagnostics.is_empty(),
        "{:?}",
        proposals.diagnostics
    );
    let plain = proposals
        .candidates
        .iter()
        .find(|candidate| candidate.features.countermelody_attack_count == 0)
        .unwrap();
    let with_counter = proposals
        .candidates
        .iter()
        .find(|candidate| candidate.features.countermelody_attack_count > 0)
        .unwrap();
    let region = |original: &acorde_core::ArrangementProposalCandidate,
                  replacement: &acorde_core::ArrangementProposalCandidate| {
        ArrangementRegionRequest {
            original: original.resolved.clone(),
            replacement: replacement.resolved.clone(),
            start_measure: 1,
            end_measure_exclusive: 3,
            locked_roles: vec![EnsembleRole::Harmony, EnsembleRole::Bass],
            locked_measures: vec![2],
            locked_chord_entries: Vec::new(),
        }
    };
    let added = regenerate_arrangement_region(&score, region(plain, with_counter));
    assert!(added.diagnostics.is_empty(), "{:?}", added.diagnostics);
    assert_eq!(added.changed_roles, vec![EnsembleRole::Countermelody]);
    assert_eq!(added.changed_measures, vec![1]);
    assert_eq!(added.role_parts.last().unwrap().part_index, 3);
    assert_eq!(
        added.role_parts.last().unwrap().role,
        EnsembleRole::Countermelody
    );
    let added_score = added.score.unwrap();
    assert_eq!(added_score.parts.len(), 4);
    for measure in [0, 2, 3] {
        assert!(
            added_score.parts[3].staves[0].measures[measure].voices[0]
                .iter()
                .all(|note| note.is_rest)
        );
    }
    assert!(
        added_score.parts[3].staves[0].measures[1].voices[0]
            .iter()
            .any(|note| !note.is_rest)
    );
    assert_eq!(
        added_score.parts[3].staves[0].measures[1].voices[0][0].dynamic,
        Some(acorde_core::Dynamic::P)
    );
    assert!(acorde_core::model::validate::validate(&added_score).is_valid());

    let removed = regenerate_arrangement_region(&score, region(with_counter, plain));
    assert!(removed.diagnostics.is_empty(), "{:?}", removed.diagnostics);
    assert_eq!(removed.changed_roles, vec![EnsembleRole::Countermelody]);
    assert_eq!(removed.changed_measures, vec![1]);
    let removed_score = removed.score.unwrap();
    assert_eq!(removed_score.parts.len(), 4);
    assert!(
        removed_score.parts[3].staves[0].measures[1].voices[0]
            .iter()
            .all(|note| note.is_rest)
    );
    assert!(
        removed_score.parts[3].staves[0].measures[2].voices[0]
            .iter()
            .any(|note| !note.is_rest)
    );
    assert!(acorde_core::model::validate::validate(&removed_score).is_valid());
}

#[test]
fn region_rejects_a_global_dynamic_change_that_would_leak_outside_it() {
    let score = source();
    let proposals = suggest_arrangement_proposals(&score, &request(3));
    assert!(
        proposals.diagnostics.is_empty(),
        "{:?}",
        proposals.diagnostics
    );
    let mut replacement = proposals.candidates[1].resolved.clone();
    replacement.ensemble_options.harmony_dynamic = Some(acorde_core::Dynamic::Pp);
    let result = regenerate_arrangement_region(
        &score,
        ArrangementRegionRequest {
            original: proposals.candidates[0].resolved.clone(),
            replacement,
            start_measure: 1,
            end_measure_exclusive: 2,
            locked_roles: Vec::new(),
            locked_measures: Vec::new(),
            locked_chord_entries: Vec::new(),
        },
    );
    assert_eq!(result.diagnostics[0].code, "incompatible-dynamics");
    assert!(result.score.is_none());
    assert!(result.role_parts.is_empty());
    assert!(result.changed_measures.is_empty());
}

#[test]
fn region_rejects_removing_the_only_dynamic_mark_before_later_notes() {
    let score = source();
    let mut request = request_with_countermelody();
    request.ensemble_template.countermelody_dynamic = Some(acorde_core::Dynamic::P);
    let proposals = suggest_arrangement_proposals(&score, &request);
    assert!(
        proposals.diagnostics.is_empty(),
        "{:?}",
        proposals.diagnostics
    );
    let plain = proposals
        .candidates
        .iter()
        .find(|candidate| candidate.features.countermelody_attack_count == 0)
        .unwrap();
    let with_counter = proposals
        .candidates
        .iter()
        .find(|candidate| candidate.features.countermelody_attack_count > 0)
        .unwrap();
    let result = regenerate_arrangement_region(
        &score,
        ArrangementRegionRequest {
            original: with_counter.resolved.clone(),
            replacement: plain.resolved.clone(),
            start_measure: 0,
            end_measure_exclusive: 1,
            locked_roles: vec![EnsembleRole::Harmony, EnsembleRole::Bass],
            locked_measures: Vec::new(),
            locked_chord_entries: Vec::new(),
        },
    );
    assert_eq!(result.diagnostics[0].code, "outside-playback-changed");
    assert!(result.score.is_none());
    assert!(result.changed_roles.is_empty());
}

#[test]
fn locked_harmony_entry_is_retained_across_complete_proposals() {
    let score = source();
    let mut request = request(5);
    let original_plan = acorde_core::suggest_chord_plans(&score, &request.harmony).candidates[0]
        .plan
        .clone();
    request.harmony.source_plan = Some(original_plan.clone());
    request.locked_chord_entries = vec![1];
    let proposals = suggest_arrangement_proposals(&score, &request);
    assert!(
        proposals.diagnostics.is_empty(),
        "{:?}",
        proposals.diagnostics
    );
    assert_eq!(proposals.candidates.len(), 5);
    for candidate in &proposals.candidates {
        let locked = candidate
            .plan
            .entries
            .iter()
            .find(|entry| entry.position == original_plan.entries[1].position)
            .unwrap();
        assert_eq!(locked.chord, original_plan.entries[1].chord);
    }
    request.locked_chord_entries = vec![999];
    let rejected = suggest_arrangement_proposals(&score, &request);
    assert!(rejected.candidates.is_empty());
    assert_eq!(rejected.diagnostics[0].code, "invalid-chord-lock");
}

#[test]
fn selected_melody_can_be_a_later_source_part() {
    let mut score = source();
    let mut melody = score.parts.remove(0);
    melody.midi_channel = 1;
    let mut foundation = Score::new("foundation", 120, 4, 4, 0, 4).parts.remove(0);
    for bar in &mut foundation.staves[0].measures {
        bar.voices[0] = vec![Note::new(Pitch::from_midi(36, false), Duration::Whole)];
    }
    score.parts.push(foundation);
    score.parts.push(melody);
    let mut request = request(3);
    request
        .ensemble_template
        .harmony
        .as_mut()
        .unwrap()
        .melody_part_index = Some(1);
    let proposals = suggest_arrangement_proposals(&score, &request);
    assert!(
        proposals.diagnostics.is_empty(),
        "{:?}",
        proposals.diagnostics
    );
    assert_eq!(proposals.candidates.len(), 3);
    for candidate in proposals.candidates {
        assert_eq!(candidate.score.parts.len(), 4);
        assert_eq!(
            serde_json::to_value(&candidate.score.parts[..2]).unwrap(),
            serde_json::to_value(&score.parts).unwrap()
        );
    }
}

#[test]
fn invalid_count_and_changed_source_return_no_proposals_or_score() {
    let score = source();
    let invalid = suggest_arrangement_proposals(&score, &request(2));
    assert!(invalid.candidates.is_empty());
    assert_eq!(invalid.diagnostics[0].code, "invalid-proposal-request");

    let candidate = suggest_arrangement_proposals(&score, &request(3))
        .candidates
        .remove(0);
    let mut changed = score.clone();
    changed.metadata.title.push_str(" changed");
    let replay = replay_arrangement_proposal(&changed, &candidate.resolved);
    assert!(replay.score.is_none());
    assert_eq!(replay.diagnostics[0].code, "input-mismatch");

    let mut tampered = candidate.resolved;
    tampered.plan.entries[0].offset_beats = 0.5;
    let rejected = replay_arrangement_proposal(&score, &tampered);
    assert!(rejected.score.is_none());
    assert_eq!(rejected.diagnostics[0].code, "inconsistent-resolved-plan");
}

#[test]
fn selected_region_changes_only_unlocked_role_and_measure() {
    let score = source();
    let candidates = suggest_arrangement_proposals(&score, &request(5)).candidates;
    let original = &candidates[1];
    let replacement = &candidates[2];
    let result = regenerate_arrangement_region(
        &score,
        ArrangementRegionRequest {
            original: original.resolved.clone(),
            replacement: replacement.resolved.clone(),
            start_measure: 1,
            end_measure_exclusive: 3,
            locked_roles: vec![EnsembleRole::Bass],
            locked_measures: vec![2],
            locked_chord_entries: Vec::new(),
        },
    );
    assert!(result.diagnostics.is_empty(), "{:?}", result.diagnostics);
    assert_eq!(result.changed_roles, vec![EnsembleRole::Harmony]);
    assert_eq!(result.changed_measures, vec![1]);
    let output = result.score.unwrap();
    let old = generated_events(&original.score);
    let new = generated_events(&output);
    let in_measure = |event: &(usize, u64, u64, u8), measure: usize| {
        event.1 >= measure as u64 * 4 * 480 && event.1 < (measure as u64 + 1) * 4 * 480
    };
    for measure in [0, 2, 3] {
        assert_eq!(
            old.iter()
                .filter(|event| in_measure(event, measure))
                .collect::<Vec<_>>(),
            new.iter()
                .filter(|event| in_measure(event, measure))
                .collect::<Vec<_>>()
        );
    }
    assert_eq!(
        old.iter().filter(|event| event.0 == 2).collect::<Vec<_>>(),
        new.iter().filter(|event| event.0 == 2).collect::<Vec<_>>()
    );
    assert_ne!(
        old.iter()
            .filter(|event| event.0 == 1 && in_measure(event, 1))
            .collect::<Vec<_>>(),
        new.iter()
            .filter(|event| event.0 == 1 && in_measure(event, 1))
            .collect::<Vec<_>>()
    );
    assert!(acorde_core::model::validate::validate(&output).is_valid());
}

#[test]
fn invalid_region_returns_no_partial_score() {
    let score = source();
    let candidates = suggest_arrangement_proposals(&score, &request(5)).candidates;
    let result = regenerate_arrangement_region(
        &score,
        ArrangementRegionRequest {
            original: candidates[0].resolved.clone(),
            replacement: candidates[1].resolved.clone(),
            start_measure: 2,
            end_measure_exclusive: 2,
            locked_roles: Vec::new(),
            locked_measures: Vec::new(),
            locked_chord_entries: Vec::new(),
        },
    );
    assert!(result.score.is_none());
    assert_eq!(result.diagnostics[0].code, "invalid-region");
    assert!(result.changed_roles.is_empty());
    assert!(result.changed_measures.is_empty());
}

#[test]
fn region_rejects_changed_locked_harmony_span() {
    let score = source();
    let candidate = suggest_arrangement_proposals(&score, &request(3))
        .candidates
        .remove(0);
    assert!(candidate.plan.entries.len() > 1);
    let mut replacement = candidate.resolved.clone();
    let different = serde_json::from_value(serde_json::json!({
        "root":"D", "kind":"minor"
    }))
    .unwrap();
    replacement.plan.entries[1].chord = different;
    replacement
        .ensemble_options
        .harmony
        .as_mut()
        .unwrap()
        .chords[1]
        .chord = replacement.plan.entries[1].chord.clone();
    replacement.ensemble_options.bass.as_mut().unwrap().chords[1].chord =
        replacement.plan.entries[1].chord.clone();
    let mut region = ArrangementRegionRequest {
        original: candidate.resolved,
        replacement,
        start_measure: 1,
        end_measure_exclusive: 3,
        locked_roles: Vec::new(),
        locked_measures: Vec::new(),
        locked_chord_entries: vec![1],
    };
    let rejected = regenerate_arrangement_region(&score, region.clone());
    assert!(rejected.score.is_none());
    assert_eq!(rejected.diagnostics[0].code, "locked-harmony-changed");
    assert!(rejected.changed_measures.is_empty());

    region.locked_chord_entries = vec![999];
    let invalid = regenerate_arrangement_region(&score, region);
    assert_eq!(invalid.diagnostics[0].code, "invalid-chord-lock");
}

#[test]
fn regenerating_with_the_same_proposal_reports_no_changed_measures() {
    let score = source();
    let candidate = suggest_arrangement_proposals(&score, &request(3))
        .candidates
        .remove(0);
    let result = regenerate_arrangement_region(
        &score,
        ArrangementRegionRequest {
            original: candidate.resolved.clone(),
            replacement: candidate.resolved,
            start_measure: 1,
            end_measure_exclusive: 3,
            locked_roles: Vec::new(),
            locked_measures: Vec::new(),
            locked_chord_entries: Vec::new(),
        },
    );
    assert!(result.diagnostics.is_empty(), "{:?}", result.diagnostics);
    assert!(result.score.is_some());
    assert!(result.changed_roles.is_empty());
    assert!(result.changed_measures.is_empty());
}
