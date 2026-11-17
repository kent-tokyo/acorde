#![cfg(any(feature = "musicxml", feature = "mscz"))]
#[path = "../../../tests/fixtures/arrangement/rhythm_section.rs"]
mod fixture;
use acorde_core::*;
fn check(convert: impl Fn(&Score) -> Score) {
    for muted in [false, true] {
        let r = arrange_rhythm_section(&fixture::source(), &fixture::request(muted)).unwrap();
        let s = convert(&r.score);
        assert_eq!(fixture::sound(&r.score), fixture::sound(&s));
        for p in r.bass_part..r.score.parts.len() {
            for (a, b) in r.score.parts[p].staves[0]
                .measures
                .iter()
                .zip(&s.parts[p].staves[0].measures)
            {
                let a = a.voices[0]
                    .iter()
                    .filter(|n| !n.is_rest)
                    .collect::<Vec<_>>();
                let b = b.voices[0]
                    .iter()
                    .filter(|n| !n.is_rest)
                    .collect::<Vec<_>>();
                assert_eq!(a.len(), b.len());
                for (a, b) in a.iter().zip(b) {
                    assert_eq!(
                        a.pitch_parentheses.iter().any(|x| *x),
                        b.pitch_parentheses.iter().any(|x| *x)
                    );
                    if p != r.kick_part {
                        assert_eq!(a.pitches, b.pitches);
                    }
                }
            }
        }
    }
}
#[cfg(feature = "musicxml")]
#[test]
fn rhythm_musicxml_preserves_sync_spelling_ghosts_and_routes() {
    check(|s| acorde_io::parse_musicxml(&acorde_io::serialize_musicxml(s).unwrap()).unwrap());
}
#[cfg(feature = "mscz")]
#[test]
fn rhythm_mscz_preserves_sync_spelling_ghosts_and_routes() {
    check(|s| acorde_io::parse_mscz(&acorde_io::serialize_mscz(s).unwrap()).unwrap());
}
