//! Tuplet engraving showcase: unbeamed triplet, beamed triplet (number only, no bracket),
//! a stem-down quintuplet, and a triplet containing a rest. `TupletGroup.actual_notes` /
//! `normal_notes` from acorde-layout is the only source of rhythm-ratio truth here — this
//! crate only turns it into a bracket + number.
//!
//! ```bash
//! cargo run -p acorde-render-svg --example render_tuplets > /tmp/tuplets.svg
//! ```

use acorde_core::{Clef, Duration, KeySignature, Measure, Note, Part, Pitch, Score, Staff, Step, TimeSignature, TupletInfo, compute_beams};
use acorde_render_svg::{render_svg, SvgRenderOptions};

fn q(step: Step, octave: i8) -> Note { Note::new(Pitch::new(step, octave), Duration::Quarter) }
fn e(step: Step, octave: i8) -> Note { Note::new(Pitch::new(step, octave), Duration::Eighth) }

fn tuplet_of(mut notes: Vec<Note>, actual: u8, normal: u8) -> Vec<Note> {
    for n in &mut notes { n.tuplet = Some(TupletInfo { actual_notes: actual, normal_notes: normal }); }
    notes
}

fn measure(notes: Vec<Note>, num: u8, den: u8, apply_beams: bool) -> Measure {
    let ts = TimeSignature { numerator: num, denominator: den };
    let mut notes = notes;
    if apply_beams {
        let beams = compute_beams(&notes, &ts);
        for (n, b) in notes.iter_mut().zip(beams) { n.beam = b; }
    }
    let mut m = Measure::empty(num, den);
    m.number = 1;
    m.voices[0] = notes;
    m
}

fn main() {
    let mut score = Score::default();
    score.settings.time_signature = TimeSignature { numerator: 4, denominator: 4 };
    score.settings.key_signature = KeySignature { fifths: 0, mode: "major".to_string() };
    let mut part = Part::new("T", "T");
    let mut staff = Staff::new(Clef::Treble);

    // measure 1: unbeamed quarter-note triplet (3 in the time of 2)
    staff.measures.push(measure(
        tuplet_of(vec![q(Step::C, 5), q(Step::D, 5), q(Step::E, 5)], 3, 2), 4, 4, false));

    // measure 2: beamed eighth-note triplet -> number only, no bracket
    staff.measures.push(measure(
        tuplet_of(vec![e(Step::C, 5), e(Step::D, 5), e(Step::E, 5)], 3, 2), 4, 4, true));

    // measure 3: quintuplet (5 in the time of 4), unbeamed, stem down (forced)
    let mut notes: Vec<Note> = vec![q(Step::B, 5), q(Step::A, 5), q(Step::G, 5), q(Step::F, 5), q(Step::E, 5)];
    for n in &mut notes { n.stem_up = Some(false); }
    staff.measures.push(measure(tuplet_of(notes, 5, 4), 4, 4, false));

    // measure 4: triplet containing a rest
    staff.measures.push(measure(
        tuplet_of(vec![q(Step::C, 5), Note::rest(Duration::Quarter), q(Step::E, 5)], 3, 2), 4, 4, false));

    part.staves.push(staff);
    score.parts = vec![part];

    let options = SvgRenderOptions { width: 900.0, staff_size: 24.0, measures_per_system: 4, interactive: true };
    let svg = render_svg(&score, &options).expect("render_svg failed");
    println!("{svg}");
}
