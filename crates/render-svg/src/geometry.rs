//! Pixel-free-to-pixel math: diatonic staff positions, ledger lines, stem direction.
//!
//! Vertical position is derived purely from `(Step, octave)` — never from `Pitch::to_midi()` —
//! so enharmonic spellings (Cb4 vs B3) land on visually distinct staff positions and
//! accidentals only ever affect horizontal placement.

use acorde_core::{Clef, Step};

use crate::RenderError;

pub(crate) fn step_idx(step: &Step) -> i32 {
    match step {
        Step::C => 0,
        Step::D => 1,
        Step::E => 2,
        Step::F => 3,
        Step::G => 4,
        Step::A => 5,
        Step::B => 6,
    }
}

/// Absolute diatonic (natural-letter) index: `octave*7 + step_idx`.
pub(crate) fn diatonic_index(step: &Step, octave: i8) -> i32 {
    octave as i32 * 7 + step_idx(step)
}

/// Diatonic index of a clef's bottom staff line — the reference for [`staff_position`].
pub(crate) fn clef_bottom_line(clef: &Clef) -> Result<i32, RenderError> {
    match clef {
        Clef::Treble => Ok(diatonic_index(&Step::E, 4)),
        Clef::Bass => Ok(diatonic_index(&Step::G, 2)),
        Clef::Alto => Ok(diatonic_index(&Step::F, 3)),
        Clef::Tenor => Ok(diatonic_index(&Step::D, 3)),
        Clef::Percussion => Err(RenderError::UnsupportedClef),
    }
}

/// Staff position in half-line-spacing units: 0 = bottom line, 2/4/6/8 = the other four
/// lines, odd values = spaces, negative = below the staff, > 8 = above the staff.
pub(crate) fn staff_position(step: &Step, octave: i8, clef_bottom_line: i32) -> i32 {
    diatonic_index(step, octave) - clef_bottom_line
}

/// y-offset (px) from the staff's bottom-line y for a given staff position.
pub(crate) fn position_y(position: i32, staff_size: f32) -> f32 {
    -(position as f32) * (staff_size / 2.0)
}

/// Ledger line positions (even integers) needed to reach a note at `position`.
///
/// Empty when the note sits within (or on the edge of) the staff, i.e. `0..=8`.
pub(crate) fn ledger_positions(position: i32) -> Vec<i32> {
    let mut out = Vec::new();
    if position <= -2 {
        let top = if position % 2 == 0 {
            position
        } else {
            position + 1
        };
        let mut p = -2;
        while p >= top {
            out.push(p);
            p -= 2;
        }
    } else if position >= 10 {
        let bottom = if position % 2 == 0 {
            position
        } else {
            position - 1
        };
        let mut p = 10;
        while p <= bottom {
            out.push(p);
            p += 2;
        }
    }
    out
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn diatonic_index_matches_expected() {
        assert_eq!(diatonic_index(&Step::C, 4), 28);
        assert_eq!(diatonic_index(&Step::E, 4), 30);
    }

    #[test]
    fn enharmonic_spellings_land_on_different_positions() {
        let bottom = clef_bottom_line(&Clef::Treble).unwrap();
        // Cb4, C4, C#4 all share step=C, octave=4 → identical staff position regardless of alter.
        let pos = staff_position(&Step::C, 4, bottom);
        assert_eq!(pos, staff_position(&Step::C, 4, bottom));
        // But B3 (a genuinely different staff position) must differ from C4.
        assert_ne!(pos, staff_position(&Step::B, 3, bottom));
    }

    #[test]
    fn treble_bottom_line_is_e4() {
        let bottom = clef_bottom_line(&Clef::Treble).unwrap();
        assert_eq!(staff_position(&Step::E, 4, bottom), 0);
        assert_eq!(staff_position(&Step::G, 4, bottom), 2);
        assert_eq!(staff_position(&Step::B, 4, bottom), 4); // middle line
        assert_eq!(staff_position(&Step::F, 5, bottom), 8); // top line
    }

    #[test]
    fn bass_bottom_line_is_g2() {
        let bottom = clef_bottom_line(&Clef::Bass).unwrap();
        assert_eq!(staff_position(&Step::G, 2, bottom), 0);
        assert_eq!(staff_position(&Step::D, 3, bottom), 4); // middle line
        assert_eq!(staff_position(&Step::A, 3, bottom), 8); // top line
    }

    #[test]
    fn middle_c_one_ledger_line_below_treble() {
        let bottom = clef_bottom_line(&Clef::Treble).unwrap();
        let pos = staff_position(&Step::C, 4, bottom);
        assert_eq!(pos, -2);
        assert_eq!(ledger_positions(pos), vec![-2]);
    }

    #[test]
    fn b3_needs_one_ledger_line_hanging_below_it() {
        let bottom = clef_bottom_line(&Clef::Treble).unwrap();
        let pos = staff_position(&Step::B, 3, bottom);
        assert_eq!(pos, -3);
        assert_eq!(ledger_positions(pos), vec![-2]);
    }

    #[test]
    fn a3_needs_two_ledger_lines_below_treble() {
        let bottom = clef_bottom_line(&Clef::Treble).unwrap();
        let pos = staff_position(&Step::A, 3, bottom);
        assert_eq!(pos, -4);
        assert_eq!(ledger_positions(pos), vec![-2, -4]);
    }

    #[test]
    fn notes_within_staff_need_no_ledger_lines() {
        assert!(ledger_positions(0).is_empty());
        assert!(ledger_positions(4).is_empty());
        assert!(ledger_positions(8).is_empty());
    }

    #[test]
    fn a5_needs_one_ledger_line_above_treble() {
        let bottom = clef_bottom_line(&Clef::Treble).unwrap();
        let pos = staff_position(&Step::A, 5, bottom);
        assert_eq!(pos, 10);
        assert_eq!(ledger_positions(pos), vec![10]);
    }

    #[test]
    fn c6_needs_two_ledger_lines_above_treble() {
        let bottom = clef_bottom_line(&Clef::Treble).unwrap();
        let pos = staff_position(&Step::C, 6, bottom);
        assert_eq!(pos, 12);
        assert_eq!(ledger_positions(pos), vec![10, 12]);
    }

    #[test]
    fn percussion_clef_is_unsupported() {
        assert!(matches!(
            clef_bottom_line(&Clef::Percussion),
            Err(RenderError::UnsupportedClef)
        ));
    }

    #[test]
    fn position_y_bottom_line_is_zero() {
        assert_eq!(position_y(0, 20.0), 0.0);
    }

    #[test]
    fn position_y_increases_upward() {
        // SVG y grows downward; higher staff positions must have smaller (more negative) y.
        assert!(position_y(2, 20.0) < position_y(0, 20.0));
    }
}
