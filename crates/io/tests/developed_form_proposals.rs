#![cfg(feature = "musicxml")]

#[path = "../../../tests/fixtures/arrangement/form_development_v2.rs"]
mod fixture;

fn candidates() -> Vec<acorde_core::DevelopedFormProposalCandidate> {
    let form = fixture::request();
    acorde_core::suggest_developed_form_proposals(
        &fixture::source(),
        &acorde_core::EnsembleFormProposalRequest {
            ensemble: form.ensemble,
            sections: form
                .sections
                .into_iter()
                .map(|section| acorde_core::FormProposalBoundary {
                    label: section.label,
                    start_measure: section.start_measure,
                    end_measure_exclusive: section.end_measure_exclusive,
                })
                .collect(),
            phrase_end_measures: form.phrase_end_measures,
            count: 3,
        },
    )
    .unwrap()
    .candidates
}

#[test]
fn selected_developed_forms_survive_musicxml() {
    for candidate in candidates() {
        let back =
            acorde_io::parse_musicxml(&acorde_io::serialize_musicxml(&candidate.score).unwrap())
                .unwrap();
        assert_eq!(fixture::events(&candidate.score), fixture::events(&back));
    }
}

#[cfg(feature = "mscz")]
#[test]
fn selected_developed_forms_survive_mscz() {
    for candidate in candidates() {
        let back =
            acorde_io::parse_mscz(&acorde_io::serialize_mscz(&candidate.score).unwrap()).unwrap();
        assert_eq!(fixture::events(&candidate.score), fixture::events(&back));
    }
}
