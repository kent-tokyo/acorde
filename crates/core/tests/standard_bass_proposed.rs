use acorde_core::model::standard_bass::{
    StandardBassChordButton, StandardBassNoteButton, StandardBassProfile,
};
use acorde_core::model::standard_bass_arrangement::{
    StandardBassArrangementOptions, StandardBassHarmonySource,
};
use acorde_core::model::standard_bass_proposed::arrange_standard_bass_proposed_harmony;
use acorde_core::{Duration, Note, Pitch, Score, Step};

fn source() -> Score {
    let mut score = Score::new("proposed", 120, 4, 4, 0, 2);
    let steps = [
        Step::C,
        Step::E,
        Step::G,
        Step::E,
        Step::F,
        Step::A,
        Step::G,
        Step::C,
    ];
    for (i, measure) in score.parts[0].staves[0].measures.iter_mut().enumerate() {
        measure.voices[0] = vec![
            Note::new(Pitch::new(steps[i * 4 % 8].clone(), 4), Duration::Half),
            Note::new(
                Pitch::new(steps[(i * 4 + 1) % 8].clone(), 4),
                Duration::Half,
            ),
        ];
    }
    score
}

fn options() -> StandardBassArrangementOptions {
    let bass = |id: &str, midi| StandardBassNoteButton {
        id: id.into(),
        position: 0,
        midi,
    };
    let chord = |id: &str, midi: &[u8]| StandardBassChordButton {
        id: id.into(),
        position: 0,
        midi: midi.to_vec(),
    };
    StandardBassArrangementOptions {
        harmony_source: Default::default(),
        melody_part: 0,
        melody_staff: 0,
        melody_voice: 0,
        allow_source_omissions: false,
        chords: vec![],
        profile: StandardBassProfile {
            bass_buttons: vec![
                bass("C bass", 36),
                bass("F bass", 41),
                bass("G bass", 43),
                bass("A bass", 45),
            ],
            chord_buttons: vec![
                chord("C chord", &[60, 64, 67]),
                chord("F chord", &[65, 69, 72]),
                chord("G chord", &[67, 71, 74]),
                chord("Am chord", &[69, 72, 76]),
            ],
            max_distance: 1,
        },
        allow_chord_omissions: true,
        allow_chord_additions: true,
    }
}

#[test]
fn proposed_harmony_is_marked_editorial_and_the_source_is_untouched() {
    let score = source();
    let before = serde_json::to_value(&score).unwrap();
    let result = arrange_standard_bass_proposed_harmony(&score, &options());
    assert_eq!(serde_json::to_value(&score).unwrap(), before);
    assert_eq!(
        result.harmony_source,
        StandardBassHarmonySource::EditorialProposal
    );
    assert!(result.score.is_some(), "{:?}", result.diagnostics);
    assert!(!result.actions.is_empty());
    assert!(result.actions.iter().all(|a| !a.candidate.label.is_empty()));
}

#[test]
fn a_missing_part_is_a_diagnostic() {
    let mut o = options();
    o.melody_part = 3;
    let result = arrange_standard_bass_proposed_harmony(&source(), &o);
    assert!(result.score.is_none());
    assert_eq!(result.diagnostics[0].code, "melody-selection");
}
