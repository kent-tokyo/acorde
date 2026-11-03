use acorde_core::model::standard_bass::{
    StandardBassChordButton, StandardBassNoteButton, StandardBassProfile, StandardBassRealization,
    StandardBassRequest, standard_bass_candidates, standard_bass_double_chord_candidates,
    standard_bass_request_for_chord,
};

fn profile() -> StandardBassProfile {
    StandardBassProfile {
        bass_buttons: vec![
            StandardBassNoteButton {
                id: "C bass".into(),
                position: 0,
                midi: 36,
            },
            StandardBassNoteButton {
                id: "F bass".into(),
                position: 1,
                midi: 41,
            },
            StandardBassNoteButton {
                id: "G bass".into(),
                position: 2,
                midi: 43,
            },
        ],
        chord_buttons: vec![
            StandardBassChordButton {
                id: "E minor".into(),
                position: 1,
                midi: vec![64, 67, 71],
            },
            StandardBassChordButton {
                id: "G major".into(),
                position: 2,
                midi: vec![67, 71, 74],
            },
            // This particular profile explicitly omits the fifth from G7.
            StandardBassChordButton {
                id: "G seventh".into(),
                position: 2,
                midi: vec![67, 71, 77],
            },
        ],
        max_distance: 2,
    }
}

fn request(pitches: &[u8], bass: u8) -> StandardBassRequest {
    StandardBassRequest {
        pitch_classes: pitches.into(),
        bass_pitch_class: bass,
        allow_omissions: false,
        allow_additions: false,
    }
}

#[test]
fn combinations_use_the_actual_bass_and_sounding_voicing() {
    let p = profile();
    let maj7 = standard_bass_candidates(&p, &request(&[0, 4, 7, 11], 0)).unwrap();
    assert_eq!(maj7.len(), 1);
    assert_eq!(
        (&*maj7[0].bass_button_id, &*maj7[0].chord_button_id),
        ("C bass", "E minor")
    );
    assert_eq!(maj7[0].chord_midi, vec![64, 67, 71]);
    assert!(maj7[0].requires_performer_review);
    // The omitted fifth is reported, not falsely advertised as a full G7.
    let mut seventh = request(&[2, 5, 7, 11], 7);
    assert!(standard_bass_candidates(&p, &seventh).unwrap().is_empty());
    seventh.allow_omissions = true;
    let g7 = standard_bass_candidates(&p, &seventh).unwrap();
    assert!(
        g7.iter()
            .any(|c| c.chord_button_id == "G seventh" && c.omitted_pitch_classes == [2])
    );
    // F bass + G major is G7/F, not G7 with G in the bass.
    let slash = standard_bass_candidates(&p, &request(&[2, 5, 7, 11], 5)).unwrap();
    assert!(
        slash
            .iter()
            .any(|c| c.chord_button_id == "G major" && c.bass_midi == 41)
    );
}

#[test]
fn bounds_and_explicit_permissions_are_enforced() {
    let mut p = profile();
    p.max_distance = 0;
    assert!(
        standard_bass_candidates(&p, &request(&[0, 4, 7, 11], 0))
            .unwrap()
            .is_empty()
    );
    p.max_distance = 2;
    let mut r = request(&[0, 4, 7], 0);
    assert!(standard_bass_candidates(&p, &r).unwrap().is_empty());
    r.allow_additions = true;
    let result = standard_bass_candidates(&p, &r).unwrap();
    assert!(result.iter().any(|c| c.added_pitch_classes == [11]));
    p.chord_buttons[0].midi = vec![35, 64];
    assert!(
        standard_bass_candidates(&p, &request(&[0, 4, 11], 0))
            .unwrap()
            .is_empty()
    );
    p.chord_buttons[0].id = p.bass_buttons[0].id.clone();
    assert!(standard_bass_candidates(&p, &r).is_err());
    assert!(standard_bass_candidates(&profile(), &request(&[12], 0)).is_err());
}

#[test]
fn minor_sevenths_and_instrument_inventory_are_not_guessed() {
    let p = StandardBassProfile {
        bass_buttons: vec![
            StandardBassNoteButton {
                id: "D".into(),
                position: 0,
                midi: 38,
            },
            StandardBassNoteButton {
                id: "A".into(),
                position: 1,
                midi: 45,
            },
            StandardBassNoteButton {
                id: "G".into(),
                position: 2,
                midi: 43,
            },
        ],
        chord_buttons: vec![
            StandardBassChordButton {
                id: "F major".into(),
                position: 0,
                midi: vec![65, 69, 72],
            },
            StandardBassChordButton {
                id: "C major".into(),
                position: 1,
                midi: vec![60, 64, 67],
            },
        ],
        max_distance: 1,
    };
    for (notes, bass, button) in [
        (&[2, 5, 9, 0][..], 2, "F major"),
        (&[9, 0, 4, 7][..], 9, "C major"),
    ] {
        let found = standard_bass_candidates(&p, &request(notes, bass)).unwrap();
        assert_eq!(found.len(), 1);
        assert_eq!(found[0].chord_button_id, button);
        assert!(found[0].omitted_pitch_classes.is_empty());
    }
    assert!(
        standard_bass_candidates(&p, &request(&[7, 11, 2, 5], 7))
            .unwrap()
            .is_empty()
    );
}

#[test]
fn results_are_stably_sorted_independent_of_inventory_order() {
    let mut p = profile();
    p.chord_buttons.push(StandardBassChordButton {
        id: "A E minor".into(),
        position: 1,
        midi: vec![64, 67, 71],
    });
    let expected = standard_bass_candidates(&p, &request(&[0, 4, 7, 11], 0)).unwrap();
    assert_eq!(
        expected
            .iter()
            .map(|c| c.chord_button_id.as_str())
            .collect::<Vec<_>>(),
        ["A E minor", "E minor"]
    );
    p.chord_buttons.reverse();
    p.bass_buttons.reverse();
    assert_eq!(
        standard_bass_candidates(&p, &request(&[0, 4, 7, 11], 0)).unwrap(),
        expected
    );
}

#[test]
fn chord_symbols_resolve_slash_bass_and_reject_unsupported_input() {
    let symbol: acorde_core::ChordSymbol =
        serde_json::from_str(r#"{"root":"G","kind":"dominant","bass":"F"}"#).unwrap();
    let req = standard_bass_request_for_chord(&symbol, false, false).unwrap();
    assert_eq!(req.bass_pitch_class, 5);
    assert_eq!(req.pitch_classes, [2, 5, 7, 11]);
    assert!(
        standard_bass_candidates(&profile(), &req)
            .unwrap()
            .iter()
            .any(|c| c.chord_button_id == "G major")
    );
    let mut unsupported = symbol.clone();
    unsupported.root = "G###".into();
    assert!(standard_bass_request_for_chord(&unsupported, false, false).is_err());
    unsupported = symbol;
    unsupported.degrees.push(acorde_core::ChordDegree {
        value: 9,
        alter: 0,
        kind: "add".into(),
    });
    assert!(standard_bass_request_for_chord(&unsupported, false, false).is_err());
}

#[test]
fn candidates_are_labelled_and_separate_direct_from_combined_buttons() {
    let p = profile();
    let maj7 = &standard_bass_candidates(&p, &request(&[0, 4, 7, 11], 0)).unwrap()[0];
    assert_eq!(maj7.label, "C bass + Em chord");
    assert_eq!(maj7.realization, StandardBassRealization::Combined);
    // G7/F is a G-major chord over an F bass, not a root-position G7.
    let mut slash = request(&[2, 5, 7, 11], 5);
    slash.allow_additions = true;
    slash.allow_omissions = true;
    let found = standard_bass_candidates(&p, &slash).unwrap();
    let g_over_f = found.iter().find(|c| c.bass_button_id == "F bass").unwrap();
    assert_eq!(g_over_f.label, "F bass + G chord");
    assert_eq!(g_over_f.realization, StandardBassRealization::Combined);
    let g = standard_bass_candidates(&p, &request(&[7, 11, 2], 7)).unwrap();
    assert_eq!(g[0].label, "G bass + G chord");
    assert_eq!(g[0].realization, StandardBassRealization::Direct);
}

#[test]
fn two_chord_buttons_reach_a_ninth_chord_no_single_button_gives() {
    let mut p = profile();
    p.chord_buttons.push(StandardBassChordButton {
        id: "D major".into(),
        position: 1,
        midi: vec![62, 66, 69],
    });
    // Cmaj9 without the fifth-heavy duplication: C E G B D needs Em + D.
    let nine = request(&[0, 2, 4, 7, 11], 0);
    assert!(standard_bass_candidates(&p, &nine).unwrap().is_empty());
    let mut loose = nine.clone();
    loose.allow_additions = true;
    let found = standard_bass_double_chord_candidates(&p, &loose).unwrap();
    assert!(found.iter().any(|c| {
        c.label == "C bass + Em chord + D chord" || c.label == "C bass + D chord + Em chord"
    }));
    assert!(found.iter().all(|c| c.requires_performer_review));
    // Deterministic and bounded.
    assert_eq!(
        found,
        standard_bass_double_chord_candidates(&p, &loose).unwrap()
    );
    assert!(found.len() <= 256);
}
