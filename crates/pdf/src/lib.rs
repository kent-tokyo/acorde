//! Vector PDF export for [`acorde_core::Score`].
//!
//! The crate deliberately sits after `acorde-render-svg` in the pipeline:
//!
//! ```text
//! Score -> acorde-render-svg -> vector SVG -> Harumi -> PDF bytes
//! ```
//!
//! It does not alter score semantics or logical layout. The caller supplies the font used for
//! textual SVG elements, while Acorde's notation glyphs remain vector geometry. The current
//! [`export_svg_pages_pdf`] joins independently sized page SVGs into one vector PDF. Producing
//! those page SVGs from print-layout systems remains a separate rendering step.

use acorde_core::Score;
use acorde_layout::{PrintLayoutError, PrintLayoutResult};
use acorde_render_svg::{RenderError, SvgRenderOptions, render_svg};
use roxmltree::{Document, Node, NodeType};
use std::f32::consts::PI;
use thiserror::Error;

/// Maximum accepted SVG source size before invoking Harumi.
pub const MAX_SVG_BYTES: usize = 16 * 1024 * 1024;
/// Maximum flattened vector points from one SVG path.
pub const MAX_PATH_POINTS: usize = 100_000;
/// Maximum number of SVG pages accepted by one in-memory PDF export.
pub const MAX_PDF_PAGES: usize = 256;
/// Points per millimetre for callers selecting a physical PDF page size.
pub const POINTS_PER_MM: f32 = 72.0 / 25.4;

/// Errors returned by PDF rendering.
#[derive(Debug, Error)]
pub enum PdfError {
    #[error("SVG input is too large ({bytes} bytes; maximum is {maximum})")]
    SvgTooLarge { bytes: usize, maximum: usize },
    #[error("SVG XML is invalid: {0}")]
    SvgXml(#[from] roxmltree::Error),
    #[error("SVG path is invalid: {0}")]
    SvgPath(#[from] SvgPathError),
    #[error("PDF page size must contain two positive finite point values")]
    InvalidPageSize,
    #[error("score SVG rendering failed: {0}")]
    Render(#[from] RenderError),
    #[error("Harumi PDF rendering failed: {0}")]
    Harumi(#[from] harumi::Error),
}

/// A typed failure while flattening a generated SVG path into Harumi's portable subset.
#[derive(Debug, Error, PartialEq)]
pub enum SvgPathError {
    #[error("path does not start with a move command")]
    MissingMove,
    #[error("path command {command} has incomplete arguments")]
    IncompleteArguments { command: char },
    #[error("unsupported SVG path command {command}")]
    UnsupportedCommand { command: char },
    #[error("path contains an invalid numeric token")]
    InvalidNumber,
    #[error("path contains non-finite geometry")]
    NonFiniteGeometry,
    #[error("arc flags must be 0 or 1")]
    InvalidArcFlag,
    #[error("path exceeds the {maximum} point safety limit")]
    TooManyPoints { maximum: usize },
}

/// Errors while assembling independent SVG pages into one PDF.
#[derive(Debug, Error)]
pub enum PdfPagesError {
    #[error("PDF export requires between 1 and {maximum} pages")]
    InvalidPageCount { maximum: usize },
    #[error("PDF page {index} is invalid: {source}")]
    Page { index: usize, source: PdfError },
    #[error("print layout is invalid: {0}")]
    Layout(#[from] PrintLayoutError),
    #[error("print layout has {expected} pages but {actual} page SVGs were supplied")]
    PageCountMismatch { expected: usize, actual: usize },
    #[error("PDF page assembly failed: {0}")]
    Assembly(#[from] harumi::Error),
}

/// Options for one vector PDF export.
///
/// `font_bytes` must contain a TrueType or OpenType font accepted by Harumi. It is used only for
/// textual SVG elements (lyrics, dynamics, titles, and labels); notation remains vector paths.
#[derive(Debug, Clone)]
pub struct PdfRenderOptions<'a> {
    pub font_bytes: &'a [u8],
    /// Optional physical PDF media box in points. When absent, the SVG viewport dimensions are
    /// used as PDF points. Supplying a page size scales the complete vector viewport to that page.
    pub page_size_pt: Option<[f32; 2]>,
    /// SVG engraving options used by [`export_score_pdf`].
    pub svg: SvgRenderOptions,
}

/// One self-contained page SVG and its physical PDF media-box size.
///
/// The SVG should correspond to one page from an Acorde print layout. Callers can convert
/// `PageArtifact::width_mm` and `height_mm` using [`POINTS_PER_MM`].
#[derive(Debug, Clone, Copy)]
pub struct SvgPdfPage<'a> {
    pub svg: &'a str,
    pub page_size_pt: [f32; 2],
}

impl<'a> PdfRenderOptions<'a> {
    /// Construct options with Acorde's default SVG engraving choices.
    pub fn new(font_bytes: &'a [u8]) -> Self {
        Self {
            font_bytes,
            page_size_pt: None,
            svg: SvgRenderOptions::default(),
        }
    }

    /// Return the A4 portrait media-box dimensions in PDF points.
    pub const fn a4_portrait_pt() -> [f32; 2] {
        [210.0 * POINTS_PER_MM, 297.0 * POINTS_PER_MM]
    }
}

/// Render `score` to SVG and export that vector viewport as one PDF page.
pub fn export_score_pdf(
    score: &Score,
    options: &PdfRenderOptions<'_>,
) -> Result<Vec<u8>, PdfError> {
    let svg = render_svg(score, &options.svg)?;
    export_svg_pdf(&svg, options)
}

/// Convert a self-contained Acorde SVG viewport into one vector PDF page.
///
/// Harumi preserves Bézier paths and ellipses, but does not accept the rotation on Acorde's
/// notehead ellipses. Only those rotated ellipses are polygonized before PDF conversion.
/// It is intended for Acorde-generated SVG, not as a general-purpose SVG sanitizer or renderer.
pub fn export_svg_pdf(svg: &str, options: &PdfRenderOptions<'_>) -> Result<Vec<u8>, PdfError> {
    if svg.len() > MAX_SVG_BYTES {
        return Err(PdfError::SvgTooLarge {
            bytes: svg.len(),
            maximum: MAX_SVG_BYTES,
        });
    }
    if let Some(size) = options.page_size_pt
        && (!size[0].is_finite() || !size[1].is_finite() || size[0] <= 0.0 || size[1] <= 0.0)
    {
        return Err(PdfError::InvalidPageSize);
    }
    let normalized = normalize_svg_rotated_ellipses(svg)?;
    harumi::svg_to_pdf_with_options(
        normalized.as_bytes(),
        harumi::SvgOptions {
            font_bytes: options.font_bytes,
            page_size: options.page_size_pt,
        },
    )
    .map_err(PdfError::from)
}

/// Join independently sized Acorde page SVGs into one in-memory vector PDF.
///
/// Input order is preserved as PDF page order. Every page is converted through the same bounded
/// SVG path as [`export_svg_pdf`], with its own media box; no page is shrunk to the first page's
/// size. The caller supplies page SVGs and any font needed by their text elements.
pub fn export_svg_pages_pdf(
    pages: &[SvgPdfPage<'_>],
    font_bytes: &[u8],
) -> Result<Vec<u8>, PdfPagesError> {
    let Some((first, rest)) = pages.split_first() else {
        return Err(PdfPagesError::InvalidPageCount {
            maximum: MAX_PDF_PAGES,
        });
    };
    if pages.len() > MAX_PDF_PAGES {
        return Err(PdfPagesError::InvalidPageCount {
            maximum: MAX_PDF_PAGES,
        });
    }
    for (index, page) in pages.iter().enumerate() {
        if page
            .page_size_pt
            .iter()
            .any(|size| !size.is_finite() || *size <= 0.0)
        {
            return Err(PdfPagesError::Page {
                index,
                source: PdfError::InvalidPageSize,
            });
        }
        if page.svg.len() > MAX_SVG_BYTES {
            return Err(PdfPagesError::Page {
                index,
                source: PdfError::SvgTooLarge {
                    bytes: page.svg.len(),
                    maximum: MAX_SVG_BYTES,
                },
            });
        }
    }
    let convert = |index: usize, page: &SvgPdfPage<'_>| {
        export_svg_pdf(
            page.svg,
            &PdfRenderOptions {
                font_bytes,
                page_size_pt: Some(page.page_size_pt),
                svg: SvgRenderOptions::default(),
            },
        )
        .map_err(|source| PdfPagesError::Page { index, source })
    };
    let mut document = harumi::Document::from_bytes(&convert(0, first)?)?;
    for (index, page) in rest.iter().enumerate() {
        document.merge_from(harumi::Document::from_bytes(&convert(index + 1, page)?)?)?;
    }
    document.save_to_bytes().map_err(PdfPagesError::from)
}

/// Assemble one PDF page per page of a validated [`PrintLayoutResult`].
///
/// `page_svgs` must be in layout page order. The layout supplies each page's physical dimensions
/// in millimetres; the caller supplies the completed SVG for each page. No score layout is
/// recomputed and no printer or filesystem is accessed.
pub fn export_print_layout_pdf<S: AsRef<str>>(
    layout: &PrintLayoutResult,
    page_svgs: &[S],
    font_bytes: &[u8],
) -> Result<Vec<u8>, PdfPagesError> {
    layout.validate()?;
    if page_svgs.len() != layout.pages.len() {
        return Err(PdfPagesError::PageCountMismatch {
            expected: layout.pages.len(),
            actual: page_svgs.len(),
        });
    }
    let pages = layout
        .pages
        .iter()
        .zip(page_svgs)
        .map(|(page, svg)| SvgPdfPage {
            svg: svg.as_ref(),
            page_size_pt: [
                page.width_mm * POINTS_PER_MM,
                page.height_mm * POINTS_PER_MM,
            ],
        })
        .collect::<Vec<_>>();
    export_svg_pages_pdf(&pages, font_bytes)
}

/// Normalize standard vector geometry emitted by `acorde-render-svg` for older Harumi backends.
///
/// New exports no longer call this compatibility helper: Harumi 1.23.7 supports the curve and
/// ellipse geometry directly. It remains available temporarily for callers that explicitly need
/// a polygon-only SVG subset.
pub fn normalize_svg_for_harumi(svg: &str) -> Result<String, PdfError> {
    if svg.len() > MAX_SVG_BYTES {
        return Err(PdfError::SvgTooLarge {
            bytes: svg.len(),
            maximum: MAX_SVG_BYTES,
        });
    }
    let document = Document::parse(svg)?;
    let mut output = String::with_capacity(svg.len());
    serialize_children(document.root(), &mut output, true)?;
    Ok(output)
}

fn normalize_svg_rotated_ellipses(svg: &str) -> Result<String, PdfError> {
    let document = Document::parse(svg)?;
    let mut output = String::with_capacity(svg.len());
    serialize_children(document.root(), &mut output, false)?;
    Ok(output)
}

fn serialize_children(
    node: Node<'_, '_>,
    output: &mut String,
    flatten_paths: bool,
) -> Result<(), PdfError> {
    for child in node.children() {
        serialize_node(child, output, flatten_paths)?;
    }
    Ok(())
}

fn serialize_node(
    node: Node<'_, '_>,
    output: &mut String,
    flatten_paths: bool,
) -> Result<(), PdfError> {
    match node.node_type() {
        NodeType::Root => serialize_children(node, output, flatten_paths),
        NodeType::Text => {
            output.push_str(&escape_xml(node.text().unwrap_or_default()));
            Ok(())
        }
        NodeType::Element => match node.tag_name().name() {
            "path" if flatten_paths => serialize_path(node, output),
            "ellipse" if flatten_paths || node.attribute("transform").is_some() => {
                serialize_ellipse(node, output)
            }
            name => serialize_element(name, node, output, flatten_paths),
        },
        _ => Ok(()),
    }
}

fn serialize_element(
    name: &str,
    node: Node<'_, '_>,
    output: &mut String,
    flatten_paths: bool,
) -> Result<(), PdfError> {
    output.push('<');
    output.push_str(name);
    serialize_attributes(node, &[], output);
    if node.children().next().is_none() {
        output.push_str("/>");
        return Ok(());
    }
    output.push('>');
    serialize_children(node, output, flatten_paths)?;
    output.push_str("</");
    output.push_str(name);
    output.push('>');
    Ok(())
}

fn serialize_path(node: Node<'_, '_>, output: &mut String) -> Result<(), PdfError> {
    let raw = node
        .attribute("d")
        .ok_or(SvgPathError::IncompleteArguments { command: 'M' })?;
    let filled = svg_fill_is_visible(node);
    for subpath in flatten_path(raw)? {
        if subpath.points.len() < 2 {
            continue;
        }
        output.push('<');
        output.push_str(if subpath.closed || filled {
            "polygon"
        } else {
            "polyline"
        });
        serialize_attributes(node, &["d"], output);
        output.push_str(" points=\"");
        for (index, point) in subpath.points.iter().enumerate() {
            if index > 0 {
                output.push(' ');
            }
            output.push_str(&format_number(point[0]));
            output.push(',');
            output.push_str(&format_number(point[1]));
        }
        output.push_str("\"/>");
    }
    Ok(())
}

fn svg_fill_is_visible(node: Node<'_, '_>) -> bool {
    let value = node
        .attribute("fill")
        .or_else(|| {
            node.attribute("style").and_then(|style| {
                style.split(';').find_map(|property| {
                    let (name, value) = property.split_once(':')?;
                    (name.trim() == "fill").then_some(value.trim())
                })
            })
        })
        .unwrap_or("black");
    value != "none" && value != "transparent"
}

fn serialize_ellipse(node: Node<'_, '_>, output: &mut String) -> Result<(), PdfError> {
    let cx = svg_number(node.attribute("cx").unwrap_or("0"))?;
    let cy = svg_number(node.attribute("cy").unwrap_or("0"))?;
    let rx = svg_number(
        node.attribute("rx")
            .ok_or(SvgPathError::IncompleteArguments { command: 'E' })?,
    )?;
    let ry = svg_number(
        node.attribute("ry")
            .ok_or(SvgPathError::IncompleteArguments { command: 'E' })?,
    )?;
    let rotation = parse_ellipse_rotation(node.attribute("transform"), cx, cy)?;
    let mut points = Vec::with_capacity(32);
    for index in 0..32 {
        let angle = index as f32 * 2.0 * PI / 32.0;
        let point = [cx + rx * angle.cos(), cy + ry * angle.sin()];
        points.push(rotate_point(point, rotation));
    }
    output.push_str("<polygon");
    serialize_attributes(node, &["cx", "cy", "rx", "ry", "transform"], output);
    output.push_str(" points=\"");
    for (index, point) in points.iter().enumerate() {
        if index > 0 {
            output.push(' ');
        }
        output.push_str(&format_number(point[0]));
        output.push(',');
        output.push_str(&format_number(point[1]));
    }
    output.push_str("\"/>");
    Ok(())
}

fn serialize_attributes(node: Node<'_, '_>, ignored: &[&str], output: &mut String) {
    for attribute in node.attributes() {
        if ignored.iter().any(|ignored| *ignored == attribute.name()) {
            continue;
        }
        output.push(' ');
        output.push_str(attribute.name());
        output.push_str("=\"");
        let value = if attribute.name() == "fill" && attribute.value() == "transparent" {
            "none"
        } else {
            attribute.value()
        };
        output.push_str(&escape_xml(value));
        output.push('"');
    }
}

fn escape_xml(value: &str) -> String {
    value
        .replace('&', "&amp;")
        .replace('<', "&lt;")
        .replace('>', "&gt;")
        .replace('"', "&quot;")
}

#[derive(Debug, Clone)]
struct Subpath {
    points: Vec<[f32; 2]>,
    closed: bool,
}

#[derive(Debug, Clone, Copy)]
enum PathToken {
    Command(char),
    Number(f32),
}

fn flatten_path(raw: &str) -> Result<Vec<Subpath>, SvgPathError> {
    let tokens = tokenize_path(raw)?;
    let mut index = 0;
    let mut command = None;
    let mut current = [0.0, 0.0];
    let mut start = [0.0, 0.0];
    let mut subpath: Option<Subpath> = None;
    let mut paths = Vec::new();
    let mut previous_cubic = None;
    let mut previous_quadratic = None;

    while index < tokens.len() {
        if let Some(PathToken::Command(next)) = tokens.get(index).copied() {
            command = Some(next);
            index += 1;
        }
        let active = command.ok_or(SvgPathError::MissingMove)?;
        if matches!(active, 'Z' | 'z') {
            let path = subpath.as_mut().ok_or(SvgPathError::MissingMove)?;
            path.closed = true;
            current = start;
            previous_cubic = None;
            previous_quadratic = None;
            command = None;
            continue;
        }
        if !matches!(tokens.get(index), Some(PathToken::Number(_))) {
            return Err(SvgPathError::IncompleteArguments { command: active });
        }

        match active {
            'M' | 'm' => {
                let point = next_point(&tokens, &mut index, active, current)?;
                if let Some(previous) = subpath.take() {
                    paths.push(previous);
                }
                current = point;
                start = point;
                subpath = Some(Subpath {
                    points: vec![point],
                    closed: false,
                });
                previous_cubic = None;
                previous_quadratic = None;
                command = Some(if active == 'M' { 'L' } else { 'l' });
            }
            'L' | 'l' => {
                let point = next_point(&tokens, &mut index, active, current)?;
                push_point(&mut subpath, point)?;
                current = point;
                previous_cubic = None;
                previous_quadratic = None;
            }
            'H' | 'h' => {
                let x = next_number(&tokens, &mut index, active)?;
                current[0] = if active == 'h' { current[0] + x } else { x };
                finite_point(current)?;
                push_point(&mut subpath, current)?;
                previous_cubic = None;
                previous_quadratic = None;
            }
            'V' | 'v' => {
                let y = next_number(&tokens, &mut index, active)?;
                current[1] = if active == 'v' { current[1] + y } else { y };
                finite_point(current)?;
                push_point(&mut subpath, current)?;
                previous_cubic = None;
                previous_quadratic = None;
            }
            'C' | 'c' => {
                let controls = next_points(&tokens, &mut index, active, current, 3)?;
                let p0 = current;
                let [c1, c2, end] = [controls[0], controls[1], controls[2]];
                append_cubic(&mut subpath, p0, c1, c2, end)?;
                current = end;
                previous_cubic = Some(c2);
                previous_quadratic = None;
            }
            'S' | 's' => {
                let controls = next_points(&tokens, &mut index, active, current, 2)?;
                let c1 = previous_cubic.map_or(current, |point| reflect(point, current));
                let c2 = controls[0];
                let end = controls[1];
                append_cubic(&mut subpath, current, c1, c2, end)?;
                current = end;
                previous_cubic = Some(c2);
                previous_quadratic = None;
            }
            'Q' | 'q' => {
                let controls = next_points(&tokens, &mut index, active, current, 2)?;
                let c = controls[0];
                let end = controls[1];
                append_quadratic(&mut subpath, current, c, end)?;
                current = end;
                previous_quadratic = Some(c);
                previous_cubic = None;
            }
            'T' | 't' => {
                let end = next_point(&tokens, &mut index, active, current)?;
                let control = previous_quadratic.map_or(current, |point| reflect(point, current));
                append_quadratic(&mut subpath, current, control, end)?;
                current = end;
                previous_quadratic = Some(control);
                previous_cubic = None;
            }
            'A' | 'a' => {
                let rx = next_number(&tokens, &mut index, active)?.abs();
                let ry = next_number(&tokens, &mut index, active)?.abs();
                let rotation = next_number(&tokens, &mut index, active)?;
                let large_arc = next_flag(&tokens, &mut index, active)?;
                let sweep = next_flag(&tokens, &mut index, active)?;
                let end = next_point(&tokens, &mut index, active, current)?;
                append_arc(
                    &mut subpath,
                    current,
                    ArcParams {
                        rx,
                        ry,
                        rotation_degrees: rotation,
                        large_arc,
                        sweep,
                    },
                    end,
                )?;
                current = end;
                previous_cubic = None;
                previous_quadratic = None;
            }
            other => return Err(SvgPathError::UnsupportedCommand { command: other }),
        }
    }
    if let Some(path) = subpath {
        paths.push(path);
    }
    if paths.is_empty() {
        return Err(SvgPathError::MissingMove);
    }
    Ok(paths)
}

fn tokenize_path(raw: &str) -> Result<Vec<PathToken>, SvgPathError> {
    let bytes = raw.as_bytes();
    let mut tokens = Vec::new();
    let mut index = 0;
    while index < bytes.len() {
        let byte = bytes[index];
        if byte.is_ascii_whitespace() || byte == b',' {
            index += 1;
            continue;
        }
        if byte.is_ascii_alphabetic() {
            tokens.push(PathToken::Command(byte as char));
            index += 1;
            continue;
        }
        let start = index;
        if matches!(byte, b'+' | b'-') {
            index += 1;
        }
        let digits_start = index;
        while index < bytes.len() && bytes[index].is_ascii_digit() {
            index += 1;
        }
        if index < bytes.len() && bytes[index] == b'.' {
            index += 1;
            while index < bytes.len() && bytes[index].is_ascii_digit() {
                index += 1;
            }
        }
        if index == digits_start && (index >= bytes.len() || bytes[index] != b'.') {
            return Err(SvgPathError::InvalidNumber);
        }
        if index < bytes.len() && matches!(bytes[index], b'e' | b'E') {
            index += 1;
            if index < bytes.len() && matches!(bytes[index], b'+' | b'-') {
                index += 1;
            }
            let exponent_start = index;
            while index < bytes.len() && bytes[index].is_ascii_digit() {
                index += 1;
            }
            if exponent_start == index {
                return Err(SvgPathError::InvalidNumber);
            }
        }
        let value = raw[start..index]
            .parse::<f32>()
            .map_err(|_| SvgPathError::InvalidNumber)?;
        if !value.is_finite() {
            return Err(SvgPathError::NonFiniteGeometry);
        }
        tokens.push(PathToken::Number(value));
    }
    Ok(tokens)
}

fn next_number(
    tokens: &[PathToken],
    index: &mut usize,
    command: char,
) -> Result<f32, SvgPathError> {
    match tokens.get(*index) {
        Some(PathToken::Number(value)) => {
            *index += 1;
            Ok(*value)
        }
        _ => Err(SvgPathError::IncompleteArguments { command }),
    }
}

fn next_flag(tokens: &[PathToken], index: &mut usize, command: char) -> Result<bool, SvgPathError> {
    match next_number(tokens, index, command)? {
        0.0 => Ok(false),
        1.0 => Ok(true),
        _ => Err(SvgPathError::InvalidArcFlag),
    }
}

fn next_point(
    tokens: &[PathToken],
    index: &mut usize,
    command: char,
    current: [f32; 2],
) -> Result<[f32; 2], SvgPathError> {
    let mut point = [
        next_number(tokens, index, command)?,
        next_number(tokens, index, command)?,
    ];
    if command.is_ascii_lowercase() {
        point[0] += current[0];
        point[1] += current[1];
    }
    finite_point(point)?;
    Ok(point)
}

fn next_points(
    tokens: &[PathToken],
    index: &mut usize,
    command: char,
    current: [f32; 2],
    count: usize,
) -> Result<Vec<[f32; 2]>, SvgPathError> {
    (0..count)
        .map(|_| next_point(tokens, index, command, current))
        .collect()
}

fn push_point(path: &mut Option<Subpath>, point: [f32; 2]) -> Result<(), SvgPathError> {
    let path = path.as_mut().ok_or(SvgPathError::MissingMove)?;
    if path.points.len() >= MAX_PATH_POINTS {
        return Err(SvgPathError::TooManyPoints {
            maximum: MAX_PATH_POINTS,
        });
    }
    path.points.push(point);
    Ok(())
}

fn append_quadratic(
    path: &mut Option<Subpath>,
    start: [f32; 2],
    control: [f32; 2],
    end: [f32; 2],
) -> Result<(), SvgPathError> {
    for step in 1..=12 {
        let t = step as f32 / 12.0;
        let inverse = 1.0 - t;
        push_point(
            path,
            [
                inverse * inverse * start[0] + 2.0 * inverse * t * control[0] + t * t * end[0],
                inverse * inverse * start[1] + 2.0 * inverse * t * control[1] + t * t * end[1],
            ],
        )?;
    }
    Ok(())
}

fn append_cubic(
    path: &mut Option<Subpath>,
    start: [f32; 2],
    control_one: [f32; 2],
    control_two: [f32; 2],
    end: [f32; 2],
) -> Result<(), SvgPathError> {
    for step in 1..=16 {
        let t = step as f32 / 16.0;
        let inverse = 1.0 - t;
        push_point(
            path,
            [
                inverse.powi(3) * start[0]
                    + 3.0 * inverse.powi(2) * t * control_one[0]
                    + 3.0 * inverse * t * t * control_two[0]
                    + t.powi(3) * end[0],
                inverse.powi(3) * start[1]
                    + 3.0 * inverse.powi(2) * t * control_one[1]
                    + 3.0 * inverse * t * t * control_two[1]
                    + t.powi(3) * end[1],
            ],
        )?;
    }
    Ok(())
}

struct ArcParams {
    rx: f32,
    ry: f32,
    rotation_degrees: f32,
    large_arc: bool,
    sweep: bool,
}

fn append_arc(
    path: &mut Option<Subpath>,
    start: [f32; 2],
    params: ArcParams,
    end: [f32; 2],
) -> Result<(), SvgPathError> {
    let mut rx = params.rx;
    let mut ry = params.ry;
    if rx <= f32::EPSILON || ry <= f32::EPSILON || start == end {
        return push_point(path, end);
    }
    let phi = params.rotation_degrees.to_radians();
    let (sin_phi, cos_phi) = phi.sin_cos();
    let dx = (start[0] - end[0]) / 2.0;
    let dy = (start[1] - end[1]) / 2.0;
    let x1 = cos_phi * dx + sin_phi * dy;
    let y1 = -sin_phi * dx + cos_phi * dy;
    let radius_scale = x1.powi(2) / rx.powi(2) + y1.powi(2) / ry.powi(2);
    if radius_scale > 1.0 {
        let scale = radius_scale.sqrt();
        rx *= scale;
        ry *= scale;
    }
    let numerator = rx.powi(2) * ry.powi(2) - rx.powi(2) * y1.powi(2) - ry.powi(2) * x1.powi(2);
    let denominator = rx.powi(2) * y1.powi(2) + ry.powi(2) * x1.powi(2);
    let factor = if denominator <= f32::EPSILON {
        0.0
    } else {
        (numerator.max(0.0) / denominator).sqrt()
    } * if params.large_arc == params.sweep {
        -1.0
    } else {
        1.0
    };
    let cx1 = factor * (rx * y1 / ry);
    let cy1 = factor * (-ry * x1 / rx);
    let center = [
        cos_phi * cx1 - sin_phi * cy1 + (start[0] + end[0]) / 2.0,
        sin_phi * cx1 + cos_phi * cy1 + (start[1] + end[1]) / 2.0,
    ];
    let u = [(x1 - cx1) / rx, (y1 - cy1) / ry];
    let v = [(-x1 - cx1) / rx, (-y1 - cy1) / ry];
    let start_angle = u[1].atan2(u[0]);
    let mut delta = (u[0] * v[1] - u[1] * v[0]).atan2(u[0] * v[0] + u[1] * v[1]);
    if !params.sweep && delta > 0.0 {
        delta -= 2.0 * PI;
    } else if params.sweep && delta < 0.0 {
        delta += 2.0 * PI;
    }
    let steps = (delta.abs() / (PI / 12.0)).ceil().max(1.0) as usize;
    for step in 1..=steps {
        let angle = start_angle + delta * step as f32 / steps as f32;
        let local = [rx * angle.cos(), ry * angle.sin()];
        let point = [
            cos_phi * local[0] - sin_phi * local[1] + center[0],
            sin_phi * local[0] + cos_phi * local[1] + center[1],
        ];
        finite_point(point)?;
        push_point(path, point)?;
    }
    Ok(())
}

fn reflect(point: [f32; 2], around: [f32; 2]) -> [f32; 2] {
    [2.0 * around[0] - point[0], 2.0 * around[1] - point[1]]
}

fn finite_point(point: [f32; 2]) -> Result<(), SvgPathError> {
    if point[0].is_finite() && point[1].is_finite() {
        Ok(())
    } else {
        Err(SvgPathError::NonFiniteGeometry)
    }
}

#[derive(Clone, Copy)]
struct Rotation {
    radians: f32,
    pivot: [f32; 2],
}

fn parse_ellipse_rotation(
    transform: Option<&str>,
    cx: f32,
    cy: f32,
) -> Result<Rotation, SvgPathError> {
    let Some(transform) = transform else {
        return Ok(Rotation {
            radians: 0.0,
            pivot: [cx, cy],
        });
    };
    let value = transform
        .trim()
        .strip_prefix("rotate(")
        .and_then(|value| value.strip_suffix(')'))
        .ok_or(SvgPathError::UnsupportedCommand { command: 'R' })?;
    let values = tokenize_transform_numbers(value)?;
    if !(1..=3).contains(&values.len()) {
        return Err(SvgPathError::InvalidNumber);
    }
    Ok(Rotation {
        radians: values[0].to_radians(),
        pivot: if values.len() == 3 {
            [values[1], values[2]]
        } else {
            [cx, cy]
        },
    })
}

fn tokenize_transform_numbers(value: &str) -> Result<Vec<f32>, SvgPathError> {
    let tokens = tokenize_path(value)?;
    tokens
        .into_iter()
        .map(|token| match token {
            PathToken::Number(value) => Ok(value),
            PathToken::Command(command) => Err(SvgPathError::UnsupportedCommand { command }),
        })
        .collect()
}

fn rotate_point(point: [f32; 2], rotation: Rotation) -> [f32; 2] {
    let (sin, cos) = rotation.radians.sin_cos();
    let x = point[0] - rotation.pivot[0];
    let y = point[1] - rotation.pivot[1];
    [
        rotation.pivot[0] + x * cos - y * sin,
        rotation.pivot[1] + x * sin + y * cos,
    ]
}

fn svg_number(value: &str) -> Result<f32, SvgPathError> {
    let value = value
        .parse::<f32>()
        .map_err(|_| SvgPathError::InvalidNumber)?;
    if value.is_finite() {
        Ok(value)
    } else {
        Err(SvgPathError::NonFiniteGeometry)
    }
}

fn format_number(value: f32) -> String {
    let rounded = if value.abs() < 0.000_001 { 0.0 } else { value };
    rounded.to_string()
}

#[cfg(test)]
mod tests {
    use acorde_core::{Duration, Note, Pitch, Score, Step, TupletBoundary, TupletInfo};
    use acorde_layout::{PrintConfig, compute_print_layout};
    use acorde_render_svg::{SvgRenderOptions, render_svg};

    use super::{
        MAX_PDF_PAGES, POINTS_PER_MM, PdfError, PdfPagesError, PdfRenderOptions, SvgPathError,
        SvgPdfPage, export_print_layout_pdf, export_score_pdf, export_svg_pages_pdf,
        normalize_svg_for_harumi,
    };

    #[test]
    fn adjacent_triplets_remain_separate_in_pdf_source() {
        let mut score = Score::new("", 120, 4, 4, 0, 1);
        let mut notes = Vec::new();
        for _ in 0..4 {
            for index in 0..3 {
                let mut note = Note::new(Pitch::new(Step::C, 4), Duration::Eighth);
                note.tuplet = Some(TupletInfo {
                    actual_notes: 3,
                    normal_notes: 2,
                });
                note.tuplet_boundary = match index {
                    0 => Some(TupletBoundary::Start),
                    2 => Some(TupletBoundary::Stop),
                    _ => None,
                };
                notes.push(note);
            }
        }
        score.parts[0].staves[0].measures[0].voices[0] = notes;
        let svg = render_svg(&score, &SvgRenderOptions::default()).expect("SVG renders");
        assert_eq!(svg.matches("class=\"acorde-tuplet-number\"").count(), 4);
        let normalized =
            super::normalize_svg_rotated_ellipses(&svg).expect("PDF source normalizes");
        assert!(!normalized.contains("rotate("));
        assert!(
            normalized.contains("<path"),
            "notation curves stay vector paths"
        );
        let pdf =
            export_score_pdf(&score, &PdfRenderOptions::new(&[])).expect("vector PDF exports");
        assert!(pdf.starts_with(b"%PDF"));
    }

    #[test]
    fn normalizes_notation_curves_and_noteheads_to_harumi_subset() {
        let source = r#"<svg width="100" height="100" viewBox="0 0 100 100"><ellipse cx="50" cy="50" rx="6" ry="4" transform="rotate(-15 50 50)" fill="black"/><path d="M 0,0 C 5,20 25,20 30,0 Q 35,-10 40,0 A 8,4 0 0 1 56,0 Z" fill="black"/></svg>"#;
        let normalized = normalize_svg_for_harumi(source).expect("normalizes SVG");
        assert!(normalized.contains("<polygon"));
        assert!(!normalized.contains("<ellipse"));
        assert!(!normalized.contains("<path"));
        assert!(!normalized.contains("rotate("));
    }

    #[test]
    fn turns_transparent_hit_regions_into_non_painting_shapes() {
        let source = r#"<svg width="10" height="10"><rect x="0" y="0" width="10" height="10" fill="transparent"/></svg>"#;
        let normalized = normalize_svg_for_harumi(source).expect("normalizes SVG");
        assert!(normalized.contains("fill=\"none\""));
    }

    #[test]
    fn rejects_malformed_curve_data() {
        let source = r#"<svg width="10" height="10"><path d="M 0 0 C 1"/></svg>"#;
        assert!(matches!(
            normalize_svg_for_harumi(source),
            Err(PdfError::SvgPath(SvgPathError::IncompleteArguments {
                command: 'C'
            }))
        ));
    }

    #[test]
    fn preserves_text_and_supported_lines() {
        let source = r#"<svg width="10" height="10"><line x1="0" y1="0" x2="10" y2="10" stroke="black"/><text x="5" y="5" text-anchor="middle">A&amp;B</text></svg>"#;
        let normalized = normalize_svg_for_harumi(source).expect("normalizes SVG");
        assert!(normalized.contains("<line"));
        assert!(normalized.contains("A&amp;B"));
    }

    #[test]
    fn turns_open_filled_paths_into_polygons() {
        let source =
            r#"<svg width="10" height="10"><path d="M 0 0 C 2 4 6 4 8 0" fill="black"/></svg>"#;
        let normalized = normalize_svg_for_harumi(source).expect("normalizes SVG");
        assert!(normalized.contains("<polygon"));
        assert!(!normalized.contains("<polyline"));
    }

    #[test]
    fn normalizes_a_real_acorde_notation_svg() {
        let mut score = Score::new("PDF normalization", 120, 4, 4, 0, 1);
        score.parts[0].staves[0].measures[0].voices[0] = vec![
            Note::new(Pitch::new(Step::C, 4), Duration::Quarter),
            Note::new(Pitch::new(Step::D, 4), Duration::Quarter),
            Note::new(Pitch::new(Step::E, 4), Duration::Quarter),
            Note::new(Pitch::new(Step::F, 4), Duration::Quarter),
        ];
        let svg = render_svg(&score, &SvgRenderOptions::default()).expect("renders SVG");
        assert!(svg.contains("<ellipse"));
        assert!(svg.contains("<path"));

        let normalized = normalize_svg_for_harumi(&svg).expect("normalizes score SVG");
        assert!(!normalized.contains("<ellipse"));
        assert!(!normalized.contains("<path"));
    }

    #[test]
    fn combines_page_svgs_in_order_with_independent_media_boxes() {
        let first = r#"<svg width="100" height="100" viewBox="0 0 100 100"><line x1="10" y1="10" x2="90" y2="90" stroke="black"/></svg>"#;
        let second = r#"<svg width="100" height="100" viewBox="0 0 100 100"><rect x="20" y="20" width="60" height="60" fill="black"/></svg>"#;
        let bytes = export_svg_pages_pdf(
            &[
                SvgPdfPage {
                    svg: first,
                    page_size_pt: [595.0, 842.0],
                },
                SvgPdfPage {
                    svg: second,
                    page_size_pt: [612.0, 792.0],
                },
            ],
            &[],
        )
        .expect("two-page vector PDF");
        let mut pdf = harumi::Document::from_bytes(&bytes).expect("valid PDF bytes");
        assert_eq!(pdf.page_count(), 2);
        assert_eq!(pdf.page(1).unwrap().size().unwrap(), (595.0, 842.0));
        assert_eq!(pdf.page(2).unwrap().size().unwrap(), (612.0, 792.0));
    }

    #[test]
    fn rejects_empty_or_invalid_multipage_pdf_inputs() {
        assert!(matches!(
            export_svg_pages_pdf(&[], &[]),
            Err(PdfPagesError::InvalidPageCount { maximum }) if maximum == MAX_PDF_PAGES
        ));
        let svg = r#"<svg width="10" height="10"><line x1="0" y1="0" x2="10" y2="10" stroke="black"/></svg>"#;
        assert!(matches!(
            export_svg_pages_pdf(
                &[SvgPdfPage {
                    svg,
                    page_size_pt: [595.0, f32::NAN],
                }],
                &[],
            ),
            Err(PdfPagesError::Page {
                index: 0,
                source: PdfError::InvalidPageSize,
            })
        ));
    }

    #[test]
    fn print_layout_supplies_page_count_and_a4_media_boxes() {
        let score = Score::new("Two-page PDF", 120, 4, 4, 0, 2);
        let layout = compute_print_layout(
            &score,
            &PrintConfig {
                measures_per_system: 1,
                systems_per_page: Some(1),
                ..PrintConfig::default()
            },
        )
        .expect("two-page layout");
        assert_eq!(layout.pages.len(), 2);
        let svg = r#"<svg width="100" height="100" viewBox="0 0 100 100"><line x1="0" y1="0" x2="100" y2="100" stroke="black"/></svg>"#;
        assert!(matches!(
            export_print_layout_pdf(&layout, &[svg], &[]),
            Err(PdfPagesError::PageCountMismatch {
                expected: 2,
                actual: 1,
            })
        ));
        let bytes = export_print_layout_pdf(&layout, &[svg, svg], &[]).expect("print PDF");
        let mut pdf = harumi::Document::from_bytes(&bytes).expect("valid PDF");
        assert_eq!(pdf.page_count(), 2);
        for page in 1..=2 {
            let (width, height) = pdf.page(page).unwrap().size().unwrap();
            assert!((width - 210.0 * POINTS_PER_MM).abs() < 0.1);
            assert!((height - 297.0 * POINTS_PER_MM).abs() < 0.1);
        }
    }
}
