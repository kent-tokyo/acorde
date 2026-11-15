//! Original 16-bar development melody. Deliberately shared by native, I/O and
//! browser tests; this is not an untouched evaluation case.
use acorde_core::{
    Duration, EnsembleFormRequest, InnerVoiceLine, InnerVoicePattern, InnerVoicesRequest, Note,
    Pitch, Score,
};

#[allow(dead_code)]
pub fn inner_voice_request() -> InnerVoicesRequest {
    InnerVoicesRequest {
        chords: request().ensemble.harmony.unwrap().chords,
        melody_part_index: 0,
        midi_program: 48,
        lower: InnerVoiceLine {
            pattern: InnerVoicePattern::FirstHalf,
            low_midi: 55,
            center_midi: 59,
            high_midi: 62,
            max_leap_semitones: 7,
        },
        upper: InnerVoiceLine {
            pattern: InnerVoicePattern::OffbeatPulse,
            low_midi: 64,
            center_midi: 67,
            high_midi: 71,
            max_leap_semitones: 7,
        },
        min_voice_gap_semitones: 2,
        min_melody_gap_semitones: 4,
        active_measures: vec![],
        phrase_end_measures: vec![3, 7, 11, 15],
    }
}

pub fn source() -> Score {
    let mut score = Score::new("Form development: returning light", 104, 4, 4, 0, 16);
    let melody = [
        [84, 86, 88, 84],
        [86, 89, 88, 86],
        [88, 91, 88, 86],
        [84, 88, 84, 0],
        [89, 88, 86, 89],
        [91, 89, 86, 88],
        [88, 86, 84, 86],
        [86, 89, 86, 0],
        [84, 86, 88, 91],
        [89, 88, 86, 88],
        [88, 91, 89, 88],
        [84, 88, 84, 0],
        [89, 88, 86, 84],
        [86, 89, 88, 86],
        [88, 86, 84, 86],
        [84, 84, 84, 0],
    ];
    for (bar, pitches) in score.parts[0].staves[0].measures.iter_mut().zip(melody) {
        bar.voices[0] = pitches
            .into_iter()
            .map(|pitch| {
                if pitch == 0 {
                    Note::rest(Duration::Quarter)
                } else {
                    Note::new(Pitch::from_midi(pitch, false), Duration::Quarter)
                }
            })
            .collect();
    }
    score
}

pub fn request() -> EnsembleFormRequest {
    let chords = ["C", "G", "Am", "C", "F", "G", "Am", "G", "C", "F", "Am", "C", "F", "G", "G", "C"]
        .into_iter().enumerate().map(|(measure, root)| serde_json::json!({
            "measure_index":measure, "offset_beats":0.0,
            "chord":{"root":root.trim_end_matches('m'),"kind":if root.ends_with('m') {"minor"} else {"major"}}
        })).collect::<Vec<_>>();
    serde_json::from_value(serde_json::json!({
        "ensemble":{
            "harmony":{"chords":chords,"pattern":"held","voicing":"global-smooth",
                "midi_program":48,"low_midi":48,"center_midi":60,"high_midi":71,
                "min_melody_gap_semitones":4,"melody_part_index":0},
            "bass":{"chords":chords,"pattern":"root-pulse","midi_program":32,
                "low_midi":28,"center_midi":40,"high_midi":47},
            "countermelody":{"chords":chords,"melody_part_index":0,
                "motif_start_measure":0,"motif_end_measure_exclusive":1,
                "midi_program":73,"low_midi":75,"center_midi":77,"high_midi":80,
                "min_melody_gap_semitones":4},
            "min_bass_harmony_gap_semitones":1,"min_harmony_countermelody_gap_semitones":3,
            "max_generated_attacks_per_measure":16
        },
        "sections":[
            {"label":"A: quiet introduction","start_measure":0,"end_measure_exclusive":4,
                "harmony":{"pattern":"held","dynamic":"P"},"bass":null,"countermelody":null},
            {"label":"B: rhythmic contrast","start_measure":4,"end_measure_exclusive":8,
                "harmony":{"pattern":"offbeat-pulse","dynamic":"Mp"},
                "bass":{"pattern":"approach","dynamic":"Mp"},"countermelody":"P"},
            {"label":"A: varied return","start_measure":8,"end_measure_exclusive":12,
                "harmony":{"pattern":"arpeggio-up-down","dynamic":"Mf"},
                "bass":{"pattern":"root-fifth","dynamic":"Mf"},"countermelody":"Mp"},
            {"label":"Closing phrase","start_measure":12,"end_measure_exclusive":15,
                "harmony":{"pattern":"meter-pulse","dynamic":"Mp"},
                "bass":{"pattern":"root-pulse","dynamic":"Mp"},"countermelody":null},
            {"label":"Final release","start_measure":15,"end_measure_exclusive":16,
                "harmony":{"pattern":"held","dynamic":"Pp"},"bass":null,"countermelody":null}
        ],
        "phrase_end_measures":[3,7,11,15]
    }))
    .expect("valid development request")
}

#[allow(dead_code)]
pub fn events(score: &Score) -> Vec<(usize, u64, u64, u8, u8)> {
    acorde_core::to_playback_events(score, &Default::default())
        .into_iter()
        .map(|event| {
            (
                event.source.expect("source-addressed playback").part,
                (event.time_beats * 480.0).round() as u64,
                (event.duration_beats * 480.0).round() as u64,
                event.pitch_midi,
                event.velocity,
            )
        })
        .collect()
}
