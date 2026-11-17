use acorde_core::*;
use serde::Deserialize;
#[allow(dead_code)]
#[path = "harmony_groove_source.rs"]
mod small;
pub const FIXTURE: &str = include_str!("harmony-groove-listening.json");
#[derive(Deserialize)]
#[serde(deny_unknown_fields)]
pub struct Corpus {
    pub schema_version: u16,
    pub cases: Vec<Case>,
}
#[derive(Deserialize)]
#[serde(deny_unknown_fields)]
pub struct Case {
    pub id: String,
    pub tempo_bpm: u16,
    pub melody: Vec<Vec<u8>>,
    pub chords: Vec<(String, String, u8)>,
}
pub fn input(case: &Case) -> (Score, HarmonyGrooveRequest) {
    let mut score = Score::new(&case.id, case.tempo_bpm, 4, 4, 0, case.melody.len() as u32);
    score.id = format!("harmony-groove-{}", case.id);
    score.parts[0].id = format!("{}-melody", case.id);
    score.parts[0].name = "Melody".into();
    for (bar, line) in case.melody.iter().enumerate() {
        let duration = match line.len() {
            1 => Duration::Whole,
            4 => Duration::Quarter,
            _ => Duration::Eighth,
        };
        score.parts[0].staves[0].measures[bar].voices[0] = line
            .iter()
            .enumerate()
            .map(|(index, &pitch)| {
                let mut note = if pitch == 0 {
                    Note::rest(duration.clone())
                } else {
                    Note::new(Pitch::from_midi(pitch, false), duration.clone())
                };
                note.id = format!("{}-melody-{bar}-{index}", case.id);
                if bar == 0 && index == 0 {
                    note.dynamic = Some(Dynamic::Mf);
                }
                note
            })
            .collect();
    }
    let mut bass = Part::new("Bass", "B.");
    bass.id = format!("{}-bass", case.id);
    bass.midi_channel = 1;
    bass.midi_program = 0;
    let mut staff = score.parts[0].staves[0].clone();
    staff.clef = Clef::Bass;
    for (bar, (_, _, pitch)) in case.chords.iter().enumerate() {
        let mut note = Note::new(Pitch::from_midi(*pitch, false), Duration::Whole);
        note.id = format!("{}-bass-{bar}", case.id);
        staff.measures[bar].voices[0] = vec![note];
    }
    bass.staves.push(staff);
    score.parts.push(bass);
    let mut request = small::request();
    request.base.end_measure_exclusive = case.melody.len();
    let template = request.base.chords.entries[0].clone();
    request.base.chords.entries = case
        .chords
        .iter()
        .enumerate()
        .map(|(bar, (root, kind, _))| {
            let mut entry = template.clone();
            entry.position.measure_index = bar;
            entry.chord.root = root.clone();
            entry.chord.kind = kind.clone();
            entry
        })
        .collect();
    request.base.high_midi = 72;
    request.base.comping.max_attacks_per_measure = 4;
    request.max_changed_harmonies = 8;
    request.tensions = (0..case.chords.len())
        .map(|chord_entry| HarmonyTensionAllowance {
            chord_entry,
            tones: vec![
                HarmonyTension {
                    degree: 9,
                    alter: 0,
                },
                HarmonyTension {
                    degree: 13,
                    alter: 0,
                },
            ],
        })
        .collect();
    (score, request)
}
