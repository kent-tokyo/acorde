//! Beam engraving showcase: 5 measures exercising the beam slope/clearance heuristic and
//! secondary (16th-note) beams. `acorde_core::compute_beams` assigns each note's `BeamState`;
//! `acorde-layout` groups them into `BeamGroup`s; this crate only turns that grouping into
//! sloped, clearance-checked geometry — it never re-infers which notes are beamed together.
//!
//! ```bash
//! cargo run -p acorde-render-svg --example render_beams > /tmp/beams.svg
//! ```

use acorde_core::{Clef, Duration, KeySignature, Measure, Note, Part, Pitch, Score, Staff, Step, TimeSignature, compute_beams};
use acorde_render_svg::{render_svg, SvgRenderOptions};

fn e(step: Step, octave: i8) -> Note { Note::new(Pitch::new(step, octave), Duration::Eighth) }
fn s16(step: Step, octave: i8) -> Note { Note::new(Pitch::new(step, octave), Duration::Sixteenth) }

fn measure_with_beams(notes: Vec<Note>, num: u8, den: u8) -> Measure {
    let ts = TimeSignature { numerator: num, denominator: den };
    let beams = compute_beams(&notes, &ts);
    let mut notes = notes;
    for (n, b) in notes.iter_mut().zip(beams) { n.beam = b; }
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

    // measure 1: flat run of 4 eighths, same pitch -> flat beam
    staff.measures.push(measure_with_beams(
        vec![e(Step::C, 5), e(Step::C, 5), e(Step::C, 5), e(Step::C, 5)], 4, 4));

    // measure 2: ascending run -> sloped beam, clamped
    staff.measures.push(measure_with_beams(
        vec![e(Step::C, 4), e(Step::D, 5), e(Step::E, 4), e(Step::F, 5)], 4, 4));

    // measure 3: wide pitch swing across the group (tests clearance shift)
    staff.measures.push(measure_with_beams(
        vec![e(Step::C, 6), e(Step::C, 4), e(Step::C, 6), e(Step::C, 4)], 4, 4));

    // measure 4: mixed eighths + sixteenths (partial/secondary beam + hook)
    staff.measures.push(measure_with_beams(
        vec![e(Step::C, 5), s16(Step::D, 5), s16(Step::E, 5), e(Step::F, 5)], 4, 4));

    // measure 5: stem-down beam (high pitches force stem down via suggested_stem_up upstream;
    // here we force it directly via stem_up=Some(false))
    let mut down_notes = vec![e(Step::G, 5), e(Step::A, 5), e(Step::B, 5), e(Step::C, 6)];
    for n in &mut down_notes { n.stem_up = Some(false); }
    staff.measures.push(measure_with_beams(down_notes, 4, 4));

    part.staves.push(staff);
    score.parts = vec![part];

    let options = SvgRenderOptions { width: 1000.0, staff_size: 24.0, measures_per_system: 5, interactive: true };
    let svg = render_svg(&score, &options).expect("render_svg failed");
    println!("{svg}");
}
