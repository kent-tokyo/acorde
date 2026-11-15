#[path = "../../../tests/fixtures/arrangement/form_development_v2.rs"]
mod fixture;

use acorde_core::{
    Barline, BassLinePattern, Dynamic, EnsembleRole, FormBass, arrange_ensemble_form,
    replay_ensemble_form,
};

#[test]
fn sixteen_bar_form_coordinates_entrances_rhythm_phrase_ends_and_dynamics() {
    let source = fixture::source();
    let result = arrange_ensemble_form(&source, &fixture::request());
    assert!(result.diagnostics.is_empty(), "{:?}", result.diagnostics);
    let output = result.score.as_ref().unwrap();
    assert_eq!(
        serde_json::to_value(&output.parts[..1]).unwrap(),
        serde_json::to_value(&source.parts).unwrap()
    );
    assert_eq!(
        fixture::events(&source),
        fixture::events(output)
            .into_iter()
            .filter(|event| event.0 == 0)
            .collect::<Vec<_>>()
    );
    assert_eq!(result.measures.len(), 16);
    assert!(
        result.measures[..4]
            .iter()
            .all(|bar| bar.harmony_attacks == 1
                && bar.bass_attacks == 0
                && bar.countermelody_attacks == 0)
    );
    assert_eq!(result.measures[4].harmony_attacks, 4);
    assert_eq!(result.measures[8].harmony_attacks, 8);
    assert_eq!(result.measures[12].harmony_attacks, 4);
    assert_eq!(result.measures[15].harmony_attacks, 1);
    assert_eq!(result.measures[15].bass_attacks, 0);
    assert!(result.measures[4].countermelody_attacks > result.measures[7].countermelody_attacks);
    assert!(result.measures[8].harmony_attacks > result.measures[11].harmony_attacks);
    let bass = result
        .role_parts
        .iter()
        .find(|part| part.role == EnsembleRole::Bass)
        .unwrap()
        .part_index;
    let pitches = |measure: usize| {
        output.parts[bass].staves[0].measures[measure].voices[0]
            .iter()
            .filter(|note| !note.is_rest)
            .map(|note| note.pitches[0].to_midi() % 12)
            .collect::<Vec<_>>()
    };
    // F bass approaches the next G by F#, while the returning C alternates G.
    assert_eq!(pitches(4), [5, 0, 5, 6]);
    assert_eq!(pitches(8), [0, 7, 0, 7]);
    assert_eq!(pitches(12), [5, 5, 5, 5]);
    let resolved =
        serde_json::from_str(&serde_json::to_string(result.resolved.as_ref().unwrap()).unwrap())
            .unwrap();
    let replay = replay_ensemble_form(&source, &resolved);
    assert!(replay.diagnostics.is_empty());
    assert_eq!(result.measures, replay.measures);
    assert_eq!(
        fixture::events(output),
        fixture::events(replay.score.as_ref().unwrap())
    );
}

#[test]
fn returning_through_a_repeat_restores_the_quiet_section_level() {
    let mut source = fixture::source();
    source.parts[0].staves[0].measures[0].barline_left = Barline::RepeatStart;
    source.parts[0].staves[0].measures[7].barline_right = Barline::RepeatEnd;
    let result = arrange_ensemble_form(&source, &fixture::request());
    assert!(result.diagnostics.is_empty(), "{:?}", result.diagnostics);
    let output = result.score.as_ref().unwrap();
    let harmony = result
        .role_parts
        .iter()
        .find(|part| part.role == EnsembleRole::Harmony)
        .unwrap()
        .part_index;
    let events = fixture::events(output);
    let at = |time| {
        events
            .iter()
            .filter(|event| event.0 == harmony && event.1 == time)
            .map(|event| (event.3, event.4))
            .collect::<Vec<_>>()
    };
    assert!(!at(0).is_empty());
    assert_eq!(at(0), at(32 * 480));
    assert_eq!(
        output.parts[harmony].staves[0].measures[0].voices[0][0].dynamic,
        Some(Dynamic::P)
    );
}

#[test]
fn invalid_sections_dynamics_and_unsupported_roles_fail_atomically() {
    let source = fixture::source();
    for case in 0..8 {
        let mut request = fixture::request();
        match case {
            0 => request.sections[1].start_measure = 3,
            1 => request.sections[1].start_measure = 5,
            2 => {
                request.sections.pop();
            }
            3 => request.sections[0].harmony.as_mut().unwrap().dynamic = Dynamic::Sfz,
            4 => request.ensemble.bass = None,
            5 => request.phrase_end_measures.push(16),
            6 => request.phrase_end_measures = vec![7, 3],
            7 => request.ensemble.harmony_dynamic = Some(Dynamic::F),
            _ => unreachable!(),
        }
        let result = arrange_ensemble_form(&source, &request);
        assert!(result.score.is_none(), "case {case}");
        assert!(result.resolved.is_none());
        assert!(result.role_parts.is_empty());
        assert!(result.measures.is_empty());
        assert!(result.dynamic_marks.is_empty());
        assert!(!result.diagnostics.is_empty());
    }
}

#[test]
fn replay_rejects_changed_source_or_engine_and_keeps_collision_guards() {
    let source = fixture::source();
    let result = arrange_ensemble_form(&source, &fixture::request());
    let resolved = result.resolved.unwrap();
    let mut changed = source.clone();
    changed.settings.tempo_bpm = 110;
    assert_eq!(
        replay_ensemble_form(&changed, &resolved).diagnostics[0].code,
        "form-source-mismatch"
    );
    let mut changed = resolved.clone();
    changed.engine_version.push_str("-unknown");
    assert_eq!(
        replay_ensemble_form(&source, &changed).diagnostics[0].code,
        "unsupported-form-contract"
    );
    let mut request = fixture::request();
    request.ensemble.max_generated_attacks_per_measure = Some(1);
    assert!(arrange_ensemble_form(&source, &request).score.is_none());
    let bass = request.ensemble.bass.as_mut().unwrap();
    bass.low_midi = 60;
    bass.center_midi = 64;
    bass.high_midi = 72;
    request.ensemble.max_generated_attacks_per_measure = Some(64);
    request.sections[0].bass = Some(FormBass {
        pattern: BassLinePattern::RootPulse,
        dynamic: Dynamic::P,
    });
    assert!(arrange_ensemble_form(&source, &request).score.is_none());
}
