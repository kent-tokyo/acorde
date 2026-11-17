use std::collections::BTreeSet;

use acorde_core::{
    CountermelodyDevelopmentError, CountermelodyDevelopmentKind, Duration, MotifRhythmCell, Note,
    Pitch, arrange_countermelody_development, replay_countermelody_development, validate,
};

#[path = "../../../tests/fixtures/arrangement/countermelody_development.rs"]
mod fixture;

const KINDS: [CountermelodyDevelopmentKind; 5] = [
    CountermelodyDevelopmentKind::Imitation,
    CountermelodyDevelopmentKind::Inversion,
    CountermelodyDevelopmentKind::Augmentation,
    CountermelodyDevelopmentKind::Diminution,
    CountermelodyDevelopmentKind::Answer,
];

#[test]
fn source_rhythm_produces_five_replayable_developments() {
    let source = fixture::source();
    let original = serde_json::to_value(&source.parts).unwrap();
    let mut signatures = BTreeSet::new();
    for kind in KINDS {
        let result = arrange_countermelody_development(&source, &fixture::request(kind)).unwrap();
        assert!(validate(&result.score).is_valid());
        assert_eq!(result.motif_intervals, [2, 2]);
        assert_eq!(
            result.source_rhythm,
            [
                MotifRhythmCell {
                    ticks: 32,
                    sounding: true
                },
                MotifRhythmCell {
                    ticks: 16,
                    sounding: false
                },
                MotifRhythmCell {
                    ticks: 16,
                    sounding: true
                },
                MotifRhythmCell {
                    ticks: 64,
                    sounding: true
                },
            ]
        );
        assert_eq!(
            serde_json::to_value(&result.score.parts[..1]).unwrap(),
            original
        );
        assert_eq!(
            fixture::events(&result.score, 0)
                .into_iter()
                .filter(|event| event.0 == 0)
                .collect::<Vec<_>>(),
            fixture::events(&source, 0)
        );
        let generated = fixture::events(&result.score, 1);
        assert!(
            signatures.insert(generated.clone()),
            "{kind:?} duplicated a prior kind"
        );
        assert_eq!(result.attack_count, generated.len());
        let replay = replay_countermelody_development(&source, &result.resolved).unwrap();
        assert_eq!(
            fixture::events(&replay, 0),
            fixture::events(&result.score, 0)
        );
    }
    assert_eq!(signatures.len(), 5);
    assert_eq!(serde_json::to_value(&source.parts).unwrap(), original);
}

#[test]
fn transformed_durations_and_entry_times_are_traceable() {
    let source = fixture::source();
    let imitate = arrange_countermelody_development(&source, &fixture::request(KINDS[0])).unwrap();
    let augment = arrange_countermelody_development(&source, &fixture::request(KINDS[2])).unwrap();
    let diminish = arrange_countermelody_development(&source, &fixture::request(KINDS[3])).unwrap();
    let answer = arrange_countermelody_development(&source, &fixture::request(KINDS[4])).unwrap();
    assert_eq!(imitate.transformed_rhythm, imitate.source_rhythm);
    assert_eq!(
        augment
            .transformed_rhythm
            .iter()
            .map(|cell| cell.ticks)
            .collect::<Vec<_>>(),
        [64, 32, 32, 128]
    );
    assert_eq!(
        diminish
            .transformed_rhythm
            .iter()
            .map(|cell| cell.ticks)
            .collect::<Vec<_>>(),
        [16, 8, 8, 32]
    );
    let starts = |score| {
        fixture::events(score, 1)
            .into_iter()
            .filter(|event| event.1 < 4 * 480)
            .map(|event| event.1)
            .collect::<Vec<_>>()
    };
    assert_eq!(starts(&imitate.score), [480, 1200, 1440]);
    assert_eq!(starts(&augment.score), [0, 1440]);
    assert_eq!(starts(&diminish.score), [0, 360, 480, 960, 1320, 1440]);
    assert_eq!(starts(&answer.score), [960, 1680]);
}

#[test]
fn changed_source_and_contract_are_rejected() {
    let source = fixture::source();
    let result = arrange_countermelody_development(&source, &fixture::request(KINDS[1])).unwrap();
    let mut changed = source.clone();
    changed.parts[0].staves[0].measures[0].voices[0][0].pitches[0] =
        acorde_core::Pitch::from_midi(85, false);
    assert!(matches!(
        replay_countermelody_development(&changed, &result.resolved),
        Err(CountermelodyDevelopmentError::SourceMismatch)
    ));
    let mut contract = result.resolved;
    contract.contract_version += 1;
    assert!(matches!(
        replay_countermelody_development(&source, &contract),
        Err(CountermelodyDevelopmentError::UnsupportedContract)
    ));
}

#[test]
fn delayed_answer_rejects_a_low_overlapping_melody_atomically() {
    let mut source = fixture::source();
    source.parts[0].staves[0].measures[1].voices[0] = vec![
        Note::new(Pitch::from_midi(84, false), Duration::Half),
        Note::new(Pitch::from_midi(70, false), Duration::Half),
    ];
    let original = serde_json::to_value(&source).unwrap();
    let result = arrange_countermelody_development(
        &source,
        &fixture::request(CountermelodyDevelopmentKind::Answer),
    );
    assert!(matches!(
        result,
        Err(CountermelodyDevelopmentError::Unplayable(1))
    ));
    assert_eq!(serde_json::to_value(&source).unwrap(), original);
}

#[test]
fn diminution_rejects_an_unrepresentable_half_tick() {
    let mut source = fixture::source();
    source.parts[0].staves[0].measures[0].voices[0] = vec![
        Note::new(Pitch::from_midi(84, false), Duration::HundredTwentyEighth),
        Note::rest(Duration::Eighth),
        Note::rest(Duration::Sixteenth),
        Note::rest(Duration::ThirtySecond),
        Note::rest(Duration::SixtyFourth),
        Note::rest(Duration::HundredTwentyEighth),
        Note::rest(Duration::Eighth),
        Note::new(Pitch::from_midi(86, false), Duration::Eighth),
        Note::new(Pitch::from_midi(88, false), Duration::Half),
    ];
    assert!(validate(&source).is_valid());
    assert!(matches!(
        arrange_countermelody_development(
            &source,
            &fixture::request(CountermelodyDevelopmentKind::Diminution),
        ),
        Err(CountermelodyDevelopmentError::UnsupportedMotifRhythm)
    ));
}
