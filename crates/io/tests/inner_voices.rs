#![cfg(feature = "musicxml")]

#[path = "../../../tests/fixtures/arrangement/form_development_v2.rs"]
mod fixture;

#[test]
fn independent_voices_survive_musicxml() {
    let source = fixture::source();
    let generated =
        acorde_core::arrange_inner_voices(&source, fixture::inner_voice_request()).unwrap();
    let expected = fixture::events(&generated.score);
    let xml = acorde_io::serialize_musicxml(&generated.score).unwrap();
    let xml_score = acorde_io::parse_musicxml(&xml).unwrap();
    assert_eq!(expected, fixture::events(&xml_score));
}

#[cfg(feature = "mscz")]
#[test]
fn independent_voices_survive_mscz() {
    let generated =
        acorde_core::arrange_inner_voices(&fixture::source(), fixture::inner_voice_request())
            .unwrap();
    let expected = fixture::events(&generated.score);
    let mscz = acorde_io::serialize_mscz(&generated.score).unwrap();
    let mscz_score = acorde_io::parse_mscz(&mscz).unwrap();
    assert_eq!(expected, fixture::events(&mscz_score));
}
