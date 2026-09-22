//! Renderer-local adapters for the backend-neutral glyph collision resolver.

use acorde_layout::{
    GlyphCollisionClass, GlyphCollisionDirection, GlyphPlacement, GlyphPlacementError,
};

/// Owns one renderer-local constrained collision pass.
///
/// The owner keeps fixed placements at the front of the collection. Callers can therefore pass
/// resolved semantic annotations to a later span or tablature pass without duplicating the
/// priority, class, direction, and skip bookkeeping at every call site.
pub(crate) struct CollisionOwner {
    fixed_count: usize,
    placements: Vec<GlyphPlacement>,
    classes: Vec<GlyphCollisionClass>,
    directions: Vec<GlyphCollisionDirection>,
}

impl CollisionOwner {
    /// Start a pass with placements that must not be displaced by later owners.
    pub(crate) fn with_fixed_obstacles(obstacles: &[GlyphPlacement]) -> Self {
        let (placements, classes, directions) = fixed_obstacles(obstacles);
        Self {
            fixed_count: placements.len(),
            placements,
            classes,
            directions,
        }
    }

    /// Add one placement owned by this pass.
    pub(crate) fn push(
        &mut self,
        placement: GlyphPlacement,
        class: GlyphCollisionClass,
        direction: GlyphCollisionDirection,
    ) {
        self.placements.push(placement);
        self.classes.push(class);
        self.directions.push(direction);
    }

    /// Resolve every placement through the shared backend-neutral layout primitive.
    pub(crate) fn resolve(&mut self, gap: f32) -> Result<(), GlyphPlacementError> {
        acorde_layout::resolve_glyph_collisions_constrained(
            &mut self.placements,
            &self.classes,
            &self.directions,
            gap,
        )
        .map(|_| ())
    }

    /// Number of fixed obstacles at the start of the resolved placement list.
    pub(crate) fn fixed_count(&self) -> usize {
        self.fixed_count
    }

    /// Consume the pass after resolution.
    pub(crate) fn into_placements(self) -> Vec<GlyphPlacement> {
        self.placements
    }
}

/// Clone resolved placements as immutable, highest-priority obstacles for a later SVG pass.
///
/// The layout crate owns collision geometry. This adapter only records renderer ordering: once a
/// pass has emitted an annotation, later span or tablature passes must route around it.
pub(crate) fn fixed_obstacles(
    obstacles: &[GlyphPlacement],
) -> (
    Vec<GlyphPlacement>,
    Vec<GlyphCollisionClass>,
    Vec<GlyphCollisionDirection>,
) {
    let mut placements = obstacles.to_vec();
    for placement in &mut placements {
        placement.priority = u8::MAX;
    }
    let count = placements.len();
    (
        placements,
        vec![GlyphCollisionClass::Critical; count],
        vec![GlyphCollisionDirection::Fixed; count],
    )
}

#[cfg(test)]
mod tests {
    use super::*;
    use acorde_layout::GlyphMetrics;

    #[test]
    fn fixed_obstacles_preserve_geometry_and_dominate_later_passes() {
        let source = GlyphPlacement {
            resource_key: "dynamic:0".into(),
            metrics: GlyphMetrics {
                advance_mm: 0.0,
                left_mm: -2.0,
                top_mm: -3.0,
                width_mm: 4.0,
                height_mm: 6.0,
            },
            x_mm: 10.0,
            y_mm: 20.0,
            priority: 1,
        };

        let mut owner = CollisionOwner::with_fixed_obstacles(std::slice::from_ref(&source));
        owner.push(
            GlyphPlacement {
                resource_key: "later".into(),
                metrics: source.metrics,
                x_mm: 10.0,
                y_mm: 20.0,
                priority: 1,
            },
            GlyphCollisionClass::Annotation,
            GlyphCollisionDirection::Down,
        );
        owner.resolve(1.0).expect("collision pass resolves");
        assert_eq!(owner.fixed_count(), 1);
        let placements = owner.into_placements();
        assert_eq!(placements[0].resource_key, source.resource_key);
        assert_eq!(placements[0].metrics, source.metrics);
        assert_eq!((placements[0].x_mm, placements[0].y_mm), (10.0, 20.0));
        assert_eq!(placements[0].priority, u8::MAX);
        assert!(placements[1].y_mm > 20.0);
    }
}
