use acorde_core::{
    ChordDegree, ChordSymbol, HarmonyExpansionError, HarmonyToneOrigin, HarmonyToneRole,
    expand_harmony,
};

fn chord(root: &str, kind: &str) -> ChordSymbol {
    serde_json::from_value(serde_json::json!({"root": root, "kind": kind})).unwrap()
}

fn degree(value: u8, alter: i8, kind: &str) -> ChordDegree {
    ChordDegree {
        value,
        alter,
        kind: kind.into(),
    }
}

#[test]
fn ninths_and_thirteenths_preserve_compound_degrees_and_the_eleventh() {
    for (root, kind, expected) in [
        (
            "C",
            "major-ninth",
            vec![
                (1, 0, "C", 0),
                (3, 4, "E", 0),
                (5, 7, "G", 0),
                (7, 11, "B", 0),
                (9, 14, "D", 0),
            ],
        ),
        (
            "D",
            "minor-ninth",
            vec![
                (1, 0, "D", 0),
                (3, 3, "F", 0),
                (5, 7, "A", 0),
                (7, 10, "C", 0),
                (9, 14, "E", 0),
            ],
        ),
        (
            "G",
            "dominant-13th",
            vec![
                (1, 0, "G", 0),
                (3, 4, "B", 0),
                (5, 7, "D", 0),
                (7, 10, "F", 0),
                (9, 14, "A", 0),
                (11, 17, "C", 0),
                (13, 21, "E", 0),
            ],
        ),
    ] {
        let expanded = expand_harmony(&chord(root, kind)).unwrap();
        let tones: Vec<_> = expanded
            .tones
            .iter()
            .map(|tone| {
                (
                    tone.degree,
                    tone.semitones_above_root,
                    tone.spelling.step.to_char().to_string(),
                    tone.spelling.alter,
                )
            })
            .collect();
        assert_eq!(
            tones,
            expected
                .into_iter()
                .map(|(d, s, n, a)| (d, s, n.into(), a))
                .collect::<Vec<_>>()
        );
        assert_eq!(
            expanded.tones.last().unwrap().role,
            HarmonyToneRole::Tension
        );
    }
}

#[test]
fn alterations_are_relative_to_the_kind_and_additions_to_dominant_intervals() {
    let mut major = chord("C", "major-seventh");
    major.degrees = vec![degree(7, -1, "alter")];
    let expanded = expand_harmony(&major).unwrap();
    let seventh = expanded.tones.iter().find(|tone| tone.degree == 7).unwrap();
    assert_eq!(seventh.semitones_above_root, 10);
    assert_eq!(seventh.spelling.alter, -1);
    assert_eq!(seventh.origin, HarmonyToneOrigin::Altered);

    let mut minor = chord("C", "minor");
    minor.degrees = vec![degree(3, 1, "alter"), degree(7, 0, "add")];
    let expanded = expand_harmony(&minor).unwrap();
    assert_eq!(
        expanded
            .tones
            .iter()
            .map(|tone| tone.semitones_above_root)
            .collect::<Vec<_>>(),
        vec![0, 4, 7, 10]
    );
}

#[test]
fn omission_and_simultaneous_altered_tensions_keep_their_roles() {
    let mut symbol = chord("G", "dominant");
    symbol.degrees = vec![
        degree(5, 0, "subtract"),
        degree(9, -1, "add"),
        degree(9, 1, "add"),
        degree(9, -1, "add"),
        degree(11, 1, "add"),
    ];
    let expanded = expand_harmony(&symbol).unwrap();
    assert_eq!(
        expanded
            .tones
            .iter()
            .map(|tone| (tone.degree, tone.semitones_above_root))
            .collect::<Vec<_>>(),
        vec![(1, 0), (3, 4), (7, 10), (9, 13), (9, 15), (11, 18)]
    );
    assert_eq!(expanded.tones[3].spelling.step.to_char(), 'A');
    assert_eq!(expanded.tones[3].spelling.alter, -1);
    assert_eq!(expanded.tones[4].spelling.step.to_char(), 'A');
    assert_eq!(expanded.tones[4].spelling.alter, 1);
    assert_eq!(expanded.tones[5].spelling.step.to_char(), 'C');
    assert_eq!(expanded.tones[5].spelling.alter, 1);
    symbol.degrees.reverse();
    assert_eq!(expand_harmony(&symbol).unwrap(), expanded);
}

#[test]
fn second_and_ninth_remain_distinct_and_slash_bass_is_separate() {
    let mut symbol = chord("C", "suspended-second");
    symbol.degrees = vec![degree(9, 0, "add")];
    symbol.bass = Some("F#".into());
    let expanded = expand_harmony(&symbol).unwrap();
    let second = &expanded.tones[1];
    let ninth = &expanded.tones[3];
    assert_eq!(second.spelling, ninth.spelling);
    assert_eq!(second.role, HarmonyToneRole::Suspension);
    assert_eq!(ninth.role, HarmonyToneRole::Tension);
    assert_eq!(second.pitch_at_root_octave(4).unwrap().to_midi(), 62);
    assert_eq!(ninth.pitch_at_root_octave(4).unwrap().to_midi(), 74);
    assert_eq!(expanded.bass.unwrap().pitch_class(), 6);
    assert!(
        expanded
            .tones
            .iter()
            .all(|tone| tone.spelling.pitch_class() != 6)
    );
}

#[test]
fn spelling_survives_double_accidentals_and_octave_crossings() {
    let expanded = expand_harmony(&chord("B#", "major-seventh")).unwrap();
    let names: Vec<_> = expanded
        .tones
        .iter()
        .map(|tone| (tone.spelling.step.to_char(), tone.spelling.alter))
        .collect();
    assert_eq!(names, vec![('B', 1), ('D', 2), ('F', 2), ('A', 2)]);
    assert_eq!(
        expanded
            .tones
            .iter()
            .map(|tone| tone.pitch_at_root_octave(3).unwrap().to_midi())
            .collect::<Vec<_>>(),
        vec![60, 64, 67, 71]
    );
    let diminished = expand_harmony(&chord("C", "diminished-seventh")).unwrap();
    assert_eq!(diminished.tones[3].spelling.step.to_char(), 'B');
    assert_eq!(diminished.tones[3].spelling.alter, -2);
    assert!(expanded.tones[0].pitch_at_root_octave(127).is_err());
    assert!(expanded.tones[0].pitch_at_root_octave(-127).is_err());
}

#[test]
fn explicit_root_omission_does_not_lose_the_harmonic_root() {
    let mut symbol = chord("D", "minor-ninth");
    symbol.degrees = vec![degree(1, 0, "subtract")];
    let expanded = expand_harmony(&symbol).unwrap();
    assert_eq!(expanded.root.pitch_class(), 2);
    assert_eq!(
        expanded
            .tones
            .iter()
            .map(|tone| tone.spelling.step.to_char())
            .collect::<Vec<_>>(),
        vec!['F', 'A', 'C', 'E']
    );
}

#[test]
fn invalid_or_conflicting_directives_fail_without_silent_clamping() {
    for root in ["", "Cjunk", "C#b", "C###", "H", "♯C"] {
        assert_eq!(
            expand_harmony(&chord(root, "major")),
            Err(HarmonyExpansionError::InvalidRoot)
        );
    }
    let mut symbol = chord("C", "major");
    for invalid in [
        degree(0, 0, "add"),
        degree(14, 0, "add"),
        degree(9, 3, "add"),
        degree(9, 0, "invent"),
    ] {
        symbol.degrees = vec![invalid];
        assert!(matches!(
            expand_harmony(&symbol),
            Err(HarmonyExpansionError::InvalidDegree { .. })
        ));
    }
    symbol.degrees = vec![degree(9, 1, "alter")];
    assert!(matches!(
        expand_harmony(&symbol),
        Err(HarmonyExpansionError::MissingDegree { degree: 9, .. })
    ));
    symbol.degrees = vec![degree(3, 0, "subtract"), degree(3, -1, "alter")];
    assert_eq!(
        expand_harmony(&symbol),
        Err(HarmonyExpansionError::ConflictingDegree { degree: 3 })
    );
    symbol.degrees = vec![degree(9, 0, "add"); 33];
    assert_eq!(
        expand_harmony(&symbol),
        Err(HarmonyExpansionError::TooManyDegrees)
    );
    symbol.degrees.clear();
    symbol.bass = Some("D!".into());
    assert_eq!(
        expand_harmony(&symbol),
        Err(HarmonyExpansionError::InvalidBass)
    );
    assert_eq!(
        expand_harmony(&chord("C", "unknown")),
        Err(HarmonyExpansionError::UnsupportedKind)
    );
    assert_eq!(
        expand_harmony(&chord("C", "other")),
        Err(HarmonyExpansionError::EmptyChord)
    );
}
