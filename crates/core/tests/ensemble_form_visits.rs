#[path = "../../../tests/fixtures/arrangement/form_development_v2.rs"]
mod fixture;

use acorde_core::{
    Barline, ChordalPadPattern, Dynamic, EnsembleFormVisitsRequest, FormVisitVariation,
    arrange_ensemble_form_visits, replay_ensemble_form_visits,
};

fn repeated() -> acorde_core::Score {
    let mut score = fixture::source();
    score.parts[0].staves[0].measures[0].barline_left = Barline::RepeatStart;
    score.parts[0].staves[0].measures[7].barline_right = Barline::RepeatEnd;
    score
}

fn request() -> EnsembleFormVisitsRequest {
    let base = fixture::request();
    let mut alternative = base.clone();
    let harmony = alternative.sections[0].harmony.as_mut().unwrap();
    harmony.pattern = ChordalPadPattern::ArpeggioUpDown;
    harmony.dynamic = Dynamic::Mf;
    EnsembleFormVisitsRequest {
        base,
        variations: vec![FormVisitVariation {
            visit_index: 8,
            alternative,
        }],
    }
}

fn event_key(event: &acorde_core::PlaybackEvent) -> (usize, u64, u64, u8, u8) {
    (
        event.part_index,
        (event.time_beats * 480.0).round() as u64,
        (event.duration_beats * 480.0).round() as u64,
        event.pitch_midi,
        event.velocity,
    )
}

#[test]
fn second_visit_changes_only_generated_events_in_the_selected_bar() {
    let source = repeated();
    let result = arrange_ensemble_form_visits(&source, &request());
    assert!(result.diagnostics.is_empty(), "{:?}", result.diagnostics);
    assert_eq!(result.written_measures_by_visit.len(), 24);
    assert_eq!(
        (
            result.written_measures_by_visit[0],
            result.written_measures_by_visit[8]
        ),
        (0, 0)
    );
    let base = result.written_score.as_ref().unwrap();
    assert_eq!(
        serde_json::to_value(&base.parts[..1]).unwrap(),
        serde_json::to_value(&source.parts).unwrap()
    );
    let baseline = acorde_core::to_playback_events(base, &Default::default());
    let source_part = |events: &[acorde_core::PlaybackEvent]| {
        events
            .iter()
            .filter(|event| event.part_index == 0)
            .map(event_key)
            .collect::<Vec<_>>()
    };
    assert_eq!(source_part(&baseline), source_part(&result.events));
    let at = |events: &[acorde_core::PlaybackEvent], start: u64, end: u64| {
        events
            .iter()
            .map(event_key)
            .filter(|(part, time, _, _, _)| *part > 0 && *time >= start && *time < end)
            .collect::<Vec<_>>()
    };
    assert_eq!(at(&baseline, 0, 4 * 480), at(&result.events, 0, 4 * 480));
    assert_eq!(
        at(&baseline, 36 * 480, 40 * 480),
        at(&result.events, 36 * 480, 40 * 480)
    );
    assert_eq!(at(&baseline, 32 * 480, 36 * 480).len(), 3);
    assert_eq!(at(&result.events, 32 * 480, 36 * 480).len(), 8);
    let replay = replay_ensemble_form_visits(&source, result.resolved.as_ref().unwrap());
    assert!(replay.diagnostics.is_empty());
    assert_eq!(
        replay.events.iter().map(event_key).collect::<Vec<_>>(),
        result.events.iter().map(event_key).collect::<Vec<_>>()
    );
}

#[test]
fn invalid_or_unchanged_variations_fail_without_partial_events() {
    let source = repeated();
    for case in 0..5 {
        let mut request = request();
        match case {
            0 => request.variations[0].visit_index = 0,
            1 => request.variations[0].visit_index = 24,
            2 => request.variations[0].alternative = request.base.clone(),
            3 => request.variations.push(request.variations[0].clone()),
            4 => request.variations[0].alternative.sections[0].start_measure = 1,
            _ => unreachable!(),
        }
        let result = arrange_ensemble_form_visits(&source, &request);
        assert!(result.written_score.is_none(), "case {case}");
        assert!(result.events.is_empty());
        assert!(result.resolved.is_none());
        assert!(!result.diagnostics.is_empty());
    }
}

#[test]
fn replay_rejects_changed_source_and_engine() {
    let source = repeated();
    let result = arrange_ensemble_form_visits(&source, &request());
    let resolved = result.resolved.unwrap();
    let mut changed = source.clone();
    changed.settings.tempo_bpm += 1;
    assert_eq!(
        replay_ensemble_form_visits(&changed, &resolved).diagnostics[0].code,
        "form-source-mismatch"
    );
    let mut changed = resolved;
    changed.engine_version.push_str("-unknown");
    assert_eq!(
        replay_ensemble_form_visits(&source, &changed).diagnostics[0].code,
        "unsupported-form-visits-contract"
    );
}
