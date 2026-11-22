#![cfg(feature = "mscz")]
use acorde_core::*;
#[test]
fn lesson_response_and_standard_bass_export_preserve_playback() {
    let score = acorde_io::parse_mscz(include_bytes!(
        "../../../tests/fixtures/ordway-dreaming-of-home-and-mother.mscz"
    ))
    .unwrap();
    let chords = [
        (0, "F", "major"),
        (2, "G", "dominant"),
        (3, "F", "major"),
        (6, "G", "dominant"),
        (7, "F", "major"),
    ]
    .into_iter()
    .map(|(measure_index, root, kind)| PositionedAccordionChord {
        measure_index,
        offset_beats: 0.0,
        chord: serde_json::from_value(serde_json::json!({"root":root,"kind":kind})).unwrap(),
    })
    .collect();
    let options = SectionedAccordionOptions {
        chords,
        bass_mode: AccordionBassMode::StandardBass,
        default_pattern: Some(AccordionPattern::BassChordPulse),
        sections: vec![],
    };
    let response = AccordionResponseOptions {
        rest_notes: vec![NoteAddr {
            part: 0,
            staff: 0,
            measure: 3,
            voice: 0,
            note: 2,
        }],
    };
    let result = arrange_with_response_fills_readable_standard_bass(&score, options, response);
    assert!(
        result.arrangement.diagnostics.is_empty(),
        "{:?}",
        result.arrangement.diagnostics
    );
    let arranged = result.arrangement.score.unwrap();
    let bytes = acorde_io::serialize_mscz(&arranged).unwrap();
    let parsed = acorde_io::parse_mscz(&bytes).unwrap();
    let events = |score: &Score| {
        let mut a = to_playback_events(score, &Default::default())
            .into_iter()
            .map(|e| {
                (
                    e.pitch_midi,
                    (e.time_beats * 960.0).round() as i64,
                    (e.duration_beats * 960.0).round() as i64,
                )
            })
            .collect::<Vec<_>>();
        a.sort();
        a
    };
    assert_eq!(events(&arranged), events(&parsed));
    #[cfg(feature = "musicxml")]
    {
        // MuseScore 4.7.5 re-exported this exact response arrangement.
        let external = acorde_io::parse_musicxml(include_str!(
            "../../../tests/fixtures/issue_476_musescore475_lesson.musicxml"
        ))
        .unwrap();
        assert_eq!(events(&arranged), events(&external));
    }
}
