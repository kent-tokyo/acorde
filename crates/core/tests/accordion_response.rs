use acorde_core::{
    AccordionBassMode, AccordionResponseOptions, Barline, Duration, Note, NoteAddr, Pitch,
    PositionedAccordionChord, Score, SectionedAccordionOptions, arrange_with_response_fills,
};

fn chord(measure_index: usize, offset_beats: f64, root: &str) -> PositionedAccordionChord {
    PositionedAccordionChord {
        measure_index,
        offset_beats,
        chord: serde_json::from_value(serde_json::json!({"root":root,"kind":"major"})).unwrap(),
    }
}

fn rest_address(measure: usize) -> NoteAddr {
    NoteAddr {
        part: 0,
        staff: 0,
        measure,
        voice: 0,
        note: 1,
    }
}

fn fixture() -> (Score, SectionedAccordionOptions) {
    let mut score = Score::new("selected responses", 120, 4, 4, 0, 2);
    for measure in &mut score.parts[0].staves[0].measures {
        measure.voices[0] = vec![
            Note::new(Pitch::from_midi(72, false), Duration::Quarter),
            Note::rest(Duration::Half),
            Note::new(Pitch::from_midi(76, false), Duration::Quarter),
        ];
    }
    let options = SectionedAccordionOptions {
        chords: vec![chord(0, 0.0, "C"), chord(1, 2.0, "G")],
        bass_mode: AccordionBassMode::FreeBass,
        default_pattern: None,
        sections: Vec::new(),
    };
    (score, options)
}

#[test]
fn selected_rests_receive_late_chord_tone_responses_without_changing_source_melody() {
    let (score, accompaniment) = fixture();
    let original = serde_json::to_value(&score.parts[0].staves[0]).unwrap();
    let result = arrange_with_response_fills(
        &score,
        accompaniment,
        AccordionResponseOptions {
            rest_notes: vec![rest_address(1), rest_address(0)],
        },
    );
    assert!(
        result.arrangement.diagnostics.is_empty(),
        "{:?}",
        result.arrangement.diagnostics
    );
    assert_eq!(
        result
            .fills
            .iter()
            .map(|fill| fill.pitch_midi)
            .collect::<Vec<_>>(),
        vec![72, 74]
    );
    let output = result.arrangement.score.as_ref().unwrap();
    for (index, bar) in output.parts[0].staves[0].measures.iter().enumerate() {
        assert_eq!(
            serde_json::to_value(&bar.voices[0]).unwrap(),
            original["measures"][index]["voices"][0]
        );
        assert_eq!(bar.voices[1].len(), 4);
        assert!(bar.voices[1][0].is_rest);
        assert!(bar.voices[1][1].is_rest);
        assert_eq!(bar.voices[1][2].pitches[0].to_midi(), [72, 74][index]);
        assert!(bar.voices[1][3].is_rest);
        assert!((bar.voices[1].iter().map(Note::beats).sum::<f64>() - 4.0).abs() < 1e-9);
    }
    assert_eq!(result.fills[0].generated_note.note, 2);
    assert_eq!(result.arrangement.generated_notes.len(), 2);
    assert!(
        output.parts[0].staves[1]
            .measures
            .iter()
            .all(|bar| bar.voices[0].iter().all(|note| note.is_rest))
    );
    assert_eq!(
        serde_json::to_value(&score.parts[0].staves[0]).unwrap(),
        original
    );
    let response_times = acorde_core::to_playback_events(output, &Default::default())
        .iter()
        .filter(|event| {
            event
                .source
                .as_ref()
                .is_some_and(|source| source.staff == 0 && source.voice == 1)
        })
        .map(|event| event.time_beats)
        .collect::<Vec<_>>();
    assert_eq!(response_times, vec![2.0, 6.0]);
}

#[test]
fn selected_responses_follow_written_repeat_playback() {
    let (mut score, accompaniment) = fixture();
    score.parts[0].staves[0].measures[0].barline_left = Barline::RepeatStart;
    score.parts[0].staves[0].measures[1].barline_right = Barline::RepeatEnd;
    let result = arrange_with_response_fills(
        &score,
        accompaniment,
        AccordionResponseOptions {
            rest_notes: vec![rest_address(0)],
        },
    );
    assert!(
        result.arrangement.diagnostics.is_empty(),
        "{:?}",
        result.arrangement.diagnostics
    );
    let output = result.arrangement.score.unwrap();
    assert_eq!(acorde_core::measure_sequence(&output), vec![0, 1, 0, 1]);
    let response_times = acorde_core::to_playback_events(&output, &Default::default())
        .iter()
        .filter(|event| {
            event
                .source
                .as_ref()
                .is_some_and(|source| source.staff == 0 && source.voice == 1)
        })
        .map(|event| event.time_beats)
        .collect::<Vec<_>>();
    assert_eq!(response_times, vec![2.0, 10.0]);
}

#[test]
fn invalid_or_unplayable_rest_selection_returns_no_partial_arrangement() {
    let (score, accompaniment) = fixture();
    let mut invalid_voice = rest_address(0);
    invalid_voice.voice = 1;
    let mut huge_index = rest_address(0);
    huge_index.note = usize::MAX;
    for selection in [
        vec![invalid_voice],
        vec![huge_index],
        vec![rest_address(0), rest_address(0)],
    ] {
        let result = arrange_with_response_fills(
            &score,
            accompaniment.clone(),
            AccordionResponseOptions {
                rest_notes: selection,
            },
        );
        assert!(result.arrangement.score.is_none());
        assert!(result.arrangement.generated_notes.is_empty());
        assert!(result.fills.is_empty());
    }
    let mut low = score;
    low.parts[0].staves[0].measures[1].voices[0][0].pitches[0] = Pitch::from_midi(64, false);
    low.parts[0].staves[0].measures[1].voices[0][2].pitches[0] = Pitch::from_midi(60, false);
    let result = arrange_with_response_fills(
        &low,
        accompaniment,
        AccordionResponseOptions {
            rest_notes: vec![rest_address(0), rest_address(1)],
        },
    );
    assert!(result.arrangement.score.is_none());
    assert!(result.arrangement.generated_notes.is_empty());
    assert!(result.fills.is_empty());
    assert_eq!(result.arrangement.diagnostics[0].code, "response-range");
}
