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
//! 2. Flatten when an interior head reaches the outermost endpoint on the beam side; otherwise
//!    clamp the rise between endpoints to [`MAX_BEAM_RISE_U`] staff-spaces — a shallow-slope cap,
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
const TWO_NOTE_BEAM_RISE_U: f32 = 0.25;
const BEAM_THICKNESS_U: f32 = 0.5;
const BEAM_LEVEL_GAP_U: f32 = 0.9;
const HOOK_LEN_U: f32 = 0.6;

fn beam_level(duration: &Duration) -> u8 {
    match duration {
        Duration::Eighth => 1,
        Duration::Sixteenth => 2,
        Duration::ThirtySecond => 3,
        Duration::SixtyFourth => 4,
        Duration::HundredTwentyEighth => 5,
        _ => 0,
    }
}

pub(crate) struct BeamPlan {
    /// Local index within the group (0..len) -> resolved stem tip y.
    pub tips: HashMap<usize, f32>,
    pub svg: String,
}

/// Return the conservative vertical bounds of a planned beam group in staff-space units.
///
/// The bounds include the primary and secondary beam thicknesses, including isolated hooks.
/// Callers use normalized x-coordinates because only the relative slope matters here; the
/// renderer's actual pixel scale is applied by [`plan_beam_group`].
pub(crate) fn vertical_extents(
    durations: &[Duration],
    xs: &[f32],
    attach_ys: &[f32],
    stem_up: bool,
) -> (f32, f32) {
    if durations.len() < 2 || xs.len() != durations.len() || attach_ys.len() != durations.len() {
        return (0.0, 0.0);
    }
    let plan = plan_beam_group(durations, xs, attach_ys, stem_up, 1.0);
    let mut min_y = plan.tips.values().copied().fold(f32::INFINITY, f32::min);
    let mut max_y = plan
        .tips
        .values()
        .copied()
        .fold(f32::NEG_INFINITY, f32::max);
    let max_level = durations.iter().map(beam_level).max().unwrap_or(1);
    let level_extent =
        (max_level.saturating_sub(1) as f32 * BEAM_LEVEL_GAP_U) + BEAM_THICKNESS_U / 2.0;
    // Secondary beams sit inside the primary one, between it and the noteheads.
    if stem_up {
        min_y -= BEAM_THICKNESS_U / 2.0;
        max_y += level_extent;
    } else {
        min_y -= level_extent;
        max_y += BEAM_THICKNESS_U / 2.0;
    }
    (min_y, max_y)
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
    // Beams join the stems, which stand at the notehead's right (stem up) or left (stem down)
    // edge, not at the notehead centre; `xs` are notehead centres.
    let stem_offset = glyphs::NOTEHEAD_RX_U * space * 0.92 * -dir;
    let stem_xs: Vec<f32> = xs.iter().map(|x| x + stem_offset).collect();
    let xs = stem_xs.as_slice();
    // Cover the stem's own width at the group's outer stems.
    let half_stem = glyphs::STEM_WIDTH_U * space / 2.0;

    let natural = |i: usize| attach_ys[i] + dir * glyphs::DEFAULT_STEM_LEN_U * space;
    let raw_rise = natural(n - 1) - natural(0);
    let outer = if stem_up {
        attach_ys[0].min(attach_ys[n - 1])
    } else {
        attach_ys[0].max(attach_ys[n - 1])
    };
    let flatten = attach_ys[1..n - 1]
        .iter()
        .any(|&y| (y - outer) * dir >= 0.0);
    let levels = durations.iter().map(beam_level).max().unwrap_or(1).max(1);
    // Verovio's short two-note step applies only when the stems are at most
    // three staff spaces apart; wider pairs can rise by one staff space.
    let short_pair = n == 2 && levels <= 2 && (xs[n - 1] - xs[0]).abs() <= 3.0 * space;
    let max_rise = if short_pair {
        TWO_NOTE_BEAM_RISE_U
    } else {
        MAX_BEAM_RISE_U
    } * space;
    let clamped_rise = if flatten {
        0.0
    } else {
        raw_rise.clamp(-max_rise, max_rise)
    };
    let dx = xs[n - 1] - xs[0];
    let slope = if dx.abs() > 1e-6 {
        clamped_rise / dx
    } else {
        0.0
    };
    let base_y = |x: f32| natural(0) + slope * (x - xs[0]);

    // Clearance: shift the whole beam away from noteheads if any stem would be too short. The
    // inner (secondary) beams need their own room between the primary beam and the heads.
    let min_stem = (MIN_STEM_LEN_U + f32::from(levels - 1) * BEAM_LEVEL_GAP_U) * space;
    let mut max_deficit = 0.0f32;
    for (&x, &attach_y) in xs.iter().zip(attach_ys) {
        // A beam on the wrong side of a head has a negative length. Taking an
        // absolute value would leave large interior pitch leaps intersecting it.
        let stem_len = (base_y(x) - attach_y) * dir;
        let deficit = min_stem - stem_len;
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
        xs[0] - half_stem,
        beam_y(xs[0] - half_stem),
        xs[n - 1] + half_stem,
        beam_y(xs[n - 1] + half_stem),
        BEAM_THICKNESS_U * space,
    ));

    // Secondary+ beams: contiguous runs (or single-note hooks) at each extra level.
    let max_level = durations.iter().map(beam_level).max().unwrap_or(1);
    for level in 1..max_level {
        let needs: Vec<bool> = durations.iter().map(|d| beam_level(d) > level).collect();
        // Secondary beams stack inward, toward the noteheads, as in engraved music.
        let level_y = |x: f32| beam_y(x) - dir * level as f32 * BEAM_LEVEL_GAP_U * space;
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

/// Plan a *kneed* beam group whose stems point both ways — typically a cross-staff group,
/// upper-staff notes stemmed down and lower-staff notes stemmed up to one beam between the
/// staves. A two-note cross-staff beam slopes toward the second staff when
/// clearance permits. Other mixed beams stay horizontal. Secondary beams stack toward the noteheads of
/// the run's first note, and any stem reaching a secondary beam on its far side is extended.
pub(crate) fn plan_kneed_beam_group(
    durations: &[Duration],
    xs: &[f32],
    attach_ys: &[f32],
    stem_ups: &[bool],
    space: f32,
    cross_staff: bool,
) -> BeamPlan {
    let n = durations.len();
    debug_assert_eq!(xs.len(), n);
    debug_assert_eq!(attach_ys.len(), n);
    debug_assert_eq!(stem_ups.len(), n);
    let stem_xs: Vec<f32> = xs
        .iter()
        .zip(stem_ups)
        .map(|(&x, &up)| x + glyphs::NOTEHEAD_RX_U * space * 0.92 * if up { 1.0 } else { -1.0 })
        .collect();
    let half_stem = glyphs::STEM_WIDTH_U * space / 2.0;
    let levels = durations.iter().map(beam_level).max().unwrap_or(1).max(1);
    let min_stem = MIN_STEM_LEN_U * space;
    let stack = f32::from(levels - 1) * BEAM_LEVEL_GAP_U * space;
    let reference_pair = cross_staff
        && n == 2
        && levels <= 3
        && durations.iter().all(|duration| beam_level(duration) > 0)
        && stem_ups[0] != stem_ups[1];
    // The pinned cross-staff references use three quarters of a staff space
    // between parallel beams, measured at the same x coordinate.
    let level_gap = if reference_pair {
        0.75
    } else {
        BEAM_LEVEL_GAP_U
    } * space;
    // Express both clearance bounds at the first stem, so they also work for a slope.
    let bounds = |rise: f32| {
        let dx = stem_xs[n - 1] - stem_xs[0];
        let slope = if dx.abs() > 1e-6 { rise / dx } else { 0.0 };
        let mut low = f32::NEG_INFINITY;
        let mut high = f32::INFINITY;
        for (index, ((&x, &y), &up)) in stem_xs.iter().zip(attach_ys).zip(stem_ups).enumerate() {
            let required = y - slope * (x - stem_xs[0]);
            // Secondary beams on the notehead side need room in the staff gap too.
            let extra = if reference_pair {
                let own = beam_level(&durations[index]).max(1);
                let other = beam_level(&durations[1 - index]).max(1);
                let shared = own.min(other).saturating_sub(1);
                let unshared = own.saturating_sub(other);
                let toward_head = unshared + if up == stem_ups[0] { shared } else { 0 };
                f32::from(toward_head) * level_gap
            } else {
                0.0
            };
            if up {
                high = high.min(required - min_stem - extra);
            } else {
                low = low.max(required + min_stem + extra);
            }
        }
        (low, high)
    };
    let desired_rise = if reference_pair && (stem_xs[1] - stem_xs[0]).abs() > 1e-6 {
        if stem_ups[0] { -space } else { space }
    } else {
        0.0
    };
    let (candidate_low, candidate_high) = bounds(desired_rise);
    let rise = if candidate_low <= candidate_high {
        desired_rise
    } else {
        0.0
    };
    let (lowest, highest) = bounds(rise);
    let beam_y = match (lowest.is_finite(), highest.is_finite()) {
        (true, true) => (lowest + highest) / 2.0,
        (true, false) => lowest + stack,
        (false, true) => highest - stack,
        (false, false) => attach_ys.first().copied().unwrap_or(0.0),
    };
    let dx = stem_xs[n - 1] - stem_xs[0];
    let slope = if dx.abs() > 1e-6 { rise / dx } else { 0.0 };
    let beam_at = |x: f32| beam_y + slope * (x - stem_xs[0]);
    let mut tips: HashMap<usize, f32> = stem_xs
        .iter()
        .enumerate()
        .map(|(i, &x)| {
            // In the Verovio mixed-stem reference, down-stems reach the lower edge
            // of the single beam while up-stems terminate at its centre.
            let edge = if reference_pair && !stem_ups[i] {
                BEAM_THICKNESS_U * space / 2.0
            } else {
                0.0
            };
            (i, beam_at(x) + edge)
        })
        .collect();
    let mut svg = glyphs::beam_segment(
        stem_xs[0] - half_stem,
        beam_at(stem_xs[0] - half_stem),
        stem_xs[n - 1] + half_stem,
        beam_at(stem_xs[n - 1] + half_stem),
        BEAM_THICKNESS_U * space,
    );
    let max_level = durations.iter().map(beam_level).max().unwrap_or(1);
    for level in 1..max_level {
        let needs: Vec<bool> = durations.iter().map(|d| beam_level(d) > level).collect();
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
            // Toward the first note's head: below the beam for an up-stem, above for a down.
            let side = if stem_ups[run_start] { 1.0 } else { -1.0 };
            let offset = side * level as f32 * level_gap;
            for (index, &up) in stem_ups
                .iter()
                .enumerate()
                .take(run_end + 1)
                .skip(run_start)
            {
                let edge = if reference_pair {
                    (if up { -0.5 } else { 0.5 }) * BEAM_THICKNESS_U * space
                } else {
                    0.0
                };
                let y = beam_at(stem_xs[index]) + offset + edge;
                let tip = tips.entry(index).or_insert(beam_at(stem_xs[index]));
                *tip = if up { tip.min(y) } else { tip.max(y) };
            }
            let (x0, x1) = if run_start == run_end {
                let hook_dir = if run_start > 0 { -1.0 } else { 1.0 };
                let x0 = stem_xs[run_start];
                let hook_len = if reference_pair { 1.25 } else { HOOK_LEN_U } * space;
                (x0, x0 + hook_dir * hook_len)
            } else {
                (stem_xs[run_start], stem_xs[run_end])
            };
            svg.push_str(&glyphs::beam_segment(
                x0,
                beam_at(x0) + offset,
                x1,
                beam_at(x1) + offset,
                BEAM_THICKNESS_U * space,
            ));
        }
    }
    BeamPlan { tips, svg }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn cross_staff_mixed_durations_place_hooks_toward_the_short_note() {
        let space = 20.0;
        let xs = [0.0, 100.0];
        for short in [Duration::Sixteenth, Duration::ThirtySecond] {
            for short_index in [0, 1] {
                let mut durations = [Duration::Eighth, Duration::Eighth];
                durations[short_index] = short.clone();
                for (ys, ups, direction) in [
                    ([0.0, 250.0], [false, true], 1.0),
                    ([250.0, 0.0], [true, false], -1.0),
                ] {
                    let plan = plan_kneed_beam_group(&durations, &xs, &ys, &ups, space, true);
                    let polygons: Vec<Vec<(f32, f32)>> = plan
                        .svg
                        .split(r#"points=""#)
                        .skip(1)
                        .map(|fragment| {
                            fragment
                                .split('"')
                                .next()
                                .unwrap()
                                .split_whitespace()
                                .map(|p| {
                                    let (x, y) = p.split_once(',').unwrap();
                                    (x.parse().unwrap(), y.parse().unwrap())
                                })
                                .collect()
                        })
                        .collect();
                    assert_eq!(polygons.len(), usize::from(beam_level(&short)));
                    let delta = plan.tips[&1] - plan.tips[&0];
                    assert!((delta - direction * 0.75 * space).abs() < 0.02);
                    let primary_slope = (polygons[0][1].1 - polygons[0][0].1)
                        / (polygons[0][1].0 - polygons[0][0].0);
                    for (i, polygon) in polygons.iter().enumerate().skip(1) {
                        let hook_len = polygon[1].0 - polygon[0].0;
                        let hook_direction = if short_index == 0 { 1.0 } else { -1.0 };
                        assert!((hook_len - hook_direction * 1.25 * space).abs() < 0.02);
                        let slope = (polygon[1].1 - polygon[0].1) / hook_len;
                        assert!((slope - primary_slope).abs() < 0.001);
                        let primary_y =
                            polygons[0][0].1 + primary_slope * (polygon[0].0 - polygons[0][0].0);
                        let side = if ups[short_index] { 1.0 } else { -1.0 };
                        assert!(
                            (polygon[0].1 - primary_y - side * i as f32 * 0.75 * space).abs()
                                < 0.03
                        );
                    }
                }
            }
        }
    }

    #[test]
    fn two_note_cross_staff_multiple_beams_follow_reference_geometry() {
        let xs = [0.0, 100.0];
        let space = 20.0;
        for (duration, count) in [(Duration::Sixteenth, 2), (Duration::ThirtySecond, 3)] {
            let durations = [duration.clone(), duration];
            for (ys, ups) in [([0.0, 250.0], [false, true]), ([250.0, 0.0], [true, false])] {
                let plan = plan_kneed_beam_group(&durations, &xs, &ys, &ups, space, true);
                let polygons: Vec<Vec<(f32, f32)>> = plan
                    .svg
                    .split(r#"points=""#)
                    .skip(1)
                    .map(|fragment| {
                        fragment
                            .split('"')
                            .next()
                            .unwrap()
                            .split_whitespace()
                            .map(|point| {
                                let (x, y) = point.split_once(',').unwrap();
                                (x.parse().unwrap(), y.parse().unwrap())
                            })
                            .collect()
                    })
                    .collect();
                assert_eq!(polygons.len(), count);
                for pair in polygons.windows(2) {
                    let slope0 = (pair[0][1].1 - pair[0][0].1) / (pair[0][1].0 - pair[0][0].0);
                    let slope1 = (pair[1][1].1 - pair[1][0].1) / (pair[1][1].0 - pair[1][0].0);
                    let first_at_same_x = pair[0][0].1 + slope0 * (pair[1][0].0 - pair[0][0].0);
                    let gap = pair[1][0].1 - first_at_same_x;
                    let expected = if ups[0] { 0.75 } else { -0.75 } * space;
                    assert!((gap - expected).abs() < 0.02);
                    assert!((slope0 - slope1).abs() < 0.001);
                }
                let delta = plan.tips[&1] - plan.tips[&0];
                let expected = if ups[0] {
                    (-1.0 + 0.75 * (count - 1) as f32 + 0.25) * space
                } else {
                    (0.5 - 0.75 * (count - 1) as f32) * space
                };
                assert!((delta - expected).abs() < 0.02, "{delta} vs {expected}");
            }
        }
    }

    #[test]
    fn cross_staff_two_note_beam_slopes_only_when_clearance_allows() {
        let durations = [Duration::Eighth, Duration::Eighth];
        let xs = [0.0, 100.0];
        let space = 20.0;
        for (ys, stems, direction) in [
            ([0.0, 200.0], [false, true], 1.0),
            ([200.0, 0.0], [true, false], -1.0),
        ] {
            let plan = plan_kneed_beam_group(&durations, &xs, &ys, &stems, space, true);
            assert!((plan.tips[&1] - plan.tips[&0] - direction * 0.75 * space).abs() < 0.01);
            for (index, &up) in stems.iter().enumerate() {
                let clearance = (plan.tips[&index] - ys[index]) * if up { -1.0 } else { 1.0 };
                assert!(clearance >= MIN_STEM_LEN_U * space - 0.01);
            }
        }
        // A gap exactly large enough for a horizontal beam must not be tilted into a head.
        let flat =
            plan_kneed_beam_group(&durations, &xs, &[0.0, 80.0], &[false, true], space, true);
        assert!((flat.tips[&1] - flat.tips[&0] + 0.25 * space).abs() < 0.01);
        // Mixed stems on one staff are not governed by the cross-staff rule.
        let same_staff =
            plan_kneed_beam_group(&durations, &xs, &[0.0, 200.0], &[false, true], space, false);
        assert!((same_staff.tips[&1] - same_staff.tips[&0]).abs() < 0.01);
    }

    #[test]
    fn kneed_beam_lies_between_down_and_up_stemmed_heads() {
        let durations = vec![Duration::Eighth, Duration::Sixteenth, Duration::Sixteenth];
        let xs = vec![0.0, 20.0, 40.0];
        // Two upper-staff heads (stems down) and one lower-staff head (stem up).
        let attach_ys = vec![100.0, 90.0, 400.0];
        let plan = plan_kneed_beam_group(
            &durations,
            &xs,
            &attach_ys,
            &[false, false, true],
            20.0,
            false,
        );
        let primary = plan.tips[&0];
        assert!(primary > 100.0 + MIN_STEM_LEN_U * 20.0);
        assert!(primary < 400.0 - MIN_STEM_LEN_U * 20.0);
        // The sixteenth run's secondary beam sits above the primary (toward the first run
        // note's head), so the up-stemmed lower note is lengthened to reach it.
        assert!(plan.tips[&2] < primary);
        assert!((plan.tips[&1] - primary).abs() < 0.01);
    }

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
    fn two_note_beam_rise_matches_reference_quarter_space() {
        let xs = vec![0.0, 50.0];
        let space = 20.0;
        for duration in [Duration::Eighth, Duration::Sixteenth] {
            let durations = vec![duration; 2];
            for stem_up in [true, false] {
                for rise in [-100.0, -10.0, 10.0, 100.0] {
                    let plan =
                        plan_beam_group(&durations, &xs, &[100.0, 100.0 + rise], stem_up, space);
                    let delta = plan.tips[&1] - plan.tips[&0];
                    assert!((delta - rise.signum() * TWO_NOTE_BEAM_RISE_U * space).abs() < 0.01);
                }
            }
        }
        for duration in [Duration::ThirtySecond, Duration::SixtyFourth] {
            let plan = plan_beam_group(
                &[duration.clone(), duration],
                &xs,
                &[100.0, 60.0],
                true,
                space,
            );
            assert!((plan.tips[&1] - plan.tips[&0] + space).abs() < 0.01);
        }
        let wide = plan_beam_group(
            &[Duration::Eighth, Duration::Eighth],
            &[0.0, 80.0],
            &[100.0, 60.0],
            true,
            space,
        );
        assert!((wide.tips[&1] - wide.tips[&0] + space).abs() < 0.01);
    }

    #[test]
    fn interior_beam_side_extreme_makes_the_beam_horizontal() {
        let durations = vec![Duration::Eighth; 3];
        let xs = vec![0.0, 20.0, 40.0];
        for (stem_up, ys) in [
            (true, [100.0, 80.0, 90.0]),
            (true, [100.0, 90.0, 90.0]),
            (false, [80.0, 100.0, 90.0]),
            (false, [80.0, 90.0, 90.0]),
        ] {
            let plan = plan_beam_group(&durations, &xs, &ys, stem_up, 20.0);
            assert!((plan.tips[&0] - plan.tips[&2]).abs() < 0.01);
        }
        for (stem_up, ys) in [(true, [100.0, 90.0, 80.0]), (false, [80.0, 90.0, 100.0])] {
            let plan = plan_beam_group(&durations, &xs, &ys, stem_up, 20.0);
            assert!((plan.tips[&0] - plan.tips[&2]).abs() > 0.01);
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
    fn beam_moves_past_an_interior_head_on_the_wrong_side() {
        let durations = vec![Duration::Eighth; 3];
        let xs = vec![0.0, 20.0, 40.0];
        let space = 20.0;
        for stem_up in [true, false] {
            let dir = if stem_up { -1.0 } else { 1.0 };
            let ys = vec![100.0, 100.0 + dir * 7.0 * space, 100.0];
            let plan = plan_beam_group(&durations, &xs, &ys, stem_up, space);
            for (index, y) in ys.iter().enumerate() {
                let signed_length = (plan.tips[&index] - y) * dir;
                assert!(signed_length >= MIN_STEM_LEN_U * space - 0.01);
            }
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

    #[test]
    fn sloped_beam_bounds_include_clearance_shift_and_secondary_levels() {
        let durations = vec![
            Duration::Sixteenth,
            Duration::Sixteenth,
            Duration::Sixteenth,
        ];
        let xs = vec![0.0, 0.5, 1.0];
        let attach_ys = vec![0.0, -1.0, 0.0];
        let (min_y, max_y) = vertical_extents(&durations, &xs, &attach_ys, true);
        assert!(
            min_y < -3.0,
            "top bound must include secondary beams: {min_y}"
        );
        assert!(
            max_y < -1.5,
            "the inner (secondary) beam must clear the highest notehead: {max_y}"
        );
        let (min_down, max_down) = vertical_extents(&durations, &xs, &attach_ys, false);
        assert!(
            min_down > 1.5,
            "the inner (secondary) beam must clear the lowest notehead: {min_down}"
        );
        assert!(
            max_down > 3.0,
            "bottom bound must include secondary beams: {max_down}"
        );
    }
}
