//! Turn generated SVG text into the exact font outlines measured by the print pass.

use crate::{FontMetricsError, TextFont};
use std::fmt::Write as _;

#[derive(Debug, thiserror::Error)]
pub enum OutlineTextError {
    #[error("generated page XML is invalid: {0}")]
    Xml(#[from] roxmltree::Error),
    #[error("text outline failed: {0}")]
    Font(#[from] FontMetricsError),
    #[error("generated SVG text has unsupported {attribute}")]
    Unsupported { attribute: &'static str },
    #[error("generated SVG text contains non-finite or invalid coordinates")]
    Geometry,
    #[error("outlined page exceeds 16 MiB")]
    PageTooLarge,
}

fn number(node: roxmltree::Node<'_, '_>, attribute: &str) -> Result<f32, OutlineTextError> {
    let value = node
        .attribute(attribute)
        .ok_or(OutlineTextError::Geometry)?
        .parse::<f32>()
        .map_err(|_| OutlineTextError::Geometry)?;
    if !value.is_finite() {
        return Err(OutlineTextError::Geometry);
    }
    Ok(value)
}

pub(crate) fn outline_page_text(
    svg: &str,
    font: &TextFont<'_>,
) -> Result<String, OutlineTextError> {
    if svg.len() > 16 * 1024 * 1024 {
        return Err(OutlineTextError::PageTooLarge);
    }
    let document = roxmltree::Document::parse(svg)?;
    let mut output = String::with_capacity(svg.len());
    let mut cursor = 0;
    for node in document.descendants().filter(|n| n.has_tag_name("text")) {
        if node.children().any(|child| child.is_element()) {
            return Err(OutlineTextError::Unsupported {
                attribute: "nested text markup",
            });
        }
        let text = node.text().unwrap_or("");
        let x = number(node, "x")?;
        let y = number(node, "y")?;
        let size = number(node, "font-size")?;
        let metrics = font.measure(text, size)?;
        let (advance, tracking, x_scale) = if node.attribute("textLength").is_some() {
            let target = number(node, "textLength")?;
            if target <= 0.0 {
                return Err(OutlineTextError::Geometry);
            }
            match node.attribute("lengthAdjust").unwrap_or("spacing") {
                "spacing" if metrics.cluster_count > 1 => (
                    target,
                    (target - metrics.advance_mm) / (metrics.cluster_count - 1) as f32,
                    1.0,
                ),
                "spacing" => (metrics.advance_mm, 0.0, 1.0),
                "spacingAndGlyphs" if metrics.advance_mm > 0.0 => {
                    (target, 0.0, target / metrics.advance_mm)
                }
                _ => {
                    return Err(OutlineTextError::Unsupported {
                        attribute: "lengthAdjust",
                    });
                }
            }
        } else {
            (metrics.advance_mm, 0.0, 1.0)
        };
        let path = font.outline_with_tracking(text, size, tracking)?;
        let offset = match node.attribute("text-anchor").unwrap_or("start") {
            "start" => 0.0,
            "middle" => advance / 2.0,
            "end" => advance,
            _ => {
                return Err(OutlineTextError::Unsupported {
                    attribute: "text-anchor",
                });
            }
        };
        let slant = match node.attribute("font-style").unwrap_or("normal") {
            "normal" => 0.0,
            "italic" | "oblique" => -0.212_556_56,
            _ => {
                return Err(OutlineTextError::Unsupported {
                    attribute: "font-style",
                });
            }
        };
        let weight = match node.attribute("font-weight").unwrap_or("normal") {
            "normal" | "400" => 0.0,
            "bold" | "700" => size * 0.025,
            _ => {
                return Err(OutlineTextError::Unsupported {
                    attribute: "font-weight",
                });
            }
        };
        let range = node.range();
        output.push_str(&svg[cursor..range.start]);
        let _ = write!(
            output,
            "<g data-acorde-outlined-text=\"true\" data-text=\"{}\" data-font-size=\"{size}\" data-text-advance=\"{}\"",
            crate::render::escape_xml(text),
            advance
        );
        for attribute in node.attributes() {
            if !matches!(
                attribute.name(),
                "x" | "y"
                    | "font-size"
                    | "font-family"
                    | "font-style"
                    | "font-weight"
                    | "text-anchor"
                    | "textLength"
                    | "lengthAdjust"
            ) {
                let _ = write!(
                    output,
                    " {}=\"{}\"",
                    attribute.name(),
                    crate::render::escape_xml(attribute.value())
                );
            }
        }
        if node.attribute("data-acorde-enclosure") == Some("rectangle") {
            let left = x - offset
                + metrics.left_mm
                + (slant * metrics.top_mm).min(slant * metrics.bottom_mm)
                - size * 0.16;
            let right = x - offset
                + metrics.right_mm
                + (slant * metrics.top_mm).max(slant * metrics.bottom_mm)
                + size * 0.16;
            let top = y + metrics.top_mm - size * 0.16;
            let height = metrics.bottom_mm - metrics.top_mm + size * 0.32;
            let _ = write!(
                output,
                "><title>{}</title><rect class=\"acorde-text-enclosure\" x=\"{left}\" y=\"{top}\" width=\"{}\" height=\"{height}\" fill=\"none\" stroke=\"black\" stroke-width=\"{}\"/><path d=\"{path}\" transform=\"matrix({x_scale} 0 {slant} 1 {} {y})\"",
                crate::render::escape_xml(text),
                right - left,
                size * 0.04,
                x - offset
            );
        } else {
            let _ = write!(
                output,
                "><title>{}</title><path d=\"{path}\" transform=\"matrix({x_scale} 0 {slant} 1 {} {y})\"",
                crate::render::escape_xml(text),
                x - offset
            );
        }
        if weight > 0.0 {
            let _ = write!(
                output,
                " stroke=\"black\" stroke-width=\"{weight}\" stroke-linejoin=\"round\""
            );
        }
        output.push_str("/></g>");
        cursor = range.end;
        if output.len() > 16 * 1024 * 1024 {
            return Err(OutlineTextError::PageTooLarge);
        }
    }
    output.push_str(&svg[cursor..]);
    if output.len() > 16 * 1024 * 1024 {
        return Err(OutlineTextError::PageTooLarge);
    }
    Ok(output)
}
