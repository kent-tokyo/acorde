use std::collections::BTreeSet;

use acorde_core::{
    CountermelodyDevelopmentKind, CountermelodyDevelopmentRequest,
    arrange_countermelody_development, suggest_arrangement_proposals, to_playback_events, validate,
};

#[allow(dead_code)]
#[path = "../../../tests/fixtures/arrangement/development_v2.rs"]
mod development_v2;
#[path = "../../../tests/fixtures/arrangement/development_v2_request.rs"]
mod development_v2_request;
use development_v2::{Corpus, FIXTURE, score};

#[test]
fn development_work_countermelodies_expose_rhythm_transform_coverage() {
    let corpus: Corpus = serde_json::from_str(FIXTURE).unwrap();
    let mut works = 0usize;
    let mut successes = 0usize;
    for case in &corpus.cases {
        let source = score(case);
        let proposed =
            suggest_arrangement_proposals(&source, &development_v2_request::request(case).unwrap());
        let Some(base) = proposed
            .candidates
            .iter()
            .find(|candidate| candidate.features.countermelody_attack_count > 0)
        else {
            continue;
        };
        let Some(countermelody) = base.resolved.ensemble_options.countermelody.clone() else {
            continue;
        };
        works += 1;
        let original_parts = serde_json::to_value(&source.parts).unwrap();
        let mut distinct = BTreeSet::new();
        for kind in [
            CountermelodyDevelopmentKind::Imitation,
            CountermelodyDevelopmentKind::Inversion,
            CountermelodyDevelopmentKind::Augmentation,
            CountermelodyDevelopmentKind::Diminution,
            CountermelodyDevelopmentKind::Answer,
        ] {
            let request = CountermelodyDevelopmentRequest {
                base: countermelody.clone(),
                kind,
            };
            match arrange_countermelody_development(&source, &request) {
                Ok(result) => {
                    assert!(validate(&result.score).is_valid(), "{} {kind:?}", case.id);
                    assert_eq!(
                        serde_json::to_value(&result.score.parts[..source.parts.len()]).unwrap(),
                        original_parts,
                        "{} {kind:?}",
                        case.id
                    );
                    let signature = to_playback_events(&result.score, &Default::default())
                        .into_iter()
                        .filter(|event| {
                            event
                                .source
                                .as_ref()
                                .is_some_and(|address| address.part >= source.parts.len())
                        })
                        .map(|event| ((event.time_beats * 480.0).round() as u64, event.pitch_midi))
                        .collect::<Vec<_>>();
                    distinct.insert(signature);
                    successes += 1;
                }
                Err(error) => panic!("{} {kind:?}: {error:?}", case.id),
            }
        }
        assert_eq!(distinct.len(), 5, "{}", case.id);
    }
    assert_eq!(works, 10);
    assert_eq!(successes, 50);
}
