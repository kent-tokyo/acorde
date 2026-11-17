#![cfg(feature = "musicxml")]

#[path = "../../../tests/fixtures/arrangement/form_development_v2.rs"]
mod fixture;

fn score() -> acorde_core::Score {
    acorde_core::arrange_ensemble_form_development(
        &fixture::source(),
        &acorde_core::EnsembleFormDevelopmentRequest {
            form: fixture::request(),
            countermelody_kind: acorde_core::CountermelodyDevelopmentKind::Imitation,
        },
    )
    .unwrap()
    .score
}

fn phrase_join_score() -> acorde_core::Score {
    acorde_core::arrange_ensemble_form_development_with_phrase_joins(
        &fixture::source(),
        &acorde_core::EnsembleFormDevelopmentRequest {
            form: fixture::request(),
            countermelody_kind: acorde_core::CountermelodyDevelopmentKind::Imitation,
        },
        0,
    )
    .unwrap()
    .score
}

#[test]
fn developed_form_survives_musicxml() {
    let score = score();
    let back = acorde_io::parse_musicxml(&acorde_io::serialize_musicxml(&score).unwrap()).unwrap();
    assert_eq!(fixture::events(&score), fixture::events(&back));
}

#[test]
fn phrase_join_survives_musicxml() {
    let score = phrase_join_score();
    let back = acorde_io::parse_musicxml(&acorde_io::serialize_musicxml(&score).unwrap()).unwrap();
    assert_eq!(fixture::events(&score), fixture::events(&back));
}

#[cfg(feature = "mscz")]
#[test]
fn developed_form_survives_mscz() {
    let score = score();
    let back = acorde_io::parse_mscz(&acorde_io::serialize_mscz(&score).unwrap()).unwrap();
    assert_eq!(fixture::events(&score), fixture::events(&back));
}

#[cfg(feature = "mscz")]
#[test]
fn phrase_join_survives_mscz() {
    let score = phrase_join_score();
    let back = acorde_io::parse_mscz(&acorde_io::serialize_mscz(&score).unwrap()).unwrap();
    assert_eq!(fixture::events(&score), fixture::events(&back));
}
