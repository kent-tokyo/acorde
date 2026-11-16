use acorde_core::{
    Duration, EnsembleMelodyRequest, Note, Pitch, Score, arrange_ensemble_with_melody,
    to_playback_events,
};

fn melody_events(score: &Score) -> Vec<(u64, u64, u8)> {
    let mut events = to_playback_events(score, &Default::default())
        .into_iter()
        .filter(|event| !event.is_metronome)
        .map(|event| {
            (
                (event.time_beats * 480.0).round() as u64,
                (event.duration_beats * 480.0).round() as u64,
                event.pitch_midi,
            )
        })
        .collect::<Vec<_>>();
    events.sort_unstable();
    events
}

#[test]
fn melody_handoff_with_a_broken_tie_round_trips_through_musicxml() {
    let mut source = Score::new("tied melody handoff", 120, 4, 4, 0, 3);
    for measure in 0..3 {
        let mut chord = Note::new(Pitch::from_midi(72, false), Duration::Whole);
        chord.pitches.push(Pitch::from_midi(76, false));
        source.parts[0].staves[0].measures[measure].voices[0] = vec![chord];
    }
    source.parts[0].staves[0].measures[0].voices[0][0]
        .set_pitch_ties(&[true, false], &[false, false]);
    source.parts[0].staves[0].measures[1].voices[0][0]
        .set_pitch_ties(&[true, false], &[true, false]);
    source.parts[0].staves[0].measures[2].voices[0][0]
        .set_pitch_ties(&[false, false], &[true, false]);
    let request: EnsembleMelodyRequest = serde_json::from_value(serde_json::json!({
        "ensemble":{}, "melody_part_index":0, "melody_staff_index":0,
        "melody_sections":[{"start_measure":1,"end_measure_exclusive":2,"enabled":false}]
    }))
    .unwrap();
    let result = arrange_ensemble_with_melody(&source, request);
    assert!(result.diagnostics.is_empty(), "{:?}", result.diagnostics);
    let arranged = result.score.unwrap();
    assert_eq!(
        arranged.parts[0].staves[0].measures[0].voices[0][0].pitch_tie_starts_or_uniform(),
        [false, false]
    );
    assert_eq!(
        arranged.parts[0].staves[0].measures[2].voices[0][0].pitch_tie_ends_or_uniform(),
        [false, false]
    );
    let xml = acorde_io::serialize_musicxml(&arranged).unwrap();
    let parsed = acorde_io::parse_musicxml(&xml).unwrap();
    assert_eq!(melody_events(&arranged), melody_events(&parsed));
    assert_eq!(
        melody_events(&arranged),
        vec![
            (0, 1920, 72),
            (0, 1920, 76),
            (3840, 1920, 72),
            (3840, 1920, 76),
        ]
    );
}
