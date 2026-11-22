#![cfg(any(feature = "musicxml", feature = "mscz"))]
#[path = "../../../tests/fixtures/arrangement/harmony_groove_source.rs"]
mod fixture;
fn roundtrip(convert: impl Fn(&acorde_core::Score) -> acorde_core::Score) {
    let source = fixture::grace_source();
    let result =
        acorde_core::suggest_harmony_groove_arrangements(&source, &fixture::request()).unwrap();
    assert!(result.complete);
    for candidate in result.candidates {
        let score = &candidate.arrangement.score;
        let restored = convert(score);
        assert_eq!(fixture::sound(score), fixture::sound(&restored));
        for (a, b) in score.parts[candidate.arrangement.added_part].staves[0]
            .measures
            .iter()
            .zip(&restored.parts[candidate.arrangement.added_part].staves[0].measures)
        {
            assert_eq!(a.voices[0].len(), b.voices[0].len());
            for (a, b) in a.voices[0].iter().zip(&b.voices[0]) {
                assert_eq!(a.pitches, b.pitches);
                assert_eq!(a.articulations, b.articulations);
                assert_eq!((a.tie_start, a.tie_end), (b.tie_start, b.tie_end));
            }
        }
    }
}
#[cfg(feature = "musicxml")]
#[test]
fn groove_candidates_musicxml_preserve_events_spelling_ties_and_accents() {
    roundtrip(|s| acorde_io::parse_musicxml(&acorde_io::serialize_musicxml(s).unwrap()).unwrap());
}
#[cfg(feature = "mscz")]
#[test]
fn groove_candidates_mscz_preserve_events_spelling_ties_and_accents() {
    roundtrip(|s| acorde_io::parse_mscz(&acorde_io::serialize_mscz(s).unwrap()).unwrap());
}

#[cfg(feature = "mscz")]
#[test]
fn jenny_lind_grace_allows_chord_plan_and_groove_replay() {
    use acorde_core::*;
    // The score cited in issue #469, copied from fisarmonica.org/public/scores.
    let source = acorde_io::parse_mscz(include_bytes!(
        "../../../tests/fixtures/jenny-linds-polka.mscz"
    ))
    .unwrap();
    let plans = suggest_chord_plans(&source, &Default::default());
    let mut request = fixture::request();
    request.base.chords = plans.candidates.first().expect("chord plan").plan.clone();
    request.base.bass = None;
    request.base.voices = 3;
    request.base.low_midi = 36;
    request.base.high_midi = 72;
    request.base.center_midi = 52;
    request.base.end_measure_exclusive = source.parts[0].staves[0].measures.len();
    request.tensions.clear();
    request.max_changed_harmonies = 0;
    let result = suggest_harmony_groove_arrangements(&source, &request).unwrap();
    assert!(!result.candidates.is_empty(), "{result:?}");
    for candidate in result.candidates {
        let replay = replay_harmony_groove_arrangement(&source, &candidate.resolved).unwrap();
        assert_eq!(
            fixture::sound(&candidate.arrangement.score),
            fixture::sound(&replay.arrangement.score)
        );
        assert_eq!(
            fixture::sound(&source)
                .into_iter()
                .filter(|x| x.0 == 0)
                .collect::<Vec<_>>(),
            fixture::sound(&candidate.arrangement.score)
                .into_iter()
                .filter(|x| x.0 == 0)
                .collect::<Vec<_>>()
        );
    }
}
