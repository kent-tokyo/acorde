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
