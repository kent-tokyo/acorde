//! Beam geometry: turns one `acorde_layout::BeamGroup` (already-decided note grouping — this
//! module never re-infers which notes belong together) into a sloped, clearance-checked beam
//! line plus per-note stem tips, extensible to any number of secondary beam levels (16th,
//! 32nd, 64th).
//!
//! # Slope algorithm
//!
//! A beam drawn as a straight line from the first note's default stem tip to the last note's
//! default stem tip can produce absurdly long or short interior stems when the group spans a
//! wide pitch range. Instead:
//!
//! 1. Compute the "natural" (default-length) stem tip for the first and last note.
//! 2. Clamp the rise between them to [`MAX_BEAM_RISE_U`] staff-spaces — a shallow-slope cap,
//!    not a full spacing optimizer, but enough to prevent extreme angles.
//! 3. Shift the whole beam (parallel, away from the noteheads) if any *interior* note's
//!    resulting stem would be shorter than [`MIN_STEM_LEN_U`] — the "beam must clear every
//!    notehead" rule.
//!
//! # Secondary beams
//!
//! Each note's required beam level count comes from its own `Duration` (eighth = 1, 16th = 2,
//! …). Level 0 (primary) always spans the whole group. Level ≥ 1 spans only the contiguous
//! run of notes that need it; an isolated single note at a level gets a short hook stub
//! (pointing toward whichever neighbor is closer to the middle of the group) instead of a
//! run — standard partial-beam notation.

use std::collections::HashMap;

use acorde_core::Duration;

use crate::glyphs;

const MIN_STEM_LEN_U: f32 = 2.0;
const MAX_BEAM_RISE_U: f32 = 1.0;
const BEAM_THICKNESS_U: f32 = 0.5;
const BEAM_LEVEL_GAP_U: f32 = 0.9;
const HOOK_LEN_U: f32 = 0.6;

fn beam_level(duration: &Duration) -> u8 {
    match duration {
        Duration::Eighth => 1,
        Duration::Sixteenth => 2,
        Duration::ThirtySecond => 3,
        Duration::SixtyFourth => 4,
        _ => 0,
    }
}

pub(crate) struct BeamPlan {
    /// Local index within the group (0..len) -> resolved stem tip y.
    pub tips: HashMap<usize, f32>,
    pub svg: String,
}

/// Plan one beam group's geometry and emit its SVG.
///
/// `durations`/`xs`/`attach_ys` are parallel arrays, one entry per note in the group (already
/// sliced to just this group by the caller — this module has no notion of a "voice").
/// `attach_ys` is each note's stem-side notehead y (already resolved for chords).
pub(crate) fn plan_beam_group(
    durations: &[Duration],
    xs: &[f32],
    attach_ys: &[f32],
    stem_up: bool,
    space: f32,
) -> BeamPlan {
    let n = durations.len();
    debug_assert_eq!(xs.len(), n);
    debug_assert_eq!(attach_ys.len(), n);
    let dir = if stem_up { -1.0 } else { 1.0 };
    let mut svg = String::new();

    let natural = |i: usize| attach_ys[i] + dir * glyphs::DEFAULT_STEM_LEN_U * space;
    let raw_rise = natural(n - 1) - natural(0);
    let clamped_rise = raw_rise.clamp(-MAX_BEAM_RISE_U * space, MAX_BEAM_RISE_U * space);
    let dx = xs[n - 1] - xs[0];
    let slope = if dx.abs() > 1e-6 {
        clamped_rise / dx
    } else {
        0.0
    };
    let base_y = |x: f32| natural(0) + slope * (x - xs[0]);

    // Clearance: shift the whole beam away from noteheads if any stem would be too short.
    let mut max_deficit = 0.0f32;
    for (&x, &attach_y) in xs.iter().zip(attach_ys) {
        let stem_len = (base_y(x) - attach_y).abs();
        let deficit = MIN_STEM_LEN_U * space - stem_len;
        if deficit > max_deficit {
            max_deficit = deficit;
        }
    }
    let beam_y = |x: f32| base_y(x) + dir * max_deficit;

    let mut tips = HashMap::with_capacity(n);
    for (i, &x) in xs.iter().enumerate() {
        tips.insert(i, beam_y(x));
    }

    // Primary beam (level 0) always spans the full group.
    svg.push_str(&glyphs::beam_segment(
        xs[0],
        beam_y(xs[0]),
        xs[n - 1],
        beam_y(xs[n - 1]),
        BEAM_THICKNESS_U * space,
    ));

    // Secondary+ beams: contiguous runs (or single-note hooks) at each extra level.
    let max_level = durations.iter().map(beam_level).max().unwrap_or(1);
    for level in 1..max_level {
        let needs: Vec<bool> = durations.iter().map(|d| beam_level(d) > level).collect();
        let level_y = |x: f32| beam_y(x) + dir * level as f32 * BEAM_LEVEL_GAP_U * space;
        let mut i = 0;
        while i < n {
            if !needs[i] {
                i += 1;
                continue;
            }
            let run_start = i;
            while i < n && needs[i] {
                i += 1;
            }
            let run_end = i - 1;
            if run_start == run_end {
                // Isolated note: hook toward the middle of the group (whichever neighbor
                // exists — the earlier one unless this is the group's first note).
                let hook_dir = if run_start > 0 { -1.0 } else { 1.0 };
                let x0 = xs[run_start];
                let x1 = x0 + hook_dir * HOOK_LEN_U * space;
                svg.push_str(&glyphs::beam_segment(
                    x0,
                    level_y(x0),
                    x1,
                    level_y(x1),
                    BEAM_THICKNESS_U * space,
                ));
            } else {
                svg.push_str(&glyphs::beam_segment(
                    xs[run_start],
                    level_y(xs[run_start]),
                    xs[run_end],
                    level_y(xs[run_end]),
                    BEAM_THICKNESS_U * space,
                ));
            }
        }
    }

    BeamPlan { tips, svg }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn two_note_flat_beam_gives_default_stem_length() {
        // Same pitch (attach_ys equal): rise is 0, so both stems should equal the default.
        let durations = vec![Duration::Eighth, Duration::Eighth];
        let xs = vec![0.0, 20.0];
        let attach_ys = vec![100.0, 100.0];
        let plan = plan_beam_group(&durations, &xs, &attach_ys, true, 20.0);
        assert_eq!(plan.tips.len(), 2);
        for &tip in plan.tips.values() {
            assert!((tip - (100.0 - glyphs::DEFAULT_STEM_LEN_U * 20.0)).abs() < 0.01);
        }
    }

    #[test]
    fn steep_pitch_span_clamps_beam_rise() {
        // A huge pitch difference (10 staff-spaces) must not produce a 10-space beam rise.
        let durations = vec![Duration::Eighth, Duration::Eighth];
        let xs = vec![0.0, 20.0];
        let space = 20.0;
        let attach_ys = vec![100.0, 100.0 - 10.0 * space]; // second note far higher
        let plan = plan_beam_group(&durations, &xs, &attach_ys, true, space);
        let tip0 = plan.tips[&0];
        let tip1 = plan.tips[&1];
        assert!(
            (tip0 - tip1).abs() <= MAX_BEAM_RISE_U * space + 0.01,
            "beam rise must be clamped to {MAX_BEAM_RISE_U} staff-spaces, got {}",
            (tip0 - tip1).abs()
        );
    }

    #[test]
    fn interior_note_never_gets_a_too_short_stem() {
        // Middle note sits much closer to the (flat) beam than the default stem would allow;
        // the whole beam must shift to preserve a minimum stem length for every note.
        let durations = vec![Duration::Eighth, Duration::Eighth, Duration::Eighth];
        let xs = vec![0.0, 20.0, 40.0];
        let space = 20.0;
        // Middle note very close (in the stem-up direction, i.e. numerically just below)
        // the other two notes' natural stem tips.
        let attach_ys = vec![
            100.0,
            100.0 - glyphs::DEFAULT_STEM_LEN_U * space + 5.0,
            100.0,
        ];
        let plan = plan_beam_group(&durations, &xs, &attach_ys, true, space);
        for (i, &attach_y) in attach_ys.iter().enumerate() {
            let stem_len = (plan.tips[&i] - attach_y).abs();
            assert!(
                stem_len >= MIN_STEM_LEN_U * space - 0.01,
                "note {i} stem length {stem_len} is below the {MIN_STEM_LEN_U}-space minimum"
            );
        }
    }

    #[test]
    fn sixteenth_notes_get_a_second_beam_level_spanning_the_whole_group() {
        let durations = vec![Duration::Sixteenth, Duration::Sixteenth];
        let xs = vec![0.0, 20.0];
        let attach_ys = vec![100.0, 100.0];
        let plan = plan_beam_group(&durations, &xs, &attach_ys, true, 20.0);
        // 2 beam segments expected: primary + one secondary run.
        assert_eq!(plan.svg.matches("acorde-beam").count(), 2);
    }

    #[test]
    fn isolated_sixteenth_among_eighths_gets_a_hook_not_a_full_run() {
        let durations = vec![Duration::Sixteenth, Duration::Eighth, Duration::Eighth];
        let xs = vec![0.0, 20.0, 40.0];
        let attach_ys = vec![100.0, 100.0, 100.0];
        let plan = plan_beam_group(&durations, &xs, &attach_ys, true, 20.0);
        // Primary (1) + one isolated hook segment at level 1 (1) = 2 beam polygons.
        assert_eq!(plan.svg.matches("acorde-beam").count(), 2);
    }

    #[test]
    fn stem_down_beam_sits_below_noteheads() {
        let durations = vec![Duration::Eighth, Duration::Eighth];
        let xs = vec![0.0, 20.0];
        let attach_ys = vec![100.0, 100.0];
        let plan = plan_beam_group(&durations, &xs, &attach_ys, false, 20.0);
        for &tip in plan.tips.values() {
            assert!(
                tip > 100.0,
                "stem-down beam tips must be below (larger y than) the notehead"
            );
        }
    }
}
