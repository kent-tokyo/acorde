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
        assert_eq!(
            result.candidates[1].difference_codes,
            ["harmony-changed"],
            "{}",
            case.id
        );
        let harmony = &result.candidates[1];
        for code in [
            "harmony-events-changed",
            "bass-events-changed",
            "countermelody-events-changed",
        ] {
            assert!(
                harmony
                    .observed_difference_codes
                    .iter()
                    .any(|item| item == code),
                "{}: missing observed {code}",
                case.id
            );
        }
        assert_eq!(harmony.axis_distance_from_first.rhythm, 0, "{}", case.id);
        assert!(harmony.axis_distance_from_first.harmony > 0);
        assert_eq!(harmony.axis_distance_from_first.form, 0);
        assert!(
            !harmony
                .observed_difference_codes
                .iter()
                .any(|code| code == "source-melody-events-changed")
        );
        let rhythmic = &result.candidates[2];
        assert!(
            rhythmic
                .observed_difference_codes
                .iter()
                .any(|code| code == "kick-events-changed")
        );
        assert!(rhythmic.axis_distance_from_first.rhythm > 0);
        assert_eq!(rhythmic.axis_distance_from_first.form, 0);
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
