use std::collections::BTreeSet;

use acorde_core::{
    EnsembleFormProposalRequest, FormProposalBoundary, FormProposalError, Score,
    replay_ensemble_form_proposal, suggest_arrangement_proposals, suggest_ensemble_form_proposals,
    to_playback_events, validate,
};

#[allow(dead_code)]
#[path = "../../../tests/fixtures/arrangement/development_v2.rs"]
mod development_v2;
#[path = "../../../tests/fixtures/arrangement/development_v2_request.rs"]
mod development_v2_request;
use development_v2::{Case, Corpus, FIXTURE, score};

fn events(score: &Score, parts: impl Fn(usize) -> bool) -> Vec<(usize, u64, u64, u8, u8)> {
    to_playback_events(score, &Default::default())
        .into_iter()
        .filter_map(|event| {
            event.source.and_then(|source| {
                parts(source.part).then_some((
                    source.part,
                    (event.time_beats * 480.0).round() as u64,
                    (event.duration_beats * 480.0).round() as u64,
                    event.pitch_midi,
                    event.velocity,
                ))
            })
        })
        .collect()
}

fn request(case: &Case, source: &Score, count: u8) -> EnsembleFormProposalRequest {
    let proposals =
        suggest_arrangement_proposals(source, &development_v2_request::request(case).unwrap());
    assert!(proposals.diagnostics.is_empty(), "{}", case.id);
    let base = proposals
        .candidates
        .iter()
        .find(|candidate| candidate.features.countermelody_attack_count > 0)
        .unwrap_or(&proposals.candidates[0]);
    EnsembleFormProposalRequest {
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
        count,
    }
}

#[test]
fn five_sectional_forms_cover_the_twelve_development_works() {
    let corpus: Corpus = serde_json::from_str(FIXTURE).unwrap();
    assert_eq!(corpus.cases.len(), 12);
    let mut repaired_works = 0;
    for case in &corpus.cases {
        let source = score(case);
        let source_copy = serde_json::to_value(&source).unwrap();
        let source_parts = source.parts.len();
        let source_events = events(&source, |part| part < source_parts);
        let proposed = suggest_ensemble_form_proposals(&source, &request(case, &source, 5))
            .unwrap_or_else(|error| panic!("{}: {error:?}", case.id));
        assert_eq!(proposed.candidates.len(), 5, "{}", case.id);
        let mut unique = BTreeSet::new();
        let mut repaired = false;
        for (index, candidate) in proposed.candidates.iter().enumerate() {
            assert!(validate(&candidate.score).is_valid(), "{}", case.id);
            assert_eq!(
                serde_json::to_value(&candidate.score.parts[..source_parts]).unwrap(),
                serde_json::to_value(&source.parts).unwrap(),
                "{} source parts",
                case.id
            );
            assert_eq!(
                events(&candidate.score, |part| part < source_parts),
                source_events,
                "{} source playback",
                case.id
            );
            let generated = events(&candidate.score, |part| part >= source_parts);
            assert!(unique.insert(generated), "{} duplicate form", case.id);
            assert_eq!(candidate.measures.len(), case.melody.len());
            if index == 0 {
                assert_eq!(candidate.difference_codes, ["reference-form".to_string()]);
            } else {
                assert!(
                    candidate
                        .difference_codes
                        .iter()
                        .any(|code| code == "section-activity-changed")
                );
            }
            repaired |= !candidate.resolved.omitted_countermelody_sections.is_empty();
            for &section in &candidate.resolved.omitted_countermelody_sections {
                assert!(
                    candidate.resolved.form.request.sections[section]
                        .countermelody
                        .is_none()
                );
            }
            let replayed = replay_ensemble_form_proposal(&source, &candidate.resolved).unwrap();
            assert_eq!(
                events(&replayed, |_| true),
                events(&candidate.score, |_| true),
                "{} replay",
                case.id
            );
        }
        repaired_works += usize::from(repaired);
        assert_eq!(serde_json::to_value(&source).unwrap(), source_copy);
    }
    assert!(repaired_works >= 4);
}

#[test]
fn malformed_requests_and_changed_source_fail_atomically() {
    let corpus: Corpus = serde_json::from_str(FIXTURE).unwrap();
    let case = &corpus.cases[0];
    let source = score(case);
    let mut form_request = request(case, &source, 3);
    assert_eq!(
        suggest_ensemble_form_proposals(&source, &form_request)
            .unwrap()
            .candidates
            .len(),
        3
    );
    form_request.count = 2;
    assert!(matches!(
        suggest_ensemble_form_proposals(&source, &form_request),
        Err(FormProposalError::InvalidRequest)
    ));
    form_request.count = 3;
    form_request.sections[1].start_measure += 1;
    assert!(matches!(
        suggest_ensemble_form_proposals(&source, &form_request),
        Err(FormProposalError::InvalidRequest)
    ));
    let mut generated =
        suggest_ensemble_form_proposals(&source, &request(case, &source, 3)).unwrap();
    let resolved = generated.candidates.remove(0).resolved;
    let mut changed_form = resolved.clone();
    changed_form.form.request.sections[0].harmony = None;
    assert!(matches!(
        replay_ensemble_form_proposal(&source, &changed_form),
        Err(FormProposalError::UnsupportedContract)
    ));
    let mut changed_profile = resolved.clone();
    changed_profile.profile_id = "unknown-profile".into();
    assert!(matches!(
        replay_ensemble_form_proposal(&source, &changed_profile),
        Err(FormProposalError::UnsupportedContract)
    ));
    let mut changed = source.clone();
    changed.parts[0].staves[0].measures[0].voices[0][0].pitches[0] =
        acorde_core::Pitch::from_midi(68, false);
    assert!(matches!(
        replay_ensemble_form_proposal(&changed, &resolved),
        Err(FormProposalError::SourceMismatch)
    ));
}
