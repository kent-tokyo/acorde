#![cfg(feature = "musicxml")]

#[path = "../../../tests/fixtures/arrangement/form_development_v2.rs"]
mod fixture;

use acorde_core::{
    InstrumentDefinition, InstrumentPartAssignment, InstrumentPlanRequest, InstrumentRange,
    PartTechnique, arrange_ensemble_form, plan_instruments,
};

fn instrumented_form() -> acorde_core::Score {
    let source = fixture::source();
    let form = arrange_ensemble_form(&source, &fixture::request());
    let score = form.score.unwrap();
    let harmony_part = form
        .role_parts
        .iter()
        .find(|part| part.role == acorde_core::EnsembleRole::Harmony)
        .unwrap()
        .part_index;
    let mut instrument =
        InstrumentDefinition::new("electric-piano-harmony", "Electric piano harmony");
    instrument.midi_program = 2;
    instrument.midi_channel = score.parts[harmony_part].midi_channel;
    instrument.sounding_range = Some(InstrumentRange {
        lowest: 48,
        highest: 84,
    });
    plan_instruments(
        &score,
        &InstrumentPlanRequest {
            assignments: vec![InstrumentPartAssignment {
                part_index: harmony_part,
                instrument,
                sounding_range: InstrumentRange {
                    lowest: 48,
                    highest: 84,
                },
                technique: PartTechnique::KeyboardHand {
                    max_simultaneous_pitches: 5,
                    max_span_semitones: 24,
                },
            }],
        },
    )
    .unwrap()
    .score
}

#[test]
fn instrumented_form_survives_musicxml_roundtrip() {
    let score = instrumented_form();
    let parsed =
        acorde_io::parse_musicxml(&acorde_io::serialize_musicxml(&score).unwrap()).unwrap();
    assert_eq!(fixture::events(&score), fixture::events(&parsed));
    assert!(parsed.parts.iter().any(|part| part.midi_program == 2));
    assert!(
        parsed
            .parts
            .iter()
            .any(|part| part.name == "Electric piano harmony")
    );
}

#[cfg(feature = "mscz")]
#[test]
fn instrumented_form_survives_mscz_roundtrip() {
    let score = instrumented_form();
    let parsed = acorde_io::parse_mscz(&acorde_io::serialize_mscz(&score).unwrap()).unwrap();
    assert_eq!(fixture::events(&score), fixture::events(&parsed));
    assert!(parsed.parts.iter().any(|part| part.midi_program == 2));
    assert!(
        parsed
            .parts
            .iter()
            .any(|part| part.name == "Electric piano harmony")
    );
}
