use acorde_core::{Clef, Duration, Measure, Note, Part, Pitch, Score, Staff, Step};
use acorde_layout::{GlyphCollisionClass, GlyphCollisionDirection, LayoutConfig, compute_layout};
use acorde_render_svg::{
    RenderAnnotation, RenderAnnotationError, SvgAnnotation, SvgAnnotationCollisionPolicy,
    SvgAnnotationMetrics, SvgRenderOptions, render_svg_with_annotations,
};

fn score() -> Score {
    let mut score = Score::default();
    let mut part = Part::new("Test", "T");
    let mut staff = Staff::new(Clef::Treble);
    let mut measure = Measure::empty(4, 4);
    measure.voices[0].push(Note::new(Pitch::new(Step::C, 4), Duration::Quarter));
    staff.measures.push(measure);
    part.staves.push(staff);
    score.parts = vec![part];
    score
}

struct Provider {
    name: &'static str,
    marks: Vec<SvgAnnotation>,
    collision_aware: bool,
}

impl RenderAnnotation for Provider {
    fn id(&self) -> &str {
        self.name
    }

    fn annotate(
        &self,
        _score: &Score,
        _layout: &acorde_layout::LayoutResult,
        _metadata: &acorde_render_svg::RenderMetadata,
    ) -> Vec<SvgAnnotation> {
        self.marks.clone()
    }

    fn collision_policy(
        &self,
        _annotation: &SvgAnnotation,
    ) -> Option<(SvgAnnotationMetrics, SvgAnnotationCollisionPolicy)> {
        self.collision_aware.then_some((
            SvgAnnotationMetrics {
                left_px: 0.0,
                top_px: -10.0,
                width_px: 20.0,
                height_px: 10.0,
            },
            SvgAnnotationCollisionPolicy {
                class: GlyphCollisionClass::Annotation,
                direction: GlyphCollisionDirection::Down,
                priority: 0,
            },
        ))
    }
}

#[test]
fn annotations_are_sorted_and_escaped() {
    let score = score();
    let layout = compute_layout(&score, &LayoutConfig::default());
    let first = Provider {
        name: "z-provider",
        collision_aware: false,
        marks: vec![SvgAnnotation {
            id: "z-mark".into(),
            x: 10.0,
            y: 20.0,
            text: "A < B".into(),
        }],
    };
    let second = Provider {
        name: "a-provider",
        collision_aware: false,
        marks: vec![SvgAnnotation {
            id: "a-mark".into(),
            x: 30.0,
            y: 40.0,
            text: "quoted \"text\"".into(),
        }],
    };
    let providers: [&dyn RenderAnnotation; 2] = [&first, &second];
    let svg =
        render_svg_with_annotations(&score, &layout, &SvgRenderOptions::default(), &providers)
            .unwrap();
    assert!(
        svg.find("data-acorde-annotation-id=\"a-mark\"").unwrap()
            < svg.find("data-acorde-annotation-id=\"z-mark\"").unwrap()
    );
    assert!(svg.contains("A &lt; B"));
    assert!(svg.contains("quoted &quot;text&quot;"));
}

#[test]
fn collision_aware_annotations_use_the_shared_escape_lane() {
    let score = score();
    let layout = compute_layout(&score, &LayoutConfig::default());
    let provider = Provider {
        name: "collision",
        collision_aware: true,
        marks: vec![
            SvgAnnotation {
                id: "a-anchor".into(),
                x: 10.0,
                y: 20.0,
                text: "anchor".into(),
            },
            SvgAnnotation {
                id: "b-lyric".into(),
                x: 10.0,
                y: 20.0,
                text: "lyric".into(),
            },
        ],
    };
    let providers: [&dyn RenderAnnotation; 1] = [&provider];
    let svg =
        render_svg_with_annotations(&score, &layout, &SvgRenderOptions::default(), &providers)
            .expect("collision-aware annotations render");

    assert!(svg.contains(r#"data-acorde-annotation-id="a-anchor" x="10" y="20""#));
    assert!(svg.contains(r#"data-acorde-annotation-id="b-lyric" x="10" y="32""#));
}

#[test]
fn collision_aware_annotations_reject_invalid_bounds() {
    let score = score();
    let layout = compute_layout(&score, &LayoutConfig::default());
    struct InvalidBoundsProvider;
    impl RenderAnnotation for InvalidBoundsProvider {
        fn id(&self) -> &str {
            "invalid-bounds"
        }

        fn annotate(
            &self,
            _score: &Score,
            _layout: &acorde_layout::LayoutResult,
            _metadata: &acorde_render_svg::RenderMetadata,
        ) -> Vec<SvgAnnotation> {
            vec![SvgAnnotation {
                id: "bad".into(),
                x: 10.0,
                y: 20.0,
                text: "bad".into(),
            }]
        }

        fn collision_policy(
            &self,
            _annotation: &SvgAnnotation,
        ) -> Option<(SvgAnnotationMetrics, SvgAnnotationCollisionPolicy)> {
            Some((
                SvgAnnotationMetrics {
                    left_px: 0.0,
                    top_px: 0.0,
                    width_px: f32::NAN,
                    height_px: 1.0,
                },
                SvgAnnotationCollisionPolicy {
                    class: GlyphCollisionClass::Annotation,
                    direction: GlyphCollisionDirection::Down,
                    priority: 0,
                },
            ))
        }
    }

    let provider = InvalidBoundsProvider;
    let error =
        render_svg_with_annotations(&score, &layout, &SvgRenderOptions::default(), &[&provider])
            .expect_err("non-finite collision metric is invalid");
    assert!(matches!(
        error,
        acorde_render_svg::RenderError::Annotation(
            RenderAnnotationError::InvalidCollisionMetrics { .. }
        )
    ));
}

#[test]
fn duplicate_provider_ids_are_rejected() {
    let score = score();
    let layout = compute_layout(&score, &LayoutConfig::default());
    let first = Provider {
        name: "same",
        marks: Vec::new(),
        collision_aware: false,
    };
    let second = Provider {
        name: "same",
        marks: Vec::new(),
        collision_aware: false,
    };
    let providers: [&dyn RenderAnnotation; 2] = [&first, &second];
    let error =
        render_svg_with_annotations(&score, &layout, &SvgRenderOptions::default(), &providers)
            .unwrap_err();
    assert!(matches!(
        error,
        acorde_render_svg::RenderError::Annotation(RenderAnnotationError::DuplicateProviderId(_))
    ));
}

#[test]
fn oversized_annotation_text_is_rejected() {
    let score = score();
    let layout = compute_layout(&score, &LayoutConfig::default());
    let provider = Provider {
        name: "large",
        collision_aware: false,
        marks: vec![SvgAnnotation {
            id: "large-mark".into(),
            x: 10.0,
            y: 20.0,
            text: "x".repeat(16 * 1024 + 1),
        }],
    };
    let providers: [&dyn RenderAnnotation; 1] = [&provider];
    let error =
        render_svg_with_annotations(&score, &layout, &SvgRenderOptions::default(), &providers)
            .unwrap_err();
    assert!(matches!(
        error,
        acorde_render_svg::RenderError::Annotation(
            RenderAnnotationError::AnnotationTextTooLarge { .. }
        )
    ));
}

#[test]
fn excessive_annotation_count_is_rejected() {
    let score = score();
    let layout = compute_layout(&score, &LayoutConfig::default());
    let provider = Provider {
        name: "many",
        collision_aware: false,
        marks: (0..10_001)
            .map(|index| SvgAnnotation {
                id: format!("mark-{index}"),
                x: 10.0,
                y: 20.0,
                text: "mark".into(),
            })
            .collect(),
    };
    let providers: [&dyn RenderAnnotation; 1] = [&provider];
    let error =
        render_svg_with_annotations(&score, &layout, &SvgRenderOptions::default(), &providers)
            .unwrap_err();
    assert!(matches!(
        error,
        acorde_render_svg::RenderError::Annotation(RenderAnnotationError::TooManyAnnotations {
            count: 10_001
        })
    ));
}
