#![cfg(feature = "musicxml")]

#[path = "../../../tests/fixtures/arrangement/countermelody_development.rs"]
mod fixture;

#[test]
fn developed_countermelodies_survive_musicxml() {
    let source = fixture::source();
    for kind in [
        acorde_core::CountermelodyDevelopmentKind::Imitation,
        acorde_core::CountermelodyDevelopmentKind::Inversion,
        acorde_core::CountermelodyDevelopmentKind::Augmentation,
        acorde_core::CountermelodyDevelopmentKind::Diminution,
        acorde_core::CountermelodyDevelopmentKind::Answer,
    ] {
        let result =
            acorde_core::arrange_countermelody_development(&source, &fixture::request(kind))
                .unwrap();
        let reparsed =
            acorde_io::parse_musicxml(&acorde_io::serialize_musicxml(&result.score).unwrap())
                .unwrap();
        assert_eq!(
            fixture::events(&result.score, 0),
            fixture::events(&reparsed, 0)
        );
    }
}

#[cfg(feature = "mscz")]
#[test]
fn developed_countermelodies_survive_mscz() {
    let source = fixture::source();
    for kind in [
        acorde_core::CountermelodyDevelopmentKind::Imitation,
        acorde_core::CountermelodyDevelopmentKind::Inversion,
        acorde_core::CountermelodyDevelopmentKind::Augmentation,
        acorde_core::CountermelodyDevelopmentKind::Diminution,
        acorde_core::CountermelodyDevelopmentKind::Answer,
    ] {
        let result =
            acorde_core::arrange_countermelody_development(&source, &fixture::request(kind))
                .unwrap();
        let reparsed =
            acorde_io::parse_mscz(&acorde_io::serialize_mscz(&result.score).unwrap()).unwrap();
        assert_eq!(
            fixture::events(&result.score, 0),
            fixture::events(&reparsed, 0)
        );
    }
}
