use acorde_core::*;
use serde::Deserialize;
#[allow(dead_code)]
#[path = "phrase_harmony_targets.rs"]
pub mod small;
pub const FIXTURE: &str = include_str!("harmony-targets-listening.json");
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
    pub chords: Vec<(String, String)>,
}
pub fn input(case: &Case) -> (Score, Vec<(&'static str, PhraseHarmonyTargetRequest)>) {
    let count = case.melody.len();
    let mut source = Score::new(&case.id, case.tempo_bpm, 4, 4, 0, count as u32);
    source.id = format!("harmony-targets-{}", case.id);
    source.parts[0].id = format!("{}-melody", case.id);
    source.parts[0].name = "Melody".into();
    for (bar, line) in case.melody.iter().enumerate() {
        let duration = match line.len() {
            1 => Duration::Whole,
            4 => Duration::Quarter,
            _ => Duration::Eighth,
        };
        source.parts[0].staves[0].measures[bar].voices[0] = line
            .iter()
            .enumerate()
            .map(|(n, &midi)| {
                let mut note = if midi == 0 {
                    Note::rest(duration.clone())
                } else {
                    Note::new(Pitch::from_midi(midi, false), duration.clone())
                };
                note.id = format!("{}-melody-{bar}-{n}", case.id);
                if bar == 0 && n == 0 {
                    note.dynamic = Some(Dynamic::Mf);
                }
                note
            })
            .collect();
    }
    let mut request = small::request();
    request.base.end_measure_exclusive = count;
    let template = request.base.chords.entries[0].clone();
    request.base.chords.entries = case
        .chords
        .iter()
        .enumerate()
        .map(|(m, (root, kind))| {
            let mut entry = template.clone();
            entry.position.measure_index = m;
            entry.chord.root = root.clone();
            entry.chord.kind = kind.clone();
            entry
        })
        .collect();
    request.base.high_midi = 72;
    request.max_changed_harmonies = count as u8;
    request.secondary_dominants = (0..count)
        .step_by(2)
        .map(|i| SecondaryDominantTarget {
            approach_entry: i,
            arrival_entry: i + 1,
        })
        .collect();
    let mut common = request.clone();
    common
        .secondary_dominants
        .retain(|t| t.approach_entry < count - 4);
    common.modulations = vec![PhraseModulationTarget {
        start_entry: count - 4,
        arrival_entry: count - 1,
        source_key: KeySignature {
            fifths: 0,
            mode: "major".into(),
        },
        destination_key: KeySignature {
            fifths: 1,
            mode: "major".into(),
        },
        approaches: vec![ModulationApproach::CommonChord],
    }];
    let mut dominant = common.clone();
    dominant.modulations[0].approaches = vec![ModulationApproach::Dominant];
    (
        source,
        vec![
            ("secondary", request),
            ("common-modulation", common),
            ("dominant-modulation", dominant),
        ],
    )
}
