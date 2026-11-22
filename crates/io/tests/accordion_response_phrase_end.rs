#![cfg(feature = "mscz")]
use acorde_core::*;

#[test]
fn original_score_phrase_ending_rest() {
    // The score cited in issue #468, copied from fisarmonica.org/public/scores.
    let source = acorde_io::parse_mscz(include_bytes!(
        "../../../tests/fixtures/ordway-dreaming-of-home-and-mother.mscz"
    ))
    .unwrap();
    let options = SectionedAccordionOptions {
        chords: vec![PositionedAccordionChord {
            measure_index: 0,
            offset_beats: 0.0,
            chord: serde_json::from_value(serde_json::json!({"root":"F","kind":"major"})).unwrap(),
        }],
        bass_mode: AccordionBassMode::FreeBass,
        default_pattern: None,
        sections: Vec::new(),
    };
    let addr = NoteAddr {
        part: 0,
        staff: 0,
        measure: 3,
        voice: 0,
        note: 2,
    };
    let result = arrange_with_response_fills(
        &source,
        options,
        AccordionResponseOptions {
            rest_notes: vec![addr],
        },
    );
    assert!(
        result.arrangement.diagnostics.is_empty(),
        "{:?}",
        result.arrangement.diagnostics
    );
    let output = result.arrangement.score.unwrap();
    assert_eq!(result.fills.len(), 1);
    assert_eq!(
        serde_json::to_value(&source.parts[0].staves[0].measures[3].voices[0]).unwrap(),
        serde_json::to_value(&output.parts[0].staves[0].measures[3].voices[0]).unwrap()
    );
    let played = |s: &Score| {
        to_playback_events(s, &Default::default())
            .into_iter()
            .filter_map(|e| {
                Some((
                    e.source?.staff,
                    e.pitch_midi,
                    (e.time_beats * 960.0).round() as i64,
                    (e.duration_beats * 960.0).round() as i64,
                ))
            })
            .collect::<Vec<_>>()
    };
    let imported = acorde_io::parse_mscz(&acorde_io::serialize_mscz(&output).unwrap()).unwrap();
    assert_eq!(played(&output), played(&imported));
}
