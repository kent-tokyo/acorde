//! Collision-lane allocation for score spans.

use std::collections::HashMap;

use acorde_core::{Duration, Score};
use acorde_layout::{
    GlyphCollisionClass, GlyphCollisionDirection, GlyphMetrics, GlyphPlacement, LayoutResult,
    NoteAddr, SpanMark,
};

use crate::SVG_ANNOTATION_COLLISION_GAP_PX;
use crate::collision::CollisionOwner;
use crate::render::{NoteKey, NotePoint};

/// Conventional single-voice slurs sit opposite stems. For a lower voice,
/// keep the slur below the staff to distinguish it from the upper voice;
/// a single-voice phrase with mixed stem directions uses the upper side.
pub(crate) fn slur_above(
    score: &Score,
    layout: &LayoutResult,
    points: &HashMap<NoteKey, NotePoint>,
    start: &NoteAddr,
    end: &NoteAddr,
    up1: bool,
    up2: bool,
) -> bool {
    if let Some(placement) = layout
        .typed_spanners
        .iter()
        .find(|span| {
            span.kind == acorde_core::NotationSpannerKind::Slur
                && span.start == *start
                && span.end == *end
        })
        .and_then(|span| span.placement.as_deref())
    {
        match placement {
            "above" => return true,
            "below" => return false,
            _ => {}
        }
    }
    if start.voice != end.voice {
        return start.voice == 0;
    }
    if start.voice > 0 {
        return false;
    }
    if !(up1 && up2) {
        return true;
    }
    if start.part != end.part || start.staff != end.staff {
        return false;
    }
    let lower = (start.measure, start.note).min((end.measure, end.note));
    let upper = (start.measure, start.note).max((end.measure, end.note));
    points
        .iter()
        .any(|(&(part, staff, measure, voice, index), &(_, _, up, _))| {
            if up
                || part != start.part
                || staff != start.staff
                || voice != start.voice
                || (measure, index) <= lower
                || (measure, index) >= upper
            {
                return false;
            }
            score
                .parts
                .get(part)
                .and_then(|p| p.staves.get(staff))
                .and_then(|s| s.measures.get(measure))
                .and_then(|m| m.voices.get(voice))
                .and_then(|v| v.get(index))
                .is_some_and(|note| {
                    !note.is_rest
                        && note.cross_staff.is_none()
                        && !matches!(
                            note.duration,
                            Duration::Whole | Duration::Breve | Duration::Longa
                        )
                })
        })
}

/// Place an unqualified hairpin below the notes of its staff. The canonical hairpin model
/// currently does not retain an authored above/below placement.
pub(crate) fn hairpin_baseline(
    points: &HashMap<NoteKey, NotePoint>,
    staff_bottoms: &HashMap<(usize, usize, usize), f32>,
    start: &NoteAddr,
    row: usize,
    x_range: (f32, f32),
    fallback_y: f32,
    space: f32,
) -> (f32, bool) {
    let mut bottom: Option<f32> = None;
    for (&(part, staff, _, _, _), &(x, y, _, point_row)) in points {
        if part == start.part
            && staff == start.staff
            && point_row == row
            && x >= x_range.0.min(x_range.1) - 0.5 * space
            && x <= x_range.0.max(x_range.1) + 0.5 * space
        {
            bottom = Some(bottom.map_or(y, |v| v.max(y)));
        }
    }
    let staff_floor = staff_bottoms
        .get(&(start.part, start.staff, row))
        .copied()
        .unwrap_or(fallback_y);
    (
        (bottom.unwrap_or(fallback_y) + 4.0 * space).max(staff_floor + 1.2 * space),
        false,
    )
}

/// Allocate deterministic escape lanes for spans sharing a system.
#[allow(clippy::too_many_arguments)]
pub(crate) fn resolve_lane_offsets(
    score: &Score,
    layout: &LayoutResult,
    points: &HashMap<NoteKey, NotePoint>,
    staff_bottoms: &HashMap<(usize, usize, usize), f32>,
    width: f32,
    left_margin_u: f32,
    right_margin_u: f32,
    space: f32,
    resolved_annotation_obstacles: &[GlyphPlacement],
) -> HashMap<(usize, usize), f32> {
    let mut keys = Vec::new();
    let mut original_y = Vec::new();
    let mut owner = CollisionOwner::with_fixed_obstacles(resolved_annotation_obstacles);

    for (span_index, span) in layout.spans.iter().enumerate() {
        let (start, end) = match span {
            SpanMark::Hairpin { start, end, .. }
            | SpanMark::Ottava { start, end, .. }
            | SpanMark::Pedal { start, end }
            | SpanMark::Slur { start, end }
            | SpanMark::TrillLine { start, end }
            | SpanMark::Glissando { start, end }
            | SpanMark::Harmony { start, end, .. } => (start, end),
        };
        let (Some(&(x1, y1, up1, row1)), Some(&(x2, y2, up2, row2))) = (
            points.get(&(
                start.part,
                start.staff,
                start.measure,
                start.voice,
                start.note,
            )),
            points.get(&(end.part, end.staff, end.measure, end.voice, end.note)),
        ) else {
            continue;
        };
        let slur_side = matches!(span, SpanMark::Slur { .. })
            .then(|| slur_above(score, layout, points, start, end, up1, up2));
        let mut add_segment = |row: usize, left: f32, right: f32, anchor_y: f32, stem_up: bool| {
            let (baseline, above) = if matches!(span, SpanMark::Hairpin { .. }) {
                hairpin_baseline(
                    points,
                    staff_bottoms,
                    start,
                    row,
                    (left, right),
                    anchor_y,
                    space,
                )
            } else {
                slur_side
                    .map(|above| (anchor_y, above))
                    .unwrap_or_else(|| lane_baseline(span, anchor_y, stem_up, space))
            };
            keys.push((span_index, row));
            original_y.push(baseline);
            owner.push(
                GlyphPlacement {
                    resource_key: format!("span:{span_index}:{row}"),
                    metrics: GlyphMetrics {
                        advance_mm: 0.0,
                        left_mm: left.min(right),
                        top_mm: -0.4 * space,
                        width_mm: (right - left).abs().max(0.1 * space),
                        height_mm: 0.8 * space,
                    },
                    x_mm: 0.0,
                    y_mm: baseline,
                    priority: 1,
                },
                GlyphCollisionClass::Annotation,
                if above {
                    GlyphCollisionDirection::Up
                } else {
                    GlyphCollisionDirection::Down
                },
            );
        };
        if row1 == row2 {
            add_segment(row1, x1, x2, (y1 + y2) / 2.0, up1 || up2);
        } else {
            add_segment(row1, x1, width - right_margin_u * space, y1, up1);
            add_segment(row2, left_margin_u * space, x2, y2, up2);
        }
    }
    if keys.is_empty() || owner.resolve(SVG_ANNOTATION_COLLISION_GAP_PX).is_err() {
        return HashMap::new();
    }
    let fixed_count = owner.fixed_count();
    keys.into_iter()
        .zip(original_y)
        .zip(owner.into_placements().into_iter().skip(fixed_count))
        .map(|((key, original_y), placement)| (key, placement.y_mm - original_y))
        .collect()
}

pub(crate) fn lane_baseline(
    span: &SpanMark,
    anchor_y: f32,
    stem_up: bool,
    space: f32,
) -> (f32, bool) {
    match span {
        SpanMark::Hairpin { .. } => (anchor_y + (if stem_up { 2.0 } else { -4.0 }) * space, false),
        SpanMark::Pedal { .. } => (anchor_y + 2.0 * space, false),
        SpanMark::Ottava { kind, .. } => {
            let above = matches!(
                kind,
                acorde_core::OttavaKind::Va8 | acorde_core::OttavaKind::Ma15
            );
            (anchor_y + (if above { -5.8 } else { 1.5 }) * space, above)
        }
        SpanMark::Harmony { .. } => (anchor_y - 6.8 * space, true),
        SpanMark::Slur { .. } | SpanMark::TrillLine { .. } | SpanMark::Glissando { .. } => {
            (anchor_y, stem_up)
        }
    }
}
