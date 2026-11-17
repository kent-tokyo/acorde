use acorde_core::{
    CountermelodyDevelopmentKind, IntegratedArrangementRequest, MelodyAnticipationEdit, NoteAddr,
    PhraseMelodyRequest, Score,
};

#[allow(dead_code)]
#[path = "countermelody_development.rs"]
mod countermelody_development;
#[allow(dead_code)]
#[path = "rhythm_section_listening.rs"]
mod rhythm;

pub const FIXTURE: &str = include_str!("rhythm-section-listening.json");
pub use rhythm::Corpus;

pub fn input(
    case: &rhythm::Case,
) -> Result<(Score, IntegratedArrangementRequest), Box<dyn std::error::Error>> {
    let mut melody = rhythm::Case {
        id: format!("integrated-{}", case.id),
        tempo_bpm: case.tempo_bpm,
        melody: case.melody.clone(),
        chords: case.chords.clone(),
    };
    melody.melody[0][1] = 0;
    let (source, harmonies) = rhythm::harmony_input(&melody);
    let mut harmony_request = harmonies
        .into_iter()
        .find(|(label, _)| *label == "common-modulation")
        .ok_or("common modulation harmony missing")?
        .1;
    harmony_request.count = 3;
    let (_, rhythms) = rhythm::input(case)?;
    let selected_rhythms = rhythms
        .into_iter()
        .filter(|(label, _)| matches!(*label, "pulse" | "coordinated"))
        .map(|(_, request)| request)
        .collect::<Vec<_>>();
    let count = case.melody.len();
    let melodies = [
        CountermelodyDevelopmentKind::Imitation,
        CountermelodyDevelopmentKind::Inversion,
        CountermelodyDevelopmentKind::Answer,
    ]
    .into_iter()
    .map(|kind| {
        let mut countermelody = countermelody_development::request(kind);
        countermelody.base.midi_program = 0;
        countermelody.base.low_midi = 60;
        countermelody.base.center_midi = 67;
        countermelody.base.high_midi = 73;
        countermelody.base.phrase_end_measures = (3..count).step_by(4).collect();
        PhraseMelodyRequest {
            countermelody,
            countermelody_active_measures: (0..count).map(|measure| measure % 4 < 3).collect(),
            phrase_entry_max_leap_semitones: Some(12),
            start_measure: 0,
            end_measure_exclusive: count,
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
                early_ticks: 16,
            }],
        }
    })
    .collect();
    Ok((
        source,
        IntegratedArrangementRequest {
            harmony: harmony_request,
            rhythms: selected_rhythms,
            melodies,
            start_measure: 0,
            end_measure_exclusive: count,
            max_changed_harmonies: count as u8,
            max_rhythm_attacks: (count * 8) as u16,
            max_melody_anticipations: 1,
            count: 3,
            max_realizations: 12,
        },
    ))
}
