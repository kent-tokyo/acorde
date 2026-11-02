use acorde_core::model::arrange::{AccordionBassMode, PositionedAccordionChord};
use acorde_core::model::standard_bass::{
    StandardBassChordButton, StandardBassNoteButton, StandardBassProfile,
};
use acorde_core::model::standard_bass_arrangement::{
    StandardBassArrangementOptions, arrange_standard_bass,
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
