//! Renders a hand-built SATB chorale measure (C major, I–ii6–V7–I) to SVG on stdout.
//!
//! ```bash
//! cargo run -p acorde-render-svg --example render_satb > /tmp/satb.svg
//! ```

use acorde_core::{
    Clef, Duration, KeySignature, Measure, Note, Part, Pitch, Score, Staff, Step, TimeSignature,
};
use acorde_render_svg::{SvgRenderOptions, render_svg};

fn quarter(step: Step, octave: i8) -> Note {
    Note::new(Pitch::new(step, octave), Duration::Quarter)
}

fn main() {
    let mut score = Score::default();
    score.metadata.title = "SATB Chorale (I - ii6 - V7 - I)".to_string();
    score.settings.time_signature = TimeSignature {
        numerator: 4,
        denominator: 4,
    };
    score.settings.key_signature = KeySignature {
        fifths: 0,
        mode: "major".to_string(),
    };

    let mut part = Part::new("Chorale", "Ch.");

    let mut treble = Staff::new(Clef::Treble);
    let mut m_treble = Measure::empty(4, 4);
    m_treble.number = 1;
    m_treble.voices[0] = vec![
        quarter(Step::C, 5),
        quarter(Step::D, 5),
        quarter(Step::B, 4),
        quarter(Step::C, 5),
    ]; // soprano
    m_treble.voices[1] = vec![
        quarter(Step::G, 4),
        quarter(Step::F, 4),
        quarter(Step::G, 4),
        quarter(Step::E, 4),
    ]; // alto
    treble.measures.push(m_treble);

    let mut bass = Staff::new(Clef::Bass);
    let mut m_bass = Measure::empty(4, 4);
    m_bass.number = 1;
    m_bass.voices[0] = vec![
        quarter(Step::E, 4),
        quarter(Step::D, 4),
        quarter(Step::D, 4),
        quarter(Step::C, 4),
    ]; // tenor
    m_bass.voices[1] = vec![
        quarter(Step::C, 3),
        quarter(Step::G, 2),
        quarter(Step::G, 2),
        quarter(Step::C, 3),
    ]; // bass
    bass.measures.push(m_bass);

    part.staves.push(treble);
    part.staves.push(bass);
    score.parts = vec![part];

    let options = SvgRenderOptions {
        width: 500.0,
        staff_size: 24.0,
        measures_per_system: 4,
        interactive: true,
    };
    let svg = render_svg(&score, &options).expect("render_svg failed");
    println!("{svg}");
}
