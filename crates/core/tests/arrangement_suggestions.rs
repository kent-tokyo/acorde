//! The unified entry point returns the same scores as each engine's own API, and
//! replays them from the tagged contract.
#![cfg(feature = "unstable-arrangement")]

use acorde_core::{
    ArrangementAssessmentLimits, ArrangementAssessmentOptions, ArrangementAssessmentRequest,
    ArrangementEngineRequest, ArrangementRequest, ArrangementSuggestions, CrossPlanFormRequest,
    EnsembleFormProposalRequest, FormProposalBoundary, ResolvedArrangement, Score,
    replay_arrangement, suggest_arrangement_proposals, suggest_arrangements,
    suggest_cross_plan_forms, suggest_developed_form_proposals, suggest_ensemble_form_proposals,
    suggest_harmony_groove_arrangements, suggest_integrated_arrangements, to_playback_events,
};

#[allow(dead_code)]
#[path = "../../../tests/fixtures/arrangement/development_v2.rs"]
mod development_v2;
#[path = "../../../tests/fixtures/arrangement/development_v2_request.rs"]
mod development_v2_request;
#[allow(dead_code)]
#[path = "../../../tests/fixtures/arrangement/harmony_groove_source.rs"]
mod groove_fixture;
#[path = "../../../tests/fixtures/arrangement/integrated_arrangement.rs"]
mod integrated_fixture;

use development_v2::{Case, Corpus, FIXTURE};

fn sound(score: &Score) -> Vec<(usize, u64, u64, i32, u8)> {
    to_playback_events(score, &Default::default())
        .into_iter()
        .filter_map(|event| {
            event.source.map(|address| {
                (
                    address.part,
                    (event.time_beats * 480.0).round() as u64,
                    (event.duration_beats * 480.0).round() as u64,
                    event.pitch_midi_cents,
                    event.velocity,
                )
            })
        })
        .collect()
}

/// Same scores in the same order, replayable through the unified contract, and
/// the request and contracts survive a JSON round trip.
fn check(source: &Score, engine: ArrangementEngineRequest, expected: Vec<Score>) {
    let name = engine.engine();
    let request = ArrangementRequest::new(engine);
    let json = serde_json::to_value(&request).unwrap();
    assert_eq!(json["engine"], name);
    let request: ArrangementRequest = serde_json::from_value(json).unwrap();
    let result = suggest_arrangements(source, &request).unwrap();
    assert_eq!(result.engine, name);
    assert!(result.set_aside.is_empty());
    assert_eq!(result.candidates.len(), expected.len(), "{name}");
    assert!(!expected.is_empty(), "{name}");
    for (candidate, expected) in result.candidates.iter().zip(&expected) {
        assert_eq!(sound(&candidate.score), sound(expected), "{name}");
        assert!(candidate.assessment.is_none());
        let resolved: ResolvedArrangement =
            serde_json::from_value(serde_json::to_value(&candidate.resolved).unwrap()).unwrap();
        let replayed =
            replay_arrangement(source, &resolved).unwrap_or_else(|error| panic!("{name}: {error}"));
        assert_eq!(sound(&replayed), sound(expected), "{name} replay");
    }
    let mut ids: Vec<_> = result.candidates.iter().map(|c| c.id.clone()).collect();
    ids.sort();
    ids.dedup();
    assert_eq!(ids.len(), result.candidates.len(), "{name} ids are unique");
}

fn sections(case: &Case) -> Vec<FormProposalBoundary> {
    case.sections
        .iter()
        .map(|section| FormProposalBoundary {
            label: section.label.clone(),
            start_measure: section.start_measure,
            end_measure_exclusive: section.end_measure_exclusive,
        })
        .collect()
}

/// A form request on a development work whose base proposal has a countermelody.
fn form_request(count: u8) -> (Score, EnsembleFormProposalRequest) {
    let corpus: Corpus = serde_json::from_str(FIXTURE).unwrap();
    for case in &corpus.cases {
        let source = development_v2::score(case);
        let proposals =
            suggest_arrangement_proposals(&source, &development_v2_request::request(case).unwrap());
        if let Some(base) = proposals
            .candidates
            .iter()
            .find(|candidate| candidate.features.countermelody_attack_count > 0)
        {
            let request = EnsembleFormProposalRequest {
                ensemble: base.resolved.ensemble_options.clone(),
                sections: sections(case),
                phrase_end_measures: Vec::new(),
                count,
            };
            return (source, request);
        }
    }
    panic!("no development work has a countermelody proposal");
}

#[test]
fn roles_engine_matches_suggest_arrangement_proposals() {
    let corpus: Corpus = serde_json::from_str(FIXTURE).unwrap();
    let case = &corpus.cases[0];
    let source = development_v2::score(case);
    let request = development_v2_request::request(case).unwrap();
    let expected = suggest_arrangement_proposals(&source, &request)
        .candidates
        .into_iter()
        .map(|c| c.score)
        .collect();
    check(&source, ArrangementEngineRequest::Roles(request), expected);
}

#[test]
fn integrated_engine_matches_suggest_integrated_arrangements() {
    let source = integrated_fixture::source();
    let request = integrated_fixture::request();
    let expected = suggest_integrated_arrangements(&source, &request)
        .unwrap()
        .candidates
        .into_iter()
        .map(|c| c.score)
        .collect();
    check(
        &source,
        ArrangementEngineRequest::Integrated(request),
        expected,
    );
}

#[test]
fn harmony_groove_engine_matches_suggest_harmony_groove_arrangements() {
    let source = groove_fixture::source();
    let request = groove_fixture::request();
    let expected = suggest_harmony_groove_arrangements(&source, &request)
        .unwrap()
        .candidates
        .into_iter()
        .map(|c| c.arrangement.score)
        .collect();
    check(
        &source,
        ArrangementEngineRequest::HarmonyGroove(request),
        expected,
    );
}

#[test]
fn form_engines_match_their_own_apis() {
    let (source, request) = form_request(3);
    let expected = suggest_ensemble_form_proposals(&source, &request)
        .unwrap()
        .candidates
        .into_iter()
        .map(|c| c.score)
        .collect();
    check(
        &source,
        ArrangementEngineRequest::EnsembleForm(request.clone()),
        expected,
    );
    let expected = suggest_developed_form_proposals(&source, &request)
        .unwrap()
        .candidates
        .into_iter()
        .map(|c| c.score)
        .collect();
    check(
        &source,
        ArrangementEngineRequest::DevelopedForm(request),
        expected,
    );
}

#[test]
fn cross_plan_engine_matches_suggest_cross_plan_forms() {
    let corpus: Corpus = serde_json::from_str(FIXTURE).unwrap();
    let case = &corpus.cases[0];
    let source = development_v2::score(case);
    let mut arrangement = development_v2_request::request(case).unwrap();
    arrangement.count = 3;
    let request = CrossPlanFormRequest {
        arrangement,
        sections: sections(case),
        phrase_end_measures: Vec::new(),
    };
    let expected = suggest_cross_plan_forms(&source, &request)
        .unwrap()
        .candidates
        .into_iter()
        .map(|c| c.score)
        .collect();
    check(
        &source,
        ArrangementEngineRequest::CrossPlanForm(request),
        expected,
    );
}

fn assessed(source: &Score, limits: ArrangementAssessmentLimits) -> ArrangementSuggestions {
    let request = ArrangementRequest {
        engine: ArrangementEngineRequest::Integrated(integrated_fixture::request()),
        assessment: Some(ArrangementAssessmentOptions {
            request: ArrangementAssessmentRequest::new(0),
            limits,
        }),
    };
    suggest_arrangements(source, &request).unwrap()
}

#[test]
fn assessment_is_attached_and_limits_set_candidates_aside() {
    let source = integrated_fixture::source();
    let all = assessed(&source, ArrangementAssessmentLimits::default());
    assert!(all.set_aside.is_empty());
    assert!(all.candidates.iter().all(|c| c.assessment.is_some()));

    // A limit one below the largest count sets aside exactly the candidates at
    // that count and keeps the rest in order.
    let clashes: Vec<u64> = all
        .candidates
        .iter()
        .map(|c| c.assessment.as_ref().unwrap().melody_semitone_clash_ticks)
        .collect();
    let worst = *clashes.iter().max().unwrap();
    assert!(worst > 0, "the fixture has a clash to filter");
    let limited = assessed(
        &source,
        ArrangementAssessmentLimits {
            max_melody_semitone_clash_ticks: Some(worst - 1),
            ..Default::default()
        },
    );
    let kept: Vec<_> = all
        .candidates
        .iter()
        .zip(&clashes)
        .filter(|(_, ticks)| **ticks < worst)
        .map(|(c, _)| c.id.clone())
        .collect();
    assert_eq!(
        limited
            .candidates
            .iter()
            .map(|c| c.id.clone())
            .collect::<Vec<_>>(),
        kept
    );
    assert_eq!(
        limited.set_aside.len(),
        clashes.iter().filter(|&&ticks| ticks == worst).count()
    );
    for item in &limited.set_aside {
        assert_eq!(item.exceeded, vec!["max-melody-semitone-clash-ticks"]);
    }
    let relaxed = assessed(
        &source,
        ArrangementAssessmentLimits {
            max_melody_semitone_clash_ticks: Some(worst),
            ..Default::default()
        },
    );
    assert!(relaxed.set_aside.is_empty());
}

#[test]
fn errors_are_atomic() {
    let source = integrated_fixture::source();
    let mut request = integrated_fixture::request();
    request.count = 9;
    let error = suggest_arrangements(
        &source,
        &ArrangementRequest::new(ArrangementEngineRequest::Integrated(request)),
    )
    .unwrap_err();
    assert!(
        error.to_string().starts_with("integrated engine:"),
        "{error}"
    );

    let request = ArrangementRequest {
        engine: ArrangementEngineRequest::Integrated(integrated_fixture::request()),
        assessment: Some(ArrangementAssessmentOptions {
            request: ArrangementAssessmentRequest::new(99),
            limits: Default::default(),
        }),
    };
    assert!(suggest_arrangements(&source, &request).is_err());

    let result = suggest_arrangements(
        &source,
        &ArrangementRequest::new(ArrangementEngineRequest::Integrated(
            integrated_fixture::request(),
        )),
    )
    .unwrap();
    let mut changed = source.clone();
    changed.parts[0].staves[0].measures[0].voices[0][0].pitches[0] =
        acorde_core::Pitch::from_midi(61, false);
    assert!(replay_arrangement(&changed, &result.candidates[0].resolved).is_err());
}
