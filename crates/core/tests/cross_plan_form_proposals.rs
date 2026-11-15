use std::collections::BTreeSet;

use acorde_core::{
    CrossPlanFormError, CrossPlanFormRequest, FormProposalBoundary, replay_cross_plan_form,
    suggest_cross_plan_forms, to_playback_events, validate,
};

#[allow(dead_code)]
#[path = "../../../tests/fixtures/arrangement/development_v2.rs"]
mod development_v2;
#[path = "../../../tests/fixtures/arrangement/development_v2_request.rs"]
mod development_v2_request;

fn request(case: &development_v2::Case) -> CrossPlanFormRequest {
    let mut arrangement = development_v2_request::request(case).unwrap();
    arrangement.count = 3;
    CrossPlanFormRequest {
        arrangement,
        sections: case
            .sections
            .iter()
            .map(|section| FormProposalBoundary {
                label: section.label.clone(),
                start_measure: section.start_measure,
                end_measure_exclusive: section.end_measure_exclusive,
            })
            .collect(),
        phrase_end_measures: Vec::new(),
    }
}

fn events(score: &acorde_core::Score) -> Vec<(usize, u64, u64, i32, u8)> {
    to_playback_events(score, &Default::default())
        .into_iter()
        .filter_map(|event| {
            event.source.map(|source| {
                (
                    source.part,
                    (event.time_beats * 480.0).round() as u64,
                    (event.duration_beats * 480.0).round() as u64,
                    event.pitch_midi_cents,
                    event.velocity,
                )
            })
        })
        .collect()
}

fn harmony(plan: &acorde_core::ChordPlan) -> Vec<(usize, u32, u32, String)> {
    plan.entries
        .iter()
        .map(|entry| {
            (
                entry.position.measure_index,
                entry.position.offset.numerator,
                entry.position.offset.denominator,
                serde_json::to_string(&entry.chord).unwrap(),
            )
        })
        .collect()
}

#[test]
fn complete_forms_cross_harmony_plans_without_changing_source_parts() {
    let corpus: development_v2::Corpus = serde_json::from_str(development_v2::FIXTURE).unwrap();
    let case = &corpus.cases[0];
    let source = development_v2::score(case);
    let result = suggest_cross_plan_forms(&source, &request(case)).unwrap();
    assert_eq!(result.candidates.len(), 3);
    let distinct_harmony = result
        .candidates
        .iter()
        .map(|candidate| harmony(&candidate.plan))
        .collect::<BTreeSet<_>>();
    assert!(distinct_harmony.len() >= 2);
    let mut generated = BTreeSet::new();
    for (index, candidate) in result.candidates.iter().enumerate() {
        assert!(validate(&candidate.score).is_valid());
        assert_eq!(
            serde_json::to_value(&candidate.score.parts[..source.parts.len()]).unwrap(),
            serde_json::to_value(&source.parts).unwrap()
        );
        assert!(generated.insert(events(&candidate.score)));
        assert_eq!(
            events(&candidate.score),
            events(&replay_cross_plan_form(&source, &candidate.resolved).unwrap())
        );
        if index > 0 {
            assert!(candidate.nearest_selected_distance > 0);
        }
    }
    assert!(result.candidates[1..].iter().any(|candidate| {
        candidate
            .difference_codes
            .contains(&"harmony-plan-changed".to_string())
            && candidate.axis_distance_from_first.harmony > 0
    }));
}

#[test]
fn changed_source_and_tampered_plan_fail_replay() {
    let corpus: development_v2::Corpus = serde_json::from_str(development_v2::FIXTURE).unwrap();
    let case = &corpus.cases[0];
    let source = development_v2::score(case);
    let result = suggest_cross_plan_forms(&source, &request(case)).unwrap();
    let resolved = &result.candidates[1].resolved;
    let mut changed_source = source.clone();
    changed_source.metadata.title.push('!');
    assert!(matches!(
        replay_cross_plan_form(&changed_source, resolved),
        Err(CrossPlanFormError::SourceMismatch)
    ));
    let mut changed_plan = resolved.clone();
    changed_plan.arrangement.plan.entries[0].chord.root = "D".into();
    assert!(matches!(
        replay_cross_plan_form(&source, &changed_plan),
        Err(CrossPlanFormError::UnsupportedContract)
    ));
}

#[test]
fn cross_plan_selection_preserves_locked_harmony_and_change_budget() {
    let corpus: development_v2::Corpus = serde_json::from_str(development_v2::FIXTURE).unwrap();
    let case = &corpus.cases[0];
    let source = development_v2::score(case);
    let mut request = request(case);
    let original = acorde_core::suggest_arrangement_proposals(&source, &request.arrangement)
        .candidates[0]
        .plan
        .clone();
    request.arrangement.harmony.source_plan = Some(original.clone());
    request.arrangement.harmony.max_changed_spans = Some(4);
    request.arrangement.locked_chord_entries = vec![0];
    let result = suggest_cross_plan_forms(&source, &request).unwrap();
    assert!(result.candidates.len() >= 3);
    for candidate in &result.candidates {
        assert_eq!(candidate.plan.entries[0].chord, original.entries[0].chord);
        assert!(candidate.changed_harmony_spans <= 4);
        assert_eq!(
            events(&candidate.score),
            events(&replay_cross_plan_form(&source, &candidate.resolved).unwrap())
        );
    }
}

#[test]
fn cross_plan_pool_covers_development_works_with_countermelody() {
    let corpus: development_v2::Corpus = serde_json::from_str(development_v2::FIXTURE).unwrap();
    let mut covered = 0;
    for case in &corpus.cases {
        let request = request(case);
        if request
            .arrangement
            .ensemble_template
            .countermelody
            .is_none()
        {
            continue;
        }
        let source = development_v2::score(case);
        match suggest_cross_plan_forms(&source, &request) {
            Ok(result) => {
                assert_eq!(result.candidates.len(), 3, "{}", case.id);
                let plans = result
                    .candidates
                    .iter()
                    .map(|candidate| harmony(&candidate.plan))
                    .collect::<BTreeSet<_>>();
                assert!(plans.len() >= 2, "{}", case.id);
                covered += 1;
            }
            Err(CrossPlanFormError::InsufficientHarmonicPlans) if case.id == "five-step" => {}
            Err(error) => panic!("{}: unexpected cross-plan error: {error}", case.id),
        }
    }
    assert_eq!(covered, 10);
}
