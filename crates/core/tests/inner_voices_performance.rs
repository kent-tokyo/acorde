#[path = "../../../tests/fixtures/arrangement/inner_voice_repeat.rs"]
mod fixture;

use acorde_core::{
    InnerVoicesPerformanceError, arrange_inner_voices_performance, replay_inner_voices_performance,
};

fn first_pitch(score: &acorde_core::Score, part: usize, bar: usize, voice: usize) -> u8 {
    score.parts[part].staves[0].measures[bar].voices[voice]
        .iter()
        .find(|note| !note.is_rest && !note.tie_end)
        .unwrap()
        .pitches[0]
        .to_midi() as u8
}

#[test]
fn repeated_entrance_changes_pitch_only_in_performance_score() {
    let source = fixture::source();
    let result = arrange_inner_voices_performance(&source, fixture::request()).unwrap();
    assert_eq!(result.written_measures_by_visit, [0, 1, 0, 1]);
    assert_eq!(result.written_score.parts[0].staves[0].measures.len(), 2);
    assert_eq!(
        result.performance_score.parts[0].staves[0].measures.len(),
        4
    );
    assert_eq!(
        first_pitch(&result.written_score, result.added_part, 0, 0),
        64
    );
    assert_eq!(
        first_pitch(&result.performance_score, result.added_part, 0, 0),
        64
    );
    assert_eq!(
        first_pitch(&result.performance_score, result.added_part, 2, 0),
        67
    );
    assert_eq!(result.repeat_back_joins.len(), 2);
    assert!(result.repeat_back_joins[0].leap_semitones <= 2);
    assert!(result.repeat_back_joins[1].leap_semitones <= 7);
    assert_eq!(
        fixture::events(&source),
        fixture::events(&result.performance_score)
            .into_iter()
            .filter(|event| event.0 == 0)
            .collect::<Vec<_>>()
    );
    let replay = replay_inner_voices_performance(&source, &result.resolved).unwrap();
    assert_eq!(
        fixture::events(&replay.performance_score),
        fixture::events(&result.performance_score)
    );
    let mut changed = source.clone();
    changed.metadata.title.push('!');
    assert!(matches!(
        replay_inner_voices_performance(&changed, &result.resolved),
        Err(InnerVoicesPerformanceError::SourceMismatch)
    ));
}
