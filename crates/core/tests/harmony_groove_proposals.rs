#[path = "../../../tests/fixtures/arrangement/harmony_groove_source.rs"]
mod fixture;
use acorde_core::*;

#[test]
fn three_profiles_have_audible_differences_and_source_bound_replay() {
    let source = fixture::source();
    let request = fixture::request();
    let result = suggest_harmony_groove_arrangements(&source, &request).unwrap();
    assert!(result.complete, "{:?}", result);
    assert_eq!(
        result
            .candidates
            .iter()
            .map(|c| c.profile)
            .collect::<Vec<_>>(),
        vec![
            HarmonyGrooveProfile::Plain,
            HarmonyGrooveProfile::Color,
            HarmonyGrooveProfile::Rhythm
        ]
    );
    for (i, c) in result.candidates.iter().enumerate() {
        assert_eq!(
            serde_json::to_value(&c.arrangement.score.parts[..source.parts.len()]).unwrap(),
            serde_json::to_value(&source.parts).unwrap()
        );
        for other in &result.candidates[..i] {
            assert_ne!(
                fixture::sound(&c.arrangement.score),
                fixture::sound(&other.arrangement.score)
            );
        }
        let saved = serde_json::to_string(&c.resolved).unwrap();
        let replay =
            replay_harmony_groove_arrangement(&source, &serde_json::from_str(&saved).unwrap())
                .unwrap();
        assert_eq!(c.resolved, replay.resolved);
        assert_eq!(c.difference, replay.difference);
        assert_eq!(
            fixture::sound(&c.arrangement.score),
            fixture::sound(&replay.arrangement.score)
        );
    }
    let again = suggest_harmony_groove_arrangements(&source, &request).unwrap();
    for (a, b) in result.candidates.iter().zip(again.candidates) {
        assert_eq!(a.resolved, b.resolved);
        assert_eq!(
            fixture::sound(&a.arrangement.score),
            fixture::sound(&b.arrangement.score)
        );
    }
    eprintln!(
        "color additions: {:?}; calls: {}",
        result.candidates[1].difference.added_tensions, result.realizations
    );
    assert!(!result.candidates[1].difference.added_tensions.is_empty());
}

#[test]
fn replay_rechecks_register_tension_rhythm_permissions_and_source() {
    let source = fixture::source();
    let result = suggest_harmony_groove_arrangements(&source, &fixture::request()).unwrap();
    let saved = &result.candidates[2].resolved;
    let mut changed = source.clone();
    changed.parts[0].staves[0].measures[0].voices[0][0].pitches[0].alter = 1;
    assert!(matches!(
        replay_harmony_groove_arrangement(&changed, saved),
        Err(HarmonyGrooveError::ReplaySource)
    ));
    let mut bad = saved.clone();
    bad.arrangement.request.high_midi += 1;
    assert!(matches!(
        replay_harmony_groove_arrangement(&source, &bad),
        Err(HarmonyGrooveError::ReplaySelection)
    ));
    let mut bad = saved.clone();
    bad.engine_version.push('x');
    assert!(matches!(
        replay_harmony_groove_arrangement(&source, &bad),
        Err(HarmonyGrooveError::ReplayVersion)
    ));
    let mut bad = saved.clone();
    bad.request.allow_offbeats = false;
    bad.request.anticipation = None;
    assert!(matches!(
        replay_harmony_groove_arrangement(&source, &bad),
        Err(HarmonyGrooveError::ReplaySelection)
    ));
    let mut bad = result.candidates[1].resolved.clone();
    bad.request.max_changed_harmonies = 0;
    assert!(matches!(
        replay_harmony_groove_arrangement(&source, &bad),
        Err(HarmonyGrooveError::ReplaySelection)
    ));
}

#[test]
fn locks_ranges_and_change_budgets_survive_automatic_selection() {
    let source = fixture::source();
    let mut request = fixture::request();
    request.base.locked_measures = vec![0];
    request.max_changed_harmonies = 1;
    let result = suggest_harmony_groove_arrangements(&source, &request).unwrap();
    assert!(result.complete);
    for c in result.candidates {
        assert!(c.difference.changed_harmony_entries.len() <= 1);
        assert!(!c.difference.changed_harmony_entries.contains(&0));
        assert!(
            c.arrangement.score.parts[c.arrangement.added_part].staves[0].measures[0].voices[0]
                .iter()
                .all(|n| n.is_rest)
        );
        replay_harmony_groove_arrangement(&source, &c.resolved).unwrap();
    }
}

#[test]
fn unavailable_diversity_is_reported_without_duplicate_padding() {
    let source = fixture::source();
    let mut request = fixture::request();
    request.allow_rootless = false;
    request.allow_offbeats = false;
    request.anticipation = None;
    request.tensions.clear();
    let result = suggest_harmony_groove_arrangements(&source, &request).unwrap();
    assert!(!result.complete);
    assert_eq!(result.candidates.len(), 1);
    assert_eq!(
        result.missing_profiles,
        vec![HarmonyGrooveProfile::Color, HarmonyGrooveProfile::Rhythm]
    );
    assert!(!result.realization_limit_reached && !result.harmony_search_pruned);
}

#[test]
fn search_limits_and_five_candidate_requests_are_explicit() {
    let source = fixture::source();
    let mut request = fixture::request();
    request.max_realizations = 3;
    let short = suggest_harmony_groove_arrangements(&source, &request).unwrap();
    assert!(short.realization_limit_reached);
    assert!(short.realizations <= 3);
    request.max_realizations = 128;
    request.count = 5;
    let full = suggest_harmony_groove_arrangements(&source, &request).unwrap();
    assert!(full.complete);
    assert_eq!(full.candidates.len(), 5);
    for (i, c) in full.candidates.iter().enumerate() {
        for other in &full.candidates[..i] {
            assert_ne!(
                fixture::sound(&c.arrangement.score),
                fixture::sound(&other.arrangement.score)
            );
        }
    }
}

#[test]
fn permissions_cannot_overwrite_authored_tones_or_become_unbounded() {
    let source = fixture::source();
    let mut request = fixture::request();
    request.tensions[0].tones[0].degree = 7;
    assert!(matches!(
        suggest_harmony_groove_arrangements(&source, &request),
        Err(HarmonyGrooveError::InvalidTension { entry: 0 })
    ));
    request = fixture::request();
    request.base.chords.entries[0]
        .chord
        .degrees
        .push(ChordDegree {
            value: 9,
            alter: 0,
            kind: "subtract".into(),
        });
    assert!(matches!(
        suggest_harmony_groove_arrangements(&source, &request),
        Err(HarmonyGrooveError::InvalidTension { entry: 0 })
    ));
    request = fixture::request();
    request.max_realizations = 129;
    assert!(matches!(
        suggest_harmony_groove_arrangements(&source, &request),
        Err(HarmonyGrooveError::InvalidRequest)
    ));
    request = fixture::request();
    request.base.root_policy = HarmonyRootPolicy::Omit;
    assert!(matches!(
        suggest_harmony_groove_arrangements(&source, &request),
        Err(HarmonyGrooveError::InvalidRequest)
    ));
}

#[test]
fn sustained_melody_friction_changes_tension_selection() {
    let source = fixture::source();
    let mut request = fixture::request();
    request.allow_rootless = false;
    request.tensions = vec![HarmonyTensionAllowance {
        chord_entry: 2,
        tones: vec![
            HarmonyTension {
                degree: 9,
                alter: -1,
            },
            HarmonyTension {
                degree: 9,
                alter: 0,
            },
        ],
    }];
    request.max_changed_harmonies = 1;
    let result = suggest_harmony_groove_arrangements(&source, &request).unwrap();
    assert!(result.complete);
    let color = &result.candidates[1];
    assert_eq!(
        color.resolved.additions,
        vec![HarmonyTensionAddition {
            chord_entry: 2,
            tone: HarmonyTension {
                degree: 9,
                alter: 0
            }
        }]
    );
    let mut bad = color.resolved.clone();
    bad.additions[0].tone.alter = -1;
    bad.arrangement.request.chords.entries[2].chord.degrees[0].alter = -1;
    let strained = replay_harmony_groove_arrangement(&source, &bad).unwrap();
    assert!(strained.quality.melody_friction_ticks > color.quality.melody_friction_ticks);
}

#[test]
fn replay_does_not_accept_a_relabeled_plain_score_as_color() {
    let source = fixture::source();
    let result = suggest_harmony_groove_arrangements(&source, &fixture::request()).unwrap();
    let mut saved = result.candidates[0].resolved.clone();
    saved.profile = HarmonyGrooveProfile::Color;
    assert!(matches!(
        replay_harmony_groove_arrangement(&source, &saved),
        Err(HarmonyGrooveError::ReplaySelection)
    ));
}
