#[path = "../../../tests/fixtures/arrangement/integrated_listening.rs"]
mod fixture;

#[test]
fn eight_and_sixteen_bar_inputs_produce_three_playable_proposals() {
    let corpus: fixture::Corpus = serde_json::from_str(fixture::FIXTURE).unwrap();
    for case in &corpus.cases {
        let (source, request) = fixture::input(case).unwrap();
        let result = acorde_core::suggest_integrated_arrangements(&source, &request)
            .unwrap_or_else(|error| panic!("{}: {error:?}", case.id));
        assert_eq!(result.candidates.len(), 3, "{}", case.id);
        assert_eq!(result.realizations, 12, "{}", case.id);
        for axis in ["harmony-changed", "rhythm-changed", "melody-changed"] {
            assert!(
                result.candidates.iter().any(|candidate| {
                    candidate.difference_codes.iter().any(|code| code == axis)
                }),
                "{}: missing {axis}",
                case.id
            );
        }
        for candidate in &result.candidates {
            assert!(acorde_core::validate(&candidate.score).is_valid());
            assert_eq!(
                candidate.score.parts[0].staves[0].measures.len(),
                case.melody.len()
            );
        }
    }
}
