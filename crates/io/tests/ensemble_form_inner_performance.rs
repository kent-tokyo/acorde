#![cfg(feature = "musicxml")]

#[path = "../../../tests/fixtures/arrangement/form_development_v2.rs"]
mod fixture;

fn score() -> acorde_core::Score {
    let mut source = fixture::source();
    source.parts[0].staves[0].measures[0].barline_left = acorde_core::Barline::RepeatStart;
    source.parts[0].staves[0].measures[7].barline_right = acorde_core::Barline::RepeatEnd;
    acorde_core::arrange_form_inner_voices_performance(
        &source,
        &fixture::form_inner_voice_request(),
    )
    .unwrap()
    .performance_score
}

#[test]
fn sectioned_repeat_performance_survives_musicxml() {
    let score = score();
    let parsed =
        acorde_io::parse_musicxml(&acorde_io::serialize_musicxml(&score).unwrap()).unwrap();
    assert_eq!(fixture::events(&score), fixture::events(&parsed));
}

#[cfg(feature = "mscz")]
#[test]
fn sectioned_repeat_performance_survives_mscz() {
    let score = score();
    let parsed = acorde_io::parse_mscz(&acorde_io::serialize_mscz(&score).unwrap()).unwrap();
    assert_eq!(fixture::events(&score), fixture::events(&parsed));
}
