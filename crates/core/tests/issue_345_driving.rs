use acorde_core::{
    AccordionBassMode, AccordionMelodicVoice, AccordionPattern, AccordionPresetSelection,
    AccordionSwingPolicy, Articulation, Duration, LayeredAccordionOptions, Note, Pitch,
    PositionedAccordionChord, Score, Step, arrange_accordion_preset, arrange_with_layers,
    to_playback_events,
};

fn input() -> (Score, AccordionPresetSelection) {
    let mut score = Score::new("4/4", 120, 4, 4, 0, 1);
    score.parts[0].staves[0].measures[0].voices[0] =
        vec![Note::new(Pitch::new(Step::C, 5), Duration::Whole)];
    let chord = |root| {
        serde_json::from_value(serde_json::json!({"root":root,"kind":"major","bass":null})).unwrap()
    };
    let selection = AccordionPresetSelection {
        chords: vec![
            PositionedAccordionChord {
                measure_index: 0,
                offset_beats: 0.0,
                chord: chord("C"),
            },
            PositionedAccordionChord {
                measure_index: 0,
                offset_beats: 2.0,
                chord: chord("G"),
            },
        ],
        alternative_chords: None,
        inversion: None,
        bass_mode: AccordionBassMode::StandardBass,
    };
    (score, selection)
}
fn options(
    selection: &AccordionPresetSelection,
    pattern: AccordionPattern,
) -> LayeredAccordionOptions {
    LayeredAccordionOptions {
        chords: selection.chords.clone(),
        alternative_chords: None,
        inversion: None,
        accompaniment: Some(pattern),
        melodic_voice: AccordionMelodicVoice::Unchanged,
        swing: AccordionSwingPolicy::Straight,
        bass_mode: selection.bass_mode,
    }
}

#[test]
fn driving_and_light_patterns_have_expected_rhythm_expression_and_harmony() {
    let (score, selection) = input();
    let before = serde_json::to_value(&score).unwrap();
    for mode in [AccordionBassMode::StandardBass, AccordionBassMode::FreeBass] {
        let mut selection = selection.clone();
        selection.bass_mode = mode;
        let result = arrange_accordion_preset(&score, selection.clone(), "driving-bass-chord");
        assert!(result.diagnostics.is_empty(), "{:?}", result.diagnostics);
        let output = result.score.unwrap();
        assert_eq!(serde_json::to_value(&score).unwrap(), before);
        assert_eq!(
            serde_json::to_value(&output.parts[0].staves[0]).unwrap(),
            serde_json::to_value(&score.parts[0].staves[0]).unwrap()
        );
        let notes = &output.parts[0].staves[1].measures[0].voices[0];
        assert_eq!(notes.len(), 8);
        assert!(notes.iter().all(|n| n.beats() == 0.5));
        assert_eq!(
            notes
                .iter()
                .map(|n| n.pitches.iter().map(Pitch::to_midi).collect::<Vec<_>>())
                .collect::<Vec<_>>(),
            vec![
                vec![36],
                vec![60, 64, 67],
                vec![43],
                vec![60, 64, 67],
                vec![43],
                vec![67, 71, 74],
                vec![50],
                vec![67, 71, 74]
            ]
        );
        assert_eq!(notes[0].articulations, vec![Articulation::Accent]);
        for i in [1, 3, 5, 7] {
            assert_eq!(notes[i].articulations, vec![Articulation::Staccato]);
        }
        let events = to_playback_events(&output, &Default::default());
        let bass = events
            .iter()
            .filter(|e| e.source.as_ref().is_some_and(|a| a.staff == 1))
            .collect::<Vec<_>>();
        assert_eq!(bass.len(), 16);
        assert!(
            bass.iter()
                .any(|e| e.time_beats == 0.0 && e.velocity == 84 && e.duration_beats == 0.5)
        );
        assert!(
            bass.iter()
                .any(|e| e.time_beats == 0.5 && e.velocity == 64 && e.duration_beats == 0.25)
        );
        let light = arrange_accordion_preset(&score, selection, "driving-bass-chord-light")
            .score
            .unwrap();
        let light_notes = &light.parts[0].staves[1].measures[0].voices[0];
        assert_eq!(light_notes.len(), 4);
        assert!(
            light_notes
                .iter()
                .all(|n| n.beats() == 1.0 && n.articulations.is_empty())
        );
        assert_eq!(light.settings.tempo_bpm, score.settings.tempo_bpm);
    }
}

#[test]
fn pattern_keeps_phase_pickups_and_diagnoses_unsupported_inputs() {
    let (mut score, selection) = input();
    let mut opts = options(&selection, AccordionPattern::DrivingBassChord);
    let mut repeated = opts.clone();
    let mut chord = repeated.chords[0].clone();
    chord.offset_beats = 0.5;
    repeated.chords.insert(1, chord);
    let a = arrange_with_layers(&score, opts.clone()).score.unwrap();
    let b = arrange_with_layers(&score, repeated).score.unwrap();
    let notes = |s: &Score| {
        s.parts[0].staves[1].measures[0].voices[0]
            .iter()
            .map(|n| {
                (
                    n.beats(),
                    n.pitches.iter().map(Pitch::to_midi).collect::<Vec<_>>(),
                    n.articulations.clone(),
                )
            })
            .collect::<Vec<_>>()
    };
    assert_eq!(notes(&a), notes(&b));
    opts.chords[1].offset_beats = 1.5;
    let shifted = arrange_with_layers(&score, opts.clone()).score.unwrap();
    let ns = &shifted.parts[0].staves[1].measures[0].voices[0];
    assert_eq!(
        ns[3].pitches.iter().map(Pitch::to_midi).collect::<Vec<_>>(),
        vec![67, 71, 74]
    );
    assert_eq!(ns[4].pitches[0].to_midi(), 43);
    opts.chords[1].offset_beats = 1.25;
    assert_eq!(
        arrange_with_layers(&score, opts.clone()).diagnostics[0].code,
        "unsupported-rhythm"
    );
    opts.chords.truncate(1);
    score.settings.tempo_bpm = 201;
    assert_eq!(
        arrange_with_layers(&score, opts.clone()).diagnostics[0].code,
        "unsupported-standard-bass-rate"
    );
    opts.bass_mode = AccordionBassMode::FreeBass;
    assert!(arrange_with_layers(&score, opts.clone()).score.is_some());
    score.settings.tempo_bpm = 120;
    score.parts[0].staves[0].measures[0].actual_length =
        acorde_core::MeasureLength::from_beats(1.0);
    score.parts[0].staves[0].measures[0].voices[0][0].duration = Duration::Quarter;
    let pickup = arrange_with_layers(&score, opts.clone()).score.unwrap();
    let notes = &pickup.parts[0].staves[1].measures[0].voices[0];
    assert_eq!(notes.len(), 2);
    assert_eq!(notes[0].pitches[0].to_midi(), 43);
    assert!(!notes[0].articulations.contains(&Articulation::Accent));
    let bad = Score::new("3/4", 120, 3, 4, 0, 1);
    assert_eq!(
        arrange_with_layers(&bad, opts.clone()).diagnostics[0].code,
        "unsupported-meter"
    );
    opts.swing = AccordionSwingPolicy::NotatedAccompanimentTwoToOne;
    assert_eq!(
        arrange_with_layers(&score, opts).diagnostics[0].code,
        "unsupported-swing"
    );
}

#[test]
fn presets_layers_and_seeded_shuffle_replay_the_same_music() {
    use acorde_core::{
        AccordionLayerShuffleOptions, AccordionShuffleOptions, arrange_accordion_layers,
        arrange_from_resolved_layers, arrange_from_resolved_preset, shuffle_accordion_layers,
        shuffle_accordion_preset,
    };
    let (score, selection) = input();
    let events = |s: &Score| {
        to_playback_events(s, &Default::default())
            .into_iter()
            .map(|e| (e.pitch_midi, e.time_beats, e.duration_beats, e.velocity))
            .collect::<Vec<_>>()
    };
    for (id, pattern) in [
        ("driving-bass-chord", AccordionPattern::DrivingBassChord),
        (
            "driving-bass-chord-light",
            AccordionPattern::DrivingBassChordLight,
        ),
    ] {
        let a = arrange_accordion_preset(&score, selection.clone(), id);
        let replay = arrange_from_resolved_preset(&score, a.resolved_config.unwrap());
        assert_eq!(events(&a.score.unwrap()), events(&replay.score.unwrap()));
        let mut opts = options(&selection, pattern);
        opts.melodic_voice = AccordionMelodicVoice::OctaveBelow;
        let layer = arrange_accordion_layers(&score, opts);
        assert!(layer.diagnostics.is_empty(), "{:?}", layer.diagnostics);
        let replay = arrange_from_resolved_layers(&score, layer.resolved_config.unwrap());
        assert_eq!(
            events(&layer.score.unwrap()),
            events(&replay.score.unwrap())
        );
    }
    let request = AccordionShuffleOptions {
        selection: selection.clone(),
        alternative_selections: vec![],
        allowed_preset_ids: vec![
            "driving-bass-chord".into(),
            "driving-bass-chord-light".into(),
        ],
        seed: "42".into(),
        previous_config: None,
    };
    let a = shuffle_accordion_preset(&score, request.clone());
    let b = shuffle_accordion_preset(&score, request);
    assert_eq!(a.resolved_config, b.resolved_config);
    assert!(a.score.is_some(), "{:?}", a.diagnostics);
    let request = AccordionLayerShuffleOptions {
        selection,
        alternative_selections: vec![],
        allowed_accompaniments: vec![
            Some(AccordionPattern::DrivingBassChord),
            Some(AccordionPattern::DrivingBassChordLight),
        ],
        allowed_melodic_voices: vec![AccordionMelodicVoice::Unchanged],
        allowed_swing_policies: vec![AccordionSwingPolicy::Straight],
        seed: "42".into(),
        previous_config: None,
    };
    let a = shuffle_accordion_layers(&score, request.clone());
    let b = shuffle_accordion_layers(&score, request);
    assert_eq!(a.resolved_config, b.resolved_config);
    assert!(a.score.is_some(), "{:?}", a.diagnostics);
}

#[test]
fn repeats_ties_short_final_and_rate_ramps_are_preserved_or_diagnosed() {
    let (mut score, mut selection) = input();
    selection.chords.truncate(1);
    let mut next = score.parts[0].staves[0].measures[0].clone();
    next.number = 2;
    next.voices[0][0] = Note::new(Pitch::new(Step::C, 5), Duration::Whole);
    next.voices[0][0].tie_end = true;
    let mut last = next.clone();
    last.number = 3;
    last.actual_length = acorde_core::MeasureLength::from_beats(1.0);
    last.voices[0][0] = Note::new(Pitch::new(Step::D, 5), Duration::Quarter);
    score.parts[0].staves[0].measures[0].voices[0][0].tie_start = true;
    score.parts[0].staves[0].measures[0].barline_left = acorde_core::Barline::RepeatStart;
    next.barline_right = acorde_core::Barline::RepeatEnd;
    score.parts[0].staves[0].measures.extend([next, last]);
    let before = serde_json::to_value(&score).unwrap();
    let out = arrange_accordion_preset(&score, selection.clone(), "driving-bass-chord")
        .score
        .unwrap();
    assert_eq!(before, serde_json::to_value(&score).unwrap());
    assert_eq!(
        serde_json::to_value(&out.parts[0].staves[0].measures).unwrap(),
        serde_json::to_value(&score.parts[0].staves[0].measures).unwrap()
    );
    let melody = |s: &Score| {
        to_playback_events(s, &Default::default())
            .into_iter()
            .filter(|e| e.source.as_ref().is_some_and(|a| a.staff == 0))
            .map(|e| (e.pitch_midi, e.time_beats, e.duration_beats))
            .collect::<Vec<_>>()
    };
    assert_eq!(melody(&score), melody(&out));
    assert_eq!(
        out.parts[0].staves[1].measures[2].voices[0]
            .iter()
            .map(Note::beats)
            .sum::<f64>(),
        1.0
    );
    score.parts[0].staves[0].measures[1].tempo_ramp_to = Some(201);
    let result = arrange_accordion_preset(&score, selection, "driving-bass-chord-light");
    assert_eq!(result.diagnostics[0].code, "unsupported-standard-bass-rate");
    assert_eq!(result.diagnostics[0].measure_index, Some(1));
}

#[test]
fn standard_bass_rejects_seventh_chords_while_free_bass_accepts_them() {
    let (score, mut selection) = input();
    selection.chords.truncate(1);
    selection.chords[0].chord =
        serde_json::from_value(serde_json::json!({"root":"C","kind":"major-seventh","bass":null}))
            .unwrap();
    let standard = arrange_accordion_preset(&score, selection.clone(), "driving-bass-chord");
    assert_eq!(standard.diagnostics[0].code, "unsupported-standard-bass");
    selection.bass_mode = AccordionBassMode::FreeBass;
    let free = arrange_accordion_preset(&score, selection, "driving-bass-chord");
    assert!(free.score.is_some(), "{:?}", free.diagnostics);
}
