//! Hand-built SATB chorale fixtures shared by the render-svg integration tests.
//!
//! These are NOT derived from mokuren — acorde-render-svg does not depend on mokuren.
//! They approximate the shape of what mokuren's harmonizer might emit (SATB across a
//! treble/bass grand staff, one part, two voices per staff) purely as acorde-core `Score`
//! values, so the renderer can be exercised without any external crate.
#![allow(dead_code)]

use acorde_core::{
    Clef, Duration, KeySignature, Measure, Note, Part, Pitch, Score, Staff, Step, TimeSignature,
};

fn note(step: Step, octave: i8, dur: Duration) -> Note {
    Note::new(Pitch::new(step, octave), dur)
}

fn quarter(step: Step, octave: i8) -> Note {
    note(step, octave, Duration::Quarter)
}

fn altered(step: Step, octave: i8, dur: Duration, alter: i8) -> Note {
    let mut n = note(step, octave, dur);
    n.pitches[0].alter = alter;
    n
}

/// Build a single-part, two-staff (treble/bass) SATB score from four voice lines, each a
/// `Vec<Note>` for one measure. `soprano`/`alto` go on the treble staff (voices 0/1);
/// `tenor`/`bass` go on the bass staff (voices 0/1).
fn satb_score(
    fifths: i8,
    mode: &str,
    soprano: Vec<Note>,
    alto: Vec<Note>,
    tenor: Vec<Note>,
    bass: Vec<Note>,
) -> Score {
    let mut score = Score::default();
    score.settings.time_signature = TimeSignature { numerator: 4, denominator: 4 };
    score.settings.key_signature = KeySignature { fifths, mode: mode.to_string() };

    let mut part = Part::new("Chorale", "Ch.");

    let mut treble = Staff::new(Clef::Treble);
    let mut m_treble = Measure::empty(4, 4);
    m_treble.number = 1;
    m_treble.voices[0] = soprano;
    m_treble.voices[1] = alto;
    treble.measures.push(m_treble);

    let mut bass_staff = Staff::new(Clef::Bass);
    let mut m_bass = Measure::empty(4, 4);
    m_bass.number = 1;
    m_bass.voices[0] = tenor;
    m_bass.voices[1] = bass;
    bass_staff.measures.push(m_bass);

    part.staves.push(treble);
    part.staves.push(bass_staff);
    score.parts = vec![part];
    score
}

/// C major, I - ii6 - V7 - I, quarter notes throughout. No accidentals.
pub fn satb_major() -> Score {
    satb_score(
        0, "major",
        vec![quarter(Step::C, 5), quarter(Step::D, 5), quarter(Step::B, 4), quarter(Step::C, 5)],
        vec![quarter(Step::G, 4), quarter(Step::F, 4), quarter(Step::G, 4), quarter(Step::E, 4)],
        vec![quarter(Step::E, 4), quarter(Step::D, 4), quarter(Step::D, 4), quarter(Step::C, 4)],
        vec![quarter(Step::C, 3), quarter(Step::G, 2), quarter(Step::G, 2), quarter(Step::C, 3)],
    )
}

/// E minor (fifths=1, one sharp in the key signature), i - iv - V - i with a raised leading
/// tone (D#) on the dominant chord — exercises a non-zero key signature plus a mandatory
/// accidental beyond what the key signature implies.
pub fn satb_minor() -> Score {
    satb_score(
        1, "minor",
        vec![quarter(Step::E, 5), quarter(Step::E, 5), altered(Step::D, 5, Duration::Quarter, 1), quarter(Step::E, 5)],
        vec![quarter(Step::B, 4), quarter(Step::A, 4), quarter(Step::B, 4), quarter(Step::B, 4)],
        vec![quarter(Step::G, 4), quarter(Step::A, 4), quarter(Step::F, 4), quarter(Step::G, 4)],
        vec![quarter(Step::E, 3), quarter(Step::A, 2), quarter(Step::B, 2), quarter(Step::E, 3)],
    )
}

/// C major with a secondary dominant (V7/V, built on D) resolving to V (G major) —
/// exercises a chromatic accidental (F#) that is not implied by the key signature.
pub fn satb_secondary_dominant() -> Score {
    satb_score(
        0, "major",
        vec![altered(Step::F, 5, Duration::Quarter, 1), quarter(Step::G, 5), quarter(Step::D, 5), quarter(Step::D, 5)],
        vec![quarter(Step::D, 5), quarter(Step::D, 5), quarter(Step::B, 4), quarter(Step::B, 4)],
        vec![quarter(Step::A, 4), quarter(Step::G, 4), quarter(Step::G, 4), quarter(Step::G, 4)],
        vec![quarter(Step::D, 3), quarter(Step::G, 2), quarter(Step::G, 2), quarter(Step::G, 2)],
    )
}

/// C major with dotted rhythm (dotted quarter + eighth) in the soprano and a rest in the
/// alto — exercises augmentation dots, eighth-note flags, and rest glyphs.
pub fn satb_dotted_and_rest() -> Score {
    let mut dq1 = note(Step::G, 4, Duration::Quarter);
    dq1.dot_count = 1;
    let e1 = note(Step::A, 4, Duration::Eighth);
    let mut dq2 = note(Step::B, 4, Duration::Quarter);
    dq2.dot_count = 1;
    let e2 = note(Step::C, 5, Duration::Eighth);

    let mut dotted_half = note(Step::E, 4, Duration::Half);
    dotted_half.dot_count = 1;
    let rest = Note::rest(Duration::Quarter);

    satb_score(
        0, "major",
        vec![dq1, e1, dq2, e2],
        vec![dotted_half, rest],
        vec![quarter(Step::C, 4), quarter(Step::C, 4), quarter(Step::C, 4), quarter(Step::C, 4)],
        vec![quarter(Step::C, 3), quarter(Step::C, 3), quarter(Step::C, 3), quarter(Step::C, 3)],
    )
}
