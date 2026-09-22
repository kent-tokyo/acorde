//! Semantic-annotation collision ownership for SVG rendering.

use std::collections::HashMap;

use acorde_core::{Articulation, Measure};
use acorde_layout::{GlyphCollisionClass, GlyphCollisionDirection, GlyphMetrics, GlyphPlacement};

use crate::RenderError;
use crate::SVG_ANNOTATION_COLLISION_GAP_PX;
use crate::collision::CollisionOwner;
use crate::render::{NoteKey, NotePoint, render_articulation, write_annotation_text};

#[derive(Clone, Copy)]
pub(crate) struct Kinds {
    pub(crate) lyrics: bool,
    pub(crate) dynamics_and_chords: bool,
    pub(crate) articulations: bool,
}

impl Kinds {
    pub(crate) const ALL: Self = Self {
        lyrics: true,
        dynamics_and_chords: true,
        articulations: true,
    };
}

enum SemanticAnnotation<'a> {
    Text {
        class: &'static str,
        text: String,
        x: f32,
        italic: bool,
    },
    Articulation {
        articulation: &'a Articulation,
        x: f32,
        direction: f32,
    },
}

/// Render note-attached semantics through one constrained collision pass.
#[allow(clippy::too_many_arguments)]
pub(crate) fn render_measure_semantic_annotations(
    body: &mut String,
    measure: &Measure,
    part: usize,
    staff: usize,
    measure_idx: usize,
    note_points: &HashMap<NoteKey, NotePoint>,
    space: f32,
    kinds: Kinds,
) -> Result<Vec<GlyphPlacement>, RenderError> {
    let mut annotations = Vec::new();
    let mut owner = CollisionOwner::with_fixed_obstacles(&[]);

    for (voice_idx, voice) in measure.voices.iter().enumerate() {
        for (note_idx, note) in voice.iter().enumerate() {
            let Some(&(x, anchor_y, stem_up, _)) =
                note_points.get(&(part, staff, measure_idx, voice_idx, note_idx))
            else {
                continue;
            };
            if kinds.dynamics_and_chords {
                for (class, text, above, priority) in [
                    (
                        "acorde-dynamic",
                        note.dynamic
                            .as_ref()
                            .map(|value| value.to_musicxml_str().to_owned()),
                        stem_up,
                        1,
                    ),
                    (
                        "acorde-chord-symbol",
                        note.chord_symbol.as_ref().map(|value| value.display_text()),
                        true,
                        2,
                    ),
                ] {
                    let Some(text) = text else {
                        continue;
                    };
                    let distance = if class == "acorde-dynamic" { 4.0 } else { 5.6 };
                    annotations.push(SemanticAnnotation::Text {
                        class,
                        text: text.clone(),
                        x,
                        italic: true,
                    });
                    let width = text.chars().count() as f32 * 0.42 * space + 0.6 * space;
                    owner.push(
                        GlyphPlacement {
                            resource_key: format!(
                                "{class}:{part}:{staff}:{measure_idx}:{voice_idx}:{note_idx}"
                            ),
                            metrics: GlyphMetrics {
                                advance_mm: 0.0,
                                left_mm: -width / 2.0,
                                top_mm: -0.72 * space,
                                width_mm: width,
                                height_mm: 0.9 * space,
                            },
                            x_mm: x,
                            y_mm: if above {
                                anchor_y - distance * space
                            } else {
                                anchor_y + distance * space
                            },
                            priority,
                        },
                        GlyphCollisionClass::Annotation,
                        if above {
                            GlyphCollisionDirection::Up
                        } else {
                            GlyphCollisionDirection::Down
                        },
                    );
                }
            }
            if kinds.lyrics
                && let Some(lyric) = &note.lyric
            {
                let text = lyric.text.clone();
                annotations.push(SemanticAnnotation::Text {
                    class: "acorde-lyric",
                    text: text.clone(),
                    x,
                    italic: false,
                });
                let width = text.chars().count() as f32 * 0.42 * space + 0.6 * space;
                owner.push(
                    GlyphPlacement {
                        resource_key: format!(
                            "lyric:{part}:{staff}:{measure_idx}:{voice_idx}:{note_idx}"
                        ),
                        metrics: GlyphMetrics {
                            advance_mm: 0.0,
                            left_mm: -width / 2.0,
                            top_mm: -0.72 * space,
                            width_mm: width,
                            height_mm: 0.9 * space,
                        },
                        x_mm: x,
                        y_mm: anchor_y
                            + if !stem_up && note.dynamic.is_some() {
                                5.9
                            } else {
                                4.8
                            } * space,
                        priority: 1,
                    },
                    GlyphCollisionClass::Annotation,
                    GlyphCollisionDirection::Down,
                );
            }
            if kinds.articulations {
                for (articulation_idx, articulation) in note.articulations.iter().enumerate() {
                    let distance = 1.2 + articulation_idx as f32;
                    annotations.push(SemanticAnnotation::Articulation {
                        articulation,
                        x,
                        direction: if stem_up { -1.0 } else { 1.0 },
                    });
                    owner.push(
                        GlyphPlacement {
                            resource_key: format!(
                                "articulation:{part}:{staff}:{measure_idx}:{voice_idx}:{note_idx}:{articulation_idx}"
                            ),
                            metrics: GlyphMetrics {
                                advance_mm: 0.0,
                                left_mm: -0.48 * space,
                                top_mm: -0.48 * space,
                                width_mm: 0.96 * space,
                                height_mm: 0.96 * space,
                            },
                            x_mm: x,
                            y_mm: if stem_up {
                                anchor_y - distance * space
                            } else {
                                anchor_y + distance * space
                            },
                            priority: 1,
                        },
                        GlyphCollisionClass::Annotation,
                        if stem_up {
                            GlyphCollisionDirection::Up
                        } else {
                            GlyphCollisionDirection::Down
                        },
                    );
                }
            }
        }
    }
    owner
        .resolve(SVG_ANNOTATION_COLLISION_GAP_PX)
        .map_err(|_| RenderError::InvalidNotePlacement {
            field: "semantic annotation collision",
        })?;
    let placements = owner.into_placements();
    for (annotation, placement) in annotations.into_iter().zip(&placements) {
        match annotation {
            SemanticAnnotation::Text {
                class,
                text,
                x,
                italic,
            } => write_annotation_text(body, class, &text, x, placement.y_mm, space, italic),
            SemanticAnnotation::Articulation {
                articulation,
                x,
                direction,
            } => render_articulation(body, articulation, x, placement.y_mm, direction, space),
        }
    }
    Ok(placements)
}
