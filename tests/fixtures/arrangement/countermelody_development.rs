use acorde_core::{
    CountermelodyDevelopmentKind, CountermelodyDevelopmentRequest, CountermelodyOptions, Duration,
    Note, Pitch, PositionedAccordionChord, Score, to_playback_events,
};

fn chord(measure_index: usize, root: &str) -> PositionedAccordionChord {
    PositionedAccordionChord {
        measure_index,
        offset_beats: 0.0,
        chord: serde_json::from_value(serde_json::json!({"root":root,"kind":"major"})).unwrap(),
    }
}

pub fn source() -> Score {
    let mut score = Score::new("rhythmic motif", 108, 4, 4, 0, 4);
    score.parts[0].staves[0].measures[0].voices[0] = vec![
        Note::new(Pitch::from_midi(84, false), Duration::Quarter),
        Note::rest(Duration::Eighth),
        Note::new(Pitch::from_midi(86, false), Duration::Eighth),
        Note::new(Pitch::from_midi(88, false), Duration::Half),
    ];
    for bar in &mut score.parts[0].staves[0].measures[1..] {
        bar.voices[0] = vec![Note::new(Pitch::from_midi(84, false), Duration::Whole)];
    }
    score
}

pub fn request(kind: CountermelodyDevelopmentKind) -> CountermelodyDevelopmentRequest {
    CountermelodyDevelopmentRequest {
        base: CountermelodyOptions {
            chords: vec![chord(0, "C"), chord(1, "F"), chord(2, "G"), chord(3, "C")],
            melody_part_index: 0,
            motif_start_measure: 0,
            motif_end_measure_exclusive: Some(1),
            midi_program: 73,
            low_midi: 68,
            center_midi: 74,
            high_midi: 80,
            min_melody_gap_semitones: 4,
            phrase_end_measures: vec![1, 3],
        },
        kind,
    }
}

pub fn events(score: &Score, minimum_part: usize) -> Vec<(usize, u64, u64, u8, u8)> {
    to_playback_events(score, &Default::default())
        .into_iter()
        .filter_map(|event| {
            let source = event.source?;
            (source.part >= minimum_part).then_some((
                source.part,
                (event.time_beats * 480.0).round() as u64,
                (event.duration_beats * 480.0).round() as u64,
                event.pitch_midi,
                event.velocity,
            ))
        })
        .collect()
}
