#[path = "harmony_arrangement_source.rs"]
pub mod arrangement;
use acorde_core::*;
pub fn source() -> Score {
    arrangement::source()
}
pub fn grace_source() -> Score {
    let mut score = source();
    for part in &mut score.parts {
        let bars = &mut part.staves[0].measures;
        bars[0].barline_left = Barline::RepeatStart;
        bars[1].barline_right = Barline::RepeatEnd;
    }
    let notes = &mut score.parts[0].staves[0].measures[0].voices[0];
    let mut grace = Note::new(Pitch::from_midi(77, false), Duration::Eighth);
    grace.is_grace = true;
    grace.grace_slash = true;
    notes.insert(0, grace);
    score
}
pub fn request() -> HarmonyGrooveRequest {
    let mut base = arrangement::request();
    base.root_policy = HarmonyRootPolicy::Include;
    for (entry, kind) in
        base.chords
            .entries
            .iter_mut()
            .zip(["minor-seventh", "dominant", "major-seventh"])
    {
        entry.chord.kind = kind.into();
    }
    HarmonyGrooveRequest {
        base,
        tensions: (0..3)
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
            .collect(),
        allow_rootless: true,
        allow_offbeats: true,
        anticipation: Some(HarmonyGrooveAnticipation {
            leads: vec![
                HarmonyAnticipationLead::Eighth,
                HarmonyAnticipationLead::Sixteenth,
            ],
            joins: vec![
                HarmonyAnticipationJoin::Tie,
                HarmonyAnticipationJoin::Reattack,
            ],
            max_entries: 2,
        }),
        rhythmic_accent: HarmonyCompingAccent::Anticipations,
        max_added_tensions_per_chord: 1,
        max_changed_harmonies: 3,
        count: 3,
        seed: 42,
        max_realizations: 128,
    }
}
pub fn sound(score: &Score) -> Vec<(usize, u8, u64, u64, u8)> {
    arrangement::sound(score)
}
