use acorde_core::model::arrange::{AccordionBassMode, PositionedAccordionChord};
use acorde_core::model::standard_bass::StandardBassRealization;
use acorde_core::model::standard_bass::{
    StandardBassChordButton, StandardBassNoteButton, StandardBassProfile,
};
use acorde_core::model::standard_bass_arrangement::{
    StandardBassArrangementOptions, StandardBassEditionKind, StandardBassHarmonySource,
    arrange_standard_bass,
};
use acorde_core::{Barline, ChordSymbol, Duration, MeasureLength, Note, Pitch, Score, Step};

fn fixture() -> (Score, StandardBassArrangementOptions) {
    let mut source = Score::new("melody selection", 120, 4, 4, 0, 2);
    let staff = &mut source.parts[0].staves[0];
    staff.measures[0].number = 0;
    staff.measures[0].actual_length = MeasureLength::from_beats(1.0);
    staff.measures[0].voices[0] = vec![Note::new(Pitch::new(Step::C, 4), Duration::Quarter)];
    staff.measures[0].voices[1] = vec![Note::new(Pitch::new(Step::E, 5), Duration::Quarter)];
    staff.measures[1].voices[0] = vec![Note::new(Pitch::new(Step::G, 4), Duration::Whole)];
    staff.measures[1].voices[1] = vec![Note::new(Pitch::new(Step::F, 5), Duration::Whole)];
    staff.measures[1].barline_right = Barline::RepeatEnd;
    let chord: ChordSymbol =
        serde_json::from_str(r#"{"root":"C","kind":"major-seventh"}"#).unwrap();
    let options = StandardBassArrangementOptions {
        harmony_source: Default::default(),
        melody_part: 0,
        melody_staff: 0,
        melody_voice: 1,
        allow_source_omissions: true,
        chords: vec![PositionedAccordionChord {
            measure_index: 0,
            offset_beats: 0.0,
            chord,
        }],
        profile: StandardBassProfile {
            bass_buttons: vec![StandardBassNoteButton {
                id: "C bass".into(),
                position: 0,
                midi: 36,
            }],
            chord_buttons: vec![StandardBassChordButton {
                id: "E minor".into(),
                position: 1,
                midi: vec![64, 67, 71],
            }],
            max_distance: 1,
        },
        allow_chord_omissions: false,
        allow_chord_additions: false,
    };
    (source, options)
}

#[test]
fn melody_and_pickup_repeat_survive_while_all_other_pitches_are_reported() {
    let (source, options) = fixture();
    let before = serde_json::to_value(&source).unwrap();
    let result = arrange_standard_bass(&source, &options);
    assert!(result.diagnostics.is_empty(), "{:?}", result.diagnostics);
    assert_eq!(result.required_bass_mode, AccordionBassMode::StandardBass);
    assert!(result.requires_performer_review);
    assert_eq!(result.source_pitches.len(), 4);
    assert_eq!(
        result
            .source_pitches
            .iter()
            .filter(|p| p.target.is_none())
            .count(),
        2
    );
    assert_eq!(
        result
            .source_pitches
            .iter()
            .filter(|p| p.target.is_some())
            .count(),
        2
    );
    let arranged = result.score.unwrap();
    assert_eq!(serde_json::to_value(&source).unwrap(), before);
    for (old, new) in source.parts[0].staves[0]
        .measures
        .iter()
        .zip(&arranged.parts[0].staves[0].measures)
    {
        assert_eq!(old.voices[1][0].id, new.voices[0][0].id);
        assert_eq!(old.voices[1][0].pitches, new.voices[0][0].pitches);
        assert_eq!(old.voices[1][0].beats(), new.voices[0][0].beats());
        assert_eq!(old.actual_length, new.actual_length);
        assert_eq!(old.barline_right, new.barline_right);
    }
    assert_eq!(result.actions.len(), 2);
    for action in result.actions {
        assert_eq!(action.candidate.bass_button_id, "C bass");
        assert_eq!(action.candidate.chord_button_id, "E minor");
        let actual = &arranged.parts[action.target.part].staves[action.target.staff].measures
            [action.target.measure]
            .voices[action.target.voice][action.target.note];
        assert_eq!(
            actual
                .pitches
                .iter()
                .map(|p| p.to_midi())
                .collect::<Vec<_>>(),
            [36, 64, 67, 71]
        );
    }
}

#[test]
fn rejects_without_partial_output_when_omissions_are_not_authorized_or_buttons_missing() {
    let (source, mut options) = fixture();
    options.allow_source_omissions = false;
    let denied = arrange_standard_bass(&source, &options);
    assert_eq!(denied.diagnostics[0].code, "source-omission-not-authorized");
    assert!(
        denied.score.is_none() && denied.source_pitches.is_empty() && denied.actions.is_empty()
    );
    options.allow_source_omissions = true;
    options.profile.chord_buttons.clear();
    let unavailable = arrange_standard_bass(&source, &options);
    assert_eq!(
        unavailable.diagnostics[0].code,
        "no-standard-bass-candidate"
    );
    assert!(
        unavailable.score.is_none()
            && unavailable.source_pitches.is_empty()
            && unavailable.actions.is_empty()
    );
}

#[test]
fn rejects_invalid_selection_and_polyphonic_melody() {
    let (mut source, mut options) = fixture();
    options.melody_voice = 4;
    assert_eq!(
        arrange_standard_bass(&source, &options).diagnostics[0].code,
        "melody-selection"
    );
    options.melody_voice = 1;
    source.parts[0].staves[0].measures[0].voices[1][0]
        .pitches
        .push(Pitch::new(Step::C, 5));
    assert_eq!(
        arrange_standard_bass(&source, &options).diagnostics[0].code,
        "unsupported-melody"
    );
}

#[test]
fn rejects_when_other_staff_contains_a_tempo_needed_to_preserve_melody() {
    let (mut source, options) = fixture();
    let mut second = Score::new("other", 120, 4, 4, 0, 2).parts.remove(0);
    second.staves[0].measures[0].actual_length = MeasureLength::from_beats(1.0);
    second.staves[0].measures[0].tempo = Some(80);
    second.staves[0].measures[1].barline_right = Barline::RepeatEnd;
    source.parts.push(second);
    let result = arrange_standard_bass(&source, &options);
    assert!(result.score.is_none());
    assert_eq!(result.diagnostics[0].code, "melody-playback-changed");
    assert!(result.actions.is_empty() && result.source_pitches.is_empty());
}

#[test]
fn rejects_a_voice_with_no_sounding_melody() {
    let (mut source, options) = fixture();
    for measure in &mut source.parts[0].staves[0].measures {
        measure.voices[1][0].is_grace = true;
    }
    let result = arrange_standard_bass(&source, &options);
    assert!(result.score.is_none());
    assert_eq!(result.diagnostics[0].code, "melody-selection");
}

fn chord_fixture() -> (
    Score,
    StandardBassArrangementOptions,
    Vec<acorde_core::model::standard_bass_pitch_selection::MelodyPitchSelection>,
) {
    use acorde_core::model::standard_bass_pitch_selection::MelodyPitchSelection;
    use acorde_core::{AccidentalDisplay, NoteAddr};
    let (mut source, options) = fixture();
    for (measure, bar) in source.parts[0].staves[0].measures.iter_mut().enumerate() {
        let note = &mut bar.voices[1][0];
        note.pitches = vec![Pitch::new(Step::C, 5), Pitch::new(Step::E, 5)];
        note.set_pitch_ties(&[false, measure == 0], &[false, measure == 1]);
        note.set_accidental_display(1, AccidentalDisplay::Cautionary);
        note.set_parenthesized(1, true);
        note.set_pitch_fingering(1, 3);
    }
    let choices = (0..2)
        .map(|measure| MelodyPitchSelection {
            source: NoteAddr {
                part: 0,
                staff: 0,
                measure,
                voice: 1,
                note: 0,
            },
            pitch_index: 1,
        })
        .collect();
    (source, options, choices)
}

#[test]
fn explicitly_selected_chord_pitch_preserves_partial_tie_and_reports_all_losses() {
    use acorde_core::AccidentalDisplay;
    use acorde_core::model::standard_bass_pitch_selection::arrange_standard_bass_with_pitch_selection;
    let (source, options, choices) = chord_fixture();
    let before = serde_json::to_value(&source).unwrap();
    let output = arrange_standard_bass_with_pitch_selection(&source, &options, &choices);
    assert!(
        output.arrangement.diagnostics.is_empty(),
        "{:?}",
        output.arrangement.diagnostics
    );
    assert_eq!(serde_json::to_value(&source).unwrap(), before);
    assert_eq!(output.arrangement.source_pitches.len(), 6);
    assert_eq!(
        output
            .arrangement
            .source_pitches
            .iter()
            .filter(|p| p.target.is_none())
            .count(),
        4
    );
    assert!(
        output
            .arrangement
            .source_pitches
            .iter()
            .filter(|p| p.target.is_some())
            .all(|p| p.source_pitch == 1)
    );
    let score = output.arrangement.score.unwrap();
    let first = &score.parts[0].staves[0].measures[0].voices[0][0];
    let second = &score.parts[0].staves[0].measures[1].voices[0][0];
    assert_eq!(first.pitches, vec![Pitch::new(Step::E, 5)]);
    assert!(first.pitch_tie_start(0) && second.pitch_tie_end(0));
    assert_eq!(first.accidental_display(0), AccidentalDisplay::Cautionary);
    assert!(first.is_parenthesized(0));
    assert_eq!(first.pitch_fingering(0), Some(3));
}

#[test]
fn ambiguous_duplicate_invalid_or_unapproved_selections_fail_closed() {
    use acorde_core::model::standard_bass_pitch_selection::arrange_standard_bass_with_pitch_selection;
    let (source, mut options, choices) = chord_fixture();
    let assert_failed =
        |choices: &[acorde_core::model::standard_bass_pitch_selection::MelodyPitchSelection],
         code: &str| {
            let result = arrange_standard_bass_with_pitch_selection(&source, &options, choices);
            assert_eq!(result.arrangement.diagnostics[0].code, code);
            assert!(
                result.arrangement.score.is_none()
                    && result.arrangement.source_pitches.is_empty()
                    && result.arrangement.actions.is_empty()
            );
        };
    assert_failed(&choices[..1], "ambiguous-melody");
    let mut duplicate = choices.clone();
    duplicate.push(choices[0].clone());
    assert_failed(&duplicate, "melody-selection");
    let mut out_of_range = choices.clone();
    out_of_range[0].pitch_index = 99;
    assert_failed(&out_of_range, "melody-selection");
    let mut wrong_voice = choices.clone();
    wrong_voice[0].source.voice = 0;
    assert_failed(&wrong_voice, "melody-selection");
    options.allow_source_omissions = false;
    let result = arrange_standard_bass_with_pitch_selection(&source, &options, &choices);
    assert_eq!(
        result.arrangement.diagnostics[0].code,
        "source-omission-not-authorized"
    );
}

#[test]
fn duplicate_unisons_and_shifted_arpeggios_are_not_claimed_as_preserved_melody() {
    use acorde_core::model::standard_bass_pitch_selection::arrange_standard_bass_with_pitch_selection;
    let (mut source, options, choices) = chord_fixture();
    source.parts[0].staves[0].measures[0].voices[1][0].pitches[0] = Pitch::new(Step::E, 5);
    let result = arrange_standard_bass_with_pitch_selection(&source, &options, &choices);
    assert_eq!(result.arrangement.diagnostics[0].code, "unsupported-melody");
    let (mut source, options, choices) = chord_fixture();
    source.parts[0].staves[0].measures[0].voices[1][0].arpeggiate = Some(true);
    let result = arrange_standard_bass_with_pitch_selection(&source, &options, &choices);
    assert_eq!(result.arrangement.diagnostics[0].code, "unsupported-melody");
}

#[test]
fn rejects_selected_pitch_if_its_tie_continues_in_an_omitted_pitch() {
    use acorde_core::model::standard_bass_pitch_selection::arrange_standard_bass_with_pitch_selection;
    let (mut source, options, choices) = chord_fixture();
    let second = &mut source.parts[0].staves[0].measures[1].voices[1][0];
    second.pitches = vec![Pitch::new(Step::E, 5), Pitch::new(Step::F, 5)];
    second.set_pitch_ties(&[false, false], &[true, false]);
    let result = arrange_standard_bass_with_pitch_selection(&source, &options, &choices);
    assert_eq!(
        result.arrangement.diagnostics[0].code,
        "melody-playback-changed"
    );
    assert!(result.arrangement.score.is_none());
}

#[test]
fn edition_is_labelled_combined_simplified_and_keeps_the_chord_label_separate() {
    let (source, mut options) = fixture();
    options.harmony_source = StandardBassHarmonySource::EditorialProposal;
    let result = arrange_standard_bass(&source, &options);
    assert_eq!(
        result.edition_kinds,
        [
            StandardBassEditionKind::CombinedButtons,
            StandardBassEditionKind::Simplified
        ]
    );
    assert_eq!(
        result.harmony_source,
        StandardBassHarmonySource::EditorialProposal
    );
    let candidate = &result.actions[0].candidate;
    assert_eq!(candidate.label, "C bass + Em chord");
    assert_eq!(candidate.realization, StandardBassRealization::Combined);
    // Without caller-authorized omissions nothing is returned, and the kind says why.
    options.allow_source_omissions = false;
    let failed = arrange_standard_bass(&source, &options);
    assert!(failed.score.is_none());
    assert_eq!(failed.edition_kinds, [StandardBassEditionKind::NoCandidate]);
}

#[test]
fn button_annotations_preserve_harmony_source_and_playback() {
    use acorde_core::model::standard_bass_annotations::arrange_standard_bass_with_annotations;
    use acorde_core::to_playback_events;
    let (source, mut options) = fixture();
    options.profile.bass_buttons[0].id = "C bass".into();
    options.profile.chord_buttons[0].id = "Em chord".into();
    options.harmony_source = StandardBassHarmonySource::EditorialProposal;
    let before = serde_json::to_value(&source).unwrap();
    let normal = arrange_standard_bass(&source, &options).score.unwrap();
    let result = arrange_standard_bass_with_annotations(&source, &options);
    assert!(result.diagnostics.is_empty(), "{:?}", result.diagnostics);
    assert_eq!(
        result.harmony_source,
        StandardBassHarmonySource::EditorialProposal
    );
    let annotated = result.score.unwrap();
    assert_eq!(serde_json::to_value(&source).unwrap(), before);
    for action in &result.actions {
        let a = &action.target;
        let n =
            &annotated.parts[a.part].staves[a.staff].measures[a.measure].voices[a.voice][a.note];
        if !action.tied_from_previous {
            assert_eq!(n.technique_text.as_deref(), Some("C bass + Em chord"));
        }
    }
    assert_eq!(
        annotated.parts[0].staves[1].measures[0].voices[0][0]
            .chord_symbol
            .as_ref()
            .unwrap()
            .kind,
        "major-seventh"
    );
    let playback = |s: &Score| {
        to_playback_events(s, &Default::default())
            .into_iter()
            .map(|e| (e.pitch_midi, e.time_beats, e.duration_beats))
            .collect::<Vec<_>>()
    };
    assert_eq!(playback(&normal), playback(&annotated));
    options.profile.chord_buttons.clear();
    let missing = arrange_standard_bass_with_annotations(&source, &options);
    assert!(missing.score.is_none() && missing.actions.is_empty());
    assert_eq!(
        missing.edition_kinds,
        [StandardBassEditionKind::NoCandidate]
    );
}
