use crate::{LayoutConfig, compute_layout};
use acorde_core::Score;
use serde::{Deserialize, Serialize};

/// A paper size expressed in physical millimetres.
#[derive(Debug, Clone, Copy, Serialize, Deserialize, PartialEq)]
pub enum PaperSize {
    A4,
    Letter,
    Legal,
    Custom { width_mm: f32, height_mm: f32 },
}

impl PaperSize {
    fn dimensions_mm(self) -> (f32, f32) {
        match self {
            Self::A4 => (210.0, 297.0),
            Self::Letter => (215.9, 279.4),
            Self::Legal => (215.9, 355.6),
            Self::Custom {
                width_mm,
                height_mm,
            } => (width_mm, height_mm),
        }
    }
}

/// Page orientation for a logical print layout.
#[derive(Debug, Clone, Copy, Serialize, Deserialize, PartialEq, Eq)]
pub enum PageOrientation {
    Portrait,
    Landscape,
}

/// Host-neutral inputs for deterministic page and system layout.
///
/// This contract describes physical page geometry only. It intentionally does not select
/// fonts, emit PDF, access printers, or perform filesystem I/O.
#[derive(Debug, Clone, Serialize, Deserialize, PartialEq)]
#[serde(default)]
pub struct PrintConfig {
    pub paper_size: PaperSize,
    pub orientation: PageOrientation,
    pub margin_top_mm: f32,
    pub margin_right_mm: f32,
    pub margin_bottom_mm: f32,
    pub margin_left_mm: f32,
    pub bleed_top_mm: f32,
    pub bleed_right_mm: f32,
    pub bleed_bottom_mm: f32,
    pub bleed_left_mm: f32,
    pub safe_top_mm: f32,
    pub safe_right_mm: f32,
    pub safe_bottom_mm: f32,
    pub safe_left_mm: f32,
    pub system_height_mm: f32,
    pub measures_per_system: usize,
    /// Override the number of systems per page. When omitted it is derived from the usable
    /// page height and `system_height_mm`.
    pub systems_per_page: Option<usize>,
}

impl Default for PrintConfig {
    fn default() -> Self {
        Self {
            paper_size: PaperSize::A4,
            orientation: PageOrientation::Portrait,
            margin_top_mm: 16.0,
            margin_right_mm: 14.0,
            margin_bottom_mm: 16.0,
            margin_left_mm: 14.0,
            bleed_top_mm: 0.0,
            bleed_right_mm: 0.0,
            bleed_bottom_mm: 0.0,
            bleed_left_mm: 0.0,
            safe_top_mm: 0.0,
            safe_right_mm: 0.0,
            safe_bottom_mm: 0.0,
            safe_left_mm: 0.0,
            system_height_mm: 24.0,
            measures_per_system: 4,
            systems_per_page: None,
        }
    }
}

/// A logical system placed on a page.
#[derive(Debug, Clone, Serialize, Deserialize, PartialEq)]
pub struct SystemLayout {
    pub address: SystemAddress,
    pub system_index: usize,
    pub page_index: usize,
    pub measure_indices: Vec<usize>,
    pub top_mm: f32,
    pub height_mm: f32,
    pub break_reason: BreakReason,
}

/// Stable address of a page within one print-layout result.
#[derive(Debug, Clone, Copy, Serialize, Deserialize, PartialEq, Eq)]
pub struct PageAddress {
    pub page_index: usize,
}

/// Stable address of a system, including global and page-local positions.
#[derive(Debug, Clone, Copy, Serialize, Deserialize, PartialEq, Eq)]
pub struct SystemAddress {
    pub system_index: usize,
    pub page_index: usize,
    pub index_on_page: usize,
}

/// Explains why a system or page ended at its final measure.
#[derive(Debug, Clone, Copy, Serialize, Deserialize, PartialEq, Eq)]
pub enum BreakReason {
    MeasureCapacity,
    ExplicitSystemBreak,
    ExplicitPageBreak,
    PageCapacity,
    EndOfScore,
}

/// One page in a [`PrintLayoutResult`].
#[derive(Debug, Clone, Serialize, Deserialize, PartialEq)]
pub struct PageLayout {
    pub address: PageAddress,
    pub page_index: usize,
    pub width_mm: f32,
    pub height_mm: f32,
    pub content_width_mm: f32,
    pub content_height_mm: f32,
    pub bleed_top_mm: f32,
    pub bleed_right_mm: f32,
    pub bleed_bottom_mm: f32,
    pub bleed_left_mm: f32,
    pub systems: Vec<SystemLayout>,
    pub break_reason: BreakReason,
}

/// Deterministic page/system geometry for a score.
#[derive(Debug, Clone, Serialize, Deserialize, PartialEq)]
pub struct PrintLayoutResult {
    pub contract_version: u16,
    pub pages: Vec<PageLayout>,
}

#[derive(Debug, thiserror::Error, PartialEq)]
pub enum PrintLayoutError {
    #[error("paper dimensions must be finite and greater than zero")]
    InvalidPaperDimensions,
    #[error("margins must be finite and non-negative")]
    InvalidMargins,
    #[error("system height must be finite and greater than zero")]
    InvalidSystemHeight,
    #[error("margins leave no usable page area")]
    NoUsablePageArea,
}

/// Compute physical page and system placement without rendering or host integration.
pub fn compute_print_layout(
    score: &Score,
    config: &PrintConfig,
) -> Result<PrintLayoutResult, PrintLayoutError> {
    let (mut width_mm, mut height_mm) = config.paper_size.dimensions_mm();
    if !width_mm.is_finite() || !height_mm.is_finite() || width_mm <= 0.0 || height_mm <= 0.0 {
        return Err(PrintLayoutError::InvalidPaperDimensions);
    }
    if matches!(config.orientation, PageOrientation::Landscape) {
        std::mem::swap(&mut width_mm, &mut height_mm);
    }

    let margins = [
        config.margin_top_mm,
        config.margin_right_mm,
        config.margin_bottom_mm,
        config.margin_left_mm,
        config.bleed_top_mm,
        config.bleed_right_mm,
        config.bleed_bottom_mm,
        config.bleed_left_mm,
        config.safe_top_mm,
        config.safe_right_mm,
        config.safe_bottom_mm,
        config.safe_left_mm,
    ];
    if margins
        .iter()
        .any(|value| !value.is_finite() || *value < 0.0)
    {
        return Err(PrintLayoutError::InvalidMargins);
    }
    if !config.system_height_mm.is_finite() || config.system_height_mm <= 0.0 {
        return Err(PrintLayoutError::InvalidSystemHeight);
    }

    let content_width_mm = width_mm
        - config.margin_left_mm
        - config.margin_right_mm
        - config.safe_left_mm
        - config.safe_right_mm;
    let content_height_mm = height_mm
        - config.margin_top_mm
        - config.margin_bottom_mm
        - config.safe_top_mm
        - config.safe_bottom_mm;
    if content_width_mm <= 0.0 || content_height_mm <= 0.0 {
        return Err(PrintLayoutError::NoUsablePageArea);
    }

    let systems_per_page = config
        .systems_per_page
        .unwrap_or_else(|| {
            (content_height_mm / config.system_height_mm)
                .floor()
                .max(1.0) as usize
        })
        .max(1);
    let layout = compute_layout(
        score,
        &LayoutConfig {
            measures_per_row: config.measures_per_system.max(1),
            ..LayoutConfig::default()
        },
    );

    let mut pages = Vec::new();
    let mut page_systems = Vec::new();
    let mut page_index = 0;
    for (system_index, row) in layout.rows.iter().enumerate() {
        let explicit_page_break = row.measure_indices.last().is_some_and(|&measure_index| {
            score
                .parts
                .iter()
                .flat_map(|part| part.staves.iter())
                .filter_map(|staff| staff.measures.get(measure_index))
                .any(|measure| measure.page_break)
        });
        let explicit_system_break = row.measure_indices.last().is_some_and(|&measure_index| {
            score
                .parts
                .iter()
                .flat_map(|part| part.staves.iter())
                .filter_map(|staff| staff.measures.get(measure_index))
                .any(|measure| measure.system_break)
        });
        let is_last_system = system_index + 1 == layout.rows.len();
        let break_reason = if explicit_page_break {
            BreakReason::ExplicitPageBreak
        } else if explicit_system_break {
            BreakReason::ExplicitSystemBreak
        } else if is_last_system {
            BreakReason::EndOfScore
        } else {
            BreakReason::MeasureCapacity
        };
        let system = SystemLayout {
            address: SystemAddress {
                system_index,
                page_index,
                index_on_page: page_systems.len(),
            },
            system_index,
            page_index,
            measure_indices: row.measure_indices.clone(),
            top_mm: config.margin_top_mm
                + config.safe_top_mm
                + page_systems.len() as f32 * config.system_height_mm,
            height_mm: config.system_height_mm,
            break_reason,
        };
        page_systems.push(system);

        let page_is_full = page_systems.len() >= systems_per_page;
        if page_is_full || explicit_page_break {
            let page_break_reason = if explicit_page_break {
                BreakReason::ExplicitPageBreak
            } else if is_last_system {
                BreakReason::EndOfScore
            } else {
                BreakReason::PageCapacity
            };
            pages.push(PageLayout {
                address: PageAddress { page_index },
                page_index,
                width_mm,
                height_mm,
                content_width_mm,
                content_height_mm,
                bleed_top_mm: config.bleed_top_mm,
                bleed_right_mm: config.bleed_right_mm,
                bleed_bottom_mm: config.bleed_bottom_mm,
                bleed_left_mm: config.bleed_left_mm,
                systems: std::mem::take(&mut page_systems),
                break_reason: page_break_reason,
            });
            page_index += 1;
        }
    }
    if !page_systems.is_empty() || pages.is_empty() {
        pages.push(PageLayout {
            address: PageAddress { page_index },
            page_index,
            width_mm,
            height_mm,
            content_width_mm,
            content_height_mm,
            bleed_top_mm: config.bleed_top_mm,
            bleed_right_mm: config.bleed_right_mm,
            bleed_bottom_mm: config.bleed_bottom_mm,
            bleed_left_mm: config.bleed_left_mm,
            systems: page_systems,
            break_reason: BreakReason::EndOfScore,
        });
    }

    Ok(PrintLayoutResult {
        contract_version: 3,
        pages,
    })
}

#[cfg(test)]
mod tests {
    use super::*;
    use acorde_core::{Clef, Measure, Part, Score, Staff};

    fn score_with_measures(count: usize) -> Score {
        let mut score = Score::default();
        let mut part = Part::new("Piano", "Pno.");
        let mut staff = Staff::new(Clef::Treble);
        staff.measures = (0..count).map(|_| Measure::empty(4, 4)).collect();
        part.staves = vec![staff];
        score.parts = vec![part];
        score
    }

    #[test]
    fn paginates_rows_and_preserves_measure_indices() {
        let score = score_with_measures(5);
        let result = compute_print_layout(
            &score,
            &PrintConfig {
                measures_per_system: 2,
                systems_per_page: Some(2),
                ..PrintConfig::default()
            },
        )
        .expect("valid print config");
        assert_eq!(result.pages.len(), 2);
        assert_eq!(
            result.pages[0]
                .systems
                .iter()
                .map(|s| s.measure_indices.clone())
                .collect::<Vec<_>>(),
            vec![vec![0, 1], vec![2, 3]]
        );
        assert_eq!(result.pages[1].systems[0].measure_indices, vec![4]);
        assert_eq!(result.pages[1].systems[0].page_index, 1);
        assert_eq!(result.pages[1].systems[0].address.index_on_page, 0);
        assert_eq!(
            result.pages[1].systems[0].break_reason,
            BreakReason::EndOfScore
        );
        assert_eq!(result.pages[0].break_reason, BreakReason::PageCapacity);
    }

    #[test]
    fn forced_page_break_starts_next_system_on_next_page() {
        let mut score = score_with_measures(3);
        score.parts[0].staves[0].measures[0].page_break = true;
        let result = compute_print_layout(
            &score,
            &PrintConfig {
                measures_per_system: 3,
                systems_per_page: Some(8),
                ..PrintConfig::default()
            },
        )
        .expect("valid print config");
        assert_eq!(result.pages.len(), 2);
        assert_eq!(result.pages[0].systems[0].measure_indices, vec![0]);
        assert_eq!(result.pages[1].systems[0].measure_indices, vec![1, 2]);
        assert_eq!(result.pages[0].break_reason, BreakReason::ExplicitPageBreak);
        assert_eq!(
            result.pages[0].systems[0].break_reason,
            BreakReason::ExplicitPageBreak
        );
    }

    #[test]
    fn rejects_margins_that_leave_no_page_area() {
        let score = score_with_measures(1);
        let error = compute_print_layout(
            &score,
            &PrintConfig {
                margin_left_mm: 200.0,
                ..PrintConfig::default()
            },
        )
        .expect_err("invalid page area");
        assert_eq!(error, PrintLayoutError::NoUsablePageArea);
    }

    #[test]
    fn safe_area_reduces_content_and_bleed_is_exposed() {
        let score = score_with_measures(1);
        let result = compute_print_layout(
            &score,
            &PrintConfig {
                bleed_top_mm: 3.0,
                bleed_right_mm: 3.0,
                bleed_bottom_mm: 3.0,
                bleed_left_mm: 3.0,
                safe_top_mm: 5.0,
                safe_right_mm: 6.0,
                safe_bottom_mm: 7.0,
                safe_left_mm: 8.0,
                ..PrintConfig::default()
            },
        )
        .expect("valid print config");
        let page = &result.pages[0];
        assert_eq!(result.contract_version, 3);
        assert_eq!(page.bleed_left_mm, 3.0);
        assert_eq!(page.content_width_mm, 210.0 - 14.0 - 14.0 - 8.0 - 6.0);
        assert_eq!(page.content_height_mm, 297.0 - 16.0 - 16.0 - 5.0 - 7.0);
        assert_eq!(page.systems[0].top_mm, 21.0);
    }
}
