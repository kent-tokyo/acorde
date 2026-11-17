//! Two-bar repeat whose second visit needs a different first inner pitch.
use acorde_core::{
    Barline, Duration, InnerVoiceLine, InnerVoicePattern, InnerVoicesRequest, Note, Pitch, Score,
};

pub fn source() -> Score {
    let mut score = Score::new("Inner voice return", 100, 4, 4, 0, 2);
    for bar in &mut score.parts[0].staves[0].measures {
        bar.voices[0] = (0..4)
            .map(|_| Note::new(Pitch::from_midi(84, false), Duration::Quarter))
            .collect();
    }
    score.parts[0].staves[0].measures[0].barline_left = Barline::RepeatStart;
    score.parts[0].staves[0].measures[1].barline_right = Barline::RepeatEnd;
    score
}

pub fn request() -> InnerVoicesRequest {
    let chords = serde_json::from_value(serde_json::json!([
        {"measure_index":0,"offset_beats":0.0,"chord":{"root":"C","kind":"major"}},
        {"measure_index":0,"offset_beats":2.0,"chord":{"root":"F","kind":"major"}},
        {"measure_index":1,"offset_beats":0.0,"chord":{"root":"G","kind":"major"}}
    ]))
    .expect("fixed chord fixture");
    InnerVoicesRequest {
        chords,
        melody_part_index: 0,
        midi_program: 48,
        lower: InnerVoiceLine {
            pattern: InnerVoicePattern::Held,
            low_midi: 64,
            center_midi: 64,
            high_midi: 67,
            max_leap_semitones: 2,
        },
        upper: InnerVoiceLine {
            pattern: InnerVoicePattern::Held,
            low_midi: 71,
            center_midi: 72,
            high_midi: 79,
            max_leap_semitones: 7,
        },
        min_voice_gap_semitones: 4,
        min_melody_gap_semitones: 4,
        active_measures: vec![],
        phrase_end_measures: vec![],
    }
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
