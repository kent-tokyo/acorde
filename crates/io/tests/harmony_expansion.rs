#![cfg(any(feature = "musicxml", feature = "mscz"))]

use acorde_core::{ChordSymbol, Duration, Note, Score, expand_harmony, to_playback_events};

fn sound(score: &Score) -> Vec<(u8, u64, u64)> {
    to_playback_events(score, &Default::default())
        .into_iter()
        .filter(|event| !event.is_metronome)
        .map(|event| {
            (
                event.pitch_midi,
                (event.time_beats * 480.0).round() as u64,
                (event.duration_beats * 480.0).round() as u64,
            )
        })
        .collect()
}

fn roundtrip(check: impl Fn(&Score) -> Score) {
    let corpus: serde_json::Value = serde_json::from_str(include_str!(
        "../../../tests/fixtures/arrangement/harmony-expansion.json"
    ))
    .unwrap();
    for case in corpus["cases"].as_array().unwrap() {
        let chord: ChordSymbol = serde_json::from_value(case["chord"].clone()).unwrap();
        let expected = expand_harmony(&chord).unwrap();
        let pitches = expected
            .tones
            .iter()
            .map(|tone| tone.pitch_at_root_octave(3).unwrap())
            .collect::<Vec<_>>();
        assert_eq!(
            serde_json::to_value(
                pitches
                    .iter()
                    .map(|pitch| pitch.to_midi())
                    .collect::<Vec<_>>()
            )
            .unwrap(),
            case["midi"]
        );
        let mut source = Score::new("harmony spelling", 120, 4, 4, 0, 1);
        let mut note = Note::new(pitches[0].clone(), Duration::Whole);
        note.pitches = pitches.clone();
        note.chord_symbol = Some(chord.clone());
        source.parts[0].staves[0].measures[0].voices[0] = vec![note];
        let restored = check(&source);
        assert!(
            acorde_core::validate(&restored).is_valid(),
            "{}",
            case["id"]
        );
        let restored_note = &restored.parts[0].staves[0].measures[0].voices[0][0];
        assert_eq!(restored_note.pitches, pitches, "{}", case["id"]);
        let symbol = restored_note.chord_symbol.as_ref().unwrap();
        assert_eq!(symbol.root, chord.root);
        assert_eq!(symbol.kind, chord.kind);
        assert_eq!(symbol.degrees, chord.degrees);
        assert_eq!(symbol.bass, chord.bass);
        assert_eq!(expand_harmony(symbol).unwrap(), expected, "{}", case["id"]);
        assert_eq!(sound(&restored), sound(&source), "{}", case["id"]);
    }
}

#[cfg(feature = "musicxml")]
#[test]
fn expanded_harmony_musicxml_roundtrip_preserves_spelling_degrees_and_playback() {
    roundtrip(|score| {
        acorde_io::parse_musicxml(&acorde_io::serialize_musicxml(score).unwrap()).unwrap()
    });
}

#[cfg(feature = "mscz")]
#[test]
fn expanded_harmony_mscz_roundtrip_preserves_spelling_degrees_and_playback() {
    roundtrip(|score| acorde_io::parse_mscz(&acorde_io::serialize_mscz(score).unwrap()).unwrap());
}

#[cfg(feature = "mscz")]
#[test]
fn compact_flat_five_alias_retains_the_harmonic_meaning() {
    let mut source = Score::new("alias", 120, 4, 4, 0, 1);
    let chord: ChordSymbol = serde_json::from_value(serde_json::json!({
        "root":"G", "kind":"dominant",
        "degrees":[{"value":5,"alter":-1,"kind":"alter"}]
    }))
    .unwrap();
    let mut note = Note::new(acorde_core::Pitch::from_midi(72, false), Duration::Whole);
    note.chord_symbol = Some(chord.clone());
    source.parts[0].staves[0].measures[0].voices[0] = vec![note];
    let restored = acorde_io::parse_mscz(&acorde_io::serialize_mscz(&source).unwrap()).unwrap();
    let recovered = restored.parts[0].staves[0].measures[0].voices[0][0]
        .chord_symbol
        .as_ref()
        .unwrap();
    let tones = |symbol: &ChordSymbol| {
        expand_harmony(symbol)
            .unwrap()
            .tones
            .into_iter()
            .map(|tone| (tone.degree, tone.spelling, tone.role))
            .collect::<Vec<_>>()
    };
    assert_eq!(tones(recovered), tones(&chord));
    assert_eq!(recovered.kind, "dominant-flat-five");
}
