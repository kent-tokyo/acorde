//! Physical page SVGs from a validated print layout. The notation pass keeps the exact
//! system breaks in `PrintLayoutResult`; page placement never asks the renderer to reflow.

use std::fmt::{self, Write as _};

use acorde_core::Score;
use acorde_layout::{
    CropMarkPolicy, GlyphResourcePolicy, LayoutConfig, PrintColorPolicy, PrintLayoutError,
    PrintLayoutResult, PublicationTextAlignment, PublicationTextRole, RowLayout, compute_layout,
};

use crate::render::{self, PrintRowPlacement};
use crate::{RenderError, SvgRenderConfig, SvgRenderOptions, is_valid_xml_char};

const PX_PER_MM: f32 = 4.0;
const PRINT_STAFF_GAP: f32 = 6.0;
const GRAND_STAFF_PRINT_GAP: f32 = 8.0;
const MULTIVERSE_PRINT_STAFF_GAP: f32 = 9.0;
const PREFERRED_STAFF_SPACE_MM: f32 = 1.7;
const MIN_STAFF_SPACE_MM: f32 = 1.1;
const MAX_PRINT_PAGES: usize = 256;

/// A print-page request that cannot be represented faithfully by the built-in SVG renderer.
#[derive(Debug)]
pub enum PrintRenderError {
    Layout(PrintLayoutError),
    Render(RenderError),
    EmptyPages,
    TooManyPages {
        count: usize,
    },
    InconsistentPageWidth {
        page_index: usize,
    },
    InvalidSystemMeasures,
    SystemTooShort {
        height_mm: f32,
        minimum_mm: f32,
    },
    UnsupportedResource {
        page_index: usize,
        kind: &'static str,
    },
    InvalidPublicationText {
        page_index: usize,
    },
    InternalSvg,
}

impl fmt::Display for PrintRenderError {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        match self {
            Self::Layout(error) => write!(f, "invalid print layout: {error}"),
            Self::Render(error) => write!(f, "score rendering failed: {error}"),
            Self::EmptyPages => write!(f, "print layout contains no pages"),
            Self::TooManyPages { count } => write!(
                f,
                "print layout has {count} pages; maximum is {MAX_PRINT_PAGES}"
            ),
            Self::InconsistentPageWidth { page_index } => write!(
                f,
                "page {page_index} has a different notation width or left edge"
            ),
            Self::InvalidSystemMeasures => write!(
                f,
                "print systems do not cover the score's visual measures in order"
            ),
            Self::SystemTooShort {
                height_mm,
                minimum_mm,
            } => write!(
                f,
                "print system height {height_mm:.2} mm cannot fit legible notation; minimum is {minimum_mm:.2} mm"
            ),
            Self::UnsupportedResource { page_index, kind } => {
                write!(f, "page {page_index} requires unsupported {kind}")
            }
            Self::InvalidPublicationText { page_index } => {
                write!(f, "page {page_index} contains invalid XML publication text")
            }
            Self::InternalSvg => write!(f, "renderer returned an unexpected SVG document"),
        }
    }
}

impl std::error::Error for PrintRenderError {}

impl From<PrintLayoutError> for PrintRenderError {
    fn from(value: PrintLayoutError) -> Self {
        Self::Layout(value)
    }
}

impl From<RenderError> for PrintRenderError {
    fn from(value: RenderError) -> Self {
        Self::Render(value)
    }
}

/// Render one SVG per physical page using the exact system breaks and positions in `layout`.
///
/// The SVGs are self-contained and use millimetre dimensions. Built-in notation glyphs are
/// vector paths. Text still needs a font supplied by the viewer or PDF backend. Host glyphs,
/// images, frames, color preservation, and crop marks fail explicitly until those resources have
/// a reproducible print implementation.
pub fn render_print_page_svgs(
    score: &Score,
    layout: &PrintLayoutResult,
) -> Result<Vec<String>, PrintRenderError> {
    layout.validate()?;
    // Match the staff-aware layout's extra page reservation for an additional
    // lyric lane. A fixed-height layout may instead fail the legibility preflight.
    let has_multiple_lyric_verses = score
        .parts
        .iter()
        .flat_map(|part| &part.staves)
        .any(|staff| {
            staff
                .measures
                .iter()
                .flat_map(|measure| &measure.voices)
                .flatten()
                .any(|note| !note.additional_lyrics.is_empty())
        });
    let grand_staff_has_below_text = score.parts.len() == 1
        && score.parts[0].staves.len() == 2
        && score.parts[0].staves[0].measures.iter().any(|measure| {
            measure.texts.iter().any(|text| {
                text.placement
                    .as_deref()
                    .is_some_and(|placement| placement.eq_ignore_ascii_case("below"))
            })
        });
    let staff_gap = if has_multiple_lyric_verses {
        MULTIVERSE_PRINT_STAFF_GAP
    } else if grand_staff_has_below_text {
        GRAND_STAFF_PRINT_GAP
    } else {
        PRINT_STAFF_GAP
    };
    if layout.pages.is_empty() {
        return Err(PrintRenderError::EmptyPages);
    }
    if layout.pages.len() > MAX_PRINT_PAGES {
        return Err(PrintRenderError::TooManyPages {
            count: layout.pages.len(),
        });
    }
    let first = &layout.pages[0];
    let mut offsets = Vec::with_capacity(layout.pages.len());
    let mut offset_mm = 0.0_f32;
    let mut row_tops_mm = Vec::new();
    let mut min_system_height_mm = f32::INFINITY;
    let mut rows = Vec::new();
    for page in &layout.pages {
        if (page.content_width_mm - first.content_width_mm).abs() > 0.001
            || (page.content_left_mm - first.content_left_mm).abs() > 0.001
        {
            return Err(PrintRenderError::InconsistentPageWidth {
                page_index: page.page_index,
            });
        }
        if !matches!(page.glyph_resources, GlyphResourcePolicy::BuiltInVector) {
            return Err(PrintRenderError::UnsupportedResource {
                page_index: page.page_index,
                kind: "glyph resource",
            });
        }
        if !matches!(page.color_policy, PrintColorPolicy::Monochrome) {
            return Err(PrintRenderError::UnsupportedResource {
                page_index: page.page_index,
                kind: "color preservation",
            });
        }
        if !matches!(page.crop_mark_policy, CropMarkPolicy::None) {
            return Err(PrintRenderError::UnsupportedResource {
                page_index: page.page_index,
                kind: "crop marks",
            });
        }
        if !page.publication.image_resources.is_empty() {
            return Err(PrintRenderError::UnsupportedResource {
                page_index: page.page_index,
                kind: "image resource",
            });
        }
        if !page.publication.frames.is_empty() {
            return Err(PrintRenderError::UnsupportedResource {
                page_index: page.page_index,
                kind: "publication frame",
            });
        }
        if !page.publication.score_texts.is_empty() {
            return Err(PrintRenderError::UnsupportedResource {
                page_index: page.page_index,
                kind: "unpositioned score text",
            });
        }
        if page
            .publication
            .text_blocks
            .iter()
            .any(|block| block.text.chars().any(|c| !is_valid_xml_char(c)))
            || page
                .publication
                .title
                .chars()
                .any(|c| !is_valid_xml_char(c))
            || page
                .publication
                .movement_title
                .chars()
                .any(|c| !is_valid_xml_char(c))
            || page
                .publication
                .composer
                .chars()
                .any(|c| !is_valid_xml_char(c))
            || page.publication.part_labels.iter().any(|label| {
                label
                    .name
                    .chars()
                    .chain(label.short_name.chars())
                    .any(|c| !is_valid_xml_char(c))
            })
        {
            return Err(PrintRenderError::InvalidPublicationText {
                page_index: page.page_index,
            });
        }
        offsets.push(offset_mm);
        for system in &page.systems {
            min_system_height_mm = min_system_height_mm.min(system.height_mm);
            row_tops_mm.push(offset_mm + system.top_mm);
            rows.push(RowLayout {
                measure_indices: system.measure_indices.clone(),
            });
        }
        offset_mm += page.height_mm;
    }
    if !offset_mm.is_finite() || offset_mm * PX_PER_MM > 1_000_000.0 {
        return Err(PrintRenderError::TooManyPages {
            count: layout.pages.len(),
        });
    }
    let mut score_layout = compute_layout(score, &LayoutConfig::default());
    if rows
        .iter()
        .flat_map(|row| row.measure_indices.iter())
        .copied()
        .collect::<Vec<_>>()
        != score_layout.vis_slots
    {
        return Err(PrintRenderError::InvalidSystemMeasures);
    }
    score_layout.rows = rows;
    let (top_u, system_u, bottom_u) = render::print_row_metrics(score, &score_layout, staff_gap)?;
    let required_u = top_u + system_u + bottom_u;
    let minimum_mm = required_u * MIN_STAFF_SPACE_MM;
    if min_system_height_mm < minimum_mm {
        return Err(PrintRenderError::SystemTooShort {
            height_mm: min_system_height_mm,
            minimum_mm,
        });
    }
    let space_mm = (min_system_height_mm / required_u).min(PREFERRED_STAFF_SPACE_MM);
    let space_px = space_mm * PX_PER_MM;
    let options = SvgRenderOptions {
        width: first.content_width_mm * PX_PER_MM,
        staff_size: space_px,
        measures_per_system: score_layout
            .rows
            .iter()
            .map(|row| row.measure_indices.len())
            .max()
            .unwrap_or(1)
            .max(1),
        interactive: false,
    };
    let config = SvgRenderConfig::new(options)
        .with_staff_gap(staff_gap)
        .with_measure_text_scale(3.0);
    let row_tops_px = row_tops_mm
        .iter()
        .map(|top| top * PX_PER_MM + top_u * space_px)
        .collect::<Vec<_>>();
    let full_row_columns = score_layout
        .rows
        .iter()
        .map(|row| row.measure_indices.len())
        .max()
        .unwrap_or(1);
    let row_width_factors = score_layout
        .rows
        .iter()
        .enumerate()
        .map(|(index, row)| {
            if score_layout.rows.len() > 1 && index + 1 == score_layout.rows.len() {
                (row.measure_indices.len() as f32 / full_row_columns as f32).clamp(0.55, 1.0)
            } else {
                1.0
            }
        })
        .collect::<Vec<_>>();
    let placement = PrintRowPlacement {
        row_tops_px: &row_tops_px,
        row_width_factors: &row_width_factors,
        canvas_height_px: offset_mm * PX_PER_MM,
    };
    let (full_svg, _) =
        render::build_svg_with_metadata_placement(score, &score_layout, &config, Some(&placement))?;
    const BODY_PREFIX: &str = "<g class=\"acorde-score\">";
    let body = full_svg
        .split_once(BODY_PREFIX)
        .and_then(|(_, remainder)| remainder.strip_suffix("</g></svg>"))
        .ok_or(PrintRenderError::InternalSvg)?;
    let staff_offsets = render::print_staff_offsets(score, staff_gap);
    let mut pages = Vec::with_capacity(layout.pages.len());
    for (page, offset) in layout.pages.iter().zip(offsets) {
        let has_movement_title = !page.publication.movement_title.trim().is_empty()
            && page.publication.movement_title != page.publication.title;
        if page.page_index == 0
            && !page.publication.is_title_page
            && (!page.publication.title.is_empty() || has_movement_title)
            && page
                .systems
                .first()
                .is_some_and(|system| system.top_mm < if has_movement_title { 16.0 } else { 10.0 })
        {
            return Err(PrintRenderError::UnsupportedResource {
                page_index: 0,
                kind: "title margin",
            });
        }
        if !page.publication.part_labels.is_empty() && page.content_left_mm < 8.0 {
            return Err(PrintRenderError::UnsupportedResource {
                page_index: page.page_index,
                kind: "part-label margin",
            });
        }
        let mut svg = String::new();
        let _ = write!(
            svg,
            "<svg xmlns=\"http://www.w3.org/2000/svg\" width=\"{:.3}mm\" height=\"{:.3}mm\" viewBox=\"0 0 {:.3} {:.3}\" role=\"img\"><title>{}</title>",
            page.width_mm,
            page.height_mm,
            page.width_mm * PX_PER_MM,
            page.height_mm * PX_PER_MM,
            escape_xml(&page.publication.title)
        );
        let _ = write!(
            svg,
            "<g class=\"acorde-print-notation acorde-score\" transform=\"translate({:.3} {:.3})\">{body}</g>",
            page.content_left_mm * PX_PER_MM,
            -offset * PX_PER_MM
        );
        write_publication(
            &mut svg,
            page,
            PX_PER_MM,
            offset * PX_PER_MM,
            &row_tops_px,
            space_px,
            &staff_offsets,
        );
        svg.push_str("</svg>");
        pages.push(svg);
    }
    Ok(pages)
}

fn write_publication(
    svg: &mut String,
    page: &acorde_layout::PageLayout,
    scale: f32,
    offset_px: f32,
    row_tops_px: &[f32],
    space_px: f32,
    staff_offsets: &[(usize, f32, f32)],
) {
    if page.page_index == 0
        && !page.publication.is_title_page
        && let Some(system) = page.systems.first()
    {
        let has_movement_title = !page.publication.movement_title.trim().is_empty()
            && page.publication.movement_title != page.publication.title;
        // Keep the title inside the top print margin. The former 7.5 mm
        // baseline on a default A4 page placed most of its capitals only
        // 2.5 mm from the paper edge in the pinned Verovio comparison.
        let title_y = (system.top_mm - if has_movement_title { 6.5 } else { 5.0 }) * scale;
        let credit_y = (system.top_mm - if has_movement_title { 0.5 } else { 5.0 }) * scale;
        if !page.publication.title.is_empty() {
            let _ = write!(
                svg,
                "<text class=\"acorde-print-title\" x=\"{:.3}\" y=\"{title_y:.3}\" text-anchor=\"middle\" font-family=\"serif\" font-weight=\"bold\" font-size=\"{:.3}\">{}</text>",
                page.width_mm * scale / 2.0,
                5.5 * scale,
                escape_xml(&page.publication.title)
            );
        }
        if has_movement_title {
            let _ = write!(
                svg,
                "<text class=\"acorde-print-movement-title\" x=\"{:.3}\" y=\"{credit_y:.3}\" text-anchor=\"middle\" font-family=\"serif\" font-size=\"{:.3}\">{}</text>",
                page.width_mm * scale / 2.0,
                4.0 * scale,
                escape_xml(&page.publication.movement_title)
            );
        }
        if !page.publication.composer.is_empty() {
            let _ = write!(
                svg,
                "<text class=\"acorde-print-composer\" x=\"{:.3}\" y=\"{credit_y:.3}\" text-anchor=\"end\" font-family=\"serif\" font-size=\"{:.3}\">{}</text>",
                (page.content_left_mm + page.content_width_mm) * scale,
                4.0 * scale,
                escape_xml(&page.publication.composer)
            );
        }
    }
    if let Some(system) = page.systems.first()
        && let Some(&row_top) = row_tops_px.get(system.system_index)
    {
        for label in &page.publication.part_labels {
            let Some((_, first_top, _)) = staff_offsets
                .iter()
                .find(|(part, _, _)| *part == label.part_index)
            else {
                continue;
            };
            let Some((_, last_top, last_height)) = staff_offsets
                .iter()
                .rfind(|(part, _, _)| *part == label.part_index)
            else {
                continue;
            };
            let text = if page.page_index > 0 && !label.short_name.is_empty() {
                &label.short_name
            } else {
                &label.name
            };
            if text.is_empty() {
                continue;
            }
            let part_middle = (*first_top + *last_top + *last_height) / 2.0;
            let size_mm =
                (2.0 * (page.content_left_mm - 2.0) / text.chars().count().max(1) as f32).min(3.0);
            let _ = write!(
                svg,
                "<text class=\"acorde-print-part-label\" x=\"{:.3}\" y=\"{:.3}\" text-anchor=\"end\" font-family=\"serif\" font-size=\"{:.3}\">{}</text>",
                (page.content_left_mm - 1.5) * scale,
                row_top - offset_px + part_middle * space_px + size_mm * scale * 0.25,
                size_mm * scale,
                escape_xml(text)
            );
        }
    }
    for block in &page.publication.text_blocks {
        let (x, anchor) = match block.alignment {
            PublicationTextAlignment::Left => (block.x_mm, "start"),
            PublicationTextAlignment::Center => (block.x_mm + block.width_mm / 2.0, "middle"),
            PublicationTextAlignment::Right => (block.x_mm + block.width_mm, "end"),
        };
        let weight = if matches!(block.role, PublicationTextRole::Title) {
            "bold"
        } else {
            "normal"
        };
        let size = block.height_mm * 0.75 * scale;
        let _ = write!(
            svg,
            "<text class=\"acorde-publication\" x=\"{:.3}\" y=\"{:.3}\" text-anchor=\"{anchor}\" font-family=\"serif\" font-weight=\"{weight}\" font-size=\"{size:.3}\">{}</text>",
            x * scale,
            (block.y_mm + block.height_mm * 0.8) * scale,
            escape_xml(&block.text)
        );
    }
    if let Some(number) = page.page_number
        && !page
            .publication
            .text_blocks
            .iter()
            .any(|block| block.text == number.to_string())
    {
        let _ = write!(
            svg,
            "<text class=\"acorde-page-number\" x=\"{:.3}\" y=\"{:.3}\" text-anchor=\"middle\" font-family=\"serif\" font-size=\"{:.3}\">{number}</text>",
            page.width_mm * scale / 2.0,
            (page.height_mm - 6.0) * scale,
            3.0 * scale
        );
    }
}

fn escape_xml(text: &str) -> String {
    let mut escaped = String::with_capacity(text.len());
    for c in text.chars() {
        match c {
            '&' => escaped.push_str("&amp;"),
            '<' => escaped.push_str("&lt;"),
            '>' => escaped.push_str("&gt;"),
            '"' => escaped.push_str("&quot;"),
            '\'' => escaped.push_str("&apos;"),
            _ => escaped.push(c),
        }
    }
    escaped
}
