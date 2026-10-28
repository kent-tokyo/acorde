//! One small golden SVG fixture — used only as a supplementary regression pin (a byte-for-byte
//! diff is easy to eyeball for a single note). Structural correctness is covered by
//! `structural.rs`; this file should stay tiny per the "no giant golden-only comparisons" rule.

use acorde_core::{
    Clef, Duration, KeySignature, Measure, Note, Part, Pitch, Score, Staff, Step, TimeSignature,
};
use acorde_render_svg::{SvgRenderOptions, render_svg};

fn single_note_score() -> Score {
    let mut score = Score::default();
    score.settings.time_signature = TimeSignature {
        numerator: 4,
        denominator: 4,
    };
    score.settings.key_signature = KeySignature {
        fifths: 0,
        mode: "major".to_string(),
    };
    let mut part = Part::new("T", "T");
    let mut staff = Staff::new(Clef::Treble);
    let mut m = Measure::empty(4, 4);
    m.number = 1;
    m.voices[0] = vec![Note::new(Pitch::new(Step::B, 4), Duration::Quarter)];
    staff.measures.push(m);
    part.staves.push(staff);
    score.parts = vec![part];
    score
}

#[test]
fn single_note_matches_golden_svg() {
    let options = SvgRenderOptions {
        width: 200.0,
        staff_size: 20.0,
        measures_per_system: 1,
        interactive: false,
    };
    let svg = render_svg(&single_note_score(), &options).unwrap();
    if std::env::var("UPDATE_GOLDEN").is_ok() {
        std::fs::write(
            concat!(env!("CARGO_MANIFEST_DIR"), "/tests/golden/single_note.svg"),
            &svg,
        )
        .unwrap();
        return;
    }
    let golden = include_str!("golden/single_note.svg");
    assert_eq!(
        svg.trim_end(),
        golden.trim_end(),
        "SVG output for the single-note golden fixture changed — if intentional, update tests/golden/single_note.svg"
    );
}
