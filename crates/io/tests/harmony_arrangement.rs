#![cfg(any(feature = "musicxml", feature = "mscz"))]
#[path = "../../../tests/fixtures/arrangement/harmony_arrangement_source.rs"]
mod fixture;

fn roundtrip(convert: impl Fn(&acorde_core::Score) -> acorde_core::Score) {
    let mut source = fixture::source();
    source.parts[0].staves[0].measures[2].barline_right = acorde_core::Barline::RepeatEnd;
    let mut request = fixture::request();
    // Split a chord at a non-power-of-two duration, requiring tied fragments.
    request.chords.entries[1].position.offset = acorde_core::BeatFraction {
        numerator: 1,
        denominator: 2,
    };
    request.chords.entries[1].offset_beats = 0.5;
    let result = acorde_core::arrange_harmony_voicings(&source, &request).unwrap();
    let restored = convert(&result.score);
    assert!(acorde_core::validate(&restored).is_valid());
    assert_eq!(fixture::sound(&result.score), fixture::sound(&restored));
    let expected = &result.score.parts[result.added_part].staves[0];
    let actual = &restored.parts[result.added_part].staves[0];
    for (a, b) in expected.measures.iter().zip(&actual.measures) {
        assert_eq!(a.voices[0].len(), b.voices[0].len());
        for (a, b) in a.voices[0].iter().zip(&b.voices[0]) {
            assert_eq!(a.pitches, b.pitches);
            assert_eq!((a.tie_start, a.tie_end), (b.tie_start, b.tie_end));
            assert_eq!(a.beats(), b.beats());
        }
    }
}

#[cfg(feature = "musicxml")]
#[test]
fn harmony_arrangement_musicxml_preserves_spelled_ties_and_repeated_playback() {
    roundtrip(|score| {
        acorde_io::parse_musicxml(&acorde_io::serialize_musicxml(score).unwrap()).unwrap()
    });
}

#[cfg(feature = "mscz")]
#[test]
fn harmony_arrangement_mscz_preserves_spelled_ties_and_repeated_playback() {
    roundtrip(|score| acorde_io::parse_mscz(&acorde_io::serialize_mscz(score).unwrap()).unwrap());
}
