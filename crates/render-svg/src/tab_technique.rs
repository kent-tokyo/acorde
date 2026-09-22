//! Collision-lane allocation for tablature technique connections.

use acorde_layout::{GlyphCollisionClass, GlyphCollisionDirection, GlyphMetrics, GlyphPlacement};

use crate::RenderError;
use crate::SVG_ANNOTATION_COLLISION_GAP_PX;
use crate::collision::CollisionOwner;

pub(crate) struct OwnedSegment {
    pub(crate) technique: acorde_core::GuitarTechnique,
    pub(crate) string: u8,
    pub(crate) start_addr: String,
    pub(crate) end_addr: String,
    pub(crate) start: f32,
    pub(crate) y1: f32,
    pub(crate) end: f32,
    pub(crate) y2: f32,
}

pub(crate) fn resolve_lane_offsets(
    segments: &[OwnedSegment],
    space: f32,
    obstacles: &[GlyphPlacement],
) -> Result<Vec<f32>, RenderError> {
    if segments.len() < 2 && obstacles.is_empty() {
        return Ok(vec![0.0; segments.len()]);
    }
    let original_y: Vec<f32> = segments
        .iter()
        .map(|segment| (segment.y1 + segment.y2) / 2.0)
        .collect();
    let mut owner = CollisionOwner::with_fixed_obstacles(obstacles);
    for (index, segment) in segments.iter().enumerate() {
        owner.push(
            GlyphPlacement {
                resource_key: format!("tab-technique:{index}"),
                metrics: GlyphMetrics {
                    advance_mm: 0.0,
                    left_mm: segment.start,
                    top_mm: -0.35 * space,
                    width_mm: (segment.end - segment.start).abs().max(0.1 * space),
                    height_mm: 0.7 * space,
                },
                x_mm: 0.0,
                y_mm: original_y[index],
                priority: 1,
            },
            GlyphCollisionClass::Annotation,
            GlyphCollisionDirection::Up,
        );
    }
    owner
        .resolve(SVG_ANNOTATION_COLLISION_GAP_PX)
        .map_err(|_| RenderError::InvalidNotePlacement {
            field: "tablature technique collision",
        })?;
    let fixed_count = owner.fixed_count();
    Ok(owner
        .into_placements()
        .into_iter()
        .skip(fixed_count)
        .zip(original_y)
        .map(|(placement, original_y)| placement.y_mm - original_y)
        .collect())
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn local_tab_connection_escapes_semantic_annotation_obstacle() {
        let segment = OwnedSegment {
            technique: acorde_core::GuitarTechnique::Slide,
            string: 2,
            start_addr: "0:0:0:0:0".to_owned(),
            end_addr: "0:0:0:0:1".to_owned(),
            start: 30.0,
            y1: 100.0,
            end: 90.0,
            y2: 100.0,
        };
        let obstacle = GlyphPlacement {
            resource_key: "dynamic:0:0:0:0:0".into(),
            metrics: GlyphMetrics {
                advance_mm: 0.0,
                left_mm: 30.0,
                top_mm: -4.0,
                width_mm: 60.0,
                height_mm: 8.0,
            },
            x_mm: 0.0,
            y_mm: 100.0,
            priority: 1,
        };

        let offsets = resolve_lane_offsets(&[segment], 10.0, &[obstacle])
            .expect("tab lane resolves around annotation");
        assert_eq!(offsets.len(), 1);
        assert!(offsets[0] < 0.0);
    }
}
