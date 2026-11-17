#[path = "../../../tests/fixtures/arrangement/form_development_v2.rs"]
mod fixture;

use acorde_core::{
    Barline, ChordalPadPattern, Clef, Dynamic, EnsembleFormVisitsRequest, FormVisitVariation,
    arrange_ensemble_form_visits, arrange_ensemble_form_visits_score, replay_ensemble_form_visits,
    replay_ensemble_form_visits_score,
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
fn second_visit_exports_an_unfolded_score_with_the_selected_sound() {
    let source = repeated();
    let original = serde_json::to_value(&source).unwrap();
    let events = arrange_ensemble_form_visits(&source, &request());
    let result = arrange_ensemble_form_visits_score(&source, &request());
    assert!(result.diagnostics.is_empty(), "{:?}", result.diagnostics);
    let score = result.score.as_ref().unwrap();
    assert_eq!(score.parts[0].staves[0].measures.len(), 24);
    assert_eq!(
        result.written_measures_by_visit,
        events.written_measures_by_visit
    );
    assert!(
        score
            .parts
            .iter()
            .all(|part| part.staves.iter().all(|staff| {
                staff.measures.iter().all(|bar| {
                    bar.barline_left == Barline::Normal
                        && bar.barline_right == Barline::Normal
                        && bar.volta.is_none()
                        && bar.navigation.is_none()
                })
            }))
    );
    assert_eq!(
        acorde_core::to_playback_events(score, &Default::default())
            .iter()
            .map(event_key)
            .collect::<Vec<_>>(),
        events.events.iter().map(event_key).collect::<Vec<_>>()
    );
    let replay = replay_ensemble_form_visits_score(&source, result.resolved.as_ref().unwrap());
    assert!(replay.diagnostics.is_empty(), "{:?}", replay.diagnostics);
    assert_eq!(
        acorde_core::to_playback_events(&replay.score.unwrap(), &Default::default())
            .iter()
            .map(event_key)
            .collect::<Vec<_>>(),
        events.events.iter().map(event_key).collect::<Vec<_>>()
    );
    assert_eq!(serde_json::to_value(&source).unwrap(), original);
}

#[test]
fn thirty_two_bar_form_exports_a_later_repeat_variation() {
    let mut source = fixture::source();
    let original = source.parts[0].staves[0].measures.clone();
    for (offset, mut bar) in original.into_iter().enumerate() {
        bar.number += 16;
        for (voice_index, voice) in bar.voices.iter_mut().enumerate() {
            for (note_index, note) in voice.iter_mut().enumerate() {
                note.id = format!("long-form:{offset}:{voice_index}:{note_index}");
            }
        }
        source.parts[0].staves[0].measures.push(bar);
    }
    source.parts[0].staves[0].measures[16].barline_left = Barline::RepeatStart;
    source.parts[0].staves[0].measures[23].barline_right = Barline::RepeatEnd;
    let mut base = fixture::request();
    for chords in [
        &mut base.ensemble.harmony.as_mut().unwrap().chords,
        &mut base.ensemble.bass.as_mut().unwrap().chords,
        &mut base.ensemble.countermelody.as_mut().unwrap().chords,
    ] {
        let original = chords.clone();
        for mut chord in original {
            chord.measure_index += 16;
            chords.push(chord);
        }
    }
    let original_sections = base.sections.clone();
    for mut section in original_sections {
        section.start_measure += 16;
        section.end_measure_exclusive += 16;
        base.sections.push(section);
    }
    base.phrase_end_measures.extend([19, 23, 27, 31]);
    let mut alternative = base.clone();
    alternative.sections[5].harmony.as_mut().unwrap().pattern = ChordalPadPattern::ArpeggioUpDown;
    let request = EnsembleFormVisitsRequest {
        base,
        variations: vec![FormVisitVariation {
            visit_index: 24,
            alternative,
        }],
    };
    let selected = arrange_ensemble_form_visits(&source, &request);
    let result = arrange_ensemble_form_visits_score(&source, &request);
    assert!(result.diagnostics.is_empty(), "{:?}", result.diagnostics);
    assert_eq!(result.written_measures_by_visit.len(), 40);
    assert_eq!(result.written_measures_by_visit[24], 16);
    assert_eq!(
        acorde_core::to_playback_events(result.score.as_ref().unwrap(), &Default::default())
            .iter()
            .map(event_key)
            .collect::<Vec<_>>(),
        selected.events.iter().map(event_key).collect::<Vec<_>>()
    );
}

#[test]
fn unfolded_repeat_restores_the_written_key_and_clef() {
    let mut source = repeated();
    source.parts[0].staves[0].measures[4].clef = Some(Clef::Bass);
    source.parts[0].staves[0].measures[4].key_sig = Some(acorde_core::KeySignature {
        fifths: 1,
        mode: "major".into(),
    });
    let result = arrange_ensemble_form_visits_score(&source, &request());
    assert!(result.diagnostics.is_empty(), "{:?}", result.diagnostics);
    let measures = &result.score.as_ref().unwrap().parts[0].staves[0].measures;
    assert_eq!(measures[4].clef, Some(Clef::Bass));
    assert_eq!(measures[4].key_sig.as_ref().unwrap().fifths, 1);
    assert_eq!(measures[8].clef, Some(Clef::Treble));
    assert_eq!(measures[8].key_sig.as_ref().unwrap().fifths, 0);
    assert_eq!(measures[12].clef, Some(Clef::Bass));
    assert_eq!(measures[12].key_sig.as_ref().unwrap().fifths, 1);
}

#[test]
fn unresolved_view_references_fail_without_a_partial_performance_score() {
    let mut source = repeated();
    source
        .views
        .push(acorde_core::ScoreView::linked_part("melody", "Melody", 0));
    assert!(
        arrange_ensemble_form_visits(&source, &request())
            .diagnostics
            .is_empty()
    );
    let result = arrange_ensemble_form_visits_score(&source, &request());
    assert!(result.score.is_none());
    assert!(result.resolved.is_none());
    assert!(result.written_measures_by_visit.is_empty());
    assert_eq!(
        result.diagnostics[0].code,
        "unsupported-form-performance-refs"
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
    let score_replay = replay_ensemble_form_visits_score(&changed, &resolved);
    assert!(score_replay.score.is_none());
    assert!(score_replay.resolved.is_none());
    assert_eq!(score_replay.diagnostics[0].code, "form-source-mismatch");
    let mut changed = resolved;
    changed.engine_version.push_str("-unknown");
    assert_eq!(
        replay_ensemble_form_visits(&source, &changed).diagnostics[0].code,
        "unsupported-form-visits-contract"
    );
    let score_replay = replay_ensemble_form_visits_score(&source, &changed);
    assert!(score_replay.score.is_none());
    assert_eq!(
        score_replay.diagnostics[0].code,
        "unsupported-form-visits-contract"
    );
}
