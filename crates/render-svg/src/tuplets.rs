//! Tuplet geometry: turns one `acorde_layout::TupletGroup` (already-decided grouping — this
//! module never re-infers which notes form a tuplet, and never invents new rhythm semantics
//! beyond what `TupletGroup.actual_notes` already says) into a bracket + number, or just a
//! number when the group is already fully beamed (the beam itself provides the visual
//! grouping in that case — a redundant bracket line would only clutter it).
//!
//! Generalizes to any actual:normal ratio (triplets, quintuplets, septuplets, …) through the
//! same code path — only the displayed digit(s) change; no per-ratio special-casing.

use crate::glyphs;

const BRACKET_CLEARANCE_U: f32 = 0.8; // gap beyond the outermost stem/rest tip
const HOOK_LEN_U: f32 = 0.6; // vertical hook at each bracket end
const NUMBER_HALF_GAP_U: f32 = 1.0; // half-width of the gap left in the bracket line for the number
const NUMBER_HEIGHT_U: f32 = 1.1;

pub(crate) struct TupletPlan {
    pub svg: String,
}

/// `xs`/`ref_ys` are parallel arrays, one per note in the group (already sliced to just this
/// group by the caller). `ref_ys` is each note's "far" reference point — the stem tip (or
/// beam-adjusted tip) for a pitched note, or an approximate vertical center for a rest —
/// i.e. however far the bracket must clear on the `stem_up` side.
pub(crate) fn plan_tuplet(xs: &[f32], ref_ys: &[f32], actual_notes: u8, stem_up: bool, beamed_fully: bool, space: f32) -> TupletPlan {
    let dir = if stem_up { -1.0 } else { 1.0 };
    let extreme = ref_ys.iter().copied().fold(ref_ys[0], |acc, y| if stem_up { acc.min(y) } else { acc.max(y) });
    let bracket_y = extreme + dir * BRACKET_CLEARANCE_U * space;

    let x_first = xs[0];
    let x_last = xs[xs.len() - 1];
    let x_mid = (x_first + x_last) / 2.0;

    let mut svg = String::new();

    if !beamed_fully {
        let hook_end_y = bracket_y - dir * HOOK_LEN_U * space;
        svg.push_str(&glyphs::tuplet_line(x_first, hook_end_y, x_first, bracket_y, space));
        svg.push_str(&glyphs::tuplet_line(x_last, hook_end_y, x_last, bracket_y, space));
        let gap = NUMBER_HALF_GAP_U * space;
        if x_mid - gap > x_first {
            svg.push_str(&glyphs::tuplet_line(x_first, bracket_y, x_mid - gap, bracket_y, space));
        }
        if x_mid + gap < x_last {
            svg.push_str(&glyphs::tuplet_line(x_mid + gap, bracket_y, x_last, bracket_y, space));
        }
    }

    // The number's vertical center sits on the bracket line itself (the gap cut into the
    // bracket above is centered the same way), or — when there's no bracket line at all
    // (beamed case) — offset just clear of the beam on the stem side.
    let number_y = if beamed_fully { bracket_y + dir * NUMBER_HEIGHT_U * 0.5 * space } else { bracket_y };
    svg.push_str(&glyphs::tuplet_number(actual_notes, x_mid, number_y, space));

    TupletPlan { svg }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn unbeamed_triplet_draws_bracket_and_number() {
        let xs = vec![0.0, 20.0, 40.0];
        let ref_ys = vec![50.0, 50.0, 50.0];
        let plan = plan_tuplet(&xs, &ref_ys, 3, true, false, 20.0);
        assert!(plan.svg.contains("acorde-tuplet-bracket"));
        assert!(plan.svg.contains("acorde-tuplet-number"));
    }

    #[test]
    fn beamed_tuplet_omits_bracket() {
        let xs = vec![0.0, 20.0, 40.0];
        let ref_ys = vec![50.0, 50.0, 50.0];
        let plan = plan_tuplet(&xs, &ref_ys, 3, true, true, 20.0);
        assert!(!plan.svg.contains("acorde-tuplet-bracket"));
        assert!(plan.svg.contains("acorde-tuplet-number"));
    }

    #[test]
    fn bracket_sits_above_for_stem_up_and_below_for_stem_down() {
        let xs = vec![0.0, 20.0];
        let ref_ys = vec![50.0, 50.0];
        let up = plan_tuplet(&xs, &ref_ys, 3, true, false, 20.0);
        let down = plan_tuplet(&xs, &ref_ys, 3, false, false, 20.0);
        // Just confirm both render distinct, non-empty geometry; exact y assertions live in
        // the crate's integration geometry tests where real notehead coordinates exist.
        assert_ne!(up.svg, down.svg);
    }
}
