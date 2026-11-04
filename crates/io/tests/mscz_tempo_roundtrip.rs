#![cfg(all(feature = "abc", feature = "mscz"))]

use acorde_core::Score;

fn mscz_roundtrip(score: &Score) -> Score {
    let bytes = acorde_io::serialize_mscz(score).expect("MSCZ serializes");
    acorde_io::parse_mscz(&bytes).expect("MSCZ parses")
}

fn timeline(score: &Score) -> Vec<(u8, u64, u64)> {
    acorde_core::to_playback_events(score, &Default::default())
        .into_iter()
        .map(|e| {
            (
                e.pitch_midi,
                (e.time_secs * 1e6).round() as u64,
                (e.duration_secs * 1e6).round() as u64,
            )
        })
        .collect()
}

fn chord_names(score: &Score) -> Vec<(String, String, Option<String>)> {
    score.parts[0].staves[0]
        .measures
        .iter()
        .flat_map(|m| &m.voices[0])
        .filter_map(|n| {
            n.chord_symbol
                .as_ref()
                .map(|c| (c.root.clone(), c.kind.clone(), c.bass.clone()))
        })
        .collect()
}

const ABC: &str = "X:1\nT:Tempo test\nM:4/4\nL:1/4\nQ:1/4=100\nK:C\n\"C\"C4|\"G7\"G2 \"F\"F2|]";

#[test]
fn score_tempo_without_measure_tempo_survives_mscz() {
    let source = acorde_io::parse_abc(ABC).expect("ABC parses");
    assert_eq!(source.settings.tempo_bpm, 100);
    for score in [source.clone(), acorde_core::transpose(&source, 2)] {
        let restored = mscz_roundtrip(&score);
        assert_eq!(restored.settings.tempo_bpm, 100);
        // The score tempo stays a setting rather than becoming a written first-bar change.
        assert_eq!(restored.parts[0].staves[0].measures[0].tempo, None);
        assert_eq!(timeline(&restored), timeline(&score));
        assert_eq!(chord_names(&restored), chord_names(&score));
    }
}

#[test]
fn measure_tempos_win_without_extra_changes() {
    let mut score = acorde_io::parse_abc(ABC).expect("ABC parses");
    // A later change keeps the opening score tempo and adds no other mark.
    score.parts[0].staves[0].measures[1].tempo = Some(80);
    let xml = acorde_io::serialize_mscx(&score).expect("MSCX serializes");
    assert_eq!(xml.matches("<Tempo>").count(), 2);
    let restored = mscz_roundtrip(&score);
    assert_eq!(restored.settings.tempo_bpm, 100);
    assert_eq!(timeline(&restored), timeline(&score));
    // An opening bar tempo is the one tempo written for bar one.
    score.parts[0].staves[0].measures[0].tempo = Some(90);
    let xml = acorde_io::serialize_mscx(&score).expect("MSCX serializes");
    assert_eq!(xml.matches("<Tempo>").count(), 2);
    let restored = mscz_roundtrip(&score);
    assert_eq!(restored.settings.tempo_bpm, 90);
    assert_eq!(timeline(&restored), timeline(&score));
}
