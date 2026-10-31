#![cfg(all(feature = "abc", feature = "mscz"))]

use acorde_core::{Clef, Duration, Note, Pitch, Score};

#[test]
fn abc_bass_staff_clef_survives_mscz_roundtrip() {
    let abc = "X:1\nT:Bass clef round trip\nM:4/4\nL:1/4\nK:C\nV:1 clef=treble\nC4 |]\nV:2 clef=bass\nC,4 |]";
    let source = acorde_io::parse_abc(abc).expect("ABC parses");
    let before = staff_clefs(&source);
    assert_eq!(before, vec![vec![Clef::Treble], vec![Clef::Bass]]);
    let restored = mscz_roundtrip(&source);
    assert_eq!(staff_clefs(&restored), before);
    assert_eq!(staff_notes(&restored), staff_notes(&source));
}

#[test]
fn initial_clefs_without_measure_override_survive_multiple_parts_and_staves() {
    let mut score = Score::new("four clefs", 120, 4, 4, 0, 2);
    let first_staff = &mut score.parts[0].staves[0];
    first_staff.measures[0].voices[0] =
        vec![Note::new(Pitch::from_midi(60, false), Duration::Whole)];
    first_staff.measures[1].voices[0] =
        vec![Note::new(Pitch::from_midi(62, false), Duration::Whole)];
    let template = first_staff.clone();
    let mut second_part = score.parts[0].clone();
    score.parts[0].name = "upper".into();
    second_part.name = "lower".into();
    let mut bass = template.clone();
    bass.clef = Clef::Bass;
    let mut alto = template.clone();
    alto.clef = Clef::Alto;
    let mut tenor = template;
    tenor.clef = Clef::Tenor;
    score.parts[0].staves.push(bass);
    second_part.staves = vec![alto, tenor];
    score.parts.push(second_part);
    assert!(
        score
            .parts
            .iter()
            .flat_map(|part| &part.staves)
            .all(|staff| staff.measures[0].clef.is_none())
    );

    let before = staff_clefs(&score);
    assert_eq!(
        before,
        vec![
            vec![Clef::Treble, Clef::Bass],
            vec![Clef::Alto, Clef::Tenor]
        ]
    );
    let restored = mscz_roundtrip(&score);
    assert_eq!(staff_clefs(&restored), before);
    assert_eq!(staff_notes(&restored), staff_notes(&score));
}

fn mscz_roundtrip(score: &Score) -> Score {
    let bytes = acorde_io::serialize_mscz(score).expect("MSCZ serializes");
    acorde_io::parse_mscz(&bytes).expect("MSCZ parses")
}

fn staff_clefs(score: &Score) -> Vec<Vec<Clef>> {
    score
        .parts
        .iter()
        .map(|part| part.staves.iter().map(|staff| staff.clef.clone()).collect())
        .collect()
}

fn staff_notes(score: &Score) -> Vec<Vec<Vec<Vec<Vec<i16>>>>> {
    score
        .parts
        .iter()
        .map(|part| {
            part.staves
                .iter()
                .map(|staff| {
                    staff
                        .measures
                        .iter()
                        .map(|measure| {
                            measure
                                .voices
                                .iter()
                                .map(|voice| {
                                    voice
                                        .iter()
                                        .flat_map(|note| note.pitches.iter().map(Pitch::to_midi))
                                        .collect()
                                })
                                .collect()
                        })
                        .collect()
                })
                .collect()
        })
        .collect()
}
