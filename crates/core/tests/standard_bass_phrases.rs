use acorde_core::model::arrange::PositionedAccordionChord;
use acorde_core::model::standard_bass::{
    StandardBassChordButton, StandardBassNoteButton, StandardBassProfile,
};
use acorde_core::model::standard_bass_arrangement::StandardBassArrangementOptions;
use acorde_core::model::standard_bass_phrases::{MelodyPhrase, arrange_standard_bass_with_phrases};
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

fn phrase(first: usize, last: usize, voice: usize) -> MelodyPhrase {
    MelodyPhrase {
        first_measure: first,
        last_measure: last,
        part: 0,
        staff: 0,
        voice,
    }
}

#[test]
fn melody_follows_the_phrase_voice_and_reports_original_addresses() {
    let (source, options) = fixture();
    let before = serde_json::to_value(&source).unwrap();
    // Base voice 1 (E5 then F5); measure 1 follows voice 0 (G4) instead.
    let result = arrange_standard_bass_with_phrases(&source, &options, &[phrase(1, 1, 0)]);
    assert!(result.diagnostics.is_empty(), "{:?}", result.diagnostics);
    assert_eq!(serde_json::to_value(&source).unwrap(), before);
    let arranged = result.score.unwrap();
    let midi = |m: usize| arranged.parts[0].staves[0].measures[m].voices[0][0].pitches[0].to_midi();
    assert_eq!((midi(0), midi(1)), (76, 67));
    let kept: Vec<_> = result
        .source_pitches
        .iter()
        .filter(|p| p.target.is_some())
        .map(|p| (p.source.measure, p.source.voice))
        .collect();
    assert_eq!(kept, [(0, 1), (1, 0)]);
    let omitted: Vec<_> = result
        .source_pitches
        .iter()
        .filter(|p| p.target.is_none())
        .map(|p| (p.source.measure, p.source.voice))
        .collect();
    assert_eq!(omitted, [(0, 0), (1, 1)]);
}

#[test]
fn invalid_phrase_selections_fail_closed() {
    let (source, options) = fixture();
    for phrases in [
        vec![],
        vec![phrase(0, 5, 0)],
        vec![phrase(0, 1, 0), phrase(1, 1, 1)],
        vec![phrase(1, 0, 0)],
        vec![phrase(0, 0, 9)],
    ] {
        let result = arrange_standard_bass_with_phrases(&source, &options, &phrases);
        assert!(result.score.is_none() && result.actions.is_empty());
        assert_eq!(result.diagnostics[0].code, "invalid-phrase-selection");
    }
    // Measure 0 is a one-beat pickup in both voices, so mixing keeps timing; a different
    // length would be rejected rather than silently changing the melody.
}
