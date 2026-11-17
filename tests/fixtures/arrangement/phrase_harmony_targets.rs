use acorde_core::*;
pub fn source() -> Score {
    let mut score = Score::new("functional harmony", 104, 4, 4, 0, 4);
    for (bar, midi) in score.parts[0].staves[0]
        .measures
        .iter_mut()
        .zip([83, 84, 81, 83])
    {
        bar.voices[0] = vec![Note::new(Pitch::from_midi(midi, false), Duration::Whole)];
    }
    score
}
pub fn request() -> PhraseHarmonyTargetRequest {
    let entries = [
        ("C", "major"),
        ("A", "minor"),
        ("C", "major"),
        ("G", "major"),
    ]
    .into_iter()
    .enumerate()
    .map(|(i, (root, kind))| {
        serde_json::json!({
            "position":{"measure_index":i,"offset":{"numerator":0,"denominator":1}},
            "offset_beats":0.0,"chord":{"root":root,"kind":kind},"origin":"host-authored",
            "sources":[{"note":null,"reference":"functional-fixture"}]
        })
    })
    .collect::<Vec<_>>();
    serde_json::from_value(serde_json::json!({
        "base":{
            "chords":{"contract_version":1,"entries":entries},
            "melody":{"part":0,"staff":0},"bass":null,
            "start_measure":0,"end_measure_exclusive":4,"locked_measures":[],
            "root_policy":"include","voices":4,"low_midi":48,"high_midi":76,
            "center_midi":60,"min_melody_gap_semitones":3,"min_bass_gap_semitones":3,
            "max_leap_semitones":12,"parallel_perfect_penalty":0,"midi_program":0
        },
        "secondary_dominants":[{"approach_entry":0,"arrival_entry":1},{"approach_entry":2,"arrival_entry":3}],
        "modulations":[],"locked_chord_entries":[],"max_changed_harmonies":4,
        "count":1,"max_realizations":64,"seed":42,"max_melody_friction_ticks":null
    })).unwrap()
}
pub fn modulation(approaches: Vec<ModulationApproach>) -> PhraseHarmonyTargetRequest {
    let mut r = request();
    r.secondary_dominants.clear();
    r.modulations = vec![PhraseModulationTarget {
        start_entry: 0,
        arrival_entry: 3,
        source_key: KeySignature {
            fifths: 0,
            mode: "major".into(),
        },
        destination_key: KeySignature {
            fifths: 1,
            mode: "major".into(),
        },
        approaches,
    }];
    r
}
pub fn sound(score: &Score) -> Vec<(usize, u8, u64, u64, u8)> {
    let mut events = to_playback_events(score, &Default::default())
        .into_iter()
        .map(|e| {
            (
                e.part_index,
                e.pitch_midi,
                (e.time_beats * 960000.0).round() as u64,
                (e.duration_beats * 960000.0).round() as u64,
                e.velocity,
            )
        })
        .collect::<Vec<_>>();
    events.sort_unstable();
    events
}
