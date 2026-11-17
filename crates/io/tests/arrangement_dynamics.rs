#![cfg(feature = "musicxml")]

use acorde_core::{
    Duration, Dynamic, EnsembleRoleOptions, Note, Pitch, Score, arrange_ensemble_roles,
    to_playback_events,
};

#[test]
fn generated_role_levels_survive_musicxml_round_trip() {
    let mut source = Score::new("role dynamics", 120, 4, 4, 0, 2);
    for bar in &mut source.parts[0].staves[0].measures {
        bar.voices[0] = vec![Note::new(Pitch::from_midi(79, false), Duration::Whole)];
    }
    let options: EnsembleRoleOptions = serde_json::from_value(serde_json::json!({
        "harmony": {
            "chords": [
                {"measure_index":0,"offset_beats":0.0,"chord":{"root":"C","kind":"major"}},
                {"measure_index":1,"offset_beats":0.0,"chord":{"root":"F","kind":"major"}}
            ],
            "pattern":"held", "voicing":"smooth", "midi_program":0,
            "low_midi":48, "center_midi":55, "high_midi":67,
            "melody_part_index":0, "min_melody_gap_semitones":3
        },
        "bass": {
            "chords": [
                {"measure_index":0,"offset_beats":0.0,"chord":{"root":"C","kind":"major"}},
                {"measure_index":1,"offset_beats":0.0,"chord":{"root":"F","kind":"major"}}
            ],
            "pattern":"root-pulse", "midi_program":0,
            "low_midi":28, "center_midi":33, "high_midi":40
        },
        "harmony_dynamic":"Pp",
        "bass_dynamic":"Mp",
        "min_bass_harmony_gap_semitones":3
    }))
    .unwrap();
    let result = arrange_ensemble_roles(&source, options);
    assert!(result.diagnostics.is_empty(), "{:?}", result.diagnostics);
    let score = result.score.unwrap();
    let xml = acorde_io::serialize_musicxml(&score).unwrap();
    let reparsed = acorde_io::parse_musicxml(&xml).unwrap();
    assert_eq!(
        reparsed.parts[1].staves[0].measures[0].voices[0][0].dynamic,
        Some(Dynamic::Pp)
    );
    assert_eq!(
        reparsed.parts[2].staves[0].measures[0].voices[0][0].dynamic,
        Some(Dynamic::Mp)
    );
    let events = |score: &Score| {
        to_playback_events(score, &Default::default())
            .into_iter()
            .map(|event| {
                (
                    event.source.map(|source| source.part),
                    (event.time_beats * 480.0).round() as u64,
                    (event.duration_beats * 480.0).round() as u64,
                    event.pitch_midi,
                    event.velocity,
                )
            })
            .collect::<Vec<_>>()
    };
    assert_eq!(events(&score), events(&reparsed));
}
