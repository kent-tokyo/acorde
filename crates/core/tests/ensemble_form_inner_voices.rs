#[path = "../../../tests/fixtures/arrangement/form_development_v2.rs"]
mod fixture;

use acorde_core::{
    Dynamic, FormInnerVoicesError, InnerVoicePattern, arrange_form_inner_voices,
    replay_form_inner_voices,
};

#[test]
fn inner_lines_replace_pad_in_sectioned_form_and_replay() {
    let source = fixture::source();
    let request = fixture::form_inner_voice_request();
    let result = arrange_form_inner_voices(&source, &request).unwrap();
    assert_eq!(result.inner_part, result.score.parts.len() - 1);
    assert_eq!(result.role_parts.len(), 2);
    assert_eq!(
        serde_json::to_value(&result.score.parts[0]).unwrap(),
        serde_json::to_value(&source.parts[0]).unwrap()
    );
    assert_eq!(result.measures.len(), 16);
    assert!(
        result.measures[..15]
            .iter()
            .all(|measure| measure.harmony_attacks >= 2)
    );
    assert_eq!(result.measures[15].harmony_attacks, 0);
    let inner = &result.score.parts[result.inner_part].staves[0];
    assert!(
        inner.measures[15].voices[..2]
            .iter()
            .all(|voice| voice.iter().all(|note| note.is_rest))
    );
    assert_eq!(
        inner.measures[0].voices[0]
            .iter()
            .find(|note| !note.is_rest)
            .unwrap()
            .dynamic,
        Some(Dynamic::P)
    );
    assert_eq!(
        inner.measures[4].voices[0]
            .iter()
            .find(|note| !note.is_rest)
            .unwrap()
            .dynamic,
        Some(Dynamic::Mp)
    );
    assert_eq!(
        inner.measures[8].voices[0]
            .iter()
            .find(|note| !note.is_rest)
            .unwrap()
            .dynamic,
        Some(Dynamic::Mf)
    );
    let source_events = fixture::events(&source);
    assert_eq!(
        source_events,
        fixture::events(&result.score)
            .into_iter()
            .filter(|event| event.0 == 0)
            .collect::<Vec<_>>()
    );
    let replay = replay_form_inner_voices(&source, &result.resolved).unwrap();
    assert_eq!(
        fixture::events(&result.score),
        fixture::events(&replay.score)
    );
}

#[test]
fn first_dynamic_follows_earliest_voice_and_counter_crossing_is_rejected() {
    let source = fixture::source();
    let mut request = fixture::form_inner_voice_request();
    request.inner.lower.pattern = InnerVoicePattern::SecondHalf;
    request.inner.upper.pattern = InnerVoicePattern::OffbeatPulse;
    request.form.form.ensemble.max_generated_attacks_per_measure = None;
    let result = arrange_form_inner_voices(&source, &request).unwrap();
    let bar = &result.score.parts[result.inner_part].staves[0].measures[0];
    assert!(bar.voices[0].iter().all(|note| note.dynamic.is_none()));
    assert!(
        bar.voices[1]
            .iter()
            .any(|note| note.dynamic == Some(Dynamic::P))
    );

    let mut crossing = fixture::form_inner_voice_request();
    crossing.inner.upper.low_midi = 75;
    crossing.inner.upper.center_midi = 77;
    crossing.inner.upper.high_midi = 79;
    crossing.inner.upper.max_leap_semitones = 24;
    assert!(matches!(
        arrange_form_inner_voices(&source, &crossing),
        Err(FormInnerVoicesError::Collision {
            role: acorde_core::EnsembleRole::Countermelody,
            ..
        })
    ));
}

#[test]
fn form_rejects_conflicting_pad_density_and_changed_source() {
    let source = fixture::source();
    let mut request = fixture::form_inner_voice_request();
    request.form.form.ensemble.harmony = fixture::request().ensemble.harmony;
    assert!(matches!(
        arrange_form_inner_voices(&source, &request),
        Err(FormInnerVoicesError::InvalidRequest)
    ));
    let mut request = fixture::form_inner_voice_request();
    let base = acorde_core::arrange_ensemble_form_development(&source, &request.form).unwrap();
    let peak = base
        .measures
        .iter()
        .map(|bar| bar.bass_attacks + bar.countermelody_attacks)
        .max()
        .unwrap();
    request.form.form.ensemble.max_generated_attacks_per_measure = Some(peak);
    assert!(acorde_core::arrange_ensemble_form_development(&source, &request.form).is_ok());
    assert!(matches!(
        arrange_form_inner_voices(&source, &request),
        Err(FormInnerVoicesError::Density(_))
    ));
    let good = arrange_form_inner_voices(&source, &fixture::form_inner_voice_request()).unwrap();
    let mut altered = source.clone();
    altered.metadata.title.push('!');
    assert!(matches!(
        replay_form_inner_voices(&altered, &good.resolved),
        Err(FormInnerVoicesError::SourceMismatch)
    ));
}
