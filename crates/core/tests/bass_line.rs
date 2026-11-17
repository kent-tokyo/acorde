use acorde_core::{
    BassLineOptions, BassLinePattern, BassRhythmSection, BassRhythmSlot, BassRhythmTone,
    ChordalPadOptions, ChordalPadPattern, ChordalPadVoicing, Duration, Note, Pitch,
    PositionedAccordionChord, Score, arrange_bass_line, arrange_chordal_pad,
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
        rhythm_sections: vec![],
    }
}

fn slot(ticks: u16, tone: Option<BassRhythmTone>, accent: bool) -> BassRhythmSlot {
    BassRhythmSlot {
        ticks,
        tone,
        accent,
    }
}

#[test]
fn authored_cells_create_walking_syncopation_anticipation_and_ostinato() {
    let source = Score::new("rhythmic bass", 120, 4, 4, 0, 3);
    let mut request = options(BassLinePattern::RootPulse);
    request.chords.push(placed(2, 0.0, "G"));
    request.rhythm_sections = vec![
        BassRhythmSection {
            start_measure: 0,
            end_measure_exclusive: 1,
            slots: [
                BassRhythmTone::Bass,
                BassRhythmTone::Third,
                BassRhythmTone::Fifth,
                BassRhythmTone::ApproachBelow,
            ]
            .into_iter()
            .map(|tone| slot(32, Some(tone), false))
            .collect(),
        },
        BassRhythmSection {
            start_measure: 1,
            end_measure_exclusive: 2,
            slots: [
                None,
                Some(BassRhythmTone::Bass),
                None,
                Some(BassRhythmTone::Fifth),
                None,
                Some(BassRhythmTone::Bass),
                None,
                Some(BassRhythmTone::NextBass),
            ]
            .into_iter()
            .map(|tone| slot(16, tone, tone.is_some()))
            .collect(),
        },
        BassRhythmSection {
            start_measure: 2,
            end_measure_exclusive: 3,
            slots: [
                Some(BassRhythmTone::Bass),
                None,
                Some(BassRhythmTone::Fifth),
                None,
                Some(BassRhythmTone::Bass),
                None,
                Some(BassRhythmTone::Fifth),
                None,
            ]
            .into_iter()
            .map(|tone| slot(16, tone, false))
            .collect(),
        },
    ];
    let result = arrange_bass_line(&source, request);
    assert!(result.diagnostics.is_empty(), "{:?}", result.diagnostics);
    let score = result.score.as_ref().unwrap();
    assert_eq!(
        serde_json::to_value(&score.parts[0]).unwrap(),
        serde_json::to_value(&source.parts[0]).unwrap()
    );
    let classes = |measure: usize| {
        score.parts[1].staves[0].measures[measure].voices[0]
            .iter()
            .filter(|note| !note.is_rest && !note.tie_end)
            .map(|note| note.pitches[0].to_midi() % 12)
            .collect::<Vec<_>>()
    };
    assert_eq!(classes(0), [0, 4, 7, 4]);
    assert_eq!(classes(1), [5, 0, 5, 7]);
    assert_eq!(classes(2), [7, 2, 7, 2]);
    let syncopated = &score.parts[1].staves[0].measures[1].voices[0];
    assert!(syncopated.iter().any(|note| note.is_rest));
    assert_eq!(
        syncopated
            .iter()
            .filter(|note| !note.is_rest)
            .filter(|note| note
                .articulations
                .contains(&acorde_core::Articulation::Accent))
            .count(),
        4
    );
    let events = acorde_core::to_playback_events(score, &Default::default());
    let onsets = events
        .iter()
        .filter(|event| event.part_index == 1 && (4.0..8.0).contains(&event.time_beats))
        .map(|event| event.time_beats)
        .collect::<Vec<_>>();
    assert_eq!(onsets, [4.5, 5.5, 6.5, 7.5]);
    assert!(result.generated_notes.iter().all(|address| {
        !score.parts[address.part].staves[address.staff].measures[address.measure].voices
            [address.voice][address.note]
            .is_rest
    }));
    assert!(acorde_core::model::validate::validate(score).is_valid());
}

#[test]
fn authored_bass_cells_cover_pickup_and_irregular_meter_and_split_chord_changes() {
    let mut source = Score::new("irregular bass", 120, 5, 8, 0, 2);
    source.parts[0].staves[0].measures[0].actual_length =
        acorde_core::MeasureLength::from_beats(0.5);
    source.parts[0].staves[0].measures[0].voices[0] =
        vec![Note::new(Pitch::from_midi(72, false), Duration::Eighth)];
    let mut request = options(BassLinePattern::RootPulse);
    request.rhythm_sections = vec![
        BassRhythmSection {
            start_measure: 0,
            end_measure_exclusive: 1,
            slots: vec![slot(16, Some(BassRhythmTone::Bass), false)],
        },
        BassRhythmSection {
            start_measure: 1,
            end_measure_exclusive: 2,
            slots: vec![
                slot(48, Some(BassRhythmTone::Bass), true),
                slot(32, None, false),
            ],
        },
    ];
    let result = arrange_bass_line(&source, request);
    assert!(result.diagnostics.is_empty(), "{:?}", result.diagnostics);
    let score = result.score.unwrap();
    let bars = &score.parts[1].staves[0].measures;
    assert_eq!(bars[0].voices[0].iter().map(Note::beats).sum::<f64>(), 0.5);
    assert_eq!(bars[1].voices[0].iter().map(Note::beats).sum::<f64>(), 2.5);
    assert!(acorde_core::model::validate::validate(&score).is_valid());

    let source = Score::new("split bass", 120, 4, 4, 0, 1);
    let mut request = options(BassLinePattern::RootPulse);
    request.chords = vec![placed(0, 0.0, "C"), placed(0, 2.0, "F")];
    request.rhythm_sections = vec![BassRhythmSection {
        start_measure: 0,
        end_measure_exclusive: 1,
        slots: vec![slot(128, Some(BassRhythmTone::Bass), false)],
    }];
    let result = arrange_bass_line(&source, request);
    assert!(result.diagnostics.is_empty(), "{:?}", result.diagnostics);
    let score = result.score.unwrap();
    let events = acorde_core::to_playback_events(&score, &Default::default())
        .into_iter()
        .filter(|event| event.part_index == 1)
        .map(|event| (event.time_beats, event.pitch_midi % 12))
        .collect::<Vec<_>>();
    assert_eq!(events, [(0.0, 0), (2.0, 5)]);
}

#[test]
fn authored_rhythm_keeps_repeat_back_motion_in_playback_report() {
    let mut source = Score::new("repeated authored rhythm", 120, 4, 4, 0, 2);
    source.parts[0].staves[0].measures[0].barline_left = acorde_core::Barline::RepeatStart;
    source.parts[0].staves[0].measures[1].barline_right = acorde_core::Barline::RepeatEnd;
    let mut request = options(BassLinePattern::RootPulse);
    request.rhythm_sections = vec![BassRhythmSection {
        start_measure: 0,
        end_measure_exclusive: 1,
        slots: vec![
            slot(64, Some(BassRhythmTone::Bass), false),
            slot(64, None, false),
        ],
    }];
    let result = arrange_bass_line(&source, request);
    assert!(result.diagnostics.is_empty(), "{:?}", result.diagnostics);
    let report = result.motion_report.unwrap();
    assert_eq!(report.written_attack_count, 5);
    assert_eq!(report.playback_attack_count, 10);
    assert_eq!(report.backward_jump_transition_count, 1);
    assert!(report.backward_jump_motion_cost > 0);
    assert!(acorde_core::model::validate::validate(&result.score.unwrap()).is_valid());
}

#[test]
fn invalid_authored_bass_rhythm_fails_without_partial_score() {
    let source = Score::new("invalid rhythm", 120, 4, 4, 0, 2);
    let mut request = options(BassLinePattern::RootPulse);
    request.rhythm_sections = vec![BassRhythmSection {
        start_measure: 0,
        end_measure_exclusive: 1,
        slots: vec![slot(32, Some(BassRhythmTone::Bass), false)],
    }];
    let invalid_length = arrange_bass_line(&source, request.clone());
    assert_eq!(invalid_length.diagnostics[0].code, "bass-rhythm-length");
    request.rhythm_sections[0].slots = vec![slot(128, None, true)];
    let invalid_rest = arrange_bass_line(&source, request.clone());
    assert_eq!(invalid_rest.diagnostics[0].code, "invalid-bass-rhythm");
    request.rhythm_sections[0].slots = vec![slot(128, Some(BassRhythmTone::NextBass), false)];
    let too_early = arrange_bass_line(&source, request.clone());
    assert_eq!(
        too_early.diagnostics[0].code,
        "bass-rhythm-anticipation-position"
    );
    request.rhythm_sections[0].start_measure = 1;
    request.rhythm_sections[0].end_measure_exclusive = 2;
    let missing_next = arrange_bass_line(&source, request);
    assert_eq!(
        missing_next.diagnostics[0].code,
        "bass-rhythm-anticipation-position"
    );
    for rejected in [invalid_length, invalid_rest, too_early, missing_next] {
        assert!(rejected.score.is_none());
        assert!(rejected.added_part.is_none());
        assert!(rejected.generated_notes.is_empty());
        assert!(rejected.motion_report.is_none());
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
