use acorde_core::{
    CountermelodyDevelopmentKind, EnsembleFormDevelopmentError, EnsembleFormDevelopmentRequest,
    EnsembleFormProposalRequest, EnsembleRole, FormProposalBoundary,
    arrange_ensemble_form_development, arrange_ensemble_form_development_with_phrase_joins,
    replay_ensemble_form_development, suggest_arrangement_proposals,
    suggest_ensemble_form_proposals, validate,
};

#[allow(dead_code)]
#[path = "../../../tests/fixtures/arrangement/development_v2.rs"]
mod development_v2;
#[path = "../../../tests/fixtures/arrangement/development_v2_request.rs"]
mod development_v2_request;
#[path = "../../../tests/fixtures/arrangement/form_development_v2.rs"]
mod fixture;

fn request(kind: CountermelodyDevelopmentKind) -> EnsembleFormDevelopmentRequest {
    EnsembleFormDevelopmentRequest {
        form: fixture::request(),
        countermelody_kind: kind,
    }
}

#[test]
fn rhythmic_countermelody_follows_form_sections_and_replays() {
    let source = fixture::source();
    let before = serde_json::to_value(&source.parts).unwrap();
    let result = arrange_ensemble_form_development(
        &source,
        &request(CountermelodyDevelopmentKind::Imitation),
    )
    .unwrap();
    assert!(validate(&result.score).is_valid());
    assert_eq!(
        serde_json::to_value(&result.score.parts[..1]).unwrap(),
        before
    );
    assert_eq!(result.role_parts.len(), 3);
    let counter = result
        .role_parts
        .iter()
        .find(|part| part.role == EnsembleRole::Countermelody)
        .unwrap()
        .part_index;
    for (measure, report) in result.measures.iter().enumerate() {
        let active = (4..12).contains(&measure);
        assert_eq!(
            report.countermelody_attacks > 0,
            active,
            "measure {measure}"
        );
        let sounding = result.score.parts[counter].staves[0].measures[measure].voices[0]
            .iter()
            .filter(|note| !note.is_rest)
            .collect::<Vec<_>>();
        if !active {
            assert!(sounding.is_empty());
        } else {
            let expected = if measure < 8 {
                acorde_core::Dynamic::P
            } else {
                acorde_core::Dynamic::Mp
            };
            assert_eq!(sounding[0].dynamic, Some(expected));
        }
    }
    let replay = replay_ensemble_form_development(&source, &result.resolved).unwrap();
    assert_eq!(fixture::events(&replay), fixture::events(&result.score));
    assert_eq!(serde_json::to_value(&source.parts).unwrap(), before);
}

#[test]
fn density_and_source_contract_fail_atomically() {
    let source = fixture::source();
    let mut request = request(CountermelodyDevelopmentKind::Diminution);
    let dense = arrange_ensemble_form_development(&source, &request);
    assert!(matches!(
        dense,
        Err(EnsembleFormDevelopmentError::Form(problem)) if problem.code == "density-limit"
    ));
    request.countermelody_kind = CountermelodyDevelopmentKind::Imitation;
    let result = arrange_ensemble_form_development(&source, &request).unwrap();
    let mut changed = source.clone();
    changed.parts[0].staves[0].measures[0].voices[0][0].pitches[0] =
        acorde_core::Pitch::from_midi(85, false);
    assert!(matches!(
        replay_ensemble_form_development(&changed, &result.resolved),
        Err(EnsembleFormDevelopmentError::SourceMismatch)
    ));
    request
        .form
        .sections
        .iter_mut()
        .for_each(|section| section.countermelody = None);
    assert!(matches!(
        arrange_ensemble_form_development(&source, &request),
        Err(EnsembleFormDevelopmentError::MissingCountermelody)
    ));
}

#[test]
fn development_corpus_form_coverage() {
    let corpus: development_v2::Corpus = serde_json::from_str(development_v2::FIXTURE).unwrap();
    let mut eligible_works = 0;
    let mut covered_works = 0;
    let mut accepted_forms = 0;
    for case in &corpus.cases {
        let source = development_v2::score(case);
        let proposals =
            suggest_arrangement_proposals(&source, &development_v2_request::request(case).unwrap());
        let base = proposals
            .candidates
            .iter()
            .find(|candidate| candidate.features.countermelody_attack_count > 0);
        let Some(base) = base else {
            continue;
        };
        eligible_works += 1;
        let proposed = suggest_ensemble_form_proposals(
            &source,
            &EnsembleFormProposalRequest {
                ensemble: base.resolved.ensemble_options.clone(),
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
                count: 5,
            },
        )
        .unwrap();
        let mut count = 0;
        for candidate in &proposed.candidates {
            if candidate
                .resolved
                .form
                .request
                .sections
                .iter()
                .any(|section| section.countermelody.is_some())
            {
                let result = arrange_ensemble_form_development(
                    &source,
                    &EnsembleFormDevelopmentRequest {
                        form: candidate.resolved.form.request.clone(),
                        countermelody_kind: CountermelodyDevelopmentKind::Imitation,
                    },
                );
                if let Ok(result) = result {
                    assert!(validate(&result.score).is_valid(), "{}", case.id);
                    assert_eq!(
                        serde_json::to_value(&result.score.parts[..source.parts.len()]).unwrap(),
                        serde_json::to_value(&source.parts).unwrap(),
                        "{} source parts",
                        case.id
                    );
                    let replay =
                        replay_ensemble_form_development(&source, &result.resolved).unwrap();
                    assert_eq!(
                        fixture::events(&replay),
                        fixture::events(&result.score),
                        "{} replay",
                        case.id
                    );
                    count += 1;
                }
            }
        }
        assert!(count > 0, "{} has no compatible form", case.id);
        covered_works += 1;
        accepted_forms += count;
    }
    assert_eq!(eligible_works, 10);
    assert_eq!(covered_works, eligible_works);
    assert!(
        accepted_forms >= 30,
        "only {accepted_forms}/50 forms accepted"
    );
}

#[test]
fn phrase_join_entry_is_bounded_and_replayable() {
    let source = fixture::source();
    let original_request = request(CountermelodyDevelopmentKind::Imitation);
    let original_plain = arrange_ensemble_form_development(&source, &original_request).unwrap();
    let original_led =
        arrange_ensemble_form_development_with_phrase_joins(&source, &original_request, 0).unwrap();
    let plain_part = original_plain
        .role_parts
        .iter()
        .find(|part| part.role == EnsembleRole::Countermelody)
        .unwrap()
        .part_index;
    let plain_start = original_plain.score.parts[plain_part].staves[0].measures[8].voices[0]
        .iter()
        .find(|note| !note.is_rest)
        .unwrap()
        .pitches[0]
        .to_midi();
    let led_start = original_led.score.parts[plain_part].staves[0].measures[8].voices[0]
        .iter()
        .find(|note| !note.is_rest)
        .unwrap()
        .pitches[0]
        .to_midi();
    assert_eq!(plain_start, 76);
    assert_eq!(led_start, 79);
    assert_eq!(
        original_led.resolved.phrase_entry_max_leap_semitones,
        Some(0)
    );
    let original_contract = serde_json::to_value(&original_plain.resolved).unwrap();
    assert!(
        original_contract
            .get("phrase_entry_max_leap_semitones")
            .is_none()
    );
    let original_replay = replay_ensemble_form_development(
        &source,
        &serde_json::from_value(original_contract).unwrap(),
    )
    .unwrap();
    assert_eq!(
        fixture::events(&original_replay),
        fixture::events(&original_plain.score)
    );
    let mut request = request(CountermelodyDevelopmentKind::Imitation);
    for chords in [
        &mut request.form.ensemble.harmony.as_mut().unwrap().chords,
        &mut request.form.ensemble.bass.as_mut().unwrap().chords,
        &mut request.form.ensemble.countermelody.as_mut().unwrap().chords,
    ] {
        chords
            .iter_mut()
            .find(|placed| placed.measure_index == 8)
            .unwrap()
            .chord
            .root = "F".into();
    }
    let result = arrange_ensemble_form_development_with_phrase_joins(&source, &request, 2).unwrap();
    let counter = result
        .role_parts
        .iter()
        .find(|part| part.role == EnsembleRole::Countermelody)
        .unwrap()
        .part_index;
    let voice = |measure: usize| &result.score.parts[counter].staves[0].measures[measure].voices[0];
    let before = voice(7)
        .iter()
        .rev()
        .find(|note| !note.is_rest)
        .unwrap()
        .pitches[0]
        .to_midi();
    let after = voice(8).iter().find(|note| !note.is_rest).unwrap().pitches[0].to_midi();
    assert_eq!(before.abs_diff(after), 2);
    assert_eq!(result.resolved.phrase_entry_max_leap_semitones, Some(2));
    let too_tight = arrange_ensemble_form_development_with_phrase_joins(&source, &request, 1);
    assert!(matches!(
        too_tight,
        Err(EnsembleFormDevelopmentError::Form(problem))
            if problem.code == "phrase-join-unavailable" && problem.measure_index == Some(8)
    ));
    assert!(matches!(
        arrange_ensemble_form_development_with_phrase_joins(&source, &request, 25),
        Err(EnsembleFormDevelopmentError::InvalidPhraseJoinLimit)
    ));
    let replay = replay_ensemble_form_development(&source, &result.resolved).unwrap();
    assert_eq!(fixture::events(&replay), fixture::events(&result.score));
}

#[test]
fn phrase_join_corpus_coverage() {
    let corpus: development_v2::Corpus = serde_json::from_str(development_v2::FIXTURE).unwrap();
    let mut results = Vec::new();
    for case in &corpus.cases {
        let source = development_v2::score(case);
        let proposals =
            suggest_arrangement_proposals(&source, &development_v2_request::request(case).unwrap());
        let Some(base) = proposals
            .candidates
            .iter()
            .find(|candidate| candidate.features.countermelody_attack_count > 0)
        else {
            continue;
        };
        let forms = suggest_ensemble_form_proposals(
            &source,
            &EnsembleFormProposalRequest {
                ensemble: base.resolved.ensemble_options.clone(),
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
                count: 5,
            },
        )
        .unwrap();
        let counts = [4, 7].map(|limit| {
            forms
                .candidates
                .iter()
                .filter(|candidate| {
                    arrange_ensemble_form_development_with_phrase_joins(
                        &source,
                        &EnsembleFormDevelopmentRequest {
                            form: candidate.resolved.form.request.clone(),
                            countermelody_kind: CountermelodyDevelopmentKind::Imitation,
                        },
                        limit,
                    )
                    .is_ok()
                })
                .count()
        });
        results.push((case.id.clone(), counts));
    }
    assert_eq!(results.len(), 10);
    for (work, counts) in &results {
        assert!(counts[0] > 0, "{work} has no four-semitone phrase join");
        assert!(counts[1] >= counts[0], "{work} has fewer looser joins");
    }
    assert!(results.iter().map(|(_, counts)| counts[0]).sum::<usize>() >= 30);
}
