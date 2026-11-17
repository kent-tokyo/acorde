#![cfg(any(feature = "musicxml", feature = "mscz"))]
#[path = "../../../tests/fixtures/arrangement/phrase_harmony_targets.rs"]
mod fixture;
use acorde_core::*;
fn check(convert: impl Fn(&Score) -> Score) {
    let source = fixture::source();
    for request in [
        fixture::request(),
        fixture::modulation(vec![ModulationApproach::CommonChord]),
        fixture::modulation(vec![ModulationApproach::Dominant]),
    ] {
        let result = suggest_phrase_harmony_targets(&source, &request).unwrap();
        assert!(result.complete, "{:?}", result.rejections);
        for c in result.candidates {
            let original = &c.arrangement.score;
            let restored = convert(original);
            assert_eq!(fixture::sound(original), fixture::sound(&restored));
            for (a, b) in original.parts[c.arrangement.added_part].staves[0]
                .measures
                .iter()
                .zip(&restored.parts[c.arrangement.added_part].staves[0].measures)
            {
                for (a, b) in a.voices[0].iter().zip(&b.voices[0]) {
                    assert_eq!(a.pitches, b.pitches);
                    assert_eq!(a.chord_symbol, b.chord_symbol);
                    assert_eq!((a.tie_start, a.tie_end), (b.tie_start, b.tie_end));
                }
            }
        }
    }
}
#[cfg(feature = "musicxml")]
#[test]
fn targeted_harmony_musicxml_preserves_resolutions_and_spelling() {
    check(|s| acorde_io::parse_musicxml(&acorde_io::serialize_musicxml(s).unwrap()).unwrap());
}
#[cfg(feature = "mscz")]
#[test]
fn targeted_harmony_mscz_preserves_resolutions_and_spelling() {
    check(|s| acorde_io::parse_mscz(&acorde_io::serialize_mscz(s).unwrap()).unwrap());
}
