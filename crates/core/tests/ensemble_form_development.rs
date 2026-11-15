use acorde_core::{
    CountermelodyDevelopmentKind, EnsembleFormDevelopmentError, EnsembleFormDevelopmentRequest,
    EnsembleFormProposalRequest, EnsembleRole, FormProposalBoundary,
    arrange_ensemble_form_development, replay_ensemble_form_development,
    suggest_arrangement_proposals, suggest_ensemble_form_proposals, validate,
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
