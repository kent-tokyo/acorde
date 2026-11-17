use acorde_core::{
    Barline, Clef, Duration, KeySignature, MeasureLength, Note, Part, Pitch, Score, Staff,
};
use serde::{Deserialize, Serialize};

pub const FIXTURE: &str = include_str!("development-v2.json");

#[derive(Deserialize)]
#[serde(deny_unknown_fields)]
pub struct Corpus {
    pub schema_version: u16,
    pub cases: Vec<Case>,
}

#[derive(Deserialize)]
#[serde(rename_all = "lowercase")]
pub enum Unit {
    Quarter,
    Eighth,
}

#[derive(Deserialize, Serialize)]
#[serde(deny_unknown_fields)]
pub struct Section {
    pub label: String,
    pub start_measure: usize,
    pub end_measure_exclusive: usize,
}

#[derive(Deserialize)]
#[serde(deny_unknown_fields)]
pub struct Case {
    pub id: String,
    pub title: String,
    pub meter: [u8; 2],
    pub unit: Unit,
    #[serde(default)]
    pub pickup_eighths: Option<usize>,
    pub key_fifths: i8,
    pub mode: String,
    pub tempo_bpm: u16,
    pub melody_program: u8,
    #[serde(default)]
    pub second_program: Option<u8>,
    pub form: String,
    pub sections: Vec<Section>,
    #[serde(default)]
    pub key_changes: Vec<(usize, i8)>,
    #[serde(default)]
    pub repeat_range: Option<[usize; 2]>,
    pub melody: Vec<Vec<u8>>,
    #[serde(default)]
    pub second_line: Option<Vec<Vec<u8>>>,
}

fn voice(pitches: &[u8], duration: Duration, id_prefix: &str) -> Vec<Note> {
    pitches
        .iter()
        .enumerate()
        .map(|(index, pitch)| {
            let mut note = if *pitch == 0 {
                Note::rest(duration.clone())
            } else {
                Note::new(Pitch::from_midi(*pitch, false), duration.clone())
            };
            note.id = format!("{id_prefix}-{index}");
            note
        })
        .collect()
}

pub fn score(case: &Case) -> Score {
    let mut score = Score::new(
        &case.title,
        case.tempo_bpm,
        case.meter[0],
        case.meter[1],
        case.key_fifths,
        case.melody.len() as u32,
    );
    score.id = format!("arrangement-development-v2-{}", case.id);
    score.settings.key_signature.mode = case.mode.clone();
    score.parts[0].id = format!("{}-melody", case.id);
    score.parts[0].name = "Melody".into();
    score.parts[0].midi_program = case.melody_program;
    for (measure, pitches) in case.melody.iter().enumerate() {
        let duration = if measure == 0 && case.pickup_eighths.is_some() {
            Duration::Eighth
        } else {
            match case.unit {
                Unit::Quarter => Duration::Quarter,
                Unit::Eighth => Duration::Eighth,
            }
        };
        score.parts[0].staves[0].measures[measure].voices[0] =
            voice(pitches, duration, &format!("{}-melody-{measure}", case.id));
    }
    if let Some(units) = case.pickup_eighths {
        score.parts[0].staves[0].measures[0].actual_length =
            MeasureLength::from_beats(units as f64 / 2.0);
    }
    for &(measure, fifths) in &case.key_changes {
        score.parts[0].staves[0].measures[measure].key_sig = Some(KeySignature {
            fifths,
            mode: case.mode.clone(),
        });
    }
    if let Some([start, end]) = case.repeat_range {
        score.parts[0].staves[0].measures[start].barline_left = Barline::RepeatStart;
        score.parts[0].staves[0].measures[end - 1].barline_right = Barline::RepeatEnd;
    }
    if let (Some(program), Some(line)) = (case.second_program, &case.second_line) {
        let mut part = Part::new("Second voice", "II");
        part.id = format!("{}-second", case.id);
        part.midi_channel = 1;
        part.midi_program = program;
        let mut staff = Staff::new(Clef::Treble);
        for (measure, pitches) in line.iter().enumerate() {
            let mut bar = score.parts[0].staves[0].measures[measure].clone();
            bar.voices[0] = voice(
                pitches,
                match case.unit {
                    Unit::Quarter => Duration::Quarter,
                    Unit::Eighth => Duration::Eighth,
                },
                &format!("{}-second-{measure}", case.id),
            );
            staff.measures.push(bar);
        }
        part.staves.push(staff);
        score.parts.push(part);
    }
    score
}
