use acorde_core::ChordSymbol;
use acorde_core::model::standard_bass::{
    StandardBassChordButton, StandardBassNoteButton, StandardBassProfile,
};
use acorde_core::model::standard_bass_button_layout::{
    LocatedStandardBassProfile, PreviousStandardBassButtons, StandardBassButtonLocation,
    StandardBassButtonRow as Row, located_standard_bass_candidates,
};
fn chord(bass: Option<&str>) -> ChordSymbol {
    serde_json::from_value(serde_json::json!({"root":"G","kind":"dominant","bass":bass})).unwrap()
}
fn fixture() -> LocatedStandardBassProfile {
    LocatedStandardBassProfile {
        profile: StandardBassProfile {
            bass_buttons: vec![
                StandardBassNoteButton {
                    id: "G bass".into(),
                    position: 0,
                    midi: 43,
                },
                StandardBassNoteButton {
                    id: "F counterbass".into(),
                    position: 1,
                    midi: 41,
                },
            ],
            chord_buttons: vec![
                StandardBassChordButton {
                    id: "G7".into(),
                    position: 0,
                    midi: vec![67, 71, 77],
                },
                StandardBassChordButton {
                    id: "B diminished".into(),
                    position: 1,
                    midi: vec![71, 74, 77],
                },
                StandardBassChordButton {
                    id: "G major".into(),
                    position: 0,
                    midi: vec![67, 71, 74],
                },
            ],
            max_distance: 2,
        },
        locations: vec![
            StandardBassButtonLocation {
                id: "G bass".into(),
                row: Row::Fundamental,
                column: 2,
            },
            StandardBassButtonLocation {
                id: "F counterbass".into(),
                row: Row::CounterBass,
                column: 1,
            },
            StandardBassButtonLocation {
                id: "G7".into(),
                row: Row::Seventh,
                column: 2,
            },
            StandardBassButtonLocation {
                id: "B diminished".into(),
                row: Row::Diminished,
                column: 3,
            },
            StandardBassButtonLocation {
                id: "G major".into(),
                row: Row::Major,
                column: 2,
            },
        ],
    }
}
#[test]
fn direct_seventh_preferred_only_when_explicit_row_and_policy_allow_it() {
    let profile = fixture();
    let previous = PreviousStandardBassButtons {
        bass_button_id: "F counterbass".into(),
        chord_button_id: "G major".into(),
    };
    let found =
        located_standard_bass_candidates(&profile, &chord(None), true, false, Some(&previous))
            .unwrap();
    assert_eq!(found[0].candidate.chord_button_id, "G7");
    assert!(found[0].direct_seventh);
    assert_eq!(found[0].candidate.omitted_pitch_classes, [2]);
    assert_eq!(found[0].bass_location.row, Row::Fundamental);
    assert_eq!(found[0].chord_location.column, 2);
    assert_eq!(found[0].transition_column_distance, Some(1));
    let exact =
        located_standard_bass_candidates(&profile, &chord(None), false, false, None).unwrap();
    assert_eq!(exact[0].candidate.chord_button_id, "B diminished");
    assert!(!exact[0].direct_seventh);
    let slash =
        located_standard_bass_candidates(&profile, &chord(Some("F")), false, false, None).unwrap();
    assert_eq!(slash[0].candidate.bass_button_id, "F counterbass");
    assert_eq!(slash[0].candidate.chord_button_id, "G major");
    assert!(!slash[0].direct_seventh);
}
#[test]
fn incomplete_or_inconsistent_layout_is_rejected() {
    let mut profile = fixture();
    profile.locations.pop();
    assert!(located_standard_bass_candidates(&profile, &chord(None), true, false, None).is_err());
    let mut profile = fixture();
    profile.locations[0].row = Row::Major;
    assert!(located_standard_bass_candidates(&profile, &chord(None), true, false, None).is_err());
    let mut profile = fixture();
    profile.locations[3].row = Row::Seventh;
    profile.locations[3].column = 2;
    assert!(located_standard_bass_candidates(&profile, &chord(None), true, false, None).is_err());
    let previous = PreviousStandardBassButtons {
        bass_button_id: "missing".into(),
        chord_button_id: "G7".into(),
    };
    assert!(
        located_standard_bass_candidates(&fixture(), &chord(None), true, false, Some(&previous))
            .is_err()
    );
}

#[test]
fn located_arrangement_sounds_the_chosen_buttons_and_keeps_melody() {
    use acorde_core::model::arrange::PositionedAccordionChord;
    use acorde_core::model::standard_bass_arrangement::{
        StandardBassArrangementOptions, arrange_standard_bass, arrange_standard_bass_located,
    };
    use acorde_core::{Duration, Note, Pitch, Score, Step, to_playback_events};
    let mut score = Score::new("melody", 120, 4, 4, 0, 1);
    score.parts[0].staves[0].measures[0].voices[0] =
        vec![Note::new(Pitch::new(Step::C, 5), Duration::Whole)];
    let original = serde_json::to_value(&score).unwrap();
    let profile = fixture();
    let options = StandardBassArrangementOptions {
        harmony_source: Default::default(),
        melody_part: 0,
        melody_staff: 0,
        melody_voice: 0,
        allow_source_omissions: false,
        chords: vec![PositionedAccordionChord {
            measure_index: 0,
            offset_beats: 0.0,
            chord: chord(None),
        }],
        profile: profile.profile.clone(),
        allow_chord_omissions: true,
        allow_chord_additions: false,
    };
    let default = arrange_standard_bass(&score, &options);
    assert_eq!(default.actions[0].candidate.chord_button_id, "B diminished");
    let located = arrange_standard_bass_located(&score, &options, &profile);
    assert!(located.diagnostics.is_empty(), "{:?}", located.diagnostics);
    assert_eq!(located.actions[0].candidate.chord_button_id, "G7");
    assert_eq!(located.actions[0].candidate.omitted_pitch_classes, [2]);
    let output = located.score.unwrap();
    let actual: Vec<_> = output.parts[0].staves[1].measures[0].voices[0][0]
        .pitches
        .iter()
        .map(|p| p.to_midi())
        .collect();
    assert_eq!(actual, [43, 67, 71, 77]);
    let melody = |s: &Score| {
        to_playback_events(s, &Default::default())
            .into_iter()
            .filter(|e| e.source.as_ref().is_some_and(|a| a.staff == 0))
            .map(|e| (e.pitch_midi, e.time_beats, e.duration_beats))
            .collect::<Vec<_>>()
    };
    assert_eq!(melody(&score), melody(&output));
    assert_eq!(serde_json::to_value(&score).unwrap(), original);
    let mut bad = profile.clone();
    bad.locations.clear();
    let rejected = arrange_standard_bass_located(&score, &options, &bad);
    assert!(rejected.score.is_none() && rejected.actions.is_empty());
    let mut mismatch = profile;
    mismatch.profile.bass_buttons.clear();
    assert_eq!(
        arrange_standard_bass_located(&score, &options, &mismatch).diagnostics[0].code,
        "profile-mismatch"
    );
}

#[test]
fn located_screening_uses_the_selected_buttons_and_expands_repeats() {
    use acorde_core::model::arrange::PositionedAccordionChord;
    use acorde_core::model::standard_bass_arrangement::StandardBassArrangementOptions;
    use acorde_core::model::standard_bass_transition::{
        StandardBassTransitionPolicy, arrange_standard_bass_located_screened,
    };
    use acorde_core::{Barline, Duration, Note, Pitch, Score, Step};
    let mut score = Score::new("repeat", 120, 4, 4, 0, 1);
    score.parts[0].staves[0].measures[0].voices[0] =
        vec![Note::new(Pitch::new(Step::C, 5), Duration::Whole)];
    score.parts[0].staves[0].measures[0].barline_right = Barline::RepeatEnd;
    let profile = fixture();
    let options = StandardBassArrangementOptions {
        harmony_source: Default::default(),
        melody_part: 0,
        melody_staff: 0,
        melody_voice: 0,
        allow_source_omissions: false,
        chords: vec![PositionedAccordionChord {
            measure_index: 0,
            offset_beats: 0.0,
            chord: chord(None),
        }],
        profile: profile.profile.clone(),
        allow_chord_omissions: true,
        allow_chord_additions: false,
    };
    let policy = StandardBassTransitionPolicy {
        max_position_delta: 0,
        min_attack_interval_ms: 0,
    };
    let result = arrange_standard_bass_located_screened(&score, &options, &profile, &policy);
    assert!(
        result.arrangement.arrangement.diagnostics.is_empty(),
        "{:?}",
        result.arrangement.arrangement.diagnostics
    );
    assert_eq!(result.transitions.len(), 2);
    assert!(result.transitions.iter().all(|t| t.chord_button_id == "G7"));
    let denied = arrange_standard_bass_located_screened(
        &score,
        &options,
        &profile,
        &StandardBassTransitionPolicy {
            max_position_delta: 0,
            min_attack_interval_ms: 3000,
        },
    );
    assert!(denied.arrangement.arrangement.score.is_none() && denied.transitions.is_empty());
    assert_eq!(
        denied.arrangement.arrangement.diagnostics[0].code,
        "transition-too-fast"
    );
}

#[test]
fn located_tied_continuations_keep_the_same_sound_and_button() {
    use acorde_core::model::arrange::PositionedAccordionChord;
    use acorde_core::model::standard_bass_arrangement::{
        StandardBassArrangementOptions, arrange_standard_bass_located,
    };
    use acorde_core::{Duration, Note, Pitch, Score, Step};
    let mut score = Score::new("three beats", 120, 3, 4, 0, 1);
    let mut melody = Note::new(Pitch::new(Step::C, 5), Duration::Half);
    melody.dot_count = 1;
    score.parts[0].staves[0].measures[0].voices[0] = vec![melody];
    let profile = fixture();
    let options = StandardBassArrangementOptions {
        harmony_source: Default::default(),
        melody_part: 0,
        melody_staff: 0,
        melody_voice: 0,
        allow_source_omissions: false,
        chords: vec![PositionedAccordionChord {
            measure_index: 0,
            offset_beats: 0.0,
            chord: chord(None),
        }],
        profile: profile.profile.clone(),
        allow_chord_omissions: true,
        allow_chord_additions: false,
    };
    let result = arrange_standard_bass_located(&score, &options, &profile);
    assert!(result.diagnostics.is_empty(), "{:?}", result.diagnostics);
    let output = result.score.unwrap();
    let notes = &output.parts[0].staves[1].measures[0].voices[0];
    assert_eq!(notes.len(), 2);
    assert!(notes[0].tie_start && notes[1].tie_end);
    assert_eq!(notes[0].pitches, notes[1].pitches);
    assert!(!result.actions[0].tied_from_previous && result.actions[1].tied_from_previous);
    assert_eq!(result.actions[0].candidate, result.actions[1].candidate);
}

#[test]
fn explicit_melody_selection_is_combined_with_located_repeat_screening() {
    use acorde_core::model::arrange::PositionedAccordionChord;
    use acorde_core::model::standard_bass_arrangement::StandardBassArrangementOptions;
    use acorde_core::model::standard_bass_pitch_selection::MelodyPitchSelection;
    use acorde_core::model::standard_bass_transition::{
        StandardBassTransitionPolicy, arrange_standard_bass_located_selected_screened,
    };
    use acorde_core::{Barline, Duration, Note, NoteAddr, Pitch, Score, Step, to_playback_events};
    let mut score = Score::new("polyphonic melody", 120, 4, 4, 0, 1);
    let mut chord_note = Note::new(Pitch::new(Step::C, 4), Duration::Whole);
    chord_note.pitches.push(Pitch::new(Step::C, 5));
    score.parts[0].staves[0].measures[0].voices[0] = vec![chord_note];
    score.parts[0].staves[0].measures[0].barline_right = Barline::RepeatEnd;
    let before = serde_json::to_value(&score).unwrap();
    let profile = fixture();
    let mut options = StandardBassArrangementOptions {
        harmony_source: Default::default(),
        melody_part: 0,
        melody_staff: 0,
        melody_voice: 0,
        allow_source_omissions: true,
        chords: vec![PositionedAccordionChord {
            measure_index: 0,
            offset_beats: 0.0,
            chord: chord(None),
        }],
        profile: profile.profile.clone(),
        allow_chord_omissions: true,
        allow_chord_additions: false,
    };
    let selected = [MelodyPitchSelection {
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
        max_position_delta: 0,
        min_attack_interval_ms: 0,
    };
    let result = arrange_standard_bass_located_selected_screened(
        &score, &options, &selected, &profile, &policy,
    );
    let arrangement = &result.arrangement.arrangement;
    assert!(
        arrangement.diagnostics.is_empty(),
        "{:?}",
        arrangement.diagnostics
    );
    assert_eq!(result.transitions.len(), 2);
    assert!(result.transitions.iter().all(|t| t.chord_button_id == "G7"));
    assert_eq!(arrangement.source_pitches.len(), 2);
    assert!(
        arrangement
            .source_pitches
            .iter()
            .any(|p| p.source_pitch == 0 && p.target.is_none())
    );
    assert!(
        arrangement
            .source_pitches
            .iter()
            .any(|p| p.source_pitch == 1 && p.target.is_some())
    );
    let output = arrangement.score.as_ref().unwrap();
    let events = |s: &Score| {
        to_playback_events(s, &Default::default())
            .into_iter()
            .filter(|e| e.source.as_ref().is_some_and(|a| a.staff == 0) && e.pitch_midi == 72)
            .map(|e| (e.pitch_midi, e.time_beats, e.duration_beats))
            .collect::<Vec<_>>()
    };
    assert_eq!(events(&score), events(output));
    assert_eq!(serde_json::to_value(&score).unwrap(), before);
    let missing =
        arrange_standard_bass_located_selected_screened(&score, &options, &[], &profile, &policy);
    assert_eq!(
        missing.arrangement.arrangement.diagnostics[0].code,
        "ambiguous-melody"
    );
    assert!(missing.arrangement.arrangement.score.is_none() && missing.transitions.is_empty());
    options.allow_source_omissions = false;
    let denied = arrange_standard_bass_located_selected_screened(
        &score, &options, &selected, &profile, &policy,
    );
    assert_eq!(
        denied.arrangement.arrangement.diagnostics[0].code,
        "source-omission-not-authorized"
    );
    assert!(denied.arrangement.arrangement.score.is_none() && denied.transitions.is_empty());
}
