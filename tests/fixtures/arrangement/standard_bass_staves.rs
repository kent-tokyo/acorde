use acorde_core::{AccordionBassMode, AccordionPresetSelection, PositionedAccordionChord};

pub const EXAMPLES: [(&str, &str); 3] = [
    (
        "bass-pulse",
        "X:1\nM:4/4\nL:1/4\nK:C\nC D E G | A G E D | C E G A | G E D C |",
    ),
    (
        "waltz-bass",
        "X:1\nM:3/4\nL:1/4\nK:C\nC E G | A G E | F A c | G E C |",
    ),
    (
        "held-chord",
        "X:1\nM:4/4\nL:1/4\nK:C\ne e e e | c c c c | a a a a | b b b b |",
    ),
];

pub fn selection() -> AccordionPresetSelection {
    AccordionPresetSelection {
        chords: [
            ("C", "major"),
            ("A", "minor"),
            ("F", "major"),
            ("G", "major"),
        ]
        .into_iter()
        .enumerate()
        .map(|(measure_index, (root, kind))| PositionedAccordionChord {
            measure_index,
            offset_beats: 0.0,
            chord: serde_json::from_value(serde_json::json!({"root": root, "kind": kind}))
                .expect("fixture chord"),
        })
        .collect(),
        alternative_chords: None,
        inversion: None,
        bass_mode: AccordionBassMode::StandardBass,
    }
}
