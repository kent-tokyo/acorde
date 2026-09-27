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

/// Vertical extent of the staff an annotation pass works on, in SVG pixels.
#[derive(Clone, Copy)]
pub(crate) struct StaffBand {
    pub(crate) top_y: f32,
    pub(crate) bottom_y: f32,
}

/// Where an articulation belongs, following engraving practice: staccato, staccatissimo,
/// tenuto and accent go at the notehead, away from the stem; fermatas, ornaments, marcato and
/// string marks go outside the staff above (below for a lower voice).
fn articulation_at_notehead(articulation: &Articulation) -> bool {
    matches!(
        articulation,
        Articulation::Staccato
            | Articulation::Staccatissimo
            | Articulation::Tenuto
            | Articulation::Accent
    )
}

/// Render note-attached semantics through one constrained collision pass.
///
/// Lyrics, dynamics and chord symbols sit on lines measured from the staff, not from each
/// note, so a line of syllables or dynamics reads level across the bar as it does in
/// MuseScore and Verovio; the collision pass still moves one away from another.
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
    band: StaffBand,
) -> Result<Vec<GlyphPlacement>, RenderError> {
    let mut annotations = Vec::new();
    let mut owner = CollisionOwner::with_fixed_obstacles(&[]);
    let active_voices = measure
        .voices
        .iter()
        .filter(|voice| voice.iter().any(|note| !note.is_rest))
        .count();
    // A vocal staff carries its dynamics above, clear of the lyrics.
    let has_lyrics = measure
        .voices
        .iter()
        .flatten()
        .any(|note| note.lyric.is_some() || !note.additional_lyrics.is_empty());
    // Stem tips reach about 3.5 spaces from the notehead.
    let stem_reach = 3.5 * space;

    for (voice_idx, voice) in measure.voices.iter().enumerate() {
        for (note_idx, note) in voice.iter().enumerate() {
            let Some(&(x, anchor_y, stem_up, _)) =
                note_points.get(&(part, staff, measure_idx, voice_idx, note_idx))
            else {
                continue;
            };
            let lower_voice = active_voices > 1 && voice_idx % 2 == 1;
            // The note's highest and lowest drawn point (notehead or stem tip).
            let (note_top, note_bottom) = if note.is_rest {
                (anchor_y - space, anchor_y + space)
            } else if stem_up {
                (anchor_y - stem_reach, anchor_y + 0.5 * space)
            } else {
                (anchor_y - 0.5 * space, anchor_y + stem_reach)
            };
            if kinds.dynamics_and_chords {
                for (class, text, above, priority) in [
                    (
                        "acorde-dynamic",
                        note.dynamic
                            .as_ref()
                            .map(|value| value.to_musicxml_str().to_owned()),
                        has_lyrics && !lower_voice,
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
                    let y = match (class, above) {
                        ("acorde-dynamic", true) => {
                            (band.top_y - 2.0 * space).min(note_top - space)
                        }
                        ("acorde-dynamic", false) => {
                            (band.bottom_y + 2.6 * space).max(note_bottom + 1.5 * space)
                        }
                        _ => (band.top_y - 2.8 * space).min(note_top - 1.5 * space),
                    };
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
                            y_mm: y,
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
            // Verse 1 of the lyrics sits on one line below the staff (lower only when a note
            // reaches below it), with dynamics under a lyric line moved above the staff.
            let lyric_baseline = (band.bottom_y + 3.2 * space).max(note_bottom + 1.8 * space);
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
                        y_mm: lyric_baseline,
                        priority: 1,
                    },
                    GlyphCollisionClass::Annotation,
                    GlyphCollisionDirection::Down,
                );
            }
            if kinds.lyrics {
                // Verse n sits (n - 1) lyric lines below verse 1, so each verse keeps one line
                // across notes even when an earlier verse is absent on a note.
                for entry in &note.additional_lyrics {
                    let text = entry.lyric.text.clone();
                    annotations.push(SemanticAnnotation::Text {
                        class: "acorde-lyric-verse",
                        text: text.clone(),
                        x,
                        italic: false,
                    });
                    let width = text.chars().count() as f32 * 0.42 * space + 0.6 * space;
                    let verse_offset =
                        crate::render::LYRIC_VERSE_SPACING * f32::from(entry.verse - 1);
                    owner.push(
                        GlyphPlacement {
                            resource_key: format!(
                                "lyric:{part}:{staff}:{measure_idx}:{voice_idx}:{note_idx}:verse{}",
                                entry.verse
                            ),
                            metrics: GlyphMetrics {
                                advance_mm: 0.0,
                                left_mm: -width / 2.0,
                                top_mm: -0.72 * space,
                                width_mm: width,
                                height_mm: 0.9 * space,
                            },
                            x_mm: x,
                            y_mm: lyric_baseline + verse_offset * space,
                            priority: 1,
                        },
                        GlyphCollisionClass::Annotation,
                        GlyphCollisionDirection::Down,
                    );
                }
            }
            if kinds.articulations {
                for (articulation_idx, articulation) in note.articulations.iter().enumerate() {
                    if matches!(articulation, Articulation::Tremolo(_)) {
                        // Tremolo strokes cross the stem; they are not stacked with the rest.
                        crate::render::write_stem_tremolo(body, note, x, anchor_y, stem_up, space);
                        continue;
                    }
                    let stack = articulation_idx as f32 * 0.9 * space;
                    let (above, y) = if articulation_at_notehead(articulation) {
                        // Opposite the stem, just past the notehead.
                        if stem_up {
                            (false, anchor_y + 1.1 * space + stack)
                        } else {
                            (true, anchor_y - 1.1 * space - stack)
                        }
                    } else if lower_voice {
                        (
                            false,
                            (band.bottom_y + 1.4 * space).max(note_bottom + space) + stack,
                        )
                    } else {
                        (
                            true,
                            (band.top_y - 1.4 * space).min(note_top - space) - stack,
                        )
                    };
                    let stem_up = !above;
                    annotations.push(SemanticAnnotation::Articulation {
                        articulation,
                        x,
                        direction: if above { -1.0 } else { 1.0 },
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
                            y_mm: y,
                            priority: 1,
                        },
                        GlyphCollisionClass::Annotation,
                        if !stem_up {
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
            } => {
                // Dynamics are drawn as engraved letter shapes when every letter has one.
                let drawn = (class == "acorde-dynamic")
                    .then(|| crate::glyphs::dynamic_mark(class, &text, x, placement.y_mm, space))
                    .flatten();
                match drawn {
                    Some(svg) => body.push_str(&svg),
                    None => {
                        write_annotation_text(body, class, &text, x, placement.y_mm, space, italic)
                    }
                }
            }
            SemanticAnnotation::Articulation {
                articulation,
                x,
                direction,
            } => render_articulation(body, articulation, x, placement.y_mm, direction, space),
        }
    }
    Ok(placements)
}
