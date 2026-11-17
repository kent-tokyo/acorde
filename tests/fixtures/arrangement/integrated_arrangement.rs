use acorde_core::{
    CountermelodyDevelopmentKind, IntegratedArrangementRequest, MelodyAnticipationEdit, NoteAddr,
    PhraseMelodyRequest, Score,
};

#[allow(dead_code)]
#[path = "phrase_harmony_targets.rs"]
mod harmony;
#[allow(dead_code)]
#[path = "countermelody_development.rs"]
mod motif;
#[allow(dead_code)]
#[path = "rhythm_section.rs"]
mod rhythm;

pub fn source() -> Score {
    motif::source()
}

pub fn request() -> IntegratedArrangementRequest {
    let steady = rhythm::request(false);
    let mut sparse = steady.clone();
    for bar in &mut sparse.bars {
        bar.slots[2].kick = None;
        bar.slots[3].bass = None;
    }
    let melodies = [
        CountermelodyDevelopmentKind::Imitation,
        CountermelodyDevelopmentKind::Inversion,
        CountermelodyDevelopmentKind::Answer,
    ]
    .into_iter()
    .map(|kind| PhraseMelodyRequest {
        countermelody: motif::request(kind),
        countermelody_active_measures: vec![true, false, false, false],
        phrase_entry_max_leap_semitones: Some(12),
        start_measure: 0,
        end_measure_exclusive: 4,
        locked_measures: vec![],
        anticipations: vec![MelodyAnticipationEdit {
            note: NoteAddr {
                part: 0,
                staff: 0,
                measure: 0,
                voice: 0,
                note: 2,
            },
            preceding_rest: NoteAddr {
                part: 0,
                staff: 0,
                measure: 0,
                voice: 0,
                note: 1,
            },
            early_ticks: 8,
        }],
    })
    .collect();
    IntegratedArrangementRequest {
        harmony: harmony::request(),
        rhythms: vec![steady, sparse],
        melodies,
        start_measure: 0,
        end_measure_exclusive: 4,
        max_changed_harmonies: 4,
        max_rhythm_attacks: 64,
        max_melody_anticipations: 1,
        count: 3,
        max_realizations: 6,
    }
}
