//! Physical page SVGs from a validated print layout. The notation pass keeps the exact
//! system breaks in `PrintLayoutResult`; page placement never asks the renderer to reflow.

use std::fmt::{self, Write as _};

use acorde_core::Score;
use acorde_layout::{
    CropMarkPolicy, GlyphResourcePolicy, LayoutConfig, PrintColorPolicy, PrintLayoutError,
    PrintLayoutResult, PublicationTextAlignment, PublicationTextRole, RowLayout, compute_layout,
    notation_density_units,
};

use crate::render::{self, PrintRowPlacement};
use crate::{RenderError, SvgGlyphSource, SvgRenderConfig, SvgRenderOptions, is_valid_xml_char};

const PX_PER_MM: f32 = 4.0;
const PRINT_STAFF_GAP: f32 = 6.0;
const GRAND_STAFF_PRINT_GAP: f32 = 9.0;
const MULTIVERSE_PRINT_STAFF_GAP: f32 = 9.0;
const PREFERRED_STAFF_SPACE_MM: f32 = 1.8;
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
    render_print_page_svgs_with_glyphs(score, layout, &SvgGlyphSource::Vector)
}

/// [`render_print_page_svgs`] with a selected glyph source. With
/// [`SvgGlyphSource::Smufl`], pages may declare `GlyphResourcePolicy::HostProvided` with exactly
/// the source's resource id (`smufl:<family>`); any other host resource is still rejected.
pub fn render_print_page_svgs_with_glyphs(
    score: &Score,
    layout: &PrintLayoutResult,
    glyphs: &SvgGlyphSource,
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
        let resolved = match &page.glyph_resources {
            GlyphResourcePolicy::BuiltInVector => true,
            GlyphResourcePolicy::HostProvided(id) => {
                glyphs.smufl_family().is_some() && *id == glyphs.resource_id()
            }
        };
        if !resolved {
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
    let staff_gap = render::required_hairpin_staff_gap(score, &score_layout, staff_gap);
    let staff_gaps = render::required_lyric_staff_gaps(score, staff_gap);
    let (top_u, system_u, bottom_u) = render::print_row_metrics(score, &score_layout, &staff_gaps)?;
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
    let staff_offsets = render::print_staff_offsets(score, &staff_gaps);
    let config = SvgRenderConfig::new(options)
        .with_staff_gap(staff_gap)
        .with_staff_gaps(staff_gaps)
        .with_measure_text_scale(3.0)
        .with_glyph_source(glyphs.clone());
    let row_tops_px = row_tops_mm
        .iter()
        .map(|top| top * PX_PER_MM + top_u * space_px)
        .collect::<Vec<_>>();
    let row_width_factors =
        print_row_width_factors(score, &score_layout.rows, first.content_width_mm, space_mm);
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
    let fragments = top_level_fragments(body)
        .map(|fragments| {
            fragments
                .into_iter()
                .map(|fragment| (fragment, fragment_y_extent(fragment)))
                .collect::<Vec<_>>()
        })
        .unwrap_or_else(|| vec![(body, None)]);
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
            "<g class=\"acorde-print-notation acorde-score\" transform=\"translate({:.3} {:.3})\">",
            page.content_left_mm * PX_PER_MM,
            -offset * PX_PER_MM
        );
        // Each page carries only the notation that reaches it, not every other page's
        // content hidden outside its viewBox.
        let (page_top, page_bottom) = (offset * PX_PER_MM, (offset + page.height_mm) * PX_PER_MM);
        for (fragment, extent) in &fragments {
            if extent.is_none_or(|(top, bottom)| bottom >= page_top && top <= page_bottom) {
                svg.push_str(fragment);
            }
        }
        svg.push_str("</g>");
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

/// Leave a short final line ragged according to its notation demand. A single-line
/// score is also a final line: stretching a short pickup across A4 greatly changes
/// note and slur proportions. Earlier full rows remain justified, and a final row
/// with the same number of columns as those rows keeps their full width.
fn print_row_width_factors(
    score: &Score,
    rows: &[RowLayout],
    content_width_mm: f32,
    space_mm: f32,
) -> Vec<f32> {
    let full_row_columns = rows
        .iter()
        .map(|row| row.measure_indices.len())
        .max()
        .unwrap_or(1);
    let row_demand = |row: &RowLayout| {
        row.measure_indices
            .iter()
            .map(|&measure| notation_density_units(score, measure))
            .sum::<usize>() as f32
    };
    let prior_demand = rows
        .iter()
        .take(rows.len().saturating_sub(1))
        .map(row_demand)
        .fold(0.0_f32, f32::max);
    let available_spaces = content_width_mm / space_mm;
    rows.iter()
        .enumerate()
        .map(|(index, row)| {
            if index + 1 != rows.len()
                || (rows.len() > 1 && row.measure_indices.len() == full_row_columns)
            {
                return 1.0;
            }
            let demand = row_demand(row);
            // Six spaces cover the opening clef and closing barline. Two spaces
            // per notation-demand unit provide clearance at Acorde's smaller
            // print staff size without making an isolated bar page-wide.
            let natural = (6.0 + 2.0 * demand) / available_spaces;
            let relative = if prior_demand > 0.0 {
                demand / prior_demand
            } else {
                0.0
            };
            natural.max(relative).clamp(0.08, 1.0)
        })
        .collect()
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
            let lines = part_label_lines(text);
            if lines.is_empty() {
                continue;
            }
            let longest = lines
                .iter()
                .map(|line| line.chars().count())
                .max()
                .unwrap_or(1);
            // Glyphs average up to 0.6 em; keep 3 mm of paper outside the label.
            let size_mm = ((page.content_left_mm - PART_LABEL_GAP_MM - PART_LABEL_PAPER_MM)
                / (0.6 * longest.max(1) as f32))
                .min(3.0);
            let leading = size_mm * 1.15;
            let first_baseline = part_middle * space_px + size_mm * scale * 0.25
                - (lines.len() - 1) as f32 * leading * scale / 2.0;
            for (index, line) in lines.iter().enumerate() {
                let _ = write!(
                    svg,
                    "<text class=\"acorde-print-part-label\" x=\"{:.3}\" y=\"{:.3}\" text-anchor=\"end\" font-family=\"serif\" font-size=\"{:.3}\">{}</text>",
                    (page.content_left_mm - PART_LABEL_GAP_MM) * scale,
                    row_top - offset_px + first_baseline + index as f32 * leading * scale,
                    size_mm * scale,
                    escape_xml(line)
                );
            }
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

const PART_LABEL_GAP_MM: f32 = 1.5;
const PART_LABEL_PAPER_MM: f32 = 3.0;

/// Lines of a part label: its own line breaks, else a long two-word name split at the space
/// nearest its middle so it can keep a readable size in the left margin.
fn part_label_lines(text: &str) -> Vec<&str> {
    let lines = text
        .lines()
        .map(str::trim)
        .filter(|line| !line.is_empty())
        .collect::<Vec<_>>();
    if lines.len() != 1 || lines[0].chars().count() <= 8 {
        return lines;
    }
    let line = lines[0];
    let middle = line.len() / 2;
    line.match_indices(' ')
        .map(|(index, _)| index)
        .min_by_key(|index| index.abs_diff(middle))
        .map_or(lines, |index| {
            vec![line[..index].trim_end(), line[index + 1..].trim_start()]
        })
}

/// Split rendered SVG markup into its top-level elements. `None` for unbalanced markup.
fn top_level_fragments(body: &str) -> Option<Vec<&str>> {
    let mut fragments = Vec::new();
    let (mut depth, mut start, mut cursor) = (0usize, 0usize, 0usize);
    while let Some(relative) = body[cursor..].find('<') {
        let open = cursor + relative;
        let close = open + body[open..].find('>')?;
        let tag = &body[open + 1..close];
        if tag.starts_with('/') {
            depth = depth.checked_sub(1)?;
            if depth == 0 {
                fragments.push(&body[start..=close]);
            }
        } else if tag.ends_with('/') {
            if depth == 0 {
                fragments.push(&body[open..=close]);
            }
        } else {
            if depth == 0 {
                start = open;
            }
            depth += 1;
        }
        cursor = close + 1;
    }
    (depth == 0).then_some(fragments)
}

/// Vertical extent of one element in the stacked-page coordinate space. Print bodies use
/// absolute coordinates (`M`/`L`/`C`/`Q` path pairs, polygon points, positioned shapes);
/// their only transforms rotate a glyph about its own point. Text is padded by its size.
fn fragment_y_extent(fragment: &str) -> Option<(f32, f32)> {
    let mut extent: Option<(f32, f32)> = None;
    let mut add = |top: f32, bottom: f32| {
        if top.is_finite() && bottom.is_finite() {
            extent = Some(extent.map_or((top, bottom), |(t, b)| (t.min(top), b.max(bottom))));
        }
    };
    for tag in fragment.split('<').skip(1) {
        let tag = tag.split('>').next().unwrap_or_default();
        if tag.starts_with('/') {
            continue;
        }
        let attribute = |name: &str| {
            tag.split_once(&format!(" {name}=\""))
                .and_then(|(_, rest)| rest.split('"').next())
        };
        let number = |name: &str| attribute(name).and_then(|value| value.parse::<f32>().ok());
        let pad = number("font-size").unwrap_or(0.0);
        for name in ["y", "y1", "y2"] {
            if let Some(y) = number(name) {
                add(y - pad, y + pad);
            }
        }
        if let (Some(y), Some(height)) = (number("y"), number("height")) {
            add(y, y + height);
        }
        if let Some(cy) = number("cy") {
            let radius = number("ry").or_else(|| number("r")).unwrap_or(0.0);
            add(cy - radius, cy + radius);
        }
        // Absolute path commands: `A` carries seven values with y last, the others x,y pairs.
        for name in ["d", "points"] {
            let (mut arity, mut y_slot, mut slot) = (2, 1, 0);
            for token in attribute(name)
                .into_iter()
                .flat_map(|value| value.split(|c: char| c == ',' || c.is_whitespace()))
                .filter(|token| !token.is_empty())
            {
                if let Ok(value) = token.parse::<f32>() {
                    if slot == y_slot {
                        add(value, value);
                    }
                    slot = (slot + 1) % arity;
                } else {
                    (arity, y_slot, slot) = if token == "A" { (7, 6, 0) } else { (2, 1, 0) };
                }
            }
        }
    }
    extent
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

#[cfg(test)]
mod tests {
    use super::{fragment_y_extent, top_level_fragments};

    #[test]
    fn page_split_reads_arc_and_text_extents() {
        let body = r#"<g class="acorde-articulation acorde-fermata"><path d="M 229.24,1869.22 A 4.04,3.84 0 0 1 237.33,1869.22"/></g><text x="1" y="40" font-size="8">a</text><line x1="0" y1="5" x2="9" y2="5"/>"#;
        let fragments = top_level_fragments(body).expect("balanced markup");
        assert_eq!(fragments.len(), 3);
        assert_eq!(fragment_y_extent(fragments[0]), Some((1869.22, 1869.22)));
        assert_eq!(fragment_y_extent(fragments[1]), Some((32.0, 48.0)));
        assert_eq!(fragment_y_extent(fragments[2]), Some((5.0, 5.0)));
        assert!(top_level_fragments("<g><g></g>").is_none());
    }

    #[test]
    fn long_part_labels_wrap_at_breaks_or_the_middle_space() {
        assert_eq!(
            super::part_label_lines("Gesang\nVoice"),
            ["Gesang", "Voice"]
        );
        assert_eq!(super::part_label_lines("Γούντ Μπλόκ"), ["Γούντ", "Μπλόκ"]);
        assert_eq!(super::part_label_lines("Violoncello"), ["Violoncello"]);
        assert_eq!(super::part_label_lines("Piano"), ["Piano"]);
        assert!(super::part_label_lines(" \n ").is_empty());
    }
}
