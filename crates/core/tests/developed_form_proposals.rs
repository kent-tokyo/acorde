use std::collections::BTreeSet;

use acorde_core::{
    DevelopedFormProposalError, EnsembleFormProposalRequest, FormProposalBoundary,
    replay_developed_form_proposal, suggest_arrangement_proposals,
    suggest_developed_form_proposals, to_playback_events, validate,
};

#[allow(dead_code)]
#[path = "../../../tests/fixtures/arrangement/development_v2.rs"]
mod development_v2;
#[path = "../../../tests/fixtures/arrangement/development_v2_request.rs"]
mod development_v2_request;

fn request(
    case: &development_v2::Case,
    source: &acorde_core::Score,
) -> Option<EnsembleFormProposalRequest> {
    let proposed =
        suggest_arrangement_proposals(source, &development_v2_request::request(case).unwrap());
    let base = proposed
        .candidates
        .iter()
        .find(|candidate| candidate.features.countermelody_attack_count > 0)?;
    Some(EnsembleFormProposalRequest {
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
        count: 3,
    })
}

fn events(score: &acorde_core::Score, from_part: usize) -> Vec<(usize, u64, u64, u8, u8)> {
    to_playback_events(score, &Default::default())
        .into_iter()
        .filter_map(|event| {
            event.source.and_then(|address| {
                (address.part >= from_part).then_some((
                    address.part,
                    (event.time_beats * 480.0).round() as u64,
                    (event.duration_beats * 480.0).round() as u64,
                    event.pitch_midi,
                    event.velocity,
                ))
            })
        })
        .collect()
}

#[test]
fn three_developed_forms_cover_countermelody_capable_works() {
    let corpus: development_v2::Corpus = serde_json::from_str(development_v2::FIXTURE).unwrap();
    let mut covered = 0;
    let mut observed_axes = [false; 6];
    for case in &corpus.cases {
        let source = development_v2::score(case);
        let Some(request) = request(case, &source) else {
            continue;
        };
        let result = suggest_developed_form_proposals(&source, &request)
            .unwrap_or_else(|error| panic!("{}: {error:?}", case.id));
        assert_eq!(result.candidates.len(), 3, "{}", case.id);
        let mut unique = BTreeSet::new();
        let mut kinds = BTreeSet::new();
        let mut profiles = BTreeSet::new();
        for candidate in &result.candidates {
            assert!(validate(&candidate.score).is_valid(), "{}", case.id);
            assert_eq!(
                serde_json::to_value(&candidate.score.parts[..source.parts.len()]).unwrap(),
                serde_json::to_value(&source.parts).unwrap(),
                "{} source parts",
                case.id
            );
            let generated = events(&candidate.score, source.parts.len());
            assert!(unique.insert(generated.clone()), "{} duplicate", case.id);
            kinds.insert(format!(
                "{:?}",
                candidate.resolved.development.countermelody_kind
            ));
            profiles.insert(candidate.resolved.form_proposal.profile_id.clone());
            let replay = replay_developed_form_proposal(&source, &candidate.resolved).unwrap();
            assert_eq!(
                events(&replay, 0),
                events(&candidate.score, 0),
                "{} replay",
                case.id
            );
            if candidate.id != result.candidates[0].id {
                assert!(
                    candidate.axis_distance_from_first.total() > 0,
                    "{} measured difference",
                    case.id
                );
                assert!(
                    candidate.nearest_selected_distance > 0,
                    "{} nearest distance",
                    case.id
                );
                let distance = candidate.axis_distance_from_first;
                for (seen, (changed, code)) in observed_axes.iter_mut().zip([
                    (distance.harmony > 0, "harmony-pitch-classes-changed"),
                    (distance.rhythm > 0, "generated-rhythm-changed"),
                    (distance.bass > 0, "bass-events-changed"),
                    (distance.register > 0, "generated-register-changed"),
                    (distance.roles > 0, "role-activity-changed"),
                    (distance.form > 0, "section-form-changed"),
                ]) {
                    assert_eq!(
                        candidate.difference_codes.iter().any(|item| item == code),
                        changed
                    );
                    *seen |= changed;
                }
            }
        }
        for rejected in &result.rejections {
            if let Some(problem) = &rejected.diagnostic {
                assert_eq!(rejected.code, problem.code, "{} rejection", case.id);
            }
        }
        assert_eq!(kinds.len(), 3, "{} treatment variety", case.id);
        println!(
            "{}: {} kinds, {} profiles, {} rejected attempts",
            case.id,
            kinds.len(),
            profiles.len(),
            result.rejections.len()
        );
        covered += 1;
    }
    assert_eq!(covered, 10);
    assert!(observed_axes.into_iter().all(|observed| observed));
}

#[test]
fn five_developed_forms_remain_distinct_across_the_corpus() {
    let corpus: development_v2::Corpus = serde_json::from_str(development_v2::FIXTURE).unwrap();
    let mut covered = 0;
    for case in &corpus.cases {
        let source = development_v2::score(case);
        let Some(mut request) = request(case, &source) else {
            continue;
        };
        request.count = 5;
        let result = suggest_developed_form_proposals(&source, &request)
            .unwrap_or_else(|error| panic!("{}: {error:?}", case.id));
        assert_eq!(result.candidates.len(), 5, "{}", case.id);
        let generated = result
            .candidates
            .iter()
            .map(|candidate| events(&candidate.score, source.parts.len()))
            .collect::<BTreeSet<_>>();
        assert_eq!(generated.len(), 5, "{} duplicate", case.id);
        covered += 1;
    }
    assert_eq!(covered, 10);
}

#[test]
fn invalid_count_and_tampered_contract_fail() {
    let corpus: development_v2::Corpus = serde_json::from_str(development_v2::FIXTURE).unwrap();
    let case = &corpus.cases[0];
    let source = development_v2::score(case);
    let mut request = request(case, &source).unwrap();
    request.count = 2;
    assert!(matches!(
        suggest_developed_form_proposals(&source, &request),
        Err(DevelopedFormProposalError::InvalidCount)
    ));
    request.count = 3;
    request.ensemble.countermelody = None;
    assert!(matches!(
        suggest_developed_form_proposals(&source, &request),
        Err(DevelopedFormProposalError::MissingCountermelody)
    ));
    request = self::request(case, &source).unwrap();
    let result = suggest_developed_form_proposals(&source, &request).unwrap();
    let mut changed = result.candidates[0].resolved.clone();
    changed.development.form.request.sections[0].harmony = None;
    assert!(matches!(
        replay_developed_form_proposal(&source, &changed),
        Err(DevelopedFormProposalError::UnsupportedContract)
    ));
    let mut changed_source = source.clone();
    changed_source.parts[0].staves[0].measures[0].voices[0][0].pitches[0] =
        acorde_core::Pitch::from_midi(71, false);
    assert!(matches!(
        replay_developed_form_proposal(&changed_source, &result.candidates[0].resolved),
        Err(DevelopedFormProposalError::SourceMismatch)
    ));
}
