use acorde_core::{
    CountermelodyOptions, Duration, Note, Pitch, PositionedAccordionChord, Score,
    arrange_countermelody, to_playback_events,
};

fn chord(measure: usize, beat: f64, root: &str) -> PositionedAccordionChord {
    PositionedAccordionChord {
        measure_index: measure,
        offset_beats: beat,
        chord: serde_json::from_value(serde_json::json!({"root":root,"kind":"major"})).unwrap(),
    }
}

fn source() -> Score {
    let mut score = Score::new("motif source", 120, 4, 4, 0, 4);
    score.parts[0].staves[0].measures[0].voices[0] = [84, 86, 88, 84]
        .into_iter()
        .map(|midi| Note::new(Pitch::from_midi(midi, false), Duration::Quarter))
        .collect();
    for bar in &mut score.parts[0].staves[0].measures[1..] {
        bar.voices[0] = vec![Note::new(Pitch::from_midi(84, false), Duration::Whole)];
    }
    score
}

fn options() -> CountermelodyOptions {
    CountermelodyOptions {
        chords: vec![
            chord(0, 0.0, "C"),
            chord(0, 2.0, "F"),
            chord(1, 0.0, "G"),
            chord(2, 0.0, "C"),
            chord(3, 0.0, "F"),
        ],
        melody_part_index: 0,
        motif_start_measure: 0,
        motif_end_measure_exclusive: Some(1),
        midi_program: 73,
        low_midi: 68,
        center_midi: 74,
        high_midi: 80,
        min_melody_gap_semitones: 4,
        phrase_end_measures: Vec::new(),
    }
}

fn playback(score: &Score) -> Vec<(f64, f64, u8)> {
    to_playback_events(score, &Default::default())
        .into_iter()
        .filter(|event| event.source.as_ref().is_some_and(|addr| addr.part == 1))
        .map(|event| (event.time_beats, event.duration_beats, event.pitch_midi))
        .collect()
}

#[test]
fn authored_phrase_endings_leave_space_and_replay_exactly() {
    let score = source();
    let mut request = options();
    request.phrase_end_measures = vec![3, 1];
    let result = arrange_countermelody(&score, request);
    assert!(result.diagnostics.is_empty(), "{:?}", result.diagnostics);
    assert_eq!(result.resolved_options.phrase_end_measures, vec![1, 3]);
    let output = result.score.unwrap();
    for measure in [1, 3] {
        let attacks = output.parts[1].staves[0].measures[measure].voices[0]
            .iter()
            .filter(|note| !note.is_rest && !note.tie_end)
            .count();
        assert_eq!(attacks, 1);
    }
    let replay = arrange_countermelody(&score, result.resolved_options);
    assert_eq!(playback(&output), playback(&replay.score.unwrap()));

    let mut invalid = options();
    invalid.phrase_end_measures = vec![1, 1];
    let rejected = arrange_countermelody(&score, invalid);
    assert!(rejected.score.is_none());
    assert_eq!(rejected.diagnostics[0].code, "invalid-phrase-end");
}

#[test]
fn motif_develops_across_bars_and_chord_changes_beneath_the_source_melody() {
    let score = source();
    let before = serde_json::to_value(&score.parts).unwrap();
    let result = arrange_countermelody(&score, options());
    assert!(result.diagnostics.is_empty(), "{:?}", result.diagnostics);
    assert_eq!(result.motif_intervals, vec![2, 2, -4]);
    assert_eq!(result.attack_count, 8);
    assert_eq!(result.resolved_options.motif_end_measure_exclusive, Some(1));
    let replay = arrange_countermelody(&score, result.resolved_options.clone());
    let output = result.score.unwrap();
    assert_eq!(playback(&output), playback(&replay.score.unwrap()));
    assert_eq!(serde_json::to_value(&output.parts[..1]).unwrap(), before);
    assert_eq!(output.parts[1].midi_program, 73);
    assert_eq!(result.generated_notes.len(), 8);
    let attacks = playback(&output);
    assert_eq!(
        attacks.iter().map(|event| event.0).collect::<Vec<_>>(),
        vec![0.0, 2.0, 4.0, 6.0, 8.0, 10.0, 12.0, 14.0]
    );
    assert!(attacks.iter().all(|event| event.2 <= 80));
    assert!(
        attacks
            .iter()
            .any(|event| event.0 == 2.0 && [5, 9, 0].contains(&(event.2 % 12)))
    );
    assert!(acorde_core::model::validate::validate(&output).is_valid());
}

#[test]
fn unsupported_motif_and_register_return_no_partial_score() {
    let score = source();
    let mut no_motif = options();
    no_motif.motif_start_measure = 1;
    no_motif.motif_end_measure_exclusive = Some(3);
    let mut impossible = options();
    impossible.low_midi = 80;
    impossible.center_midi = 80;
    impossible.high_midi = 80;
    let mut bad_chord = options();
    bad_chord.chords[1].offset_beats = 1.0 / 64.0;
    for (request, code) in [
        (no_motif, "insufficient-motif"),
        (impossible, "unplayable-counter-register"),
        (bad_chord, "chord-position"),
    ] {
        let result = arrange_countermelody(&score, request);
        assert_eq!(result.diagnostics[0].code, code);
        assert!(result.score.is_none());
        assert!(result.added_part.is_none());
        assert!(result.generated_notes.is_empty());
        assert!(result.motif_intervals.is_empty());
        assert_eq!(result.attack_count, 0);
    }
}
