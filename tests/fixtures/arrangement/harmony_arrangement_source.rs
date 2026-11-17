use acorde_core::{Duration, HarmonyArrangementRequest, Note, Part, Pitch, Score};

pub fn source() -> Score {
    let mut score = Score::new("harmony arrangement", 120, 4, 4, 0, 3);
    for bar in &mut score.parts[0].staves[0].measures {
        bar.voices[0] = vec![Note::new(Pitch::from_midi(84, false), Duration::Whole)];
    }
    let mut bass = Part::new("Bass", "B.");
    bass.midi_channel = 1;
    bass.midi_program = 32;
    let mut staff = score.parts[0].staves[0].clone();
    staff.clef = acorde_core::Clef::Bass;
    for (bar, midi) in staff.measures.iter_mut().zip([38, 31, 36]) {
        bar.voices[0] = vec![Note::new(Pitch::from_midi(midi, false), Duration::Whole)];
    }
    bass.staves.push(staff);
    score.parts.push(bass);
    score
}

pub fn request() -> HarmonyArrangementRequest {
    let chords = [("D", "minor-ninth"), ("G", "dominant-13th"), ("C", "major-ninth")]
        .into_iter().enumerate().map(|(measure, (root, kind))| serde_json::json!({
            "position":{"measure_index":measure,"offset":{"numerator":0,"denominator":1}},
            "offset_beats":0.0,"chord":{"root":root,"kind":kind},"origin":"host-authored","sources":[]
        })).collect::<Vec<_>>();
    serde_json::from_value(serde_json::json!({
        "chords":{"contract_version":1,"entries":chords},
        "melody":{"part":0,"staff":0},"bass":{"part":1,"staff":0},
        "start_measure":0,"end_measure_exclusive":3,"locked_measures":[],
        "root_policy":"automatic","voices":4,"low_midi":48,"high_midi":84,"center_midi":60,
        "min_melody_gap_semitones":3,"min_bass_gap_semitones":3,"max_leap_semitones":12,
        "parallel_perfect_penalty":0,"midi_program":0
    }))
    .unwrap()
}

pub fn sound(score: &Score) -> Vec<(usize, u8, u64, u64, u8)> {
    acorde_core::to_playback_events(score, &Default::default())
        .into_iter()
        .filter_map(|event| {
            Some((
                event.source?.part,
                event.pitch_midi,
                (event.time_beats * 960.0).round() as u64,
                (event.duration_beats * 960.0).round() as u64,
                event.velocity,
            ))
        })
        .collect()
}
