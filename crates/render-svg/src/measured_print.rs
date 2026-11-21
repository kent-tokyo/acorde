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
    let rows = break_measured_print_systems(score, config, &measurements)?;
    let mut geometry = rows
        .iter()
        .map(|row| crate::render::measured_row_geometry(render_score, row))
        .collect::<Result<Vec<_>, _>>()?;
    for _ in 0..16 {
        let heights = geometry
            .iter()
            .map(|g| (g.top_u + g.system_u + g.bottom_u) * 1.8 + 0.001)
            .collect::<Vec<_>>();
        let layout = acorde_layout::paginate_measured_systems(score, config, &rows, &heights)?;
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
        )?;
        if crate::print_preflight::resolve(render_score, &layout, &mut geometry, &mut pages, &font)?
        {
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
            measurements,
        });
    }
    Err(MeasuredEngravingError::CollisionReflowLimit)
}
