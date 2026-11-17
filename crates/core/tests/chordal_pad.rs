use acorde_core::{
    ChordalPadMeterGroupSection, ChordalPadOptions, ChordalPadPattern, ChordalPadPatternSection,
    ChordalPadVoicing, Clef, Duration, Measure, MeasureLength, Note, Part, Pitch,
    PositionedAccordionChord, Score, Staff, arrange_chordal_pad,
};

fn placed(measure_index: usize, offset_beats: f64, root: &str) -> PositionedAccordionChord {
    PositionedAccordionChord {
        measure_index,
        offset_beats,
        chord: serde_json::from_value(serde_json::json!({"root":root,"kind":"major"})).unwrap(),
    }
}

fn options(pattern: ChordalPadPattern) -> ChordalPadOptions {
    ChordalPadOptions {
        chords: vec![placed(0, 0.0, "C"), placed(0, 2.0, "F")],
        pattern,
        pattern_sections: Vec::new(),
        meter_group_sections: Vec::new(),
        phrase_end_measures: Vec::new(),
        voicing: ChordalPadVoicing::RootPosition,
        midi_program: 48,
        low_midi: 48,
        high_midi: 84,
        center_midi: 64,
        min_melody_gap_semitones: None,
        melody_part_index: None,
    }
}

fn multi_part_score() -> Score {
    let mut score = Score::new("ensemble source", 120, 4, 4, 0, 2);
    for bar in &mut score.parts[0].staves[0].measures {
        bar.voices[0] = vec![Note::new(Pitch::from_midi(72, false), Duration::Whole)];
    }
    let mut other = Part::new("Oboe", "Ob.");
    other.midi_channel = 1;
    other.midi_program = 68;
    let mut staff = Staff::new(Clef::Treble);
    for number in 1..=2 {
        let mut bar = Measure::empty(4, 4);
        bar.number = number;
        bar.voices[0] = vec![Note::new(Pitch::from_midi(76, false), Duration::Whole)];
        staff.measures.push(bar);
    }
    other.staves.push(staff);
    score.parts.push(other);
    score
}

fn pad_playback(score: &Score, part: usize) -> Vec<(f64, f64, u8)> {
    acorde_core::to_playback_events(score, &Default::default())
        .into_iter()
        .filter(|event| event.source.as_ref().is_some_and(|addr| addr.part == part))
        .map(|event| (event.time_beats, event.duration_beats, event.pitch_midi))
        .collect()
}

fn attack_times(score: &Score, part: usize) -> Vec<f64> {
    let mut times = pad_playback(score, part)
        .into_iter()
        .map(|event| event.0)
        .collect::<Vec<_>>();
    times.sort_by(f64::total_cmp);
    times.dedup();
    times
}

#[test]
fn meter_pulse_follows_compound_and_irregular_groups_with_rests_and_accents() {
    let mut request = options(ChordalPadPattern::MeterPulse);
    request.chords.truncate(1);
    let compound = Score::new("compound", 120, 6, 8, 0, 1);
    let result = arrange_chordal_pad(&compound, request.clone());
    assert!(result.diagnostics.is_empty(), "{:?}", result.diagnostics);
    assert_eq!(result.resolved_meter_groups, vec![vec![3, 3]]);
    assert_eq!(result.resolved_options.meter_group_sections.len(), 1);
    let replay = arrange_chordal_pad(&compound, result.resolved_options.clone());
    let output = result.score.unwrap();
    assert_eq!(
        pad_playback(&output, 1),
        pad_playback(&replay.score.unwrap(), 1)
    );
    assert_eq!(attack_times(&output, 1), vec![0.0, 1.5]);
    assert!(pad_playback(&output, 1).iter().all(|event| event.1 == 0.75));
    let voice = &output.parts[1].staves[0].measures[0].voices[0];
    assert!(voice.iter().any(|note| note.is_rest));
    assert_eq!(voice.iter().map(Note::beats).sum::<f64>(), 3.0);
    assert_eq!(
        voice
            .iter()
            .filter(|note| note
                .articulations
                .contains(&acorde_core::Articulation::Accent))
            .count(),
        2
    );
    let mut plain_request = request.clone();
    plain_request.pattern = ChordalPadPattern::Held;
    let plain = arrange_chordal_pad(&compound, plain_request).score.unwrap();
    let first_velocity = |score: &Score| {
        acorde_core::to_playback_events(score, &Default::default())
            .into_iter()
            .find(|event| {
                event.source.as_ref().is_some_and(|addr| addr.part == 1)
                    && event.time_beats == 0.0
                    && event.pitch_midi % 12 == 0
            })
            .unwrap()
            .velocity
    };
    assert!(first_velocity(&output) > first_velocity(&plain));

    let irregular = Score::new("five", 120, 5, 8, 0, 1);
    let default = arrange_chordal_pad(&irregular, request.clone())
        .score
        .unwrap();
    assert_eq!(attack_times(&default, 1), vec![0.0, 1.5]);
    request.meter_group_sections = vec![ChordalPadMeterGroupSection {
        start_measure: 0,
        end_measure_exclusive: 1,
        groups: vec![2, 3],
    }];
    let shifted_result = arrange_chordal_pad(&irregular, request);
    assert_eq!(shifted_result.resolved_meter_groups, vec![vec![2, 3]]);
    let shifted = shifted_result.score.unwrap();
    assert_eq!(attack_times(&shifted, 1), vec![0.0, 1.0]);
    assert!(acorde_core::model::validate::validate(&shifted).is_valid());
}

#[test]
fn meter_pulse_forces_chord_change_midgroup_then_returns_to_group_pulse() {
    let score = Score::new("midgroup", 120, 5, 8, 0, 1);
    let mut request = options(ChordalPadPattern::MeterPulse);
    request.chords = vec![placed(0, 0.0, "C"), placed(0, 1.0, "F")];
    let result = arrange_chordal_pad(&score, request);
    assert!(result.diagnostics.is_empty(), "{:?}", result.diagnostics);
    let output = result.score.unwrap();
    assert_eq!(attack_times(&output, 1), vec![0.0, 1.0, 1.5]);
    assert!(
        pad_playback(&output, 1)
            .iter()
            .any(|event| event.0 == 1.0 && event.2 % 12 == 5)
    );
    assert!(
        output.parts[1].staves[0].measures[0].voices[0]
            .iter()
            .any(|note| note.is_rest)
    );
    assert_eq!(
        serde_json::to_value(&output.parts[..1]).unwrap(),
        serde_json::to_value(&score.parts).unwrap()
    );
}

#[test]
fn authored_phrase_end_reduces_attacks_and_holds_the_final_two_groups() {
    let score = Score::new("phrase", 120, 4, 4, 0, 2);
    let mut request = options(ChordalPadPattern::QuarterPulse);
    request.chords = vec![placed(0, 0.0, "C")];
    let plain = arrange_chordal_pad(&score, request.clone()).score.unwrap();
    request.phrase_end_measures = vec![1];
    let result = arrange_chordal_pad(&score, request);
    assert!(result.diagnostics.is_empty(), "{:?}", result.diagnostics);
    assert_eq!(result.phrase_start_ticks, vec![None, Some(64)]);
    assert_eq!(result.resolved_options.meter_group_sections.len(), 2);
    let replay = arrange_chordal_pad(&score, result.resolved_options.clone());
    let output = result.score.unwrap();
    assert_eq!(
        pad_playback(&output, 1),
        pad_playback(&replay.score.unwrap(), 1)
    );
    assert_eq!(
        attack_times(&plain, 1),
        vec![0.0, 1.0, 2.0, 3.0, 4.0, 5.0, 6.0, 7.0]
    );
    assert_eq!(
        attack_times(&output, 1),
        vec![0.0, 1.0, 2.0, 3.0, 4.0, 5.0, 6.0]
    );
    assert!(
        pad_playback(&output, 1)
            .iter()
            .any(|event| event.0 == 6.0 && event.1 == 2.0)
    );
    assert!(
        output.parts[1].staves[0].measures[1].voices[0]
            .last()
            .unwrap()
            .articulations
            .contains(&acorde_core::Articulation::Accent)
    );
    assert_eq!(
        serde_json::to_value(&output.parts[..1]).unwrap(),
        serde_json::to_value(&score.parts).unwrap()
    );
}

#[test]
fn meter_pulse_sections_preserve_harmony_and_phrase_ends_preserve_changes() {
    let score = Score::new("grouped section", 120, 5, 8, 0, 2);
    let mut request = options(ChordalPadPattern::Held);
    request.chords = vec![placed(0, 0.0, "C"), placed(1, 0.0, "F")];
    request.pattern_sections = vec![ChordalPadPatternSection {
        start_measure: 1,
        end_measure_exclusive: 2,
        pattern: ChordalPadPattern::MeterPulse,
    }];
    request.meter_group_sections = vec![ChordalPadMeterGroupSection {
        start_measure: 1,
        end_measure_exclusive: 2,
        groups: vec![2, 3],
    }];
    let sectioned = arrange_chordal_pad(&score, request.clone()).score.unwrap();
    assert_eq!(attack_times(&sectioned, 1), vec![0.0, 2.5, 3.5]);
    assert!(
        pad_playback(&sectioned, 1)
            .iter()
            .any(|event| event.0 == 2.5 && event.2 % 12 == 5)
    );

    request.chords.push(placed(1, 2.0, "G"));
    request.phrase_end_measures = vec![1];
    let ending = arrange_chordal_pad(&score, request);
    assert!(ending.diagnostics.is_empty(), "{:?}", ending.diagnostics);
    let output = ending.score.unwrap();
    assert!(
        pad_playback(&output, 1)
            .iter()
            .any(|event| event.0 == 4.5 && event.2 % 12 == 7)
    );
    assert_eq!(
        serde_json::to_value(&output.parts[..1]).unwrap(),
        serde_json::to_value(&score.parts).unwrap()
    );
    assert!(acorde_core::model::validate::validate(&output).is_valid());
}

#[test]
fn invalid_meter_groups_and_phrase_ends_return_no_partial_score() {
    let score = Score::new("invalid groups", 120, 5, 8, 0, 2);
    let mut wrong_sum = options(ChordalPadPattern::MeterPulse);
    wrong_sum.chords = vec![placed(0, 0.0, "C")];
    wrong_sum.meter_group_sections = vec![ChordalPadMeterGroupSection {
        start_measure: 0,
        end_measure_exclusive: 2,
        groups: vec![2, 2],
    }];
    let mut zero_group = wrong_sum.clone();
    zero_group.meter_group_sections[0].groups = vec![0, 5];
    let mut duplicate_phrase = wrong_sum.clone();
    duplicate_phrase.meter_group_sections.clear();
    duplicate_phrase.phrase_end_measures = vec![1, 1];
    let mut outside_phrase = duplicate_phrase.clone();
    outside_phrase.phrase_end_measures = vec![2];
    for (request, expected) in [
        (wrong_sum, "meter-group-section"),
        (zero_group, "meter-group-section"),
        (duplicate_phrase, "phrase-ending"),
        (outside_phrase, "phrase-ending"),
    ] {
        let result = arrange_chordal_pad(&score, request);
        assert!(result.score.is_none());
        assert!(result.added_part.is_none());
        assert!(result.generated_notes.is_empty());
        assert!(result.voicings.is_empty());
        assert!(result.motion_report.is_none());
        assert!(result.resolved_meter_groups.is_empty());
        assert!(result.phrase_start_ticks.is_empty());
        assert_eq!(result.diagnostics[0].code, expected);
    }
}

#[test]
fn held_pad_appends_a_part_and_preserves_all_source_parts() {
    let score = multi_part_score();
    let original_parts = serde_json::to_value(&score.parts).unwrap();
    let result = arrange_chordal_pad(&score, options(ChordalPadPattern::Held));
    assert!(result.diagnostics.is_empty(), "{:?}", result.diagnostics);
    assert_eq!(result.added_part, Some(2));
    assert_eq!(result.generated_notes.len(), 3);
    let output = result.score.unwrap();
    assert_eq!(output.parts.len(), 3);
    assert_eq!(
        serde_json::to_value(&output.parts[..2]).unwrap(),
        original_parts
    );
    assert_eq!(serde_json::to_value(&score.parts).unwrap(), original_parts);
    assert_eq!(output.parts[2].midi_channel, 2);
    assert_eq!(output.parts[2].midi_program, 48);
    assert!(acorde_core::model::validate::validate(&output).is_valid());
    let bars = &output.parts[2].staves[0].measures;
    assert_eq!(bars[0].voices[0].len(), 2);
    assert_eq!(bars[1].voices[0].len(), 1);
    let classes = |note: &Note| {
        note.pitches
            .iter()
            .map(|pitch| pitch.to_midi() % 12)
            .collect::<Vec<_>>()
    };
    assert_eq!(classes(&bars[0].voices[0][0]), vec![0, 4, 7]);
    assert_eq!(classes(&bars[0].voices[0][1]), vec![5, 9, 0]);
    assert_eq!(classes(&bars[1].voices[0][0]), vec![5, 9, 0]);
}

#[test]
fn quarter_pulse_changes_rhythm_without_changing_harmony_or_source() {
    let score = multi_part_score();
    let held = arrange_chordal_pad(&score, options(ChordalPadPattern::Held))
        .score
        .unwrap();
    let pulse = arrange_chordal_pad(&score, options(ChordalPadPattern::QuarterPulse))
        .score
        .unwrap();
    assert_eq!(
        pulse.parts[2].staves[0]
            .measures
            .iter()
            .map(|bar| bar.voices[0].len())
            .collect::<Vec<_>>(),
        vec![4, 4]
    );
    assert_eq!(
        held.parts[2].staves[0]
            .measures
            .iter()
            .map(|bar| bar.voices[0].len())
            .collect::<Vec<_>>(),
        vec![2, 1]
    );
    let attacks = |score: &Score| {
        acorde_core::to_playback_events(score, &Default::default())
            .into_iter()
            .filter(|event| {
                event
                    .source
                    .as_ref()
                    .is_some_and(|address| address.part == 2)
            })
            .map(|event| (event.time_beats, event.pitch_midi % 12))
            .collect::<Vec<_>>()
    };
    assert!(attacks(&pulse).len() > attacks(&held).len());
    assert_eq!(
        serde_json::to_value(&pulse.parts[..2]).unwrap(),
        serde_json::to_value(&score.parts).unwrap()
    );
}

#[test]
fn smooth_voicing_reduces_motion_without_changing_chords_or_source() {
    let score = multi_part_score();
    let mut root_options = options(ChordalPadPattern::Held);
    root_options.chords.push(placed(1, 0.0, "G"));
    root_options.chords.push(placed(1, 2.0, "C"));
    let root_result = arrange_chordal_pad(&score, root_options.clone());
    let root_report = root_result.motion_report.unwrap();
    let root = root_result.score.unwrap();
    root_options.voicing = ChordalPadVoicing::Smooth;
    let smooth_result = arrange_chordal_pad(&score, root_options);
    assert!(
        smooth_result.diagnostics.is_empty(),
        "{:?}",
        smooth_result.diagnostics
    );
    let smooth = smooth_result.score.unwrap();
    let smooth_report = smooth_result.motion_report.as_ref().unwrap();
    assert!(smooth_report.written_motion_cost < root_report.written_motion_cost);
    assert_eq!(
        smooth_report.playback_motion_cost,
        smooth_report.written_motion_cost
    );
    assert_eq!(smooth_report.backward_jump_transition_count, 0);
    let chords = |score: &Score| {
        score.parts[2].staves[0]
            .measures
            .iter()
            .flat_map(|bar| &bar.voices[0])
            .map(|note| {
                note.pitches
                    .iter()
                    .map(|pitch| pitch.to_midi() as u8)
                    .collect::<Vec<_>>()
            })
            .collect::<Vec<_>>()
    };
    let root_chords = chords(&root);
    let smooth_chords = chords(&smooth);
    let motion = |chords: &[Vec<u8>]| -> u32 {
        chords
            .windows(2)
            .map(|pair| {
                pair[0]
                    .iter()
                    .map(|from| {
                        pair[1]
                            .iter()
                            .map(|to| from.abs_diff(*to) as u32)
                            .min()
                            .unwrap()
                    })
                    .sum::<u32>()
                    + pair[1]
                        .iter()
                        .map(|to| {
                            pair[0]
                                .iter()
                                .map(|from| to.abs_diff(*from) as u32)
                                .min()
                                .unwrap()
                        })
                        .sum::<u32>()
            })
            .sum()
    };
    assert!(motion(&smooth_chords) < motion(&root_chords));
    for (left, right) in root_chords.iter().zip(&smooth_chords) {
        let mut left = left.iter().map(|pitch| pitch % 12).collect::<Vec<_>>();
        let mut right = right.iter().map(|pitch| pitch % 12).collect::<Vec<_>>();
        left.sort_unstable();
        right.sort_unstable();
        assert_eq!(left, right);
    }
    assert_eq!(
        serde_json::to_value(&smooth.parts[..2]).unwrap(),
        serde_json::to_value(&score.parts).unwrap()
    );
    assert!(acorde_core::model::validate::validate(&smooth).is_valid());
}

#[test]
fn motion_report_counts_repeat_back_transition_in_playback_order() {
    let mut score = Score::new("repeat edge", 120, 4, 4, 0, 2);
    score.parts[0].staves[0].measures[0].barline_left =
        acorde_core::model::notation::Barline::RepeatStart;
    score.parts[0].staves[0].measures[1].barline_right =
        acorde_core::model::notation::Barline::RepeatEnd;
    let mut request = options(ChordalPadPattern::Held);
    request.chords = vec![placed(0, 0.0, "C"), placed(1, 0.0, "F")];
    let result = arrange_chordal_pad(&score, request);
    assert!(result.diagnostics.is_empty(), "{:?}", result.diagnostics);
    assert_eq!(result.voicings.len(), 2);
    assert_eq!(result.voicings[0].measure_index, 0);
    assert_eq!(result.voicings[1].measure_index, 1);
    let report = result.motion_report.unwrap();
    assert!(report.written_motion_cost > 0);
    assert_eq!(report.playback_transition_count, 3);
    assert_eq!(report.backward_jump_transition_count, 1);
    assert_eq!(report.backward_jump_motion_cost, report.written_motion_cost);
    assert_eq!(report.playback_motion_cost, report.written_motion_cost * 3);
    assert!(acorde_core::model::validate::validate(&result.score.unwrap()).is_valid());
}

#[test]
fn global_smooth_considers_repeat_back_edge_and_preserves_source() {
    let mut score = Score::new("repeated progression", 120, 4, 4, 0, 3);
    score.parts[0].staves[0].measures[0].barline_left =
        acorde_core::model::notation::Barline::RepeatStart;
    score.parts[0].staves[0].measures[2].barline_right =
        acorde_core::model::notation::Barline::RepeatEnd;
    let mut request = options(ChordalPadPattern::Held);
    request.chords = vec![
        placed(0, 0.0, "C"),
        placed(1, 0.0, "D"),
        placed(2, 0.0, "E"),
    ];
    request.voicing = ChordalPadVoicing::Smooth;
    let local = arrange_chordal_pad(&score, request.clone());
    request.voicing = ChordalPadVoicing::GlobalSmooth;
    let global = arrange_chordal_pad(&score, request.clone());
    assert!(global.diagnostics.is_empty(), "{:?}", global.diagnostics);
    let local_motion = local.motion_report.as_ref().unwrap();
    let global_motion = global.motion_report.as_ref().unwrap();
    assert!(
        global_motion.playback_motion_cost < local_motion.playback_motion_cost,
        "local={local_motion:?}, global={global_motion:?}"
    );
    assert!(global_motion.written_motion_cost > local_motion.written_motion_cost);
    assert!(global_motion.backward_jump_motion_cost < local_motion.backward_jump_motion_cost);
    assert_eq!(global_motion.backward_jump_transition_count, 1);
    assert!(!global_motion.search_limit_reached);
    assert_eq!(global.voicings.len(), local.voicings.len());
    for (chosen, original) in global.voicings.iter().zip(&local.voicings) {
        let mut chosen_classes = chosen
            .chord_midi
            .iter()
            .map(|pitch| pitch % 12)
            .collect::<Vec<_>>();
        let mut original_classes = original
            .chord_midi
            .iter()
            .map(|pitch| pitch % 12)
            .collect::<Vec<_>>();
        chosen_classes.sort_unstable();
        original_classes.sort_unstable();
        assert_eq!(chosen_classes, original_classes);
    }
    let output = global.score.unwrap();
    assert_eq!(
        serde_json::to_value(&output.parts[..1]).unwrap(),
        serde_json::to_value(&score.parts).unwrap()
    );
    for voicing in &global.voicings {
        let played = output.parts[1].staves[0].measures[voicing.measure_index].voices[0][0]
            .pitches
            .iter()
            .map(|pitch| pitch.to_midi() as u8)
            .collect::<Vec<_>>();
        assert_eq!(played, voicing.chord_midi);
    }
    assert!(acorde_core::model::validate::validate(&output).is_valid());
    let repeat = arrange_chordal_pad(&score, request);
    assert_eq!(repeat.motion_report, Some(global_motion.clone()));
    assert_eq!(repeat.voicings, global.voicings);
}

#[test]
fn open_voicing_spreads_chord_tones_and_rejects_a_too_narrow_register() {
    let score = multi_part_score();
    let mut request = options(ChordalPadPattern::Held);
    request.voicing = ChordalPadVoicing::Open;
    let result = arrange_chordal_pad(&score, request.clone());
    assert!(result.diagnostics.is_empty(), "{:?}", result.diagnostics);
    let output = result.score.unwrap();
    for (note, root) in output.parts[2].staves[0].measures[0].voices[0]
        .iter()
        .zip([0, 5])
    {
        let pitches = note
            .pitches
            .iter()
            .map(|pitch| pitch.to_midi() as u8)
            .collect::<Vec<_>>();
        let classes = pitches.iter().map(|pitch| pitch % 12).collect::<Vec<_>>();
        assert_eq!(classes.len(), 3);
        assert_eq!(
            classes
                .iter()
                .copied()
                .collect::<std::collections::BTreeSet<_>>()
                .len(),
            3
        );
        assert!(classes.contains(&root));
        assert!(pitches[2] - pitches[0] >= 12);
        assert!(pitches.iter().all(|pitch| (48..=84).contains(pitch)));
    }
    assert_eq!(
        serde_json::to_value(&output.parts[..2]).unwrap(),
        serde_json::to_value(&score.parts).unwrap()
    );
    assert!(acorde_core::model::validate::validate(&output).is_valid());

    request.low_midi = 60;
    request.high_midi = 72;
    request.center_midi = 66;
    let rejected = arrange_chordal_pad(&score, request);
    assert!(rejected.score.is_none());
    assert_eq!(rejected.diagnostics[0].code, "unsupported-chord-or-range");
}

#[test]
fn melody_clearance_uses_the_lowest_note_anywhere_in_a_chord_region() {
    let mut score = Score::new("changing melody", 120, 4, 4, 0, 1);
    score.parts[0].staves[0].measures[0].voices[0] = vec![
        Note::new(Pitch::from_midi(72, false), Duration::Half),
        Note::new(Pitch::from_midi(60, false), Duration::Half),
    ];
    let original = serde_json::to_value(&score.parts).unwrap();
    let mut request = options(ChordalPadPattern::Held);
    request.chords = vec![placed(0, 0.0, "C")];
    let without_clearance = arrange_chordal_pad(&score, request.clone()).score.unwrap();
    request.min_melody_gap_semitones = Some(4);
    let with_clearance = arrange_chordal_pad(&score, request.clone());
    assert!(
        with_clearance.diagnostics.is_empty(),
        "{:?}",
        with_clearance.diagnostics
    );
    let output = with_clearance.score.unwrap();
    let pitches = |score: &Score| {
        score.parts[1].staves[0].measures[0].voices[0][0]
            .pitches
            .iter()
            .map(|pitch| pitch.to_midi() as u8)
            .collect::<Vec<_>>()
    };
    assert_eq!(pitches(&without_clearance), vec![60, 64, 67]);
    assert_eq!(pitches(&output), vec![48, 52, 55]);
    assert_eq!(serde_json::to_value(&output.parts[..1]).unwrap(), original);
    assert!(acorde_core::model::validate::validate(&output).is_valid());

    request.low_midi = 60;
    request.high_midi = 72;
    request.center_midi = 66;
    let impossible = arrange_chordal_pad(&score, request);
    assert!(impossible.score.is_none());
    assert_eq!(impossible.diagnostics[0].code, "unsupported-chord-or-range");

    let mut crossing = Score::new("cross-bar clearance", 120, 4, 4, 0, 2);
    crossing.parts[0].staves[0].measures[0].voices[0] =
        vec![Note::new(Pitch::from_midi(72, false), Duration::Whole)];
    crossing.parts[0].staves[0].measures[1].voices[0] =
        vec![Note::new(Pitch::from_midi(60, false), Duration::Whole)];
    let mut crossing_request = options(ChordalPadPattern::Held);
    crossing_request.chords = vec![placed(0, 0.0, "C")];
    crossing_request.min_melody_gap_semitones = Some(4);
    let crossing_output = arrange_chordal_pad(&crossing, crossing_request)
        .score
        .unwrap();
    assert_eq!(pitches(&crossing_output), vec![48, 52, 55]);
}

#[test]
fn melody_clearance_can_target_another_source_part_and_rejects_bad_requests() {
    let mut score = multi_part_score();
    for bar in &mut score.parts[0].staves[0].measures {
        bar.voices[0] = vec![Note::new(Pitch::from_midi(60, false), Duration::Whole)];
    }
    let mut request = options(ChordalPadPattern::Held);
    request.chords = vec![placed(0, 0.0, "C")];
    request.min_melody_gap_semitones = Some(4);
    let default = arrange_chordal_pad(&score, request.clone()).score.unwrap();
    request.melody_part_index = Some(1);
    let selected = arrange_chordal_pad(&score, request.clone()).score.unwrap();
    let highest = |score: &Score| {
        score.parts[2].staves[0].measures[0].voices[0][0]
            .pitches
            .iter()
            .map(|pitch| pitch.to_midi())
            .max()
            .unwrap()
    };
    assert_eq!(highest(&default), 55);
    assert_eq!(highest(&selected), 67);
    assert_eq!(
        serde_json::to_value(&selected.parts[..2]).unwrap(),
        serde_json::to_value(&score.parts).unwrap()
    );

    request.melody_part_index = Some(9);
    let invalid_part = arrange_chordal_pad(&score, request.clone());
    assert!(invalid_part.score.is_none());
    assert_eq!(invalid_part.diagnostics[0].code, "melody-part");
    request.melody_part_index = None;
    request.min_melody_gap_semitones = Some(25);
    let invalid_gap = arrange_chordal_pad(&score, request);
    assert!(invalid_gap.score.is_none());
    assert_eq!(invalid_gap.diagnostics[0].code, "invalid-register");
}

#[test]
fn close_voicings_use_an_inversion_when_root_position_cannot_clear_melody() {
    let mut score = Score::new("inversion clearance", 120, 4, 4, 0, 1);
    score.parts[0].staves[0].measures[0].voices[0] =
        vec![Note::new(Pitch::from_midi(68, false), Duration::Whole)];
    let mut request = options(ChordalPadPattern::Held);
    request.chords = vec![placed(0, 0.0, "C")];
    request.low_midi = 50;
    request.high_midi = 72;
    request.min_melody_gap_semitones = Some(3);
    for voicing in [ChordalPadVoicing::Smooth, ChordalPadVoicing::GlobalSmooth] {
        request.voicing = voicing;
        let result = arrange_chordal_pad(&score, request.clone());
        assert!(result.diagnostics.is_empty(), "{:?}", result.diagnostics);
        let output = result.score.unwrap();
        let pitches = output.parts[1].staves[0].measures[0].voices[0][0]
            .pitches
            .iter()
            .map(|pitch| pitch.to_midi() as u8)
            .collect::<Vec<_>>();
        let mut classes = pitches.iter().map(|pitch| pitch % 12).collect::<Vec<_>>();
        classes.sort_unstable();
        assert_eq!(classes, vec![0, 4, 7]);
        assert!(pitches.iter().all(|pitch| (50..=65).contains(pitch)));
        assert_ne!(pitches[0] % 12, 0);
    }
}

#[test]
fn offbeat_pulse_strikes_between_beats_and_reports_only_sounding_notes() {
    let score = multi_part_score();
    let result = arrange_chordal_pad(&score, options(ChordalPadPattern::OffbeatPulse));
    assert!(result.diagnostics.is_empty(), "{:?}", result.diagnostics);
    let output = result.score.unwrap();
    let voice = &output.parts[2].staves[0].measures[0].voices[0];
    assert_eq!(voice.len(), 8);
    assert_eq!(voice.iter().map(Note::beats).sum::<f64>(), 4.0);
    assert!(voice.iter().step_by(2).all(|note| note.is_rest));
    assert!(voice.iter().skip(1).step_by(2).all(|note| !note.is_rest));
    assert_eq!(result.generated_notes.len(), 8);
    assert!(result.generated_notes.iter().all(|address| {
        !output.parts[address.part].staves[address.staff].measures[address.measure].voices
            [address.voice][address.note]
            .is_rest
    }));
    let starts = acorde_core::to_playback_events(&output, &Default::default())
        .into_iter()
        .filter(|event| {
            event
                .source
                .as_ref()
                .is_some_and(|address| address.part == 2)
        })
        .map(|event| event.time_beats)
        .collect::<Vec<_>>();
    for expected in [0.5, 1.5, 2.5, 3.5, 4.5, 5.5, 6.5, 7.5] {
        assert!(
            starts.contains(&expected),
            "missing {expected} in {starts:?}"
        );
    }
    assert!(acorde_core::model::validate::validate(&output).is_valid());
    assert_eq!(
        serde_json::to_value(&output.parts[..2]).unwrap(),
        serde_json::to_value(&score.parts).unwrap()
    );
}

#[test]
fn arpeggio_up_down_plays_single_chord_tones_and_changes_harmony_on_time() {
    let score = multi_part_score();
    let result = arrange_chordal_pad(&score, options(ChordalPadPattern::ArpeggioUpDown));
    assert!(result.diagnostics.is_empty(), "{:?}", result.diagnostics);
    let output = result.score.unwrap();
    let voice = &output.parts[2].staves[0].measures[0].voices[0];
    assert_eq!(voice.len(), 8);
    assert!(voice.iter().all(|note| note.pitches.len() == 1));
    assert!(voice.iter().all(|note| note.beats() == 0.5));
    let classes = voice
        .iter()
        .map(|note| note.pitches[0].to_midi() % 12)
        .collect::<Vec<_>>();
    assert_eq!(classes, vec![0, 4, 7, 4, 5, 9, 0, 9]);
    assert_eq!(result.generated_notes.len(), 16);
    assert!(acorde_core::model::validate::validate(&output).is_valid());
}

#[test]
fn pattern_sections_change_texture_while_chords_carry_across_bars() {
    let score = Score::new("four sections", 120, 4, 4, 0, 4);
    let mut request = options(ChordalPadPattern::Held);
    request.pattern_sections = vec![
        ChordalPadPatternSection {
            start_measure: 3,
            end_measure_exclusive: 4,
            pattern: ChordalPadPattern::QuarterPulse,
        },
        ChordalPadPatternSection {
            start_measure: 1,
            end_measure_exclusive: 2,
            pattern: ChordalPadPattern::OffbeatPulse,
        },
        ChordalPadPatternSection {
            start_measure: 2,
            end_measure_exclusive: 3,
            pattern: ChordalPadPattern::ArpeggioUpDown,
        },
    ];
    let result = arrange_chordal_pad(&score, request);
    assert!(result.diagnostics.is_empty(), "{:?}", result.diagnostics);
    assert_eq!(result.resolved_options.pattern_sections[0].start_measure, 1);
    let output = result.score.unwrap();
    let bars = &output.parts[1].staves[0].measures;
    assert_eq!(
        bars.iter()
            .map(|bar| bar.voices[0].len())
            .collect::<Vec<_>>(),
        vec![2, 8, 8, 4]
    );
    assert!(bars[1].voices[0].iter().step_by(2).all(|note| note.is_rest));
    assert!(bars[2].voices[0].iter().all(|note| note.pitches.len() == 1));
    assert!(bars[3].voices[0].iter().all(|note| note.pitches.len() == 3));
    let starts = acorde_core::to_playback_events(&output, &Default::default())
        .into_iter()
        .filter(|event| {
            event
                .source
                .as_ref()
                .is_some_and(|address| address.part == 1)
        })
        .map(|event| event.time_beats)
        .collect::<Vec<_>>();
    assert!(starts.contains(&4.5));
    assert!(starts.contains(&8.0));
    assert!(starts.contains(&12.0));
    assert_eq!(
        serde_json::to_value(&output.parts[0]).unwrap(),
        serde_json::to_value(&score.parts[0]).unwrap()
    );
    assert!(acorde_core::model::validate::validate(&output).is_valid());
}

#[test]
fn invalid_pattern_sections_return_no_partial_score() {
    let score = multi_part_score();
    let cases = [
        vec![ChordalPadPatternSection {
            start_measure: 1,
            end_measure_exclusive: 1,
            pattern: ChordalPadPattern::OffbeatPulse,
        }],
        vec![ChordalPadPatternSection {
            start_measure: 1,
            end_measure_exclusive: 3,
            pattern: ChordalPadPattern::OffbeatPulse,
        }],
        vec![
            ChordalPadPatternSection {
                start_measure: 0,
                end_measure_exclusive: 2,
                pattern: ChordalPadPattern::Held,
            },
            ChordalPadPatternSection {
                start_measure: 1,
                end_measure_exclusive: 2,
                pattern: ChordalPadPattern::ArpeggioUpDown,
            },
        ],
    ];
    for sections in cases {
        let mut request = options(ChordalPadPattern::Held);
        request.pattern_sections = sections;
        let result = arrange_chordal_pad(&score, request);
        assert!(result.score.is_none());
        assert!(result.generated_notes.is_empty());
        assert_eq!(result.diagnostics[0].code, "pattern-section");
    }
}

#[test]
fn short_harmony_regions_still_have_a_complete_rhythm() {
    let score = Score::new("short regions", 120, 5, 8, 0, 1);
    for pattern in [
        ChordalPadPattern::OffbeatPulse,
        ChordalPadPattern::ArpeggioUpDown,
    ] {
        let mut request = options(pattern);
        request.chords = vec![placed(0, 0.0, "C"), placed(0, 0.25, "F")];
        let result = arrange_chordal_pad(&score, request);
        assert!(result.diagnostics.is_empty(), "{:?}", result.diagnostics);
        let output = result.score.unwrap();
        let voice = &output.parts[1].staves[0].measures[0].voices[0];
        assert_eq!(voice.iter().map(Note::beats).sum::<f64>(), 2.5);
        if pattern == ChordalPadPattern::OffbeatPulse {
            assert!(!voice[0].is_rest);
            assert!(voice.last().unwrap().is_rest);
        }
        assert!(acorde_core::model::validate::validate(&output).is_valid());
    }
}

#[test]
fn offbeat_rests_count_toward_the_total_generation_limit() {
    let score = Score::new("long offbeat", 120, 16, 4, 0, 512);
    let mut request = options(ChordalPadPattern::OffbeatPulse);
    request.chords.truncate(1);
    let result = arrange_chordal_pad(&score, request);
    assert!(result.score.is_none());
    assert!(result.generated_notes.is_empty());
    assert!(result.voicings.is_empty());
    assert!(result.motion_report.is_none());
    assert_eq!(result.diagnostics[0].code, "generated-note-limit");
}

#[test]
fn fractional_meter_regions_are_tied_exactly() {
    let score = Score::new("five eight", 120, 5, 8, 0, 1);
    let mut request = options(ChordalPadPattern::Held);
    request.chords[1].offset_beats = 1.0;
    let result = arrange_chordal_pad(&score, request);
    assert!(result.diagnostics.is_empty(), "{:?}", result.diagnostics);
    let output = result.score.unwrap();
    let voice = &output.parts[1].staves[0].measures[0].voices[0];
    assert_eq!(voice.len(), 3);
    assert_eq!(voice.iter().map(Note::beats).sum::<f64>(), 2.5);
    assert!(voice[1].tie_start);
    assert!(voice[2].tie_end);
    assert!(acorde_core::model::validate::validate(&output).is_valid());
    let events = acorde_core::to_playback_events(&output, &Default::default());
    assert!(events.iter().any(|event| {
        event
            .source
            .as_ref()
            .is_some_and(|address| address.part == 1)
            && event.time_beats == 1.0
            && event.duration_beats == 1.5
    }));
}

#[test]
fn short_first_pickup_keeps_the_shared_playback_clock() {
    let mut score = Score::new("pickup", 120, 4, 4, 0, 2);
    let first = &mut score.parts[0].staves[0].measures[0];
    first.actual_length = MeasureLength::from_beats(1.0);
    first.voices[0] = vec![Note::new(Pitch::from_midi(72, false), Duration::Quarter)];
    let mut request = options(ChordalPadPattern::Held);
    request.chords = vec![placed(0, 0.0, "C"), placed(1, 0.0, "F")];
    let result = arrange_chordal_pad(&score, request);
    assert!(result.diagnostics.is_empty(), "{:?}", result.diagnostics);
    let output = result.score.unwrap();
    assert_eq!(
        output.parts[1].staves[0].measures[0].actual_length,
        MeasureLength::from_beats(1.0)
    );
    assert_eq!(
        output.parts[1].staves[0].measures[0].voices[0][0].beats(),
        1.0
    );
    assert!(acorde_core::model::validate::validate(&output).is_valid());
    let events = acorde_core::to_playback_events(&output, &Default::default());
    assert!(events.iter().any(|event| {
        event
            .source
            .as_ref()
            .is_some_and(|address| address.part == 1 && address.measure == 1)
            && event.time_beats == 1.0
    }));
}

#[test]
fn invalid_chords_positions_and_registers_return_no_partial_score() {
    let score = multi_part_score();
    let mut bad = Vec::new();
    let mut missing = options(ChordalPadPattern::Held);
    missing.chords.remove(0);
    bad.push((missing, "missing-initial-chord"));
    let mut off_grid = options(ChordalPadPattern::Held);
    off_grid.chords[1].offset_beats = 1.0 / 64.0;
    bad.push((off_grid, "chord-position"));
    let mut out_of_range = options(ChordalPadPattern::Held);
    out_of_range.low_midi = 60;
    out_of_range.high_midi = 64;
    out_of_range.center_midi = 62;
    bad.push((out_of_range, "unsupported-chord-or-range"));
    let mut slash = options(ChordalPadPattern::Held);
    slash.chords[0].chord.bass = Some("E".into());
    bad.push((slash, "unsupported-chord-or-range"));
    let mut bad_program = options(ChordalPadPattern::Held);
    bad_program.midi_program = 200;
    bad.push((bad_program, "invalid-register"));
    let mut duplicate = options(ChordalPadPattern::Held);
    duplicate.chords.push(placed(0, 2.0, "G"));
    bad.push((duplicate, "chord-position"));
    let mut non_finite = options(ChordalPadPattern::Held);
    non_finite.chords[1].offset_beats = f64::NAN;
    bad.push((non_finite, "chord-position"));
    for (request, code) in bad {
        let result = arrange_chordal_pad(&score, request);
        assert!(result.score.is_none());
        assert!(result.added_part.is_none());
        assert!(result.generated_notes.is_empty());
        assert_eq!(result.diagnostics[0].code, code);
    }
}

#[test]
fn a_score_using_all_melodic_midi_channels_cannot_receive_another_part() {
    let mut score = Score::new("all channels", 120, 4, 4, 0, 1);
    for channel in (1..16).filter(|channel| *channel != 9) {
        let mut part = Part::new("Used", "Used");
        part.midi_channel = channel;
        let mut staff = Staff::new(Clef::Treble);
        staff.measures.push(Measure::empty(4, 4));
        part.staves.push(staff);
        score.parts.push(part);
    }
    let mut request = options(ChordalPadPattern::Held);
    request.chords.truncate(1);
    let result = arrange_chordal_pad(&score, request);
    assert!(result.score.is_none());
    assert!(result.generated_notes.is_empty());
    assert_eq!(result.diagnostics[0].code, "no-midi-channel");
}
