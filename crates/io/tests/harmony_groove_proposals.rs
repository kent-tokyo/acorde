#![cfg(any(feature = "musicxml", feature = "mscz"))]
#[path = "../../../tests/fixtures/arrangement/harmony_groove_source.rs"]
mod fixture;
fn roundtrip(convert: impl Fn(&acorde_core::Score) -> acorde_core::Score) {
    let source = fixture::source();
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
