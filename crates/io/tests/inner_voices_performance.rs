#![cfg(feature = "musicxml")]

#[path = "../../../tests/fixtures/arrangement/inner_voice_repeat.rs"]
mod fixture;

#[test]
fn visit_specific_inner_voices_survive_musicxml() {
    let result =
        acorde_core::arrange_inner_voices_performance(&fixture::source(), fixture::request())
            .unwrap();
    let parsed = acorde_io::parse_musicxml(
        &acorde_io::serialize_musicxml(&result.performance_score).unwrap(),
    )
    .unwrap();
    assert_eq!(
        fixture::events(&result.performance_score),
        fixture::events(&parsed)
    );
}

#[cfg(feature = "mscz")]
#[test]
fn visit_specific_inner_voices_survive_mscz() {
    let result =
        acorde_core::arrange_inner_voices_performance(&fixture::source(), fixture::request())
            .unwrap();
    let parsed =
        acorde_io::parse_mscz(&acorde_io::serialize_mscz(&result.performance_score).unwrap())
            .unwrap();
    assert_eq!(
        fixture::events(&result.performance_score),
        fixture::events(&parsed)
    );
}
