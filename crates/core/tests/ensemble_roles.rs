use acorde_core::{
    BassLineOptions, BassLinePattern, ChordalPadOptions, ChordalPadPattern, ChordalPadVoicing,
    CountermelodyOptions, Duration, Dynamic, EnsembleRole, EnsembleRoleOptions,
    EnsembleRoleSection, Note, Pitch, PositionedAccordionChord, Score, arrange_ensemble_roles,
    to_playback_events,
};

fn placed(measure_index: usize, root: &str) -> PositionedAccordionChord {
    PositionedAccordionChord {
        measure_index,
        offset_beats: 0.0,
        chord: serde_json::from_value(serde_json::json!({"root":root,"kind":"major"})).unwrap(),
    }
}

fn source() -> Score {
    let mut score = Score::new("ensemble roles", 120, 4, 4, 0, 3);
    for bar in &mut score.parts[0].staves[0].measures {
        bar.voices[0] = vec![Note::new(Pitch::from_midi(79, false), Duration::Whole)];
    }
    score
}

fn options() -> EnsembleRoleOptions {
    let chords = vec![placed(0, "C"), placed(1, "F"), placed(2, "G")];
    EnsembleRoleOptions {
        harmony: Some(ChordalPadOptions {
            chords: chords.clone(),
            pattern: ChordalPadPattern::Held,
            pattern_sections: vec![],
            meter_group_sections: vec![],
            phrase_end_measures: vec![],
            voicing: ChordalPadVoicing::RootPosition,
            midi_program: 48,
            low_midi: 48,
            high_midi: 84,
            center_midi: 64,
            min_melody_gap_semitones: Some(3),
            melody_part_index: Some(0),
        }),
        bass: Some(BassLineOptions {
            chords,
            pattern: BassLinePattern::RootFifth,
            midi_program: 32,
            low_midi: 28,
            high_midi: 60,
            center_midi: 40,
        }),
        countermelody: None,
        harmony_dynamic: None,
        bass_dynamic: None,
        countermelody_dynamic: None,
        sections: vec![],
        restart_countermelody_at_section_entry: false,
        min_bass_harmony_gap_semitones: 3,
        min_harmony_countermelody_gap_semitones: 0,
        max_generated_attacks_per_measure: Some(5),
    }
}

fn role_events(score: &Score) -> Vec<(usize, f64, f64, u8)> {
    to_playback_events(score, &Default::default())
        .into_iter()
        .filter_map(|event| {
            let part = event.source?.part;
            (part > 0).then_some((
                part,
                event.time_beats,
                event.duration_beats,
                event.pitch_midi,
            ))
        })
        .collect()
}

#[test]
fn role_dynamics_change_generated_playback_without_changing_the_source() {
    let score = source();
    let original_parts = serde_json::to_value(&score.parts).unwrap();
    let mut request = options();
    request.harmony_dynamic = Some(Dynamic::Pp);
    request.bass_dynamic = Some(Dynamic::Mp);
    request.sections = vec![EnsembleRoleSection {
        role: EnsembleRole::Bass,
        start_measure: 0,
        end_measure_exclusive: 1,
        enabled: false,
    }];
    let result = arrange_ensemble_roles(&score, request);
    assert!(result.diagnostics.is_empty(), "{:?}", result.diagnostics);
    let resolved: EnsembleRoleOptions =
        serde_json::from_value(serde_json::to_value(&result.resolved_options).unwrap()).unwrap();
    let output = result.score.unwrap();
    assert_eq!(
        serde_json::to_value(&output.parts[..1]).unwrap(),
        original_parts
    );
    assert_eq!(
        output.parts[1].staves[0].measures[0].voices[0][0].dynamic,
        Some(Dynamic::Pp)
    );
    assert!(
        output.parts[2].staves[0].measures[0].voices[0]
            .iter()
            .all(|note| note.is_rest)
    );
    assert_eq!(
        output.parts[2].staves[0].measures[1].voices[0][0].dynamic,
        Some(Dynamic::Mp)
    );
    let velocities = |score: &Score, part: usize| {
        to_playback_events(score, &Default::default())
            .into_iter()
            .filter(|event| {
                event
                    .source
                    .as_ref()
                    .is_some_and(|source| source.part == part)
            })
            .map(|event| event.velocity)
            .collect::<Vec<_>>()
    };
    assert!(
        velocities(&output, 0)
            .iter()
            .all(|velocity| *velocity == 64)
    );
    assert!(
        velocities(&output, 1)
            .iter()
            .all(|velocity| *velocity == 36)
    );
    assert!(
        velocities(&output, 2)
            .iter()
            .all(|velocity| *velocity == 60)
    );
    let replay = arrange_ensemble_roles(&score, resolved).score.unwrap();
    for part in 0..3 {
        assert_eq!(velocities(&output, part), velocities(&replay, part));
    }

    let legacy = arrange_ensemble_roles(&score, options()).score.unwrap();
    assert!(
        velocities(&legacy, 1)
            .iter()
            .all(|velocity| *velocity == 64)
    );
    assert!(
        velocities(&legacy, 2)
            .iter()
            .all(|velocity| *velocity == 64)
    );
    let mut invalid = options();
    invalid.harmony_dynamic = Some(Dynamic::Sfz);
    let rejected = arrange_ensemble_roles(&score, invalid);
    assert_eq!(rejected.diagnostics[0].code, "unsupported-dynamic");
    assert!(rejected.score.is_none());
}

#[test]
fn harmony_and_bass_switch_independently_while_source_and_replay_stay_fixed() {
    let score = source();
    let original = serde_json::to_value(&score.parts).unwrap();
    let mut request = options();
    request.sections = vec![
        EnsembleRoleSection {
            role: EnsembleRole::Bass,
            start_measure: 0,
            end_measure_exclusive: 1,
            enabled: false,
        },
        EnsembleRoleSection {
            role: EnsembleRole::Harmony,
            start_measure: 1,
            end_measure_exclusive: 2,
            enabled: false,
        },
        EnsembleRoleSection {
            role: EnsembleRole::Bass,
            start_measure: 2,
            end_measure_exclusive: 3,
            enabled: false,
        },
    ];
    let result = arrange_ensemble_roles(&score, request);
    assert!(result.diagnostics.is_empty(), "{:?}", result.diagnostics);
    assert_eq!(result.role_parts.len(), 2);
    assert_eq!(
        result
            .activity
            .iter()
            .map(|state| (state.harmony, state.bass))
            .collect::<Vec<_>>(),
        vec![(true, false), (false, true), (true, false)]
    );
    let replay = arrange_ensemble_roles(&score, result.resolved_options.clone());
    let output = result.score.unwrap();
    assert_eq!(role_events(&output), role_events(&replay.score.unwrap()));
    assert_eq!(serde_json::to_value(&output.parts[..1]).unwrap(), original);
    assert!(acorde_core::model::validate::validate(&output).is_valid());
    for (measure, role) in [
        (0, EnsembleRole::Bass),
        (1, EnsembleRole::Harmony),
        (2, EnsembleRole::Bass),
    ] {
        let part = result
            .role_parts
            .iter()
            .find(|part| part.role == role)
            .unwrap()
            .part_index;
        assert!(
            output.parts[part].staves[0].measures[measure].voices[0]
                .iter()
                .all(|note| note.is_rest)
        );
    }
    assert!(result.generated_notes.iter().all(|addr| {
        !output.parts[addr.part].staves[addr.staff].measures[addr.measure].voices[addr.voice]
            [addr.note]
            .is_rest
    }));
}

#[test]
fn collision_density_and_invalid_sections_return_no_partial_score() {
    let score = source();
    let mut colliding = options();
    let bass = colliding.bass.as_mut().unwrap();
    bass.low_midi = 48;
    bass.center_midi = 60;
    bass.high_midi = 60;
    let mut dense = options();
    dense.max_generated_attacks_per_measure = Some(4);
    let mut overlapping = options();
    overlapping.sections = vec![
        EnsembleRoleSection {
            role: EnsembleRole::Bass,
            start_measure: 0,
            end_measure_exclusive: 2,
            enabled: false,
        },
        EnsembleRoleSection {
            role: EnsembleRole::Bass,
            start_measure: 1,
            end_measure_exclusive: 3,
            enabled: true,
        },
    ];
    let mut missing = options();
    missing.bass = None;
    missing.sections = vec![EnsembleRoleSection {
        role: EnsembleRole::Bass,
        start_measure: 0,
        end_measure_exclusive: 1,
        enabled: true,
    }];
    for (request, code) in [
        (colliding, "bass-harmony-collision"),
        (dense, "density-limit"),
        (overlapping, "invalid-role-section"),
        (missing, "missing-role-options"),
    ] {
        let result = arrange_ensemble_roles(&score, request);
        assert_eq!(result.diagnostics[0].code, code);
        assert!(result.score.is_none());
        assert!(result.activity.is_empty());
        assert!(result.role_parts.is_empty());
        assert!(result.generated_notes.is_empty());
    }
}

#[test]
fn countermelody_switches_independently_and_checks_harmony_collisions() {
    let mut score = Score::new("four role measures", 120, 4, 4, 0, 4);
    score.parts[0].staves[0].measures[0].voices[0] = [84, 86, 88, 84]
        .into_iter()
        .map(|midi| Note::new(Pitch::from_midi(midi, false), Duration::Quarter))
        .collect();
    for bar in &mut score.parts[0].staves[0].measures[1..] {
        bar.voices[0] = vec![Note::new(Pitch::from_midi(84, false), Duration::Whole)];
    }
    let mut request = options();
    request
        .harmony
        .as_mut()
        .unwrap()
        .chords
        .push(placed(3, "F"));
    request.bass.as_mut().unwrap().chords.push(placed(3, "F"));
    request.countermelody = Some(CountermelodyOptions {
        chords: request.harmony.as_ref().unwrap().chords.clone(),
        melody_part_index: 0,
        motif_start_measure: 0,
        motif_end_measure_exclusive: Some(1),
        midi_program: 73,
        low_midi: 75,
        center_midi: 77,
        high_midi: 80,
        min_melody_gap_semitones: 4,
        phrase_end_measures: Vec::new(),
    });
    request.max_generated_attacks_per_measure = Some(7);
    request.min_harmony_countermelody_gap_semitones = 3;
    request.countermelody_dynamic = Some(Dynamic::P);
    request.sections = vec![EnsembleRoleSection {
        role: EnsembleRole::Countermelody,
        start_measure: 1,
        end_measure_exclusive: 2,
        enabled: false,
    }];
    let result = arrange_ensemble_roles(&score, request.clone());
    assert!(result.diagnostics.is_empty(), "{:?}", result.diagnostics);
    assert_eq!(result.role_parts.len(), 3);
    assert_eq!(
        result
            .activity
            .iter()
            .map(|state| state.countermelody)
            .collect::<Vec<_>>(),
        vec![true, false, true, true]
    );
    let output = result.score.unwrap();
    assert_eq!(
        output.parts[3].staves[0].measures[0].voices[0][0].dynamic,
        Some(Dynamic::P)
    );
    assert!(
        to_playback_events(&output, &Default::default())
            .into_iter()
            .filter(|event| event.source.as_ref().is_some_and(|source| source.part == 3))
            .all(|event| event.velocity == 48)
    );
    assert_eq!(
        serde_json::to_value(&output.parts[..1]).unwrap(),
        serde_json::to_value(&score.parts).unwrap()
    );
    assert!(
        output.parts[3].staves[0].measures[1].voices[0]
            .iter()
            .all(|note| note.is_rest)
    );
    assert!(
        output.parts[3].staves[0].measures[2].voices[0]
            .iter()
            .any(|note| !note.is_rest)
    );
    let replay = arrange_ensemble_roles(&score, result.resolved_options);
    assert_eq!(role_events(&output), role_events(&replay.score.unwrap()));
    assert!(acorde_core::model::validate::validate(&output).is_valid());

    request.harmony.as_mut().unwrap().low_midi = 60;
    request.harmony.as_mut().unwrap().center_midi = 76;
    let collision = arrange_ensemble_roles(&score, request);
    assert_eq!(
        collision.diagnostics[0].code,
        "harmony-countermelody-collision"
    );
    assert!(collision.score.is_none());
    assert!(collision.generated_notes.is_empty());
}

#[test]
fn countermelody_reenters_from_motif_start_after_inactive_section() {
    let mut score = Score::new("counter entrance", 120, 4, 4, 0, 4);
    score.parts[0].staves[0].measures[0].voices[0] = [84, 86, 88, 84]
        .into_iter()
        .map(|midi| Note::new(Pitch::from_midi(midi, false), Duration::Quarter))
        .collect();
    for bar in &mut score.parts[0].staves[0].measures[1..] {
        bar.voices[0] = vec![Note::new(Pitch::from_midi(84, false), Duration::Whole)];
    }
    let chords = ["C", "F", "G", "C"]
        .into_iter()
        .enumerate()
        .map(|(measure, root)| placed(measure, root))
        .collect::<Vec<_>>();
    let mut request = options();
    request.harmony.as_mut().unwrap().chords = chords.clone();
    request.harmony.as_mut().unwrap().center_midi = 55;
    request.harmony.as_mut().unwrap().high_midi = 67;
    request.bass.as_mut().unwrap().chords = chords.clone();
    request.countermelody = Some(CountermelodyOptions {
        chords,
        melody_part_index: 0,
        motif_start_measure: 0,
        motif_end_measure_exclusive: Some(1),
        midi_program: 73,
        low_midi: 68,
        center_midi: 74,
        high_midi: 80,
        min_melody_gap_semitones: 4,
        phrase_end_measures: Vec::new(),
    });
    request.max_generated_attacks_per_measure = Some(7);
    request.min_harmony_countermelody_gap_semitones = 3;
    let all_active = arrange_ensemble_roles(&score, request.clone());
    assert!(
        all_active.diagnostics.is_empty(),
        "{:?}",
        all_active.diagnostics
    );
    request.sections = vec![EnsembleRoleSection {
        role: EnsembleRole::Countermelody,
        start_measure: 0,
        end_measure_exclusive: 2,
        enabled: false,
    }];
    let continuous = arrange_ensemble_roles(&score, request.clone());
    assert!(
        continuous.diagnostics.is_empty(),
        "{:?}",
        continuous.diagnostics
    );
    request.restart_countermelody_at_section_entry = true;
    let result = arrange_ensemble_roles(&score, request);
    assert!(result.diagnostics.is_empty(), "{:?}", result.diagnostics);
    let output = result.score.unwrap();
    let reference = all_active.score.unwrap();
    let pitches = |score: &Score, measure: usize| {
        score.parts[3].staves[0].measures[measure].voices[0]
            .iter()
            .filter(|note| !note.is_rest && !note.tie_end)
            .map(|note| note.pitches[0].to_midi())
            .collect::<Vec<_>>()
    };
    assert!(pitches(&output, 0).is_empty());
    assert!(pitches(&output, 1).is_empty());
    assert_eq!(pitches(&reference, 2).first(), Some(&71));
    assert_eq!(pitches(&continuous.score.unwrap(), 2).first(), Some(&71));
    assert_eq!(pitches(&output, 2).first(), Some(&74));
    let replay = arrange_ensemble_roles(&score, result.resolved_options);
    assert!(replay.diagnostics.is_empty(), "{:?}", replay.diagnostics);
    assert_eq!(role_events(&output), role_events(&replay.score.unwrap()));
}
