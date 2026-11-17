use acorde_core::{
    CountermelodyDevelopmentKind, MelodyAnticipationEdit, NoteAddr, PhraseMelodyError,
    PhraseMelodyRequest, arrange_phrase_melody, replay_phrase_melody, validate,
};

#[path = "../../../tests/fixtures/arrangement/countermelody_development.rs"]
mod fixture;

fn addr(note: usize) -> NoteAddr {
    NoteAddr {
        part: 0,
        staff: 0,
        measure: 0,
        voice: 0,
        note,
    }
}

fn request() -> PhraseMelodyRequest {
    PhraseMelodyRequest {
        countermelody: fixture::request(CountermelodyDevelopmentKind::Imitation),
        countermelody_active_measures: vec![true, false, true, false],
        phrase_entry_max_leap_semitones: Some(12),
        start_measure: 0,
        end_measure_exclusive: 1,
        locked_measures: vec![1, 3],
        anticipations: vec![MelodyAnticipationEdit {
            note: addr(2),
            preceding_rest: addr(1),
            early_ticks: 8,
        }],
    }
}

#[test]
fn permitted_anticipation_preserves_pitch_cadence_and_other_measures() {
    let source = fixture::source();
    let source_parts = serde_json::to_value(&source.parts).unwrap();
    let result = arrange_phrase_melody(&source, &request()).unwrap();
    assert!(validate(&result.score).is_valid());
    assert_eq!(
        result.resolved.source_note_ids,
        [source.parts[0].staves[0].measures[0].voices[0][2]
            .id
            .clone()]
    );
    let old_events = fixture::events(&source, 0);
    let events = fixture::events(&result.score, 0);
    let melody = events
        .iter()
        .filter(|event| event.0 == 0)
        .collect::<Vec<_>>();
    assert_eq!(melody[0], &old_events[0]);
    assert_eq!(melody[1].1, old_events[1].1 - 120);
    assert_eq!(melody[1].2, old_events[1].2 + 120);
    assert_eq!(melody[1].3, old_events[1].3);
    assert_eq!(melody[2], &old_events[2]);
    assert_eq!(
        melody[3..].iter().map(|event| **event).collect::<Vec<_>>(),
        old_events[3..]
    );
    assert_eq!(
        serde_json::to_value(&result.score.parts[0].staves[0].measures[1..]).unwrap(),
        serde_json::to_value(&source.parts[0].staves[0].measures[1..]).unwrap()
    );
    assert_eq!(serde_json::to_value(&source.parts).unwrap(), source_parts);
    assert_eq!(
        result.countermelody.generated_note_origins.len(),
        result.countermelody.generated_notes.len()
    );
    for (origin, generated) in result
        .countermelody
        .generated_note_origins
        .iter()
        .zip(&result.countermelody.generated_notes)
    {
        assert_eq!(&origin.generated_note, generated);
        assert_eq!(origin.source_note.part, 0);
        assert_eq!(origin.source_note.measure, 0);
        assert!(!source.parts[0].staves[0].measures[0].voices[0][origin.source_note.note].is_rest);
    }
    let counter_part = result.score.parts.len() - 1;
    assert!(
        fixture::events(&result.score, counter_part)
            .iter()
            .all(|event| event.1 < 1920 || (3840..5760).contains(&event.1))
    );
    let replay = replay_phrase_melody(&source, &result.resolved).unwrap();
    assert_eq!(fixture::events(&replay.score, 0), events);
}

#[test]
fn unauthorized_edits_and_stale_replay_fail_atomically() {
    let source = fixture::source();
    let original = serde_json::to_value(&source).unwrap();
    let good = arrange_phrase_melody(&source, &request()).unwrap();

    let mut locked = request();
    locked.locked_measures.push(0);
    assert!(matches!(
        arrange_phrase_melody(&source, &locked),
        Err(PhraseMelodyError::UnsupportedEdit(0))
    ));

    let mut out_of_bounds = request();
    out_of_bounds.anticipations[0].note.note = usize::MAX;
    out_of_bounds.anticipations[0].preceding_rest.note = usize::MAX - 1;
    assert!(matches!(
        arrange_phrase_melody(&source, &out_of_bounds),
        Err(PhraseMelodyError::UnsupportedEdit(0))
    ));

    let mut tied = source.clone();
    tied.parts[0].staves[0].measures[0].voices[0][2].tie_start = true;
    assert!(arrange_phrase_melody(&tied, &request()).is_err());

    let mut changed = source.clone();
    changed.parts[0].staves[0].measures[0].voices[0][0].pitches[0] =
        acorde_core::Pitch::from_midi(85, false);
    assert!(matches!(
        replay_phrase_melody(&changed, &good.resolved),
        Err(PhraseMelodyError::SourceMismatch)
    ));

    let mut changed_selection = good.resolved;
    changed_selection.source_note_ids[0].push('x');
    assert!(matches!(
        replay_phrase_melody(&source, &changed_selection),
        Err(PhraseMelodyError::SelectionMismatch)
    ));
    assert_eq!(serde_json::to_value(&source).unwrap(), original);
}
