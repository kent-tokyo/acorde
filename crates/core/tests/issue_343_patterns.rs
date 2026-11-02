use acorde_core::{
    AccordionBassMode, AccordionMelodicVoice, AccordionPattern, AccordionPresetSelection,
    AccordionSwingPolicy, Duration, LayeredAccordionOptions, Note, Pitch, PositionedAccordionChord,
    Score, Step, arrange_accordion_preset, arrange_with_layers, to_playback_events,
};

fn input() -> (Score, AccordionPresetSelection) {
    let mut score = Score::new("2/4", 60, 2, 4, 0, 1);
    score.parts[0].staves[0].measures[0].voices[0] =
        vec![Note::new(Pitch::new(Step::C, 5), Duration::Half)];
    let chord =
        serde_json::from_value(serde_json::json!({"root":"C", "kind":"major", "bass":null}))
            .unwrap();
    let selection = AccordionPresetSelection {
        chords: vec![PositionedAccordionChord {
            measure_index: 0,
            offset_beats: 0.0,
            chord,
        }],
        alternative_chords: None,
        inversion: None,
        bass_mode: AccordionBassMode::StandardBass,
    };
    (score, selection)
}

#[test]
fn full_bar_preset_patterns_have_expected_roles_and_playback() {
    for (id, durations) in [
        ("fine-bass-chord", vec![0.5, 0.5, 0.5, 0.5]),
        ("habanera", vec![0.75, 0.25, 0.5, 0.5]),
    ] {
        for bass_mode in [AccordionBassMode::StandardBass, AccordionBassMode::FreeBass] {
            let (score, mut selection) = input();
            selection.bass_mode = bass_mode;
            let before = serde_json::to_value(&score).unwrap();
            let result = arrange_accordion_preset(&score, selection, id);
            assert!(result.diagnostics.is_empty(), "{:?}", result.diagnostics);
            let output = result.score.unwrap();
            assert_eq!(before, serde_json::to_value(&score).unwrap());
            assert_eq!(
                output.parts[0].staves[0].measures[0].voices[0][0].id,
                score.parts[0].staves[0].measures[0].voices[0][0].id
            );
            let notes = &output.parts[0].staves[1].measures[0].voices[0];
            assert_eq!(
                notes.iter().map(|n| n.beats()).collect::<Vec<_>>(),
                durations
            );
            assert_eq!(
                notes
                    .iter()
                    .map(|n| n.pitches.iter().map(Pitch::to_midi).collect::<Vec<_>>())
                    .collect::<Vec<_>>(),
                vec![vec![36], vec![60, 64, 67], vec![43], vec![60, 64, 67]]
            );
            let events = to_playback_events(&output, &Default::default());
            let left = events
                .iter()
                .filter(|e| e.source.as_ref().is_some_and(|a| a.staff == 1))
                .collect::<Vec<_>>();
            assert_eq!(left.len(), 8);
            assert_eq!(left[0].time_beats, 0.0);
            assert!(
                left.iter()
                    .any(|e| e.pitch_midi == 43 && e.time_beats == 1.0)
            );
        }
    }
}

#[test]
fn pickup_and_chord_change_keep_phase_and_reject_unrepresentable_cases() {
    for pattern in [AccordionPattern::FineBassChord, AccordionPattern::Habanera] {
        let (mut score, selection) = input();
        score.parts[0].staves[0].measures[0].actual_length =
            acorde_core::MeasureLength::from_beats(1.0);
        score.parts[0].staves[0].measures[0].voices[0][0].duration = Duration::Quarter;
        let options = LayeredAccordionOptions {
            chords: selection.chords.clone(),
            alternative_chords: None,
            inversion: None,
            accompaniment: Some(pattern),
            melodic_voice: AccordionMelodicVoice::Unchanged,
            swing: AccordionSwingPolicy::Straight,
            bass_mode: AccordionBassMode::StandardBass,
        };
        let result = arrange_with_layers(&score, options.clone());
        let score = result.score.expect("pickup supported");
        let notes = &score.parts[0].staves[1].measures[0].voices[0];
        assert_eq!(
            notes.iter().map(|n| n.beats()).collect::<Vec<_>>(),
            vec![0.5, 0.5]
        );
        assert_eq!(notes[0].pitches[0].to_midi(), 43);
        let (full, _) = input();
        let mut repeated = options.clone();
        let mut same = repeated.chords[0].clone();
        same.offset_beats = 0.25;
        repeated.chords.push(same);
        let a = arrange_with_layers(&full, options.clone()).score.unwrap();
        let b = arrange_with_layers(&full, repeated).score.unwrap();
        let signature = |s: &Score| {
            s.parts[0].staves[1].measures[0].voices[0]
                .iter()
                .map(|n| {
                    (
                        n.beats(),
                        n.pitches.iter().map(Pitch::to_midi).collect::<Vec<_>>(),
                    )
                })
                .collect::<Vec<_>>()
        };
        assert_eq!(signature(&a), signature(&b));
        let mut changed = options.clone();
        let mut chord = changed.chords[0].clone();
        chord.offset_beats = 0.25;
        chord.chord =
            serde_json::from_value(serde_json::json!({"root":"G", "kind":"major", "bass":null}))
                .unwrap();
        changed.chords.push(chord);
        let output = arrange_with_layers(&full, changed.clone()).score.unwrap();
        let notes = &output.parts[0].staves[1].measures[0].voices[0];
        assert_eq!(notes[0].beats(), 0.25);
        assert_eq!(
            notes[1].beats(),
            if pattern == AccordionPattern::Habanera {
                0.5
            } else {
                0.25
            }
        );
        assert_eq!(notes[1].pitches[0].to_midi(), 43);
        changed.chords[1].offset_beats = 0.125;
        assert_eq!(
            arrange_with_layers(&full, changed).diagnostics[0].code,
            "unsupported-rhythm"
        );
        let wrong_meter = Score::new("3/4", 60, 3, 4, 0, 1);
        assert_eq!(
            arrange_with_layers(&wrong_meter, options).diagnostics[0].code,
            "unsupported-meter"
        );
    }
}

#[test]
fn presets_and_layers_can_be_replayed_and_shuffled() {
    use acorde_core::{
        AccordionLayerShuffleOptions, AccordionShuffleOptions, arrange_accordion_layers,
        arrange_from_resolved_layers, arrange_from_resolved_preset, shuffle_accordion_layers,
        shuffle_accordion_preset,
    };
    let (score, selection) = input();
    let events = |s: &Score| {
        to_playback_events(s, &Default::default())
            .into_iter()
            .map(|e| (e.pitch_midi, e.time_beats, e.duration_beats))
            .collect::<Vec<_>>()
    };
    for id in ["fine-bass-chord", "habanera"] {
        let first = arrange_accordion_preset(&score, selection.clone(), id);
        let replay = arrange_from_resolved_preset(&score, first.resolved_config.unwrap());
        assert_eq!(
            events(&first.score.unwrap()),
            events(&replay.score.unwrap())
        );
        let options = LayeredAccordionOptions {
            chords: selection.chords.clone(),
            alternative_chords: None,
            inversion: None,
            accompaniment: Some(if id == "habanera" {
                AccordionPattern::Habanera
            } else {
                AccordionPattern::FineBassChord
            }),
            melodic_voice: AccordionMelodicVoice::OctaveBelow,
            swing: AccordionSwingPolicy::Straight,
            bass_mode: AccordionBassMode::StandardBass,
        };
        let layer = arrange_accordion_layers(&score, options);
        let replay = arrange_from_resolved_layers(&score, layer.resolved_config.unwrap());
        assert_eq!(
            events(&layer.score.unwrap()),
            events(&replay.score.unwrap())
        );
    }
    let a = shuffle_accordion_preset(
        &score,
        AccordionShuffleOptions {
            selection: selection.clone(),
            alternative_selections: vec![],
            allowed_preset_ids: vec!["fine-bass-chord".into(), "habanera".into()],
            seed: "42".into(),
            previous_config: None,
        },
    );
    assert!(a.score.is_some(), "{:?}", a.diagnostics);
    let b = shuffle_accordion_layers(
        &score,
        AccordionLayerShuffleOptions {
            selection,
            alternative_selections: vec![],
            allowed_accompaniments: vec![
                Some(AccordionPattern::FineBassChord),
                Some(AccordionPattern::Habanera),
            ],
            allowed_melodic_voices: vec![AccordionMelodicVoice::Unchanged],
            allowed_swing_policies: vec![AccordionSwingPolicy::Straight],
            seed: "42".into(),
            previous_config: None,
        },
    );
    assert!(b.score.is_some(), "{:?}", b.diagnostics);
}

#[test]
fn short_final_bar_repeats_and_ties_keep_original_melody() {
    let (mut score, selection) = input();
    let original = score.parts[0].staves[0].measures[0].clone();
    let mut second = original.clone();
    second.number = 2;
    second.voices[0][0] = Note::new(Pitch::new(Step::C, 5), Duration::Half);
    second.voices[0][0].tie_end = true;
    let mut final_measure = original.clone();
    final_measure.number = 3;
    final_measure.actual_length = acorde_core::MeasureLength::from_beats(1.0);
    final_measure.voices[0][0] = Note::new(Pitch::new(Step::D, 5), Duration::Quarter);
    score.parts[0].staves[0].measures[0].voices[0][0].tie_start = true;
    score.parts[0].staves[0].measures[0].barline_left = acorde_core::Barline::RepeatStart;
    second.barline_right = acorde_core::Barline::RepeatEnd;
    score.parts[0].staves[0]
        .measures
        .extend([second, final_measure]);
    let source_before = serde_json::to_value(&score).unwrap();
    for id in ["fine-bass-chord", "habanera"] {
        let result = arrange_accordion_preset(&score, selection.clone(), id);
        assert!(result.diagnostics.is_empty(), "{:?}", result.diagnostics);
        let output = result.score.unwrap();
        assert_eq!(serde_json::to_value(&score).unwrap(), source_before);
        assert_eq!(
            serde_json::to_value(&output.parts[0].staves[0].measures).unwrap(),
            serde_json::to_value(&score.parts[0].staves[0].measures).unwrap()
        );
        let left = &output.parts[0].staves[1].measures;
        assert_eq!(
            left[2].actual_length,
            score.parts[0].staves[0].measures[2].actual_length
        );
        assert_eq!(left[2].voices[0].iter().map(Note::beats).sum::<f64>(), 1.0);
        assert_eq!(left[1].barline_right, acorde_core::Barline::RepeatEnd);
    }
}

#[test]
fn tuplets_in_melody_are_preserved() {
    let (mut score, selection) = input();
    let mut notes = Vec::new();
    for (index, pitch) in [Step::C, Step::D, Step::E].into_iter().enumerate() {
        let mut note = Note::new(Pitch::new(pitch, 5), Duration::Eighth);
        note.tuplet = Some(acorde_core::TupletInfo {
            actual_notes: 3,
            normal_notes: 2,
        });
        note.tuplet_boundary = match index {
            0 => Some(acorde_core::TupletBoundary::Start),
            2 => Some(acorde_core::TupletBoundary::Stop),
            _ => None,
        };
        notes.push(note);
    }
    notes.push(Note::new(Pitch::new(Step::G, 5), Duration::Quarter));
    score.parts[0].staves[0].measures[0].voices[0] = notes;
    let before = serde_json::to_value(&score.parts[0].staves[0]).unwrap();
    for id in ["fine-bass-chord", "habanera"] {
        let result = arrange_accordion_preset(&score, selection.clone(), id);
        assert!(result.diagnostics.is_empty(), "{:?}", result.diagnostics);
        assert_eq!(
            serde_json::to_value(&result.score.unwrap().parts[0].staves[0]).unwrap(),
            before
        );
    }
}
