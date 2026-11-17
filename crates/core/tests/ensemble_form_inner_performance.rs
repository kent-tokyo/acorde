#[path = "../../../tests/fixtures/arrangement/form_development_v2.rs"]
mod fixture;

use acorde_core::{
    Barline, FormInnerVoicesPerformanceError, arrange_form_inner_voices_performance,
    replay_form_inner_voices_performance,
};

fn repeated_source() -> acorde_core::Score {
    let mut source = fixture::source();
    source.parts[0].staves[0].measures[0].barline_left = Barline::RepeatStart;
    source.parts[0].staves[0].measures[7].barline_right = Barline::RepeatEnd;
    source
}

#[test]
fn sectioned_form_keeps_inner_voice_limits_on_repeat_visits() {
    let source = repeated_source();
    let request = fixture::form_inner_voice_request();
    let result = arrange_form_inner_voices_performance(&source, &request).unwrap();
    assert_eq!(result.written_measures_by_visit.len(), 24);
    assert_eq!(result.written_measures_by_visit[7..9], [7, 0]);
    assert_eq!(result.written_score.parts[0].staves[0].measures.len(), 16);
    assert_eq!(
        result.performance_score.parts[0].staves[0].measures.len(),
        24
    );
    assert!(
        result
            .written_score
            .parts
            .iter()
            .zip(&result.performance_score.parts)
            .all(|(written, performance)| written.id == performance.id)
    );
    assert_eq!(result.performance_measures.len(), 24);
    assert!(
        result
            .repeat_back_joins
            .iter()
            .any(|join| join.to_visit == 8 && join.voice == 0)
    );
    for join in &result.repeat_back_joins {
        let limit = if join.voice == 0 {
            request.inner.lower.max_leap_semitones
        } else {
            request.inner.upper.max_leap_semitones
        };
        assert!(join.leap_semitones <= limit);
    }
    assert!(
        result
            .performance_measures
            .iter()
            .enumerate()
            .all(
                |(visit, bar)| bar.harmony_attacks + bar.bass_attacks + bar.countermelody_attacks
                    <= request
                        .form
                        .form
                        .ensemble
                        .max_generated_attacks_per_measure
                        .unwrap()
                    && (result.written_measures_by_visit[visit] != 15 || bar.harmony_attacks == 0)
            )
    );
    assert_eq!(
        fixture::events(&source),
        fixture::events(&result.performance_score)
            .into_iter()
            .filter(|event| event.0 == 0)
            .collect::<Vec<_>>()
    );
    let replay = replay_form_inner_voices_performance(&source, &result.resolved).unwrap();
    assert_eq!(
        fixture::events(&result.performance_score),
        fixture::events(&replay.performance_score)
    );
    let mut changed = source.clone();
    changed.metadata.title.push('!');
    assert!(matches!(
        replay_form_inner_voices_performance(&changed, &result.resolved),
        Err(FormInnerVoicesPerformanceError::SourceMismatch)
    ));
}
