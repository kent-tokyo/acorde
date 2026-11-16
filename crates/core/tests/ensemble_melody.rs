use acorde_core::{
    Barline, Duration, EnsembleMelodyRequest, Note, Pitch, Score, TieSourceLocation, VoltaBracket,
    arrange_ensemble_with_melody, measure_sequence,
};

fn source() -> Score {
    let mut score = Score::new("melody handoff", 120, 4, 4, 0, 4);
    score.parts[0].staves[0].measures[0].voices[0] = [84, 86, 88, 84]
        .into_iter()
        .map(|midi| Note::new(Pitch::from_midi(midi, false), Duration::Quarter))
        .collect();
    for bar in &mut score.parts[0].staves[0].measures[1..] {
        bar.voices[0] = vec![Note::new(Pitch::from_midi(84, false), Duration::Whole)];
    }
    score
}

fn request() -> EnsembleMelodyRequest {
    let chords = serde_json::json!([
        {"measure_index":0,"offset_beats":0.0,"chord":{"root":"C","kind":"major"}},
        {"measure_index":1,"offset_beats":0.0,"chord":{"root":"F","kind":"major"}},
        {"measure_index":2,"offset_beats":0.0,"chord":{"root":"G","kind":"major"}},
        {"measure_index":3,"offset_beats":0.0,"chord":{"root":"C","kind":"major"}}
    ]);
    serde_json::from_value(serde_json::json!({
        "ensemble":{
            "harmony":{
                "chords":chords,"pattern":"held","voicing":"smooth",
                "midi_program":48,"low_midi":48,"center_midi":62,"high_midi":76,
                "min_melody_gap_semitones":3
            },
            "bass":{
                "chords":chords,"pattern":"root-fifth","midi_program":32,
                "low_midi":28,"center_midi":40,"high_midi":52
            },
            "countermelody":{
                "chords":chords,"melody_part_index":0,
                "motif_start_measure":0,"motif_end_measure_exclusive":1,
                "midi_program":73,"low_midi":75,"center_midi":77,"high_midi":80,
                "min_melody_gap_semitones":4
            },
            "sections":[
                {"role":"countermelody","start_measure":0,"end_measure_exclusive":2,"enabled":false},
                {"role":"countermelody","start_measure":3,"end_measure_exclusive":4,"enabled":false}
            ],
            "min_bass_harmony_gap_semitones":3,
            "min_harmony_countermelody_gap_semitones":3
        },
        "melody_part_index":0,
        "melody_staff_index":0,
        "melody_sections":[
            {"start_measure":2,"end_measure_exclusive":3,"enabled":false}
        ]
    }))
    .unwrap()
}

fn events(score: &Score) -> Vec<(usize, u64, u8)> {
    acorde_core::to_playback_events(score, &Default::default())
        .into_iter()
        .map(|event| {
            (
                event.source.unwrap().part,
                (event.time_beats * 480.0).round() as u64,
                event.pitch_midi,
            )
        })
        .collect()
}

#[test]
fn selected_melody_measure_hands_off_to_counterpart_without_mutating_source() {
    let source = source();
    let before = serde_json::to_value(&source).unwrap();
    let result = arrange_ensemble_with_melody(&source, request());
    assert!(result.diagnostics.is_empty(), "{:?}", result.diagnostics);
    assert_eq!(result.melody_activity, vec![true, true, false, true]);
    assert_eq!(result.role_parts.len(), 3);
    let output = result.score.unwrap();
    assert_eq!(serde_json::to_value(&source).unwrap(), before);
    assert!(
        output.parts[0].staves[0].measures[2].voices[0]
            .iter()
            .all(|note| note.is_rest)
    );
    assert!(
        output.parts[3].staves[0].measures[2].voices[0]
            .iter()
            .any(|note| !note.is_rest)
    );
    assert!(
        events(&output)
            .iter()
            .all(|event| event.0 != 0 || event.1 < 8 * 480 || event.1 >= 12 * 480)
    );
    assert!(
        events(&output)
            .iter()
            .any(|event| event.0 == 3 && event.1 >= 8 * 480 && event.1 < 12 * 480)
    );
    for measure in [0, 1, 3] {
        assert_eq!(
            serde_json::to_value(&output.parts[0].staves[0].measures[measure]).unwrap(),
            serde_json::to_value(&source.parts[0].staves[0].measures[measure]).unwrap()
        );
    }
    let replay = arrange_ensemble_with_melody(&source, result.resolved_request);
    assert_eq!(events(&output), events(&replay.score.unwrap()));
    assert!(acorde_core::model::validate::validate(&output).is_valid());
}

#[test]
fn invalid_melody_section_returns_no_partial_performance_score() {
    let source = source();
    let mut request = request();
    request.melody_sections[0].end_measure_exclusive = 5;
    let result = arrange_ensemble_with_melody(&source, request);
    assert!(result.score.is_none());
    assert_eq!(result.diagnostics[0].code, "invalid-melody-section");
    assert!(result.melody_activity.is_empty());
}

#[test]
fn melody_handoff_closes_ties_on_both_sides_of_a_silent_measure() {
    let mut source = source();
    source.parts[0].staves[0].measures[1].voices[0][0].tie_start = true;
    source.parts[0].staves[0].measures[2].voices[0][0].tie_end = true;
    source.parts[0].staves[0].measures[2].voices[0][0].tie_start = true;
    source.parts[0].staves[0].measures[3].voices[0][0].tie_end = true;
    let result = arrange_ensemble_with_melody(&source, request());
    assert!(result.diagnostics.is_empty(), "{:?}", result.diagnostics);
    let score = result.score.unwrap();
    assert!(!score.parts[0].staves[0].measures[1].voices[0][0].tie_start);
    assert!(
        score.parts[0].staves[0].measures[2].voices[0]
            .iter()
            .all(|note| note.is_rest)
    );
    assert!(!score.parts[0].staves[0].measures[3].voices[0][0].tie_end);
    let melody_events = acorde_core::to_playback_events(&score, &Default::default())
        .into_iter()
        .filter(|event| {
            event
                .source
                .as_ref()
                .is_some_and(|address| address.part == 0)
        })
        .collect::<Vec<_>>();
    assert!(melody_events.iter().any(|event| {
        (event.time_beats - 4.0).abs() < 1e-9 && (event.duration_beats - 4.0).abs() < 1e-9
    }));
    assert!(melody_events.iter().any(|event| {
        (event.time_beats - 12.0).abs() < 1e-9 && (event.duration_beats - 4.0).abs() < 1e-9
    }));
    assert!(acorde_core::model::validate::validate(&score).is_valid());
}

#[test]
fn melody_handoff_closes_only_the_crossing_partial_chord_ties() {
    let mut source = source();
    for measure in 1..4 {
        source.parts[0].staves[0].measures[measure].voices[0][0]
            .pitches
            .push(Pitch::from_midi(88, false));
    }
    source.parts[0].staves[0].measures[1].voices[0][0]
        .set_pitch_ties(&[true, false], &[false, false]);
    source.parts[0].staves[0].measures[2].voices[0][0]
        .set_pitch_ties(&[true, false], &[true, false]);
    source.parts[0].staves[0].measures[3].voices[0][0]
        .set_pitch_ties(&[false, false], &[true, false]);
    source.parts[0].staves[0].measures[3].voices[0][0].pitch_incoming_partial_ties =
        vec![true, false];
    let result = arrange_ensemble_with_melody(&source, request());
    assert!(result.diagnostics.is_empty(), "{:?}", result.diagnostics);
    let score = result.score.unwrap();
    let before = &score.parts[0].staves[0].measures[1].voices[0][0];
    let after = &score.parts[0].staves[0].measures[3].voices[0][0];
    assert_eq!(before.pitch_tie_starts_or_uniform(), vec![false, false]);
    assert_eq!(after.pitch_tie_ends_or_uniform(), vec![false, false]);
    assert_eq!(after.pitch_incoming_partial_ties, vec![true, false]);
    assert_eq!(
        before.pitches,
        source.parts[0].staves[0].measures[1].voices[0][0].pitches
    );
    assert_eq!(
        after.pitches,
        source.parts[0].staves[0].measures[3].voices[0][0].pitches
    );
    assert!(acorde_core::model::validate::validate(&score).is_valid());
}

#[test]
fn muted_first_ending_keeps_tie_into_active_second_ending() {
    let mut source = Score::new("alternate ending handoff", 120, 4, 4, 0, 3);
    for bar in &mut source.parts[0].staves[0].measures {
        bar.voices[0] = vec![Note::new(Pitch::from_midi(72, false), Duration::Whole)];
    }
    let bars = &mut source.parts[0].staves[0].measures;
    bars[0].barline_left = Barline::RepeatStart;
    bars[0].voices[0][0].tie_start = true;
    bars[1].barline_right = Barline::RepeatEnd;
    bars[1].volta = Some(VoltaBracket {
        number: 1,
        kind: "begin_end".into(),
        numbers: vec![],
    });
    bars[1].voices[0][0].tie_end = true;
    bars[2].volta = Some(VoltaBracket {
        number: 2,
        kind: "begin_end".into(),
        numbers: vec![],
    });
    bars[2].voices[0][0].pitch_incoming_partial_ties = vec![true];
    assert_eq!(measure_sequence(&source), [0, 1, 0, 2]);
    let request: EnsembleMelodyRequest = serde_json::from_value(serde_json::json!({
        "ensemble":{}, "melody_part_index":0, "melody_staff_index":0,
        "melody_sections":[{"start_measure":1,"end_measure_exclusive":2,"enabled":false}]
    }))
    .unwrap();
    let result = arrange_ensemble_with_melody(&source, request.clone());
    assert!(result.diagnostics.is_empty(), "{:?}", result.diagnostics);
    let score = result.score.unwrap();
    assert!(score.parts[0].staves[0].measures[0].voices[0][0].tie_start);
    assert!(
        score.parts[0].staves[0].measures[1].voices[0]
            .iter()
            .all(|note| note.is_rest)
    );
    assert_eq!(measure_sequence(&score), [0, 1, 0, 2]);
    let events = acorde_core::to_playback_events(&score, &Default::default())
        .into_iter()
        .map(|event| (event.time_beats, event.duration_beats, event.pitch_midi))
        .collect::<Vec<_>>();
    assert_eq!(events, [(0.0, 4.0, 72), (8.0, 8.0, 72)]);

    let mut ordinary_ending = source;
    ordinary_ending.parts[0].staves[0].measures[2].voices[0][0]
        .pitch_incoming_partial_ties
        .clear();
    ordinary_ending.parts[0].staves[0].measures[2].voices[0][0].tie_end = true;
    let ordinary = arrange_ensemble_with_melody(&ordinary_ending, request.clone());
    assert!(
        ordinary.diagnostics.is_empty(),
        "{:?}",
        ordinary.diagnostics
    );
    let ordinary_score = ordinary.score.unwrap();
    assert!(ordinary_score.parts[0].staves[0].measures[2].voices[0][0].tie_end);
    let ordinary_events = acorde_core::to_playback_events(&ordinary_score, &Default::default())
        .into_iter()
        .map(|event| (event.time_beats, event.duration_beats, event.pitch_midi))
        .collect::<Vec<_>>();
    assert_eq!(ordinary_events, [(0.0, 4.0, 72), (8.0, 8.0, 72)]);

    ordinary_ending.parts[0].staves[0].measures[2].voices[0][0].pitch_tie_sources =
        vec![Some(TieSourceLocation {
            measures: -1,
            ..Default::default()
        })];
    let mismatched = arrange_ensemble_with_melody(&ordinary_ending, request);
    assert!(
        mismatched.diagnostics.is_empty(),
        "{:?}",
        mismatched.diagnostics
    );
    let mismatched_score = mismatched.score.unwrap();
    assert!(!mismatched_score.parts[0].staves[0].measures[0].voices[0][0].tie_start);
    assert!(!mismatched_score.parts[0].staves[0].measures[2].voices[0][0].tie_end);
}
