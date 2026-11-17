use std::collections::BTreeSet;

use acorde_core::{
    Score, replay_arrangement_proposal, suggest_arrangement_proposals, to_playback_events, validate,
};

#[allow(dead_code)]
#[path = "../../../tests/fixtures/arrangement/development_v2.rs"]
mod development_v2;
use development_v2::{Corpus, FIXTURE, score};
#[path = "../../../tests/fixtures/arrangement/development_v2_request.rs"]
mod development_v2_request;

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

#[test]
fn three_replayable_proposals_cover_all_long_form_development_sources() {
    let corpus: Corpus = serde_json::from_str(FIXTURE).unwrap();
    let pinned: serde_json::Value = serde_json::from_str(include_str!(
        "../../../tests/fixtures/arrangement/development-v2-proposals-manifest.json"
    ))
    .unwrap();
    assert_eq!(corpus.schema_version, 1);
    assert_eq!(corpus.cases.len(), 12);
    assert_eq!(pinned["cases"].as_array().unwrap().len(), 12);
    let mut works_with_countermelody = 0;
    for (case, row) in corpus.cases.iter().zip(pinned["cases"].as_array().unwrap()) {
        assert_eq!(row["case_id"], case.id);
        let source = score(case);
        let request = development_v2_request::request(case).unwrap();
        assert_eq!(row["request"], serde_json::to_value(&request).unwrap());
        let result = suggest_arrangement_proposals(&source, &request);
        assert!(
            result.diagnostics.is_empty(),
            "{}: {:?}",
            case.id,
            result.diagnostics
        );
        assert_eq!(result.candidates.len(), 3, "{}", case.id);
        let source_parts = source.parts.len();
        let source_events = events(&source, |part| part < source_parts);
        let mut distinct = BTreeSet::new();
        let mut has_countermelody = false;
        for (candidate, pinned_candidate) in result
            .candidates
            .iter()
            .zip(row["candidates"].as_array().unwrap())
        {
            assert_eq!(pinned_candidate["id"], candidate.id, "{}", case.id);
            assert_eq!(
                pinned_candidate["difference_codes"],
                serde_json::to_value(&candidate.difference_codes).unwrap(),
                "{}",
                case.id
            );
            assert_eq!(
                pinned_candidate["features"]["countermelody_attack_count"],
                candidate.features.countermelody_attack_count,
                "{}",
                case.id
            );
            has_countermelody |= candidate.features.countermelody_attack_count > 0;
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
            assert!(!generated.is_empty(), "{}", case.id);
            assert!(distinct.insert(generated.clone()), "{} duplicate", case.id);
            let replay = replay_arrangement_proposal(&source, &candidate.resolved);
            assert!(
                replay.diagnostics.is_empty(),
                "{} replay: {:?}",
                case.id,
                replay.diagnostics
            );
            assert_eq!(
                events(&replay.score.unwrap(), |part| part >= source_parts),
                generated,
                "{} replayed events",
                case.id
            );
        }
        works_with_countermelody += usize::from(has_countermelody);
    }
    assert!(works_with_countermelody >= 10);
}
