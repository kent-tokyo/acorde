use acorde_core::model::arrange::PositionedAccordionChord;
use acorde_core::model::standard_bass::{
    StandardBassChordButton, StandardBassNoteButton, StandardBassProfile,
};
use acorde_core::model::standard_bass_arrangement::StandardBassArrangementOptions;
use acorde_core::model::standard_bass_transition::{
    StandardBassTransitionPolicy, arrange_standard_bass_screened,
};
use acorde_core::{Barline, ChordSymbol, Duration, Note, Pitch, Score, Step};

fn chord(root: &str) -> ChordSymbol {
    serde_json::from_value(serde_json::json!({"root":root,"kind":"major"})).unwrap()
}
fn fixture() -> (Score, StandardBassArrangementOptions) {
    let mut score = Score::new("repeating melody", 120, 4, 4, 0, 2);
    for (i, bar) in score.parts[0].staves[0].measures.iter_mut().enumerate() {
        bar.voices[0] = vec![Note::new(
            Pitch::new(if i == 0 { Step::C } else { Step::D }, 5),
            Duration::Whole,
        )];
    }
    score.parts[0].staves[0].measures[0].barline_left = Barline::RepeatStart;
    score.parts[0].staves[0].measures[1].barline_right = Barline::RepeatEnd;
    let options = StandardBassArrangementOptions {
        harmony_source: Default::default(),
        melody_part: 0,
        melody_staff: 0,
        melody_voice: 0,
        allow_source_omissions: false,
        chords: vec![
            PositionedAccordionChord {
                measure_index: 0,
                offset_beats: 0.0,
                chord: chord("C"),
            },
            PositionedAccordionChord {
                measure_index: 1,
                offset_beats: 0.0,
                chord: chord("G"),
            },
        ],
        profile: StandardBassProfile {
            bass_buttons: vec![
                StandardBassNoteButton {
                    id: "C bass".into(),
                    position: 0,
                    midi: 36,
                },
                StandardBassNoteButton {
                    id: "G bass".into(),
                    position: 5,
                    midi: 43,
                },
            ],
            chord_buttons: vec![
                StandardBassChordButton {
                    id: "C major".into(),
                    position: 0,
                    midi: vec![60, 64, 67],
                },
                StandardBassChordButton {
                    id: "G major".into(),
                    position: 5,
                    midi: vec![67, 71, 74],
                },
            ],
            max_distance: 0,
        },
        allow_chord_omissions: false,
        allow_chord_additions: false,
    };
    (score, options)
}

#[test]
fn checks_actual_repeat_playback_order_and_reports_button_movements() {
    let (score, options) = fixture();
    let before = serde_json::to_value(&score).unwrap();
    let policy = StandardBassTransitionPolicy {
        max_position_delta: 5,
        min_attack_interval_ms: 2000,
    };
    let result = arrange_standard_bass_screened(&score, &options, &[], &policy);
    assert!(
        result.arrangement.arrangement.diagnostics.is_empty(),
        "{:?}",
        result.arrangement.arrangement.diagnostics
    );
    assert!(result.arrangement.arrangement.score.is_some());
    assert_eq!(serde_json::to_value(&score).unwrap(), before);
    assert_eq!(
        result
            .transitions
            .iter()
            .map(|t| t.bass_button_id.as_str())
            .collect::<Vec<_>>(),
        ["C bass", "G bass", "C bass", "G bass"]
    );
    assert_eq!(
        result
            .transitions
            .iter()
            .map(|t| t.time_seconds)
            .collect::<Vec<_>>(),
        [0.0, 2.0, 4.0, 6.0]
    );
    assert_eq!(
        result
            .transitions
            .iter()
            .map(|t| t.position_delta)
            .collect::<Vec<_>>(),
        [None, Some(5), Some(5), Some(5)]
    );
}

#[test]
fn too_large_or_too_fast_transitions_return_no_partial_score() {
    let (score, options) = fixture();
    for (policy, code) in [
        (
            StandardBassTransitionPolicy {
                max_position_delta: 4,
                min_attack_interval_ms: 0,
            },
            "transition-too-far",
        ),
        (
            StandardBassTransitionPolicy {
                max_position_delta: 5,
                min_attack_interval_ms: 2001,
            },
            "transition-too-fast",
        ),
    ] {
        let result = arrange_standard_bass_screened(&score, &options, &[], &policy);
        assert_eq!(result.arrangement.arrangement.diagnostics[0].code, code);
        assert!(result.arrangement.arrangement.score.is_none());
        assert!(result.arrangement.arrangement.actions.is_empty());
        assert!(result.arrangement.arrangement.source_pitches.is_empty());
        assert!(result.transitions.is_empty());
    }
}

#[test]
fn tempo_changes_change_real_time_screening() {
    let (mut score, options) = fixture();
    score.settings.tempo_bpm = 240;
    let policy = StandardBassTransitionPolicy {
        max_position_delta: 5,
        min_attack_interval_ms: 1001,
    };
    let result = arrange_standard_bass_screened(&score, &options, &[], &policy);
    assert_eq!(
        result.arrangement.arrangement.diagnostics[0].code,
        "transition-too-fast"
    );
}

#[test]
fn tied_notation_fragments_are_one_button_attack() {
    let (mut score, mut options) = fixture();
    score.settings.time_signature.numerator = 3;
    score.parts[0].staves[0].measures.truncate(1);
    score.parts[0].staves[0].measures[0].barline_left = Barline::Normal;
    let mut melody = Note::new(Pitch::new(Step::C, 5), Duration::Half);
    melody.dot_count = 1;
    score.parts[0].staves[0].measures[0].voices[0] = vec![melody];
    options.chords.truncate(1);
    let result = arrange_standard_bass_screened(
        &score,
        &options,
        &[],
        &StandardBassTransitionPolicy {
            max_position_delta: 0,
            min_attack_interval_ms: 0,
        },
    );
    assert!(
        result.arrangement.arrangement.diagnostics.is_empty(),
        "{:?}",
        result.arrangement.arrangement.diagnostics
    );
    assert_eq!(result.transitions.len(), 1);
    assert!(result.arrangement.arrangement.actions.len() > 1);
}

fn with_nearby_g_buttons(options: &mut StandardBassArrangementOptions) {
    options.profile.bass_buttons.push(StandardBassNoteButton {
        id: "Z near G bass".into(),
        position: 1,
        midi: 43,
    });
    options.profile.chord_buttons.push(StandardBassChordButton {
        id: "Z near G major".into(),
        position: 1,
        midi: vec![67, 71, 74],
    });
}

#[test]
fn searches_same_sounding_alternatives_across_repeat_back_edges() {
    use acorde_core::model::standard_bass_button_search::arrange_standard_bass_find_buttons;
    use acorde_core::to_playback_events;
    let (score, mut options) = fixture();
    with_nearby_g_buttons(&mut options);
    let policy = StandardBassTransitionPolicy {
        max_position_delta: 1,
        min_attack_interval_ms: 2000,
    };
    let initial = arrange_standard_bass_screened(&score, &options, &[], &policy);
    assert_eq!(
        initial.arrangement.arrangement.diagnostics[0].code,
        "transition-too-far"
    );
    let original = acorde_core::model::standard_bass_pitch_selection::arrange_standard_bass_with_pitch_selection(&score,&options,&[]).arrangement.score.unwrap();
    let result = arrange_standard_bass_find_buttons(&score, &options, &[], &policy);
    assert!(
        result.arrangement.arrangement.diagnostics.is_empty(),
        "{:?}",
        result.arrangement.arrangement.diagnostics
    );
    assert_eq!(
        result
            .transitions
            .iter()
            .map(|t| t.bass_button_id.as_str())
            .collect::<Vec<_>>(),
        ["C bass", "Z near G bass", "C bass", "Z near G bass"]
    );
    assert_eq!(
        result
            .transitions
            .iter()
            .map(|t| t.position_delta)
            .collect::<Vec<_>>(),
        [None, Some(1), Some(1), Some(1)]
    );
    let signature = |score: &Score| {
        to_playback_events(score, &Default::default())
            .into_iter()
            .map(|e| (e.pitch_midi, e.time_beats, e.duration_beats))
            .collect::<Vec<_>>()
    };
    assert_eq!(
        signature(&original),
        signature(result.arrangement.arrangement.score.as_ref().unwrap())
    );
}

#[test]
fn no_path_or_timing_failure_returns_no_partial_arrangement() {
    use acorde_core::model::standard_bass_button_search::arrange_standard_bass_find_buttons;
    let (score, mut options) = fixture();
    let policy = StandardBassTransitionPolicy {
        max_position_delta: 1,
        min_attack_interval_ms: 0,
    };
    let unavailable = arrange_standard_bass_find_buttons(&score, &options, &[], &policy);
    assert_eq!(
        unavailable.arrangement.arrangement.diagnostics[0].code,
        "no-same-sounding-button-path"
    );
    assert!(unavailable.arrangement.arrangement.score.is_none());
    assert!(
        unavailable
            .arrangement
            .arrangement
            .source_pitches
            .is_empty()
            && unavailable.transitions.is_empty()
    );
    with_nearby_g_buttons(&mut options);
    let too_fast = arrange_standard_bass_find_buttons(
        &score,
        &options,
        &[],
        &StandardBassTransitionPolicy {
            max_position_delta: 1,
            min_attack_interval_ms: 2001,
        },
    );
    assert_eq!(
        too_fast.arrangement.arrangement.diagnostics[0].code,
        "transition-too-fast"
    );
    assert!(too_fast.arrangement.arrangement.score.is_none());
}

#[test]
fn excessive_alternatives_report_incomplete_search_not_impossibility() {
    use acorde_core::model::standard_bass_button_search::arrange_standard_bass_find_buttons;
    let (score, mut options) = fixture();
    options.profile.chord_buttons.push(StandardBassChordButton {
        id: "Z near chord".into(),
        position: 1,
        midi: vec![67, 71, 74],
    });
    for i in 0..33 {
        options.profile.bass_buttons.push(StandardBassNoteButton {
            id: format!("Z near {i:02}"),
            position: 1,
            midi: 43,
        });
    }
    let result = arrange_standard_bass_find_buttons(
        &score,
        &options,
        &[],
        &StandardBassTransitionPolicy {
            max_position_delta: 1,
            min_attack_interval_ms: 0,
        },
    );
    assert_eq!(
        result.arrangement.arrangement.diagnostics[0].code,
        "transition-search-limit"
    );
    assert!(result.arrangement.arrangement.score.is_none());
}

#[test]
fn candidate_search_accounts_for_repeat_return_that_written_order_misses() {
    use acorde_core::model::standard_bass_button_search::arrange_standard_bass_find_buttons;
    let (mut score, mut options) = fixture();
    score.parts[0].staves[0].measures[1].barline_right = Barline::Normal;
    let mut last = acorde_core::Measure::empty(4, 4);
    last.number = 3;
    last.barline_right = Barline::RepeatEnd;
    last.voices[0] = vec![Note::new(Pitch::new(Step::E, 5), Duration::Whole)];
    score.parts[0].staves[0].measures.push(last);
    options.profile.bass_buttons[1].position = 1;
    options.profile.chord_buttons[1].position = 1;
    options.chords.push(PositionedAccordionChord {
        measure_index: 2,
        offset_beats: 0.0,
        chord: chord("D"),
    });
    options.profile.bass_buttons.extend([
        StandardBassNoteButton {
            id: "A far D".into(),
            position: 2,
            midi: 38,
        },
        StandardBassNoteButton {
            id: "Z near D".into(),
            position: 1,
            midi: 38,
        },
    ]);
    options.profile.chord_buttons.extend([
        StandardBassChordButton {
            id: "A far D major".into(),
            position: 2,
            midi: vec![62, 66, 69],
        },
        StandardBassChordButton {
            id: "Z near D major".into(),
            position: 1,
            midi: vec![62, 66, 69],
        },
    ]);
    let policy = StandardBassTransitionPolicy {
        max_position_delta: 1,
        min_attack_interval_ms: 0,
    };
    let first = arrange_standard_bass_screened(&score, &options, &[], &policy);
    assert_eq!(
        first.arrangement.arrangement.diagnostics[0].code,
        "transition-too-far"
    );
    let found = arrange_standard_bass_find_buttons(&score, &options, &[], &policy);
    assert!(
        found.arrangement.arrangement.diagnostics.is_empty(),
        "{:?}",
        found.arrangement.arrangement.diagnostics
    );
    assert_eq!(
        found
            .transitions
            .iter()
            .map(|t| t.bass_button_id.as_str())
            .collect::<Vec<_>>(),
        [
            "C bass", "G bass", "Z near D", "C bass", "G bass", "Z near D"
        ]
    );
}

#[test]
fn explicit_voicing_search_changes_only_accompaniment_to_actual_button_notes() {
    use acorde_core::model::standard_bass_button_search::{
        arrange_standard_bass_find_buttons, arrange_standard_bass_find_voicings,
    };
    use acorde_core::to_playback_events;
    let (score, mut options) = fixture();
    let before = serde_json::to_value(&score).unwrap();
    options.profile.bass_buttons.push(StandardBassNoteButton {
        id: "Z near G bass".into(),
        position: 1,
        midi: 43,
    });
    options.profile.chord_buttons.push(StandardBassChordButton {
        id: "Z low G chord".into(),
        position: 1,
        midi: vec![55, 59, 62],
    });
    let policy = StandardBassTransitionPolicy {
        max_position_delta: 1,
        min_attack_interval_ms: 2000,
    };
    let exact = arrange_standard_bass_find_buttons(&score, &options, &[], &policy);
    assert_eq!(
        exact.arrangement.arrangement.diagnostics[0].code,
        "no-same-sounding-button-path"
    );
    let initial = acorde_core::model::standard_bass_pitch_selection::arrange_standard_bass_with_pitch_selection(&score,&options,&[]).arrangement.score.unwrap();
    let result = arrange_standard_bass_find_voicings(&score, &options, &[], &policy);
    assert!(
        result.arrangement.arrangement.diagnostics.is_empty(),
        "{:?}",
        result.arrangement.arrangement.diagnostics
    );
    assert_eq!(
        result
            .transitions
            .iter()
            .map(|t| t.chord_button_id.as_str())
            .collect::<Vec<_>>(),
        ["C major", "Z low G chord", "C major", "Z low G chord"]
    );
    assert_eq!(serde_json::to_value(&score).unwrap(), before);
    let output = result.arrangement.arrangement.score.unwrap();
    let note = &output.parts[0].staves[1].measures[1].voices[0][0];
    assert_eq!(
        note.pitches.iter().map(|p| p.to_midi()).collect::<Vec<_>>(),
        [43, 55, 59, 62]
    );
    let signature = |s: &Score, staff: usize| {
        to_playback_events(s, &Default::default())
            .into_iter()
            .filter(|e| e.source.as_ref().is_some_and(|a| a.staff == staff))
            .map(|e| (e.pitch_midi, e.time_beats, e.duration_beats))
            .collect::<Vec<_>>()
    };
    assert_eq!(signature(&initial, 0), signature(&output, 0));
    assert_ne!(signature(&initial, 1), signature(&output, 1));
    let bass = signature(&output, 1);
    assert!(bass.iter().any(|e| e.0 == 55));
    assert!(!bass.iter().any(|e| e.0 == 74));
}

#[test]
fn different_pitch_classes_are_not_silently_accepted_as_new_voicings() {
    use acorde_core::model::standard_bass_button_search::arrange_standard_bass_find_voicings;
    let (score, mut options) = fixture();
    options.allow_chord_omissions = true;
    options.profile.bass_buttons.push(StandardBassNoteButton {
        id: "Z near G bass".into(),
        position: 1,
        midi: 43,
    });
    options.profile.chord_buttons.push(StandardBassChordButton {
        id: "Z incomplete G".into(),
        position: 1,
        midi: vec![55, 59],
    });
    let result = arrange_standard_bass_find_voicings(
        &score,
        &options,
        &[],
        &StandardBassTransitionPolicy {
            max_position_delta: 1,
            min_attack_interval_ms: 0,
        },
    );
    assert_eq!(
        result.arrangement.arrangement.diagnostics[0].code,
        "no-equivalent-voicing-path"
    );
    assert!(result.arrangement.arrangement.score.is_none() && result.transitions.is_empty());
}

#[test]
fn alternate_voicing_updates_every_fragment_of_tied_accompaniment() {
    use acorde_core::model::standard_bass_button_search::arrange_standard_bass_find_voicings;
    let (mut score, mut options) = fixture();
    score.settings.time_signature.numerator = 3;
    for measure in &mut score.parts[0].staves[0].measures {
        let mut melody = Note::new(Pitch::new(Step::C, 5), Duration::Half);
        melody.dot_count = 1;
        measure.voices[0] = vec![melody];
    }
    options.profile.bass_buttons.push(StandardBassNoteButton {
        id: "Z near G bass".into(),
        position: 1,
        midi: 43,
    });
    options.profile.chord_buttons.push(StandardBassChordButton {
        id: "Z low G chord".into(),
        position: 1,
        midi: vec![55, 59, 62],
    });
    let result = arrange_standard_bass_find_voicings(
        &score,
        &options,
        &[],
        &StandardBassTransitionPolicy {
            max_position_delta: 1,
            min_attack_interval_ms: 0,
        },
    );
    assert!(
        result.arrangement.arrangement.diagnostics.is_empty(),
        "{:?}",
        result.arrangement.arrangement.diagnostics
    );
    let output = result.arrangement.arrangement.score.unwrap();
    let notes = &output.parts[0].staves[1].measures[1].voices[0];
    assert_eq!(notes.len(), 2);
    assert!(notes[0].tie_start && notes[1].tie_end);
    for note in notes {
        assert_eq!(
            note.pitches.iter().map(|p| p.to_midi()).collect::<Vec<_>>(),
            [43, 55, 59, 62]
        );
    }
    let attacks = acorde_core::to_playback_events(&output, &Default::default());
    assert!(
        attacks
            .iter()
            .any(|e| e.pitch_midi == 55 && e.duration_beats == 3.0)
    );
}

#[test]
fn alternate_voicing_can_change_octave_doublings_without_changing_pitch_classes() {
    use acorde_core::model::standard_bass_button_search::arrange_standard_bass_find_voicings;
    let (score, mut options) = fixture();
    options.profile.bass_buttons.push(StandardBassNoteButton {
        id: "Z near G bass".into(),
        position: 1,
        midi: 43,
    });
    options.profile.chord_buttons.push(StandardBassChordButton {
        id: "Z doubled G chord".into(),
        position: 1,
        midi: vec![55, 59, 62, 67],
    });
    let result = arrange_standard_bass_find_voicings(
        &score,
        &options,
        &[],
        &StandardBassTransitionPolicy {
            max_position_delta: 1,
            min_attack_interval_ms: 0,
        },
    );
    assert!(
        result.arrangement.arrangement.diagnostics.is_empty(),
        "{:?}",
        result.arrangement.arrangement.diagnostics
    );
    let output = result.arrangement.arrangement.score.unwrap();
    assert_eq!(
        output.parts[0].staves[1].measures[1].voices[0][0]
            .pitches
            .iter()
            .map(|p| p.to_midi())
            .collect::<Vec<_>>(),
        [43, 55, 59, 62, 67]
    );
}

fn located_for_alternative_search(
    options: &StandardBassArrangementOptions,
) -> acorde_core::model::standard_bass_button_layout::LocatedStandardBassProfile {
    use acorde_core::model::standard_bass_button_layout::{
        LocatedStandardBassProfile, StandardBassButtonLocation, StandardBassButtonRow as Row,
    };
    LocatedStandardBassProfile {
        profile: options.profile.clone(),
        locations: vec![
            StandardBassButtonLocation {
                id: "C bass".into(),
                row: Row::Fundamental,
                column: 0,
            },
            StandardBassButtonLocation {
                id: "G bass".into(),
                row: Row::Fundamental,
                column: 1,
            },
            StandardBassButtonLocation {
                id: "Z near G bass".into(),
                row: Row::Fundamental,
                column: 8,
            },
            StandardBassButtonLocation {
                id: "C major".into(),
                row: Row::Major,
                column: 0,
            },
            StandardBassButtonLocation {
                id: "G major".into(),
                row: Row::Major,
                column: 1,
            },
            StandardBassButtonLocation {
                id: "Z near G major".into(),
                row: Row::Major,
                column: 8,
            },
        ],
    }
}

#[test]
fn located_selected_search_finds_alternative_across_repeats_without_losing_melody() {
    use acorde_core::model::standard_bass_button_search::find_located_standard_bass_path;
    use acorde_core::model::standard_bass_pitch_selection::MelodyPitchSelection;
    use acorde_core::model::standard_bass_transition::arrange_standard_bass_located_selected_screened;
    use acorde_core::{NoteAddr, to_playback_events};
    let (mut score, mut options) = fixture();
    with_nearby_g_buttons(&mut options);
    let original = serde_json::to_value(&score).unwrap();
    score.parts[0].staves[0].measures[0].voices[0][0]
        .pitches
        .insert(0, Pitch::new(Step::C, 4));
    options.allow_source_omissions = true;
    let profile = located_for_alternative_search(&options);
    let selections = [MelodyPitchSelection {
        source: NoteAddr {
            part: 0,
            staff: 0,
            measure: 0,
            voice: 0,
            note: 0,
        },
        pitch_index: 1,
    }];
    let policy = StandardBassTransitionPolicy {
        max_position_delta: 1,
        min_attack_interval_ms: 2000,
    };
    let initial = arrange_standard_bass_located_selected_screened(
        &score,
        &options,
        &selections,
        &profile,
        &policy,
    );
    assert_eq!(
        initial.arrangement.arrangement.diagnostics[0].code,
        "transition-too-far"
    );
    let source = serde_json::to_value(&score).unwrap();
    let found =
        find_located_standard_bass_path(&score, &options, &selections, &profile, &policy, false);
    assert!(
        found.arrangement.arrangement.diagnostics.is_empty(),
        "{:?}",
        found.arrangement.arrangement.diagnostics
    );
    assert_eq!(
        found
            .transitions
            .iter()
            .map(|t| t.bass_button_id.as_str())
            .collect::<Vec<_>>(),
        ["C bass", "Z near G bass", "C bass", "Z near G bass"]
    );
    assert!(
        found
            .arrangement
            .arrangement
            .source_pitches
            .iter()
            .any(|p| p.source_pitch == 0 && p.source.measure == 0 && p.target.is_none())
    );
    let output = found.arrangement.arrangement.score.as_ref().unwrap();
    let signature = |s: &Score| {
        to_playback_events(s, &Default::default())
            .into_iter()
            .filter(|e| e.source.as_ref().is_some_and(|a| a.staff == 0) && e.pitch_midi >= 72)
            .map(|e| (e.pitch_midi, e.time_beats, e.duration_beats))
            .collect::<Vec<_>>()
    };
    assert_eq!(signature(&score), signature(output));
    assert_eq!(serde_json::to_value(&score).unwrap(), source);
    assert_ne!(source, original);
    let missing = find_located_standard_bass_path(&score, &options, &[], &profile, &policy, false);
    assert_eq!(
        missing.arrangement.arrangement.diagnostics[0].code,
        "ambiguous-melody"
    );
    assert!(missing.arrangement.arrangement.score.is_none() && missing.transitions.is_empty());
}

#[test]
fn located_search_changes_voicing_only_when_explicitly_allowed() {
    use acorde_core::model::standard_bass_button_search::find_located_standard_bass_path;
    let (score, mut options) = fixture();
    with_nearby_g_buttons(&mut options);
    options.profile.chord_buttons.last_mut().unwrap().midi = vec![55, 59, 62];
    let profile = located_for_alternative_search(&options);
    let policy = StandardBassTransitionPolicy {
        max_position_delta: 1,
        min_attack_interval_ms: 0,
    };
    let exact = find_located_standard_bass_path(&score, &options, &[], &profile, &policy, false);
    assert_eq!(
        exact.arrangement.arrangement.diagnostics[0].code,
        "no-same-sounding-button-path"
    );
    assert!(exact.arrangement.arrangement.score.is_none());
    let voiced = find_located_standard_bass_path(&score, &options, &[], &profile, &policy, true);
    assert!(
        voiced.arrangement.arrangement.diagnostics.is_empty(),
        "{:?}",
        voiced.arrangement.arrangement.diagnostics
    );
    assert_eq!(voiced.transitions[1].chord_button_id, "Z near G major");
    let score = voiced.arrangement.arrangement.score.unwrap();
    let actual: Vec<_> = score.parts[0].staves[1].measures[1].voices[0][0]
        .pitches
        .iter()
        .map(|p| p.to_midi())
        .collect();
    assert_eq!(actual, [43, 55, 59, 62]);
}
