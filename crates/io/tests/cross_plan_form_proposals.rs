#![cfg(feature = "musicxml")]

#[allow(dead_code)]
#[path = "../../../tests/fixtures/arrangement/development_v2.rs"]
mod development_v2;
#[allow(dead_code)]
#[path = "../../../tests/fixtures/arrangement/development_v2_request.rs"]
mod development_v2_request;

fn candidates() -> Vec<acorde_core::CrossPlanFormCandidate> {
    let corpus: development_v2::Corpus = serde_json::from_str(development_v2::FIXTURE).unwrap();
    let case = &corpus.cases[0];
    let source = development_v2::score(case);
    let mut arrangement = development_v2_request::request(case).unwrap();
    arrangement.count = 3;
    let request = acorde_core::CrossPlanFormRequest {
        arrangement,
        sections: case
            .sections
            .iter()
            .map(|section| acorde_core::FormProposalBoundary {
                label: section.label.clone(),
                start_measure: section.start_measure,
                end_measure_exclusive: section.end_measure_exclusive,
            })
            .collect(),
        phrase_end_measures: Vec::new(),
    };
    acorde_core::suggest_cross_plan_forms(&source, &request)
        .unwrap()
        .candidates
}

fn events(score: &acorde_core::Score) -> Vec<(usize, u64, u64, i32)> {
    acorde_core::to_playback_events(score, &Default::default())
        .into_iter()
        .filter_map(|event| {
            event.source.map(|source| {
                (
                    source.part,
                    (event.time_beats * 480.0).round() as u64,
                    (event.duration_beats * 480.0).round() as u64,
                    event.pitch_midi_cents,
                )
            })
        })
        .collect()
}

#[test]
fn cross_plan_forms_survive_musicxml() {
    for candidate in candidates() {
        let back =
            acorde_io::parse_musicxml(&acorde_io::serialize_musicxml(&candidate.score).unwrap())
                .unwrap();
        assert_eq!(events(&candidate.score), events(&back));
    }
}

#[cfg(feature = "mscz")]
#[test]
fn cross_plan_forms_survive_mscz() {
    for candidate in candidates() {
        let back =
            acorde_io::parse_mscz(&acorde_io::serialize_mscz(&candidate.score).unwrap()).unwrap();
        assert_eq!(events(&candidate.score), events(&back));
    }
}
