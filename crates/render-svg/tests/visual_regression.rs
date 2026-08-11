//! Minimal visual-regression foundation.
//!
//! Two complementary layers, per the design constraints (no chromedriver/system packages,
//! no pixel-diffing, pure Rust):
//!
//! 1. **Golden SVG fixtures** (`tests/golden/vr_*.svg`) — small, single-concern, byte-exact
//!    snapshots. These catch *any* change to the rendered output, deliberate or not, and
//!    force a conscious decision to update the golden file (see `UPDATE_GOLDEN` below).
//! 2. **Geometry relationship assertions** — golden files alone cannot catch a bug that is
//!    *consistently wrong* (the SVG changes identically every run, so a byte-diff finds
//!    nothing to complain about). These assertions check invariants that are true by
//!    definition of music notation — e.g. "C5 is exactly one diatonic step above B4" —
//!    verified independently of the renderer's internals, so a regression in the position
//!    formula fails a *meaningful* assertion, not just a string diff.
//!
//! Fixtures for tuplets, ties/slurs, and lyrics are intentionally not included yet —
//! acorde-render-svg doesn't render any of those yet (tracked as follow-up phases). Add
//! their golden fixtures alongside the feature that renders them, not before.

mod common;

use acorde_render_svg::{render_svg, SvgRenderOptions};

fn opts() -> SvgRenderOptions {
    SvgRenderOptions { width: 500.0, staff_size: 20.0, measures_per_system: 4, interactive: false }
}

/// Set the `UPDATE_GOLDEN=1` environment variable and run
/// `cargo test -p acorde-render-svg --test visual_regression` to regenerate every golden
/// file in this suite from the current renderer output. Review the diff before committing —
/// a passing regeneration is not the same as a correct one.
fn assert_matches_golden(svg: &str, golden_path: &str) {
    let full_path = format!("{}/tests/{golden_path}", env!("CARGO_MANIFEST_DIR"));
    if std::env::var("UPDATE_GOLDEN").is_ok() {
        std::fs::write(&full_path, svg).unwrap();
        return;
    }
    let golden = std::fs::read_to_string(&full_path)
        .unwrap_or_else(|e| panic!("failed to read golden fixture {full_path}: {e} (run with UPDATE_GOLDEN=1 to create it)"));
    assert_eq!(svg.trim_end(), golden.trim_end(),
        "{golden_path} changed — if intentional, regenerate with UPDATE_GOLDEN=1 and review the diff before committing");
}

// ── golden fixtures ──────────────────────────────────────────────────────────────

#[test]
fn golden_quarter_eighth_notes() {
    let svg = render_svg(&common::vr_quarter_eighth_notes(), &opts()).unwrap();
    assert_matches_golden(&svg, "golden/vr_quarter_eighth_notes.svg");
}

#[test]
fn golden_accidentals() {
    let svg = render_svg(&common::vr_accidentals(), &opts()).unwrap();
    assert_matches_golden(&svg, "golden/vr_accidentals.svg");
}

#[test]
fn golden_chord() {
    let svg = render_svg(&common::vr_chord(), &opts()).unwrap();
    assert_matches_golden(&svg, "golden/vr_chord.svg");
}

#[test]
fn golden_rests() {
    let svg = render_svg(&common::vr_rests(), &opts()).unwrap();
    assert_matches_golden(&svg, "golden/vr_rests.svg");
}

#[test]
fn golden_whole_rest() {
    let svg = render_svg(&common::vr_whole_rest(), &opts()).unwrap();
    assert_matches_golden(&svg, "golden/vr_whole_rest.svg");
}

#[test]
fn golden_multi_measure() {
    let svg = render_svg(&common::vr_multi_measure(), &opts()).unwrap();
    assert_matches_golden(&svg, "golden/vr_multi_measure.svg");
}

#[test]
fn golden_stem_directions() {
    let svg = render_svg(&common::vr_stem_directions(), &opts()).unwrap();
    assert_matches_golden(&svg, "golden/vr_stem_directions.svg");
}

#[test]
fn golden_beam_flat() {
    let svg = render_svg(&common::vr_beam_flat(), &opts()).unwrap();
    assert_matches_golden(&svg, "golden/vr_beam_flat.svg");
}

#[test]
fn golden_beam_sloped() {
    let svg = render_svg(&common::vr_beam_sloped(), &opts()).unwrap();
    assert_matches_golden(&svg, "golden/vr_beam_sloped.svg");
}

#[test]
fn golden_beam_mixed_durations() {
    let svg = render_svg(&common::vr_beam_mixed_durations(), &opts()).unwrap();
    assert_matches_golden(&svg, "golden/vr_beam_mixed_durations.svg");
}

// ── geometry relationship assertions ─────────────────────────────────────────────
// These do not depend on golden files — they assert facts that must hold regardless of
// margins, staff_size, or any future spacing-algorithm change.

#[test]
fn diatonic_steps_translate_to_equal_y_spacing() {
    // C major scale, one octave, all quarter notes: adjacent noteheads must be exactly
    // one diatonic step apart in y (half a line-spacing), and monotonically decreasing
    // (each note higher in pitch sits higher on the page = smaller y).
    use acorde_core::{Clef, Duration, Note, Part, Pitch, Score, Staff, Step, TimeSignature};
    let mut score = Score::default();
    score.settings.time_signature = TimeSignature { numerator: 7, denominator: 4 };
    let mut part = Part::new("T", "T");
    let mut staff = Staff::new(Clef::Treble);
    let mut m = acorde_core::Measure::empty(7, 4);
    m.number = 1;
    let steps = [Step::C, Step::D, Step::E, Step::F, Step::G, Step::A, Step::B];
    m.voices[0] = steps.iter().map(|s| Note::new(Pitch::new(s.clone(), 5), Duration::Quarter)).collect();
    staff.measures.push(m);
    part.staves.push(staff);
    score.parts = vec![part];

    let svg = render_svg(&score, &opts()).unwrap();
    let noteheads = common::extract_elements(&svg, "acorde-notehead");
    assert_eq!(noteheads.len(), 7);
    let ys: Vec<f32> = noteheads.iter().map(|e| common::attr_f32(e, "cy")).collect();

    let staff_size = opts().staff_size;
    let half_step = staff_size / 2.0;
    for pair in ys.windows(2) {
        let delta = pair[0] - pair[1]; // must be positive: next note is higher pitch, smaller y
        assert!((delta - half_step).abs() < 0.01,
            "expected exactly one diatonic step ({half_step}px) between consecutive notes, got {delta}px");
    }
}

#[test]
fn middle_c_ledger_line_passes_through_notehead() {
    use acorde_core::{Clef, Duration, Note, Part, Pitch, Score, Staff, Step, TimeSignature};
    let mut score = Score::default();
    score.settings.time_signature = TimeSignature { numerator: 4, denominator: 4 };
    let mut part = Part::new("T", "T");
    let mut staff = Staff::new(Clef::Treble);
    let mut m = acorde_core::Measure::empty(4, 4);
    m.number = 1;
    m.voices[0] = vec![Note::new(Pitch::new(Step::C, 4), Duration::Quarter)]; // middle C
    staff.measures.push(m);
    part.staves.push(staff);
    score.parts = vec![part];

    let svg = render_svg(&score, &opts()).unwrap();
    let noteheads = common::extract_elements(&svg, "acorde-notehead");
    let ledgers = common::extract_elements(&svg, "acorde-ledger");
    assert_eq!(noteheads.len(), 1);
    assert_eq!(ledgers.len(), 1, "middle C needs exactly one ledger line");
    let note_y = common::attr_f32(noteheads[0], "cy");
    let ledger_y = common::attr_f32(ledgers[0], "y1");
    assert!((note_y - ledger_y).abs() < 0.01, "the ledger line must pass through the notehead");
}

#[test]
fn stem_length_is_three_staff_spaces_regardless_of_pitch() {
    let svg = render_svg(&common::vr_quarter_eighth_notes(), &opts()).unwrap();
    let stems = common::extract_elements(&svg, "acorde-stem");
    assert!(!stems.is_empty());
    let staff_size = opts().staff_size;
    for stem in &stems {
        let y1 = common::attr_f32(stem, "y1");
        let y2 = common::attr_f32(stem, "y2");
        assert!(((y1 - y2).abs() - 3.0 * staff_size).abs() < 0.01,
            "stem length must be exactly 3 staff-spaces, got {}", (y1 - y2).abs());
    }
}

#[test]
fn two_voices_on_one_staff_stem_in_opposite_directions() {
    let svg = render_svg(&common::vr_stem_directions(), &opts()).unwrap();
    let noteheads = common::extract_elements(&svg, "acorde-notehead");
    let stems = common::extract_elements(&svg, "acorde-stem");
    assert_eq!(noteheads.len(), 8); // 4 notes x 2 voices
    assert_eq!(stems.len(), 8);

    // First 4 stems belong to voice 0 (soprano-like, higher pitch) and must point up
    // (tip y < notehead y); last 4 belong to voice 1 and must point down (tip y > notehead y).
    for stem in &stems[0..4] {
        assert!(common::attr_f32(stem, "y2") < common::attr_f32(stem, "y1"), "voice 0 must stem up");
    }
    for stem in &stems[4..8] {
        assert!(common::attr_f32(stem, "y2") > common::attr_f32(stem, "y1"), "voice 1 must stem down");
    }
}

#[test]
fn chord_shares_one_stem_across_multiple_noteheads() {
    let svg = render_svg(&common::vr_chord(), &opts()).unwrap();
    // vr_chord's first note is a 3-pitch chord; the other 3 notes are single quarter notes.
    // Total noteheads = 3 (chord) + 1 + 1 + 1 = 6. Total stems = 4 (one per Note, chord included).
    let noteheads = common::extract_elements(&svg, "acorde-notehead");
    let stems = common::extract_elements(&svg, "acorde-stem");
    assert_eq!(noteheads.len(), 6);
    assert_eq!(stems.len(), 4);
}

#[test]
fn multi_measure_barlines_are_strictly_between_measures() {
    let svg = render_svg(&common::vr_multi_measure(), &opts()).unwrap();
    let noteheads = common::extract_elements(&svg, "acorde-notehead");
    let barlines = common::extract_elements(&svg, "acorde-barline");
    assert_eq!(noteheads.len(), 12); // 3 measures x 4 notes
    assert_eq!(barlines.len(), 3); // one per measure end (including the final one)

    let note_xs: Vec<f32> = noteheads.iter().map(|e| common::attr_f32(e, "cx")).collect();
    let barline_xs: Vec<f32> = barlines.iter().map(|e| common::attr_f32(e, "x1")).collect();

    // Notes must be strictly increasing in x (time flows left to right).
    for pair in note_xs.windows(2) {
        assert!(pair[0] < pair[1], "notes must be laid out left-to-right");
    }
    // Each measure's first barline must sit to the right of that measure's last note and
    // to the left of the next measure's first note.
    assert!(barline_xs[0] > note_xs[3] && barline_xs[0] < note_xs[4]);
    assert!(barline_xs[1] > note_xs[7] && barline_xs[1] < note_xs[8]);
    assert!(barline_xs[2] > note_xs[11]);
}

// ── beam geometry ─────────────────────────────────────────────────────────────────

#[test]
fn beamed_eighth_notes_get_no_individual_flags() {
    // A beam replaces flags entirely — an eighth note that is part of a beam group must
    // never also draw its own flag wedge.
    let svg = render_svg(&common::vr_beam_flat(), &opts()).unwrap();
    assert!(!svg.contains(r#"fill="black" stroke="none""#), "beamed notes must not draw individual flags");
    assert!(svg.contains("acorde-beam"), "expected at least one beam segment");
}

#[test]
fn flat_pitch_beam_group_is_horizontal() {
    // Same-pitch notes: the beam's start and end y must be identical (no artificial slope).
    let svg = render_svg(&common::vr_beam_flat(), &opts()).unwrap();
    let beams = common::extract_elements(&svg, "acorde-beam");
    assert!(!beams.is_empty());
    for beam in &beams {
        // polygon points="x1,y1 x2,y2 x2,y3 x1,y4" — top-left and top-right y must match.
        let points = beam.split("points=\"").nth(1).unwrap().split('"').next().unwrap();
        let coords: Vec<Vec<f32>> = points.split(' ')
            .map(|pair| pair.split(',').map(|v| v.parse().unwrap()).collect())
            .collect();
        assert!((coords[0][1] - coords[1][1]).abs() < 0.01, "flat-pitch beam must be horizontal");
    }
}

#[test]
fn sloped_beam_does_not_produce_extreme_stem_lengths() {
    // Regression guard for the exact failure mode called out in the spec: a naive
    // first-note-to-last-note line must not blow up interior stem lengths. With slope
    // clamped to 1 staff-space of rise, no stem in this ascending-run fixture should exceed
    // roughly 2x the default stem length.
    let svg = render_svg(&common::vr_beam_sloped(), &opts()).unwrap();
    let stems = common::extract_elements(&svg, "acorde-stem");
    assert!(!stems.is_empty());
    let staff_size = opts().staff_size;
    for stem in &stems {
        let len = (common::attr_f32(stem, "y1") - common::attr_f32(stem, "y2")).abs();
        assert!(len <= 6.0 * staff_size,
            "stem length {len} is more than double the default — beam slope is not being clamped");
    }
}

#[test]
fn beam_never_crosses_a_notehead() {
    // The minimum-clearance guarantee: for every beamed note, the beam's y at that note's x
    // must be strictly on the stem side of the notehead (never between the notehead and the
    // beam by less than a hair, i.e. the stem must have positive, sane length).
    for score in [common::vr_beam_flat(), common::vr_beam_sloped(), common::vr_beam_mixed_durations()] {
        let svg = render_svg(&score, &opts()).unwrap();
        let noteheads = common::extract_elements(&svg, "acorde-notehead");
        let stems = common::extract_elements(&svg, "acorde-stem");
        assert_eq!(noteheads.len(), stems.len());
        for (nh, stem) in noteheads.iter().zip(&stems) {
            let note_y = common::attr_f32(nh, "cy");
            let tip_y = common::attr_f32(stem, "y2");
            let len = (note_y - tip_y).abs();
            assert!(len > 0.5, "stem must have positive length (beam must not sit on the notehead)");
        }
    }
}

#[test]
fn sixteenth_run_gets_a_second_beam_level() {
    // vr_beam_mixed_durations has exactly one contiguous 16th-note pair -> exactly one
    // secondary-level segment in addition to the primary beam.
    let svg = render_svg(&common::vr_beam_mixed_durations(), &opts()).unwrap();
    let beams = common::extract_elements(&svg, "acorde-beam");
    assert_eq!(beams.len(), 2, "expected 1 primary + 1 secondary (16th-level) beam segment");
}
