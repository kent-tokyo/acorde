#![cfg(feature = "musicxml")]

#[path = "../../../tests/fixtures/arrangement/form_development_v2.rs"]
mod fixture;

#[test]
fn form_events_and_dynamics_survive_musicxml() {
    let result = acorde_core::arrange_ensemble_form(&fixture::source(), &fixture::request());
    assert!(result.diagnostics.is_empty(), "{:?}", result.diagnostics);
    let score = result.score.unwrap();
    let reparsed =
        acorde_io::parse_musicxml(&acorde_io::serialize_musicxml(&score).unwrap()).unwrap();
    assert_eq!(fixture::events(&score), fixture::events(&reparsed));
}

fn performance_score() -> acorde_core::Score {
    let mut source = fixture::source();
    source.parts[0].staves[0].measures[0].barline_left = acorde_core::Barline::RepeatStart;
    source.parts[0].staves[0].measures[7].barline_right = acorde_core::Barline::RepeatEnd;
    let base = fixture::request();
    let mut alternative = base.clone();
    alternative.sections[0].harmony.as_mut().unwrap().pattern =
        acorde_core::ChordalPadPattern::ArpeggioUpDown;
    let request = acorde_core::EnsembleFormVisitsRequest {
        base,
        variations: vec![acorde_core::FormVisitVariation {
            visit_index: 8,
            alternative,
        }],
    };
    let result = acorde_core::arrange_ensemble_form_visits_score(&source, &request);
    assert!(result.diagnostics.is_empty(), "{:?}", result.diagnostics);
    result.score.unwrap()
}

#[test]
fn visit_specific_performance_survives_musicxml() {
    let score = performance_score();
    let reparsed =
        acorde_io::parse_musicxml(&acorde_io::serialize_musicxml(&score).unwrap()).unwrap();
    assert_eq!(score.parts[0].staves[0].measures.len(), 24);
    assert_eq!(fixture::events(&score), fixture::events(&reparsed));
}

#[cfg(feature = "mscz")]
#[test]
fn visit_specific_performance_survives_mscz() {
    let score = performance_score();
    let reparsed = acorde_io::parse_mscz(&acorde_io::serialize_mscz(&score).unwrap()).unwrap();
    assert_eq!(score.parts[0].staves[0].measures.len(), 24);
    assert_eq!(fixture::events(&score), fixture::events(&reparsed));
}

#[cfg(feature = "mscz")]
#[test]
fn form_events_and_dynamics_survive_mscz_with_repeats() {
    let mut source = fixture::source();
    source.parts[0].staves[0].measures[0].barline_left = acorde_core::Barline::RepeatStart;
    source.parts[0].staves[0].measures[7].barline_right = acorde_core::Barline::RepeatEnd;
    let result = acorde_core::arrange_ensemble_form(&source, &fixture::request());
    assert!(result.diagnostics.is_empty(), "{:?}", result.diagnostics);
    let score = result.score.unwrap();
    let reparsed = acorde_io::parse_mscz(&acorde_io::serialize_mscz(&score).unwrap()).unwrap();
    assert_eq!(fixture::events(&score), fixture::events(&reparsed));
}
