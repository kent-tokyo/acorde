//! Measured print entry point. Fonts are provided as bytes and geometry remains portable.

use crate::{FontMetricsError, PrintRenderError, RenderError, TextFont};
use acorde_core::Score;
use acorde_layout::{
    LayoutConfig, PartLayoutPolicy, PrintConfig, PrintLayoutResult, PrintMeasurements,
    break_measured_print_systems, compute_layout,
};

#[derive(Debug, thiserror::Error)]
pub enum MeasuredEngravingError {
    #[error("font outlining failed: {0}")]
    Outline(#[from] crate::OutlineTextError),
    #[error("font measurement failed: {0}")]
    Font(#[from] FontMetricsError),
    #[error("print layout failed: {0}")]
    Layout(#[from] acorde_layout::MeasuredPrintError),
    #[error("notation measurement failed: {0}")]
    Render(#[from] RenderError),
    #[error("page rendering failed: {0}")]
    Page(#[from] PrintRenderError),
    #[error("measured engraving currently requires scale 1.0 (1.8 mm staff space)")]
    UnsupportedScale,
    #[error("the requested part does not exist")]
    InvalidPart,
    #[error("generated print geometry is inconsistent")]
    InvalidGeneratedGeometry,
    #[error("print collision reflow did not converge within 16 passes")]
    CollisionReflowLimit,
    #[error("a direction is wider than the available system width")]
    DirectionTooWide,
}

/// Page SVGs and the exact physical layout that generated them.
#[derive(Debug, serde::Serialize)]
pub struct MeasuredPrintOutput {
    pub layout: PrintLayoutResult,
    pub pages: Vec<String>,
    pub staff_space_mm: f32,
    /// Additional indentation of the opening system beyond its page content inset.
    pub first_system_inset_mm: f32,
    /// Additional inset for each system, in system-index order.
    pub system_insets_mm: Vec<f32>,
    pub measurements: PrintMeasurements,
}

/// Measure caller-supplied lyric font bytes, choose safe system breaks and paginate
/// before rendering. This opt-in path fixes staff space at 1.8 mm and lyric size at
/// 4.05 mm. It returns a typed failure when physical constraints cannot be satisfied.
/// Text is outlined from these same bytes, so SVG and PDF do not depend on installed
/// fonts. Bold and italic requests use synthetic weight/slant of the supplied face.
pub fn engrave_print_pages(
    score: &Score,
    config: &PrintConfig,
    font_bytes: &[u8],
) -> Result<MeasuredPrintOutput, MeasuredEngravingError> {
    if config.scale != 1.0 {
        return Err(MeasuredEngravingError::UnsupportedScale);
    }
    let selected;
    let render_score = match config.part_layout {
        PartLayoutPolicy::FullScore => score,
        PartLayoutPolicy::ExtractedPart { part_index } => {
            selected = score
                .extract_part(part_index)
                .ok_or(MeasuredEngravingError::InvalidPart)?;
            &selected
        }
    };
    let font = TextFont::for_print(font_bytes)?;
    // The first system uses full names. Later page-start systems use short
    // names when present, falling back to the full name otherwise.
    let mut first_width = 0.0_f32;
    let mut repeated_width = 0.0_f32;
    if config.publication.show_part_names {
        for part in &render_score.parts {
            for word in part.name.split_whitespace() {
                first_width = first_width.max(font.measure(word, 2.7)?.occupied_width_mm() + 0.25);
            }
            let repeated = if part.short_name.is_empty() {
                &part.name
            } else {
                &part.short_name
            };
            for word in repeated.split_whitespace() {
                repeated_width =
                    repeated_width.max(font.measure(word, 2.7)?.occupied_width_mm() + 0.25);
            }
        }
    }
    let existing = config.margin_left_mm + config.safe_left_mm - 1.5 - 3.0;
    let label_width = config.publication.show_part_names.then_some(existing);
    let first_row_inset_mm = (first_width - existing).max(0.0);
    let repeated_row_inset_mm = (repeated_width - existing).max(0.0);
    let _text = crate::font_metrics::PrintTextScope::new(render_score, &font)?;
    let logical = compute_layout(render_score, &LayoutConfig::default());
    let (measures, headers_mm) = crate::render::measured_print_widths(render_score, &logical, 1.8)?;
    let multiverse = render_score
        .parts
        .iter()
        .flat_map(|p| &p.staves)
        .flat_map(|s| &s.measures)
        .flat_map(|m| &m.voices)
        .flatten()
        .any(|n| !n.additional_lyrics.is_empty());
    let grand_staff_text = render_score.parts.len() == 1
        && render_score.parts[0].staves.len() == 2
        && render_score.parts[0].staves[0]
            .measures
            .iter()
            .flat_map(|m| &m.texts)
            .any(|t| {
                t.placement
                    .as_deref()
                    .is_some_and(|p| p.eq_ignore_ascii_case("below"))
            });
    let base_gap = if multiverse || grand_staff_text {
        9.0
    } else {
        6.0
    };
    let gap = crate::render::required_hairpin_staff_gap(render_score, &logical, base_gap);
    let gaps = crate::render::required_lyric_staff_gaps(render_score, gap);
    let (top, system, bottom) = crate::render::print_row_metrics(render_score, &logical, &gaps)?;
    let measurements = PrintMeasurements {
        measures,
        headers_mm,
        system_height_mm: (top + system + bottom) * 1.8 + 0.001,
    };
    // Page starts are discovered after pagination. Remember every start seen so
    // far and reserve its inset in the next break pass. The set only grows, so
    // changing page boundaries cannot cycle indefinitely; old starts may be
    // conservatively narrower in the solver even if they move within a page.
    let mut page_starts = std::collections::BTreeSet::new();
    for _ in 0..=measurements.measures.len() {
        let mut breaking_measurements = measurements.clone();
        if let Some(first_header) = breaking_measurements.headers_mm.first_mut() {
            *first_header += first_row_inset_mm;
        }
        for &measure in &page_starts {
            if let Some(header) = breaking_measurements.headers_mm.get_mut(measure) {
                *header += repeated_row_inset_mm;
            }
        }
        let rows = break_measured_print_systems(score, config, &breaking_measurements)?;
        let mut geometry = rows
            .iter()
            .map(|row| crate::render::measured_row_geometry(render_score, row))
            .collect::<Result<Vec<_>, _>>()?;
        let mut needs_new_breaks = false;
        for _ in 0..16 {
            let heights = geometry
                .iter()
                .map(|g| (g.top_u + g.system_u + g.bottom_u) * 1.8 + 0.001)
                .collect::<Vec<_>>();
            let layout = acorde_layout::paginate_measured_systems(score, config, &rows, &heights)?;
            let mut system_insets_mm = vec![0.0; rows.len()];
            for page in &layout.pages {
                if let Some(system) = page.systems.first() {
                    let row = system.system_index;
                    let inset = if row == 0 {
                        first_row_inset_mm
                    } else {
                        repeated_row_inset_mm
                    };
                    system_insets_mm[row] = inset;
                    if row != 0 && inset > 0.0 {
                        let measure = *rows[row]
                            .measure_indices
                            .first()
                            .ok_or(MeasuredEngravingError::InvalidGeneratedGeometry)?;
                        needs_new_breaks |= page_starts.insert(measure);
                    }
                }
            }
            if needs_new_breaks {
                break;
            }
            // Publication labels use canonical part indices, while extracted notation
            // is rendered from a one-part projection whose local part index is zero.
            let mut projected_layout;
            let render_layout =
                if let PartLayoutPolicy::ExtractedPart { part_index } = config.part_layout {
                    projected_layout = layout.clone();
                    for page in &mut projected_layout.pages {
                        for label in &mut page.publication.part_labels {
                            if label.part_index == part_index {
                                label.part_index = 0;
                            }
                        }
                    }
                    &projected_layout
                } else {
                    &layout
                };
            let mut pages = crate::print::render_print_page_svgs_with_geometry(
                render_score,
                render_layout,
                &crate::SvgGlyphSource::Vector,
                Some(&geometry),
                label_width,
                Some(&system_insets_mm),
            )?;
            if crate::print_preflight::resolve(
                render_score,
                &layout,
                &mut geometry,
                &mut pages,
                &font,
                &system_insets_mm,
            )? {
                continue;
            }
            let pages = pages
                .iter()
                .map(|svg| crate::outline_text::outline_page_text(svg, &font))
                .collect::<Result<Vec<_>, _>>()?;
            return Ok(MeasuredPrintOutput {
                layout,
                pages,
                staff_space_mm: 1.8,
                first_system_inset_mm: first_row_inset_mm,
                system_insets_mm,
                measurements,
            });
        }
        if !needs_new_breaks {
            return Err(MeasuredEngravingError::CollisionReflowLimit);
        }
    }

    Err(MeasuredEngravingError::CollisionReflowLimit)
}
