use acorde_core::*;
use listening::small as fixture;

#[allow(dead_code)]
#[path = "../../../tests/fixtures/arrangement/harmony_targets_listening.rs"]
mod listening;

#[test]
fn fixed_eight_and_sixteen_bar_targets_are_distinct_replayable_and_non_destructive() {
    let corpus: listening::Corpus = serde_json::from_str(listening::FIXTURE).unwrap();
    assert_eq!(
        corpus
            .cases
            .iter()
            .map(|c| c.melody.len())
            .collect::<Vec<_>>(),
        vec![8, 16]
    );
    for case in corpus.cases {
        let (source, requests) = listening::input(&case);
        let before = serde_json::to_value(&source).unwrap();
        let baseline = arrange_harmony_voicings(&source, &requests[0].1.base).unwrap();
        let mut sounds = std::collections::BTreeSet::new();
        sounds.insert(fixture::sound(&baseline.score));
        for (name, request) in requests {
            let result = suggest_phrase_harmony_targets(&source, &request).unwrap();
            assert!(
                result.complete,
                "{} {name}: {:?}",
                case.id, result.rejections
            );
            let candidate = &result.candidates[0];
            assert!(
                sounds.insert(fixture::sound(&candidate.arrangement.score)),
                "{name}"
            );
            let replay = replay_phrase_harmony_target(&source, &candidate.resolved).unwrap();
            assert_eq!(
                fixture::sound(&replay.arrangement.score),
                fixture::sound(&candidate.arrangement.score)
            );
            assert_eq!(
                serde_json::to_value(&candidate.arrangement.score.parts[..source.parts.len()])
                    .unwrap(),
                serde_json::to_value(&source.parts).unwrap()
            );
        }
        assert_eq!(serde_json::to_value(&source).unwrap(), before);
    }
}

#[test]
fn minor_destinations_use_raised_leading_tones_and_native_pivot_variants() {
    let source = fixture::source();
    let mut r = fixture::modulation(vec![ModulationApproach::CommonChord]);
    r.modulations[0].destination_key = KeySignature {
        fifths: 0,
        mode: "minor".into(),
    };
    let result = suggest_phrase_harmony_targets(&source, &r).unwrap();
    assert!(result.complete, "{:?}", result.rejections);
    let c = &result.candidates[0];
    assert_eq!(
        c.resolved.arrangement.request.chords.entries[2].chord.root,
        "E"
    );
    assert_eq!(
        c.resolved.arrangement.request.chords.entries[3].chord.root,
        "A"
    );
    assert_eq!(
        c.resolved.arrangement.request.chords.entries[3].chord.kind,
        "minor"
    );
    assert_eq!(
        (
            c.resolutions[0].leading_tone.step.clone(),
            c.resolutions[0].leading_tone.alter
        ),
        (Step::G, 1)
    );
    r.modulations[0].destination_key = KeySignature {
        fifths: -3,
        mode: "minor".into(),
    };
    let result = suggest_phrase_harmony_targets(&source, &r).unwrap();
    assert!(result.complete, "{:?}", result.rejections);
    assert!(result.candidates[0].resolved.modulations[0].pivot.is_some());
}

#[test]
fn authored_omissions_are_retained_but_diminished_arrivals_are_rejected() {
    let source = fixture::source();
    let mut r = fixture::request();
    r.base.chords.entries[1].chord.degrees = vec![ChordDegree {
        value: 5,
        alter: 0,
        kind: "subtract".into(),
    }];
    let result = suggest_phrase_harmony_targets(&source, &r).unwrap();
    assert!(result.complete, "{:?}", result.rejections);
    assert_eq!(
        result.candidates[0]
            .resolved
            .arrangement
            .request
            .chords
            .entries[1],
        r.base.chords.entries[1]
    );
    r.base.chords.entries[1].chord.degrees.clear();
    r.base.chords.entries[1].chord.kind = "diminished".into();
    assert!(matches!(
        suggest_phrase_harmony_targets(&source, &r),
        Err(PhraseHarmonyTargetError::UnsupportedArrival { entry: 1 })
    ));
}

#[test]
fn dominant_entry_resolves_before_the_final_cadence() {
    let source = fixture::source();
    let r = fixture::modulation(vec![ModulationApproach::Dominant]);
    let result = suggest_phrase_harmony_targets(&source, &r).unwrap();
    assert!(result.complete, "{:?}", result.rejections);
    let c = &result.candidates[0];
    assert_eq!(
        c.resolved.arrangement.request.chords.entries[1].chord.root,
        "G"
    );
    assert!(
        c.resolutions
            .iter()
            .any(|e| e.approach_entry == 0 && e.arrival_entry == 1)
    );
    assert!(
        c.resolutions
            .iter()
            .any(|e| e.approach_entry == 2 && e.arrival_entry == 3)
    );
    let mut short = r;
    short.modulations[0].arrival_entry = 2;
    let result = suggest_phrase_harmony_targets(&source, &short).unwrap();
    assert!(result.candidates.is_empty());
    assert!(
        result
            .rejections
            .contains(&PhraseHarmonyTargetRejection::RegionTooShort {
                target: 0,
                approach: ModulationApproach::Dominant
            })
    );
}

#[test]
fn actual_repeat_destinations_cannot_skip_the_promised_arrival() {
    let mut source = fixture::source();
    source.parts[0].staves[0].measures[2].barline_right = Barline::RepeatEnd;
    let result = suggest_phrase_harmony_targets(&source, &fixture::request()).unwrap();
    assert!(result.candidates.is_empty());
    assert!(
        result
            .rejections
            .contains(&PhraseHarmonyTargetRejection::AmbiguousResolution {
                approach_entry: 2,
                arrival_entry: 3
            })
    );
    source.parts[0].staves[0].measures[2].barline_right = Barline::Normal;
    source.parts[0].staves[0].measures[3].barline_right = Barline::RepeatEnd;
    let result = suggest_phrase_harmony_targets(&source, &fixture::request()).unwrap();
    assert!(result.complete, "{:?}", result.rejections);
    assert_eq!(result.candidates[0].resolutions[0].transitions.len(), 2);
}

#[test]
fn sustained_melody_friction_is_measured_and_can_reject_the_reharmonization() {
    let mut source = fixture::source();
    source.parts[0].staves[0].measures[0].voices[0][0].pitches[0] = Pitch::from_midi(79, false);
    let mut r = fixture::request();
    let result = suggest_phrase_harmony_targets(&source, &r).unwrap();
    assert!(result.complete, "{:?}", result.rejections);
    assert!(result.candidates[0].quality.melody_friction_ticks > 0);
    r.max_melody_friction_ticks = Some(0);
    let result = suggest_phrase_harmony_targets(&source, &r).unwrap();
    assert!(result.candidates.is_empty());
    assert!(
        result
            .rejections
            .contains(&PhraseHarmonyTargetRejection::MelodyFriction)
    );
}

#[test]
fn alterations_keep_the_arrival_letter_in_flat_and_sharp_keys() {
    for (root, dominant_root, leading_step, leading_alter) in [
        ("Bb", "F", Step::A, 0),
        ("F#", "C#", Step::E, 1),
        ("Eb", "Bb", Step::D, 0),
        ("Cb", "Gb", Step::B, -1),
    ] {
        let source = fixture::source();
        let mut r = fixture::request();
        r.secondary_dominants.truncate(1);
        r.base.chords.entries[1].chord.root = root.into();
        r.base.chords.entries[1].chord.kind = "major".into();
        let result = suggest_phrase_harmony_targets(&source, &r).unwrap();
        assert!(result.complete, "{root}: {:?}", result.rejections);
        let c = &result.candidates[0];
        assert_eq!(
            c.resolved.arrangement.request.chords.entries[0].chord.root,
            dominant_root
        );
        assert_eq!(
            (
                c.resolutions[0].leading_tone.step.clone(),
                c.resolutions[0].leading_tone.alter
            ),
            (leading_step, leading_alter)
        );
    }
}

#[test]
fn applied_dominants_preserve_arrivals_spell_leading_tones_and_replay() {
    let source = fixture::source();
    let before = serde_json::to_value(&source).unwrap();
    let request = fixture::request();
    let result = suggest_phrase_harmony_targets(&source, &request).unwrap();
    assert!(result.complete, "{:?}", result.rejections);
    let c = &result.candidates[0];
    let entries = &c.arrangement.resolved.request.chords.entries;
    assert_eq!(
        (&entries[0].chord.root[..], &entries[0].chord.kind[..]),
        ("E", "dominant")
    );
    assert_eq!(
        (&entries[2].chord.root[..], &entries[2].chord.kind[..]),
        ("D", "dominant")
    );
    assert_eq!(entries[1], request.base.chords.entries[1]);
    assert_eq!(entries[3], request.base.chords.entries[3]);
    assert_eq!(c.changed_chord_entries, [0, 2]);
    assert_eq!(
        (
            c.resolutions[0].leading_tone.step.clone(),
            c.resolutions[0].leading_tone.alter
        ),
        (Step::G, 1)
    );
    assert_eq!(
        (
            c.resolutions[1].leading_tone.step.clone(),
            c.resolutions[1].leading_tone.alter
        ),
        (Step::F, 1)
    );
    for r in &c.resolutions {
        assert!(r.resolved_leading_tones > 0, "{r:?}");
        assert!(
            r.transitions
                .iter()
                .flat_map(|t| &t.voices)
                .any(|v| v.from_degree == 3 && v.to_degree == 1 && v.semitones == 1)
        );
    }
    assert_eq!(
        serde_json::to_value(&c.arrangement.score.parts[..source.parts.len()]).unwrap(),
        serde_json::to_value(&source.parts).unwrap()
    );
    assert_eq!(serde_json::to_value(&source).unwrap(), before);
    let saved = serde_json::to_string(&c.resolved).unwrap();
    let replay =
        replay_phrase_harmony_target(&source, &serde_json::from_str(&saved).unwrap()).unwrap();
    assert_eq!(
        fixture::sound(&c.arrangement.score),
        fixture::sound(&replay.arrangement.score)
    );
    assert_eq!(c.resolved, replay.resolved);
}

#[test]
fn both_modulation_routes_end_in_the_destination_dominant_and_tonic() {
    let source = fixture::source();
    for route in [
        ModulationApproach::CommonChord,
        ModulationApproach::Dominant,
    ] {
        let r = fixture::modulation(vec![route]);
        let out = suggest_phrase_harmony_targets(&source, &r).unwrap();
        assert!(out.complete, "{route:?}: {:?}", out.rejections);
        let c = &out.candidates[0];
        let chords = &c.resolved.arrangement.request.chords.entries;
        assert_eq!(
            (&chords[2].chord.root[..], &chords[2].chord.kind[..]),
            ("D", "dominant")
        );
        assert_eq!(
            (&chords[3].chord.root[..], &chords[3].chord.kind[..]),
            ("G", "major")
        );
        assert_eq!(c.resolved.modulations[0].approach, route);
        assert_eq!(
            c.resolved.modulations[0].pivot.is_some(),
            route == ModulationApproach::CommonChord
        );
        assert!(!c.resolutions[0].transitions.is_empty());
        let replay = replay_phrase_harmony_target(&source, &c.resolved).unwrap();
        assert_eq!(
            fixture::sound(&c.arrangement.score),
            fixture::sound(&replay.arrangement.score)
        );
    }
}

#[test]
fn locks_budgets_and_missing_choices_do_not_fabricate_a_candidate() {
    let source = fixture::source();
    let mut r = fixture::request();
    r.locked_chord_entries = vec![0];
    let out = suggest_phrase_harmony_targets(&source, &r).unwrap();
    assert!(!out.complete && out.candidates.is_empty());
    assert!(
        out.rejections
            .contains(&PhraseHarmonyTargetRejection::LockedChord { entry: 0 })
    );
    r.locked_chord_entries.clear();
    r.max_changed_harmonies = 1;
    let out = suggest_phrase_harmony_targets(&source, &r).unwrap();
    assert!(out.candidates.is_empty());
    assert!(
        out.rejections
            .contains(&PhraseHarmonyTargetRejection::ChangeBudget)
    );
    r.max_changed_harmonies = 4;
    r.base.locked_measures = vec![2];
    assert!(
        suggest_phrase_harmony_targets(&source, &r)
            .unwrap()
            .candidates
            .is_empty()
    );
    let mut r = fixture::request();
    r.count = 3;
    let out = suggest_phrase_harmony_targets(&source, &r).unwrap();
    assert!(!out.complete && out.candidates.len() == 1);
}

#[test]
fn replay_rechecks_versions_source_and_saved_permissions() {
    let source = fixture::source();
    let r = fixture::request();
    let c = suggest_phrase_harmony_targets(&source, &r)
        .unwrap()
        .candidates
        .remove(0);
    let mut bad = c.resolved.clone();
    bad.engine_version.push('x');
    assert!(matches!(
        replay_phrase_harmony_target(&source, &bad),
        Err(PhraseHarmonyTargetError::ReplayVersion)
    ));
    let mut changed = source.clone();
    changed.settings.tempo_bpm += 1;
    assert!(matches!(
        replay_phrase_harmony_target(&changed, &c.resolved),
        Err(PhraseHarmonyTargetError::ReplaySource)
    ));
    let mut bad = c.resolved.clone();
    bad.request.max_changed_harmonies = 0;
    assert!(replay_phrase_harmony_target(&source, &bad).is_err());
    let mut bad = c.resolved.clone();
    bad.arrangement.request.chords.entries[0].chord.root = "F".into();
    assert!(replay_phrase_harmony_target(&source, &bad).is_err());
    let mut bad = c.resolved.clone();
    bad.arrangement.engine_version.push('x');
    assert!(replay_phrase_harmony_target(&source, &bad).is_err());
}

#[test]
fn malformed_positions_and_targets_are_errors_before_search() {
    let source = fixture::source();
    let mut r = fixture::request();
    r.base.chords.entries[0].position.offset.denominator = 0;
    assert!(suggest_phrase_harmony_targets(&source, &r).is_err());
    let mut r = fixture::request();
    r.base.chords.entries[1].position.measure_index = usize::MAX;
    assert!(suggest_phrase_harmony_targets(&source, &r).is_err());
    let mut r = fixture::request();
    r.secondary_dominants[0].arrival_entry = 63;
    assert!(suggest_phrase_harmony_targets(&source, &r).is_err());
    let mut r = fixture::modulation(vec![ModulationApproach::CommonChord]);
    r.modulations[0].destination_key.mode = "unknown".into();
    assert!(suggest_phrase_harmony_targets(&source, &r).is_err());
    let mut r = fixture::modulation(vec![ModulationApproach::CommonChord]);
    r.modulations[0].arrival_entry = 1;
    assert!(suggest_phrase_harmony_targets(&source, &r).is_err());
}
