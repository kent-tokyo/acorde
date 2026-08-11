//! Hand-built SATB chorale fixtures shared by the render-svg integration tests.
//!
//! These are NOT derived from mokuren — acorde-render-svg does not depend on mokuren.
//! They approximate the shape of what mokuren's harmonizer might emit (SATB across a
//! treble/bass grand staff, one part, two voices per staff) purely as acorde-core `Score`
//! values, so the renderer can be exercised without any external crate.
#![allow(dead_code)]

use acorde_core::{
    compute_beams, Clef, Duration, KeySignature, Measure, Note, Part, Pitch, Score, Staff, Step, TimeSignature, TupletInfo,
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

// ── single-staff visual-regression fixtures ──────────────────────────────────────
//
// Smaller, single-concern fixtures (one staff, C major, 4/4 unless noted) for the
// visual-regression suite (`tests/visual_regression.rs`). Each isolates one notation
// category so a golden-file diff or a geometry assertion failure points at one cause.

/// Build a single-part, single-staff score with up to 2 voices in one measure.
pub fn single_staff_score(clef: Clef, fifths: i8, num: u8, den: u8, voice0: Vec<Note>, voice1: Vec<Note>) -> Score {
    let mut score = Score::default();
    score.settings.time_signature = TimeSignature { numerator: num, denominator: den };
    score.settings.key_signature = KeySignature { fifths, mode: "major".to_string() };
    let mut part = Part::new("T", "T");
    let mut staff = Staff::new(clef);
    let mut m = Measure::empty(num, den);
    m.number = 1;
    m.voices[0] = voice0;
    m.voices[1] = voice1;
    staff.measures.push(m);
    part.staves.push(staff);
    score.parts = vec![part];
    score
}

/// Quarter + eighth notes (with flags), one voice, C major 4/4.
pub fn vr_quarter_eighth_notes() -> Score {
    single_staff_score(
        Clef::Treble, 0, 4, 4,
        vec![
            quarter(Step::C, 5),
            note(Step::D, 5, Duration::Eighth),
            note(Step::E, 5, Duration::Eighth),
            quarter(Step::F, 5),
            quarter(Step::G, 5),
        ],
        vec![],
    )
}

/// All 5 supported accidentals in one measure: sharp, natural (cancelling the preceding
/// sharp — a plain alter=0 note alone never draws a natural sign, so this is the only way
/// to actually exercise the glyph), flat, double-sharp, double-flat.
pub fn vr_accidentals() -> Score {
    single_staff_score(
        Clef::Treble, 0, 6, 4,
        vec![
            altered(Step::F, 4, Duration::Quarter, 1),
            note(Step::F, 4, Duration::Quarter), // natural, cancels the F# above
            altered(Step::B, 4, Duration::Quarter, -1),
            note(Step::C, 5, Duration::Quarter),
            altered(Step::D, 5, Duration::Quarter, 2),
            altered(Step::E, 5, Duration::Quarter, -2),
        ],
        vec![],
    )
}

/// A single 3-pitch chord (one Note, multiple Pitches) — shared stem, 3 noteheads.
pub fn vr_chord() -> Score {
    let mut chord = quarter(Step::C, 5);
    chord.pitches.push(Pitch::new(Step::E, 5));
    chord.pitches.push(Pitch::new(Step::G, 5));
    single_staff_score(Clef::Treble, 0, 4, 4, vec![chord, quarter(Step::C, 5), quarter(Step::C, 5), quarter(Step::C, 5)], vec![])
}

/// One rest of each required duration: half, quarter, quarter, eighth, eighth
/// (7 beats — a whole rest would fill an entire measure alone, so it gets its own fixture).
pub fn vr_rests() -> Score {
    single_staff_score(
        Clef::Treble, 0, 6, 4,
        vec![
            Note::rest(Duration::Half),
            Note::rest(Duration::Quarter),
            Note::rest(Duration::Quarter),
            Note::rest(Duration::Eighth),
            Note::rest(Duration::Eighth),
        ],
        vec![],
    )
}

/// A whole rest alone (fills a full 4/4 measure).
pub fn vr_whole_rest() -> Score {
    single_staff_score(Clef::Treble, 0, 4, 4, vec![Note::rest(Duration::Whole)], vec![])
}

/// 3 measures of quarter notes — exercises barline placement and measure spacing.
pub fn vr_multi_measure() -> Score {
    let mut score = Score::default();
    score.settings.time_signature = TimeSignature { numerator: 4, denominator: 4 };
    score.settings.key_signature = KeySignature { fifths: 0, mode: "major".to_string() };
    let mut part = Part::new("T", "T");
    let mut staff = Staff::new(Clef::Treble);
    for i in 0..3 {
        let mut m = Measure::empty(4, 4);
        m.number = i + 1;
        m.voices[0] = vec![quarter(Step::C, 5), quarter(Step::D, 5), quarter(Step::E, 5), quarter(Step::F, 5)];
        staff.measures.push(m);
    }
    part.staves.push(staff);
    score.parts = vec![part];
    score
}

/// Two simultaneous voices on one staff — voice 0 must stem up, voice 1 must stem down.
pub fn vr_stem_directions() -> Score {
    single_staff_score(
        Clef::Treble, 0, 4, 4,
        vec![quarter(Step::C, 5), quarter(Step::C, 5), quarter(Step::C, 5), quarter(Step::C, 5)],
        vec![quarter(Step::E, 4), quarter(Step::E, 4), quarter(Step::E, 4), quarter(Step::E, 4)],
    )
}

/// Assign `BeamState` (via `acorde_core::compute_beams`) to a set of notes for the given
/// time signature, matching what a real `acorde-core` -> `acorde-layout` pipeline would do —
/// this crate's tests never invent beam grouping themselves.
fn beamed(mut notes: Vec<Note>, num: u8, den: u8) -> Vec<Note> {
    let ts = TimeSignature { numerator: num, denominator: den };
    let states = compute_beams(&notes, &ts);
    for (n, s) in notes.iter_mut().zip(states) {
        n.beam = s;
    }
    notes
}

/// 4 same-pitch eighth notes in 4/4 — two flat, 2-note beams (one per quarter-note beat).
pub fn vr_beam_flat() -> Score {
    let notes = beamed(vec![note(Step::C, 5, Duration::Eighth); 4], 4, 4);
    single_staff_score(Clef::Treble, 0, 4, 4, notes, vec![])
}

/// 4 ascending eighth notes spanning more than an octave — exercises slope clamping (a
/// naive first-to-last line would produce a wildly steep beam and absurd stem lengths).
pub fn vr_beam_sloped() -> Score {
    let notes = beamed(
        vec![
            note(Step::C, 4, Duration::Eighth),
            note(Step::D, 5, Duration::Eighth),
            note(Step::E, 4, Duration::Eighth),
            note(Step::F, 5, Duration::Eighth),
        ],
        4, 4,
    );
    single_staff_score(Clef::Treble, 0, 4, 4, notes, vec![])
}

/// Eighth, sixteenth, sixteenth, eighth — a secondary (16th-level) beam spans only the
/// contiguous 16th-note pair, not the full group.
pub fn vr_beam_mixed_durations() -> Score {
    let notes = beamed(
        vec![
            note(Step::C, 5, Duration::Eighth),
            note(Step::D, 5, Duration::Sixteenth),
            note(Step::E, 5, Duration::Sixteenth),
            note(Step::F, 5, Duration::Eighth),
        ],
        4, 4,
    );
    single_staff_score(Clef::Treble, 0, 4, 4, notes, vec![])
}

fn tuplet(mut notes: Vec<Note>, actual_notes: u8, normal_notes: u8) -> Vec<Note> {
    for n in &mut notes {
        n.tuplet = Some(TupletInfo { actual_notes, normal_notes });
    }
    notes
}

/// Unbeamed quarter-note triplet (3 in the time of 2) — a full bracket + "3".
pub fn vr_tuplet_triplet_unbeamed() -> Score {
    let notes = tuplet(vec![quarter(Step::C, 5), quarter(Step::D, 5), quarter(Step::E, 5)], 3, 2);
    single_staff_score(Clef::Treble, 0, 4, 4, notes, vec![])
}

/// Beamed eighth-note triplet — number only, no bracket line (the beam already groups them).
pub fn vr_tuplet_triplet_beamed() -> Score {
    let notes = vec![
        note(Step::C, 5, Duration::Eighth),
        note(Step::D, 5, Duration::Eighth),
        note(Step::E, 5, Duration::Eighth),
    ];
    // Tuplet info must be set *before* compute_beams — Note::beats() scales by
    // actual_notes/normal_notes, and compute_beams groups by beat boundaries using that
    // scaled duration. Three plain (non-tuplet) eighth notes span 1.5 beats and cross a beat
    // boundary, splitting the beam 2+1; as a 3:2 triplet they span exactly 1 beat and stay
    // in one group, which is the whole point of this fixture.
    let mut notes = tuplet(notes, 3, 2);
    let states = compute_beams(&notes, &TimeSignature { numerator: 4, denominator: 4 });
    for (n, s) in notes.iter_mut().zip(states) {
        n.beam = s;
    }
    single_staff_score(Clef::Treble, 0, 4, 4, notes, vec![])
}

/// Triplet containing a rest — the bracket must still span across it.
pub fn vr_tuplet_with_rest() -> Score {
    let notes = tuplet(vec![quarter(Step::C, 5), Note::rest(Duration::Quarter), quarter(Step::E, 5)], 3, 2);
    single_staff_score(Clef::Treble, 0, 4, 4, notes, vec![])
}

// ── SVG probing helpers (dependency-free attribute extraction) ──────────────────

/// Extract every occurrence of `class="{class}"` element's numeric attributes, in document
/// order. Only matches elements whose class is exactly `class` (not a prefix of a longer
/// class name), and only elements that are fully self-closed on one line (true for every
/// element this renderer emits).
pub fn extract_elements<'a>(svg: &'a str, class: &str) -> Vec<&'a str> {
    let class_needle = format!(r#"class="{class}""#);
    let mut out = Vec::new();
    let mut rest = svg;
    while let Some(idx) = rest.find(&class_needle) {
        let before = &rest[..idx];
        let tag_start = before.rfind('<').expect("class attribute must be inside a tag");
        let tag_end = rest[idx..].find("/>").map(|e| idx + e + 2)
            .unwrap_or_else(|| rest[idx..].find('>').map(|e| idx + e + 1).unwrap());
        out.push(&rest[tag_start..tag_end]);
        rest = &rest[tag_end..];
    }
    out
}

/// Parse a numeric attribute (e.g. `cx`, `y1`) out of a single element string like
/// `<ellipse class="acorde-notehead" cx="1.00" cy="2.00" .../>`.
pub fn attr_f32(element: &str, attr: &str) -> f32 {
    let needle = format!(r#"{attr}=""#);
    let start = element.find(&needle).unwrap_or_else(|| panic!("attribute {attr} not found in {element}")) + needle.len();
    let end = element[start..].find('"').unwrap() + start;
    element[start..end].parse().unwrap_or_else(|_| panic!("attribute {attr} not numeric in {element}"))
}
