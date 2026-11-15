use acorde_core::{
    BassLineOptions, BassLinePattern, ChordalPadOptions, ChordalPadPattern, ChordalPadVoicing,
    Duration, Note, Pitch, PositionedAccordionChord, Score, arrange_bass_line, arrange_chordal_pad,
};

fn placed(measure_index: usize, offset_beats: f64, root: &str) -> PositionedAccordionChord {
    PositionedAccordionChord {
        measure_index,
        offset_beats,
        chord: serde_json::from_value(serde_json::json!({"root":root,"kind":"major"})).unwrap(),
    }
}

fn options(pattern: BassLinePattern) -> BassLineOptions {
    BassLineOptions {
        chords: vec![placed(0, 0.0, "C"), placed(1, 0.0, "F")],
        pattern,
        midi_program: 32,
        low_midi: 28,
        high_midi: 60,
        center_midi: 40,
    }
}

#[test]
fn bass_patterns_preserve_source_and_place_approach_before_next_chord() {
    let source = Score::new("bass source", 120, 4, 4, 0, 2);
    let root = arrange_bass_line(&source, options(BassLinePattern::RootPulse));
    let fifth = arrange_bass_line(&source, options(BassLinePattern::RootFifth));
    let approach = arrange_bass_line(&source, options(BassLinePattern::Approach));
    for result in [&root, &fifth, &approach] {
        assert!(result.diagnostics.is_empty(), "{:?}", result.diagnostics);
        assert_eq!(result.added_part, Some(1));
        let output = result.score.as_ref().unwrap();
        assert_eq!(
            serde_json::to_value(&output.parts[0]).unwrap(),
            serde_json::to_value(&source.parts[0]).unwrap()
        );
        assert!(acorde_core::model::validate::validate(output).is_valid());
        let motion = result.motion_report.as_ref().unwrap();
        assert_eq!(motion.written_attack_count, 8);
        assert_eq!(motion.playback_attack_count, 8);
        assert_eq!(motion.playback_motion_cost, motion.written_motion_cost);
    }
    let classes = |result: &acorde_core::BassLineResult, bar: usize| {
        result.score.as_ref().unwrap().parts[1].staves[0].measures[bar].voices[0]
            .iter()
            .map(|note| note.pitches[0].to_midi() % 12)
            .collect::<Vec<_>>()
    };
    assert_eq!(classes(&root, 0), vec![0, 0, 0, 0]);
    assert_eq!(classes(&fifth, 0), vec![0, 7, 0, 7]);
    assert_eq!(classes(&approach, 0), vec![0, 7, 0, 4]);
    assert_eq!(classes(&approach, 1)[0], 5);
}

#[test]
fn bass_motion_report_counts_repeat_back_leap() {
    let mut source = Score::new("repeated bass", 120, 4, 4, 0, 2);
    source.parts[0].staves[0].measures[0].barline_left =
        acorde_core::model::notation::Barline::RepeatStart;
    source.parts[0].staves[0].measures[1].barline_right =
        acorde_core::model::notation::Barline::RepeatEnd;
    let result = arrange_bass_line(&source, options(BassLinePattern::RootPulse));
    assert!(result.diagnostics.is_empty(), "{:?}", result.diagnostics);
    let report = result.motion_report.unwrap();
    assert_eq!(report.written_attack_count, 8);
    assert_eq!(report.playback_attack_count, 16);
    assert!(report.written_motion_cost > 0);
    assert_eq!(report.playback_motion_cost, report.written_motion_cost * 3);
    assert_eq!(report.backward_jump_motion_cost, report.written_motion_cost);
    assert_eq!(report.backward_jump_transition_count, 1);
    assert!(acorde_core::model::validate::validate(&result.score.unwrap()).is_valid());
}

#[test]
fn bass_can_follow_midbar_changes_and_layer_with_chordal_pad() {
    let source = Score::new("layered", 120, 4, 4, 0, 2);
    let chords = vec![
        placed(0, 0.0, "C"),
        placed(0, 2.0, "F"),
        placed(1, 0.0, "G"),
    ];
    let pad = arrange_chordal_pad(
        &source,
        ChordalPadOptions {
            chords: chords.clone(),
            pattern: ChordalPadPattern::OffbeatPulse,
            pattern_sections: Vec::new(),
            meter_group_sections: Vec::new(),
            phrase_end_measures: Vec::new(),
            voicing: ChordalPadVoicing::Smooth,
            midi_program: 48,
            low_midi: 48,
            high_midi: 84,
            center_midi: 64,
            min_melody_gap_semitones: None,
            melody_part_index: None,
        },
    );
    let pad_score = pad.score.unwrap();
    let mut bass_options = options(BassLinePattern::Approach);
    bass_options.chords = chords;
    let result = arrange_bass_line(&pad_score, bass_options);
    assert!(result.diagnostics.is_empty(), "{:?}", result.diagnostics);
    let output = result.score.unwrap();
    assert_eq!(output.parts.len(), 3);
    assert_eq!(output.parts[2].midi_channel, 2);
    assert_eq!(
        serde_json::to_value(&output.parts[..2]).unwrap(),
        serde_json::to_value(&pad_score.parts).unwrap()
    );
    let events = acorde_core::to_playback_events(&output, &Default::default());
    assert!(events.iter().any(|event| {
        event
            .source
            .as_ref()
            .is_some_and(|address| address.part == 2)
            && event.time_beats == 2.0
            && event.pitch_midi % 12 == 5
    }));
    assert!(acorde_core::model::validate::validate(&output).is_valid());
}

#[test]
fn pickup_and_five_eight_close_the_written_bar_exactly() {
    let mut source = Score::new("five", 120, 5, 8, 0, 2);
    source.parts[0].staves[0].measures[0].actual_length =
        acorde_core::MeasureLength::from_beats(0.5);
    source.parts[0].staves[0].measures[0].voices[0] =
        vec![Note::new(Pitch::from_midi(72, false), Duration::Eighth)];
    let mut request = options(BassLinePattern::RootFifth);
    request.chords = vec![placed(0, 0.0, "C"), placed(1, 0.0, "F")];
    let result = arrange_bass_line(&source, request);
    assert!(result.diagnostics.is_empty(), "{:?}", result.diagnostics);
    let output = result.score.unwrap();
    let bars = &output.parts[1].staves[0].measures;
    assert_eq!(
        bars[0].voices[0]
            .iter()
            .map(|note| note.beats())
            .sum::<f64>(),
        0.5
    );
    assert_eq!(
        bars[1].voices[0]
            .iter()
            .map(|note| note.beats())
            .sum::<f64>(),
        2.5
    );
    assert!(acorde_core::model::validate::validate(&output).is_valid());
}

#[test]
fn invalid_bass_inputs_return_no_partial_score() {
    let source = Score::new("invalid", 120, 4, 4, 0, 2);
    let mut missing = options(BassLinePattern::RootPulse);
    missing.chords.remove(0);
    let mut off_grid = options(BassLinePattern::RootPulse);
    off_grid.chords[1].offset_beats = 1.0 / 64.0;
    let mut range = options(BassLinePattern::Approach);
    range.low_midi = 36;
    range.high_midi = 36;
    range.center_midi = 36;
    for (request, expected) in [
        (missing, "missing-initial-chord"),
        (off_grid, "chord-position"),
        (range, "unplayable-register"),
    ] {
        let result = arrange_bass_line(&source, request);
        assert!(result.score.is_none());
        assert!(result.added_part.is_none());
        assert!(result.generated_notes.is_empty());
        assert!(result.motion_report.is_none());
        assert_eq!(result.diagnostics[0].code, expected);
    }
}
