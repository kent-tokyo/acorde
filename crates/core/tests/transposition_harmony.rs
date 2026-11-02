use acorde_core::{
    ChordSymbol, Clef, Duration, KeySignature, Note, Part, Pitch, Score, Staff, Step,
    transpose_checked,
};

fn chord(root: &str, bass: Option<&str>) -> ChordSymbol {
    serde_json::from_value(serde_json::json!({"root":root,"kind":"major-seventh","bass":bass}))
        .unwrap()
}
fn fixture() -> Score {
    let mut s = Score::new("transposition", 96, 4, 4, 0, 2);
    let mut n = Note::new(Pitch::new(Step::C, 4), Duration::Whole);
    n.chord_symbol = Some(chord("C", Some("E")));
    s.parts[0].staves[0].measures[0].voices[0] = vec![n];
    let mut rest = Note::rest(Duration::Whole);
    rest.chord_symbol = Some(chord("Bb", Some("D")));
    s.parts[0].staves[0].measures[1].voices[0] = vec![rest];
    s.parts[0].staves[0].measures[1].key_sig = Some(KeySignature {
        fifths: -1,
        mode: "major".into(),
    });
    s
}

#[test]
fn updates_roots_slash_bass_rest_harmony_and_local_keys_without_mutating_source() {
    let source = fixture();
    let before = serde_json::to_string(&source).unwrap();
    let out = transpose_checked(&source, 2).unwrap();
    assert_eq!(out.settings.key_signature.fifths, 2);
    let bars = &out.parts[0].staves[0].measures;
    let first = &bars[0].voices[0][0];
    assert_eq!(first.pitches[0], Pitch::new(Step::D, 4));
    assert_eq!(first.chord_symbol.as_ref().unwrap().root, "D");
    assert_eq!(
        first.chord_symbol.as_ref().unwrap().bass.as_deref(),
        Some("F#")
    );
    assert_eq!(first.chord_symbol.as_ref().unwrap().kind, "major-seventh");
    assert_eq!(
        first.id,
        source.parts[0].staves[0].measures[0].voices[0][0].id
    );
    assert_eq!(bars[1].key_sig.as_ref().unwrap().fifths, 1);
    assert_eq!(
        bars[1].voices[0][0].chord_symbol.as_ref().unwrap().root,
        "C"
    );
    assert!(bars[1].voices[0][0].is_rest);
    assert_eq!(serde_json::to_string(&source).unwrap(), before);
}

#[test]
fn follows_destination_key_spelling_and_preserves_octave_spelling() {
    let mut source = fixture();
    let flat = transpose_checked(&source, 3).unwrap();
    assert_eq!(
        flat.parts[0].staves[0].measures[0].voices[0][0].pitches[0],
        Pitch::with_alter(Step::E, 4, -1)
    );
    source.settings.key_signature.fifths = 4; // E major -> F-sharp major
    source.parts[0].staves[0].measures[0].voices[0][0].pitches[0] =
        Pitch::with_alter(Step::D, 4, 1);
    source.parts[0].staves[0].measures[0].voices[0][0].chord_symbol = Some(chord("D#", None));
    let sharp = transpose_checked(&source, 2).unwrap();
    assert_eq!(
        sharp.parts[0].staves[0].measures[0].voices[0][0].pitches[0],
        Pitch::with_alter(Step::E, 4, 1)
    );
    assert_eq!(
        sharp.parts[0].staves[0].measures[0].voices[0][0]
            .chord_symbol
            .as_ref()
            .unwrap()
            .root,
        "E#"
    );
    source.settings.key_signature.fifths = 7;
    for amount in [-12, 12] {
        let out = transpose_checked(&source, amount).unwrap();
        assert_eq!(out.settings.key_signature.fifths, 7);
        assert_eq!(
            out.parts[0].staves[0].measures[0].voices[0][0].pitches[0].step,
            Step::D
        );
        assert_eq!(
            out.parts[0].staves[0].measures[0].voices[0][0].chord_symbol,
            source.parts[0].staves[0].measures[0].voices[0][0].chord_symbol
        );
    }
}

#[test]
fn all_keys_and_semitones_preserve_exact_pitch_delta_and_note_timing() {
    for key in -7..=7 {
        let mut s = Score::new("keys", 96, 4, 4, key, 1);
        let n = &mut s.parts[0].staves[0].measures[0].voices[0];
        *n = (60..72)
            .map(|m| Note::new(Pitch::from_midi(m, key < 0), Duration::Sixteenth))
            .collect();
        for amount in -12..=12 {
            let out = transpose_checked(&s, amount).unwrap();
            for (old, new) in s.parts[0].staves[0].measures[0].voices[0]
                .iter()
                .zip(&out.parts[0].staves[0].measures[0].voices[0])
            {
                assert_eq!(
                    new.pitches[0].to_midi(),
                    old.pitches[0].to_midi() + i16::from(amount),
                    "key {key}, shift {amount}"
                );
                assert_eq!(old.beats(), new.beats());
                assert_eq!(old.id, new.id);
            }
        }
    }
}

#[test]
fn checked_transposition_rejects_pitch_clipping_and_preserves_unpitched_drums() {
    let mut s = fixture();
    s.parts[0].staves[0].measures[0].voices[0][0].pitches[0] = Pitch::from_midi(127, false);
    assert!(transpose_checked(&s, 1).is_err());
    s.parts[0].staves[0].measures[0].voices[0][0].pitches[0] = Pitch::from_midi(0, false);
    assert!(transpose_checked(&s, -1).is_err());
    let mut drums = Part::new("Drums", "Dr.");
    drums.midi_channel = 9;
    drums.staves.push(Staff::new(Clef::Percussion));
    drums.staves[0].measures = s.parts[0].staves[0].measures.clone();
    s.parts[0].staves[0].measures[0].voices[0][0].pitches[0] = Pitch::new(Step::C, 4);
    s.parts.push(drums);
    let out = transpose_checked(&s, -2).unwrap();
    assert_eq!(
        serde_json::to_value(&out.parts[1]).unwrap(),
        serde_json::to_value(&s.parts[1]).unwrap()
    );
}

#[test]
fn minor_mode_and_raised_leading_tone_move_with_slash_chord() {
    let mut source = fixture();
    source.settings.key_signature.mode = "minor".into();
    let n = &mut source.parts[0].staves[0].measures[0].voices[0][0];
    n.pitches[0] = Pitch::with_alter(Step::G, 4, 1);
    n.chord_symbol = Some(chord("E", Some("G#")));
    n.chord_symbol.as_mut().unwrap().kind = "dominant".into();
    let out = transpose_checked(&source, -2).unwrap();
    assert_eq!(out.settings.key_signature.mode, "minor");
    assert_eq!(out.settings.key_signature.fifths, -2);
    let n = &out.parts[0].staves[0].measures[0].voices[0][0];
    assert_eq!(n.pitches[0], Pitch::with_alter(Step::F, 4, 1));
    assert_eq!(n.chord_symbol.as_ref().unwrap().root, "D");
    assert_eq!(n.chord_symbol.as_ref().unwrap().bass.as_deref(), Some("F#"));
}
