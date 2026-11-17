#[path = "../../../tests/fixtures/arrangement/harmony_groove_listening.rs"]
mod fixture;
#[test]
fn fixed_eight_and_sixteen_bar_inputs_produce_three_replayable_profiles() {
    let corpus: fixture::Corpus = serde_json::from_str(fixture::FIXTURE).unwrap();
    assert_eq!(corpus.schema_version, 1);
    assert_eq!(
        corpus
            .cases
            .iter()
            .map(|c| c.melody.len())
            .collect::<Vec<_>>(),
        vec![8, 16]
    );
    for case in corpus.cases {
        let (source, request) = fixture::input(&case);
        assert!(acorde_core::validate(&source).is_valid());
        let started = std::time::Instant::now();
        let result = acorde_core::suggest_harmony_groove_arrangements(&source, &request).unwrap();
        assert!(
            result.complete,
            "{}: {:?}",
            case.id, result.missing_profiles
        );
        eprintln!(
            "{}: {} realizations in {:?}",
            case.id,
            result.realizations,
            started.elapsed()
        );
        for c in result.candidates {
            acorde_core::replay_harmony_groove_arrangement(&source, &c.resolved).unwrap();
        }
    }
}
