use crate::{LayoutConfig, SpanMark, compute_layout};
use acorde_core::{Barline, Score};
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

/// Policy for the page number exposed in logical page metadata.
#[derive(Debug, Clone, Copy, Serialize, Deserialize, PartialEq, Eq)]
pub enum PageNumbering {
    None,
    OneBased,
}

/// Policy for distributing systems when automatic pagination would leave a one-system final page.
#[derive(Debug, Clone, Copy, Serialize, Deserialize, PartialEq, Eq, Default)]
pub enum FinalPagePolicy {
    /// Preserve the configured page capacity, even when the final page is short.
    #[default]
    AllowSingleSystem,
    /// Redistribute automatically paginated systems as evenly as possible across pages.
    Balance,
}

/// Policy for reserving the first system for a partial pickup measure.
#[derive(Debug, Clone, Copy, Serialize, Deserialize, PartialEq, Eq, Default)]
pub enum PickupPolicy {
    /// Detect a non-empty partial first measure automatically (the default).
    #[default]
    Auto,
    /// Do not infer pickup measures from score content.
    Preserve,
    /// Detect a non-empty first measure shorter than its time signature and isolate it.
    DetectFirstMeasure,
}

/// Policy for preserving repeat-ending notation while systems are reflowed.
#[derive(Debug, Clone, Copy, Serialize, Deserialize, PartialEq, Eq, Default)]
pub enum NotationBreakPolicy {
    /// Keep the score's normal automatic system breaks.
    #[default]
    Preserve,
    /// Keep each contiguous volta ending in one system when it fits.
    KeepVoltaTogether,
    /// Keep each repeat section on one page when it fits the page capacity.
    KeepRepeatsTogether,
}

/// Color intent for a print-capable host.
#[derive(Debug, Clone, Copy, Serialize, Deserialize, PartialEq, Eq, Default)]
pub enum PrintColorPolicy {
    #[default]
    Monochrome,
    Preserve,
}

/// Whether a host should expose crop marks at the configured bleed boundary.
#[derive(Debug, Clone, Copy, Serialize, Deserialize, PartialEq, Eq, Default)]
pub enum CropMarkPolicy {
    #[default]
    None,
    BleedEdges,
}

/// How a host resolves fonts and notation glyph resources for print output.
#[derive(Debug, Clone, Serialize, Deserialize, PartialEq, Eq, Default)]
pub enum GlyphResourcePolicy {
    /// Use the renderer's deterministic built-in vector glyphs where available.
    #[default]
    BuiltInVector,
    /// Resolve a host-owned resource identified by this stable application key.
    HostProvided(String),
}

/// A contiguous range of physical measures that must remain in one printed system.
///
/// Both endpoints are zero-based and inclusive. This is intentionally a layout request,
/// not a score-model mutation, so hosts can apply publication presets without changing the
/// editable score.
#[derive(Debug, Clone, Copy, Serialize, Deserialize, PartialEq, Eq)]
pub struct KeepTogetherRange {
    pub first_measure: usize,
    pub last_measure: usize,
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
    /// Content scale factor. `1.0` preserves the configured system height.
    pub scale: f32,
    pub measures_per_system: usize,
    /// Optional measure capacity for the first system, useful for pickup/title systems.
    #[serde(default)]
    pub first_system_measures: Option<usize>,
    #[serde(default)]
    pub pickup_policy: PickupPolicy,
    #[serde(default)]
    pub notation_break_policy: NotationBreakPolicy,
    /// Override the number of systems per page. When omitted it is derived from the usable
    /// page height and `system_height_mm`.
    pub systems_per_page: Option<usize>,
    pub page_numbering: PageNumbering,
    #[serde(default)]
    pub final_page_policy: FinalPagePolicy,
    #[serde(default)]
    pub color_policy: PrintColorPolicy,
    #[serde(default)]
    pub crop_mark_policy: CropMarkPolicy,
    #[serde(default)]
    pub glyph_resources: GlyphResourcePolicy,
    /// Physical measure ranges that must not be split across systems.
    #[serde(default)]
    pub keep_together: Vec<KeepTogetherRange>,
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
            scale: 1.0,
            measures_per_system: 4,
            first_system_measures: None,
            pickup_policy: PickupPolicy::Auto,
            notation_break_policy: NotationBreakPolicy::Preserve,
            systems_per_page: None,
            page_numbering: PageNumbering::OneBased,
            final_page_policy: FinalPagePolicy::AllowSingleSystem,
            color_policy: PrintColorPolicy::Monochrome,
            crop_mark_policy: CropMarkPolicy::None,
            glyph_resources: GlyphResourcePolicy::BuiltInVector,
            keep_together: Vec::new(),
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
    /// Physical intervals represented by the system, including multi-rest spans.
    #[serde(default)]
    pub measure_spans: Vec<MeasureSpan>,
    /// Span segments touching this system, with start/end ownership for host continuation marks.
    #[serde(default)]
    pub span_segments: Vec<SpanSegment>,
    /// Repeat, ending, navigation, and rehearsal marks belonging to this system.
    #[serde(default)]
    pub measure_marks: Vec<MeasureMark>,
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

/// Physical measure interval represented by one visual measure slot.
#[derive(Debug, Clone, Copy, Serialize, Deserialize, PartialEq, Eq)]
pub struct MeasureSpan {
    pub first_measure: usize,
    pub last_measure: usize,
}

/// A span's intersection with one printed system.
#[derive(Debug, Clone, Copy, Serialize, Deserialize, PartialEq, Eq)]
pub struct SpanSegment {
    pub span_index: usize,
    pub starts_here: bool,
    pub ends_here: bool,
}

/// A cross-system span's intersection with one printed page.
#[derive(Debug, Clone, Copy, Serialize, Deserialize, PartialEq, Eq)]
pub struct PageSpanSegment {
    pub span_index: usize,
    pub starts_here: bool,
    pub ends_here: bool,
}

/// Host-neutral notation marks attached to one physical measure in a print system.
///
/// This is presentation metadata only: playback order remains the responsibility of
/// [`acorde_core::measure_sequence`].
#[derive(Debug, Clone, Serialize, Deserialize, PartialEq, Eq)]
pub struct MeasureMark {
    pub measure_index: usize,
    pub repeat_start: bool,
    pub repeat_end: bool,
    pub volta_number: Option<u8>,
    pub volta_kind: Option<String>,
    pub navigation: Option<String>,
    pub rehearsal: Option<String>,
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
    pub page_number: Option<usize>,
    #[serde(default)]
    pub color_policy: PrintColorPolicy,
    #[serde(default)]
    pub crop_mark_policy: CropMarkPolicy,
    #[serde(default)]
    pub glyph_resources: GlyphResourcePolicy,
    pub width_mm: f32,
    pub height_mm: f32,
    pub content_width_mm: f32,
    pub content_height_mm: f32,
    pub bleed_top_mm: f32,
    pub bleed_right_mm: f32,
    pub bleed_bottom_mm: f32,
    pub bleed_left_mm: f32,
    pub systems: Vec<SystemLayout>,
    /// Span intersections on this page, aggregated from its systems.
    #[serde(default)]
    pub span_segments: Vec<PageSpanSegment>,
    /// Repeat and navigation marks on this page, in physical measure order.
    #[serde(default)]
    pub measure_marks: Vec<MeasureMark>,
    pub break_reason: BreakReason,
}

impl PageLayout {
    /// Return the inclusive physical measure range represented on this page.
    pub fn measure_span(&self) -> Option<MeasureSpan> {
        let mut spans = self
            .systems
            .iter()
            .flat_map(|system| system.measure_spans.iter().copied());
        let first = spans.next()?;
        Some(spans.fold(first, |range, span| MeasureSpan {
            first_measure: range.first_measure.min(span.first_measure),
            last_measure: range.last_measure.max(span.last_measure),
        }))
    }

    /// Whether a span continues into or out of another printed page.
    pub fn has_span_continuation(&self) -> bool {
        self.span_segments
            .iter()
            .any(|segment| !segment.starts_here || !segment.ends_here)
    }
}

/// Deterministic page/system geometry for a score.
#[derive(Debug, Clone, Serialize, Deserialize, PartialEq)]
pub struct PrintLayoutResult {
    pub contract_version: u16,
    pub pages: Vec<PageLayout>,
}

impl PrintLayoutResult {
    /// Retrieve one page artifact by its stable address without recomputing layout.
    pub fn page(&self, address: PageAddress) -> Option<&PageLayout> {
        self.pages.get(address.page_index)
    }
}

#[derive(Debug, thiserror::Error, PartialEq)]
pub enum PrintLayoutError {
    #[error("paper dimensions must be finite and greater than zero")]
    InvalidPaperDimensions,
    #[error("margins must be finite and non-negative")]
    InvalidMargins,
    #[error("system height must be finite and greater than zero")]
    InvalidSystemHeight,
    #[error("print scale must be finite and greater than zero")]
    InvalidScale,
    #[error("margins leave no usable page area")]
    NoUsablePageArea,
    #[error("keep-together range is outside the score or reversed")]
    InvalidKeepTogetherRange,
    #[error("keep-together range exceeds the measures-per-system capacity")]
    KeepTogetherExceedsSystemCapacity,
    #[error("keep-together range conflicts with an explicit system or page break")]
    KeepTogetherConflictsWithExplicitBreak,
    #[error("repeat section exceeds the systems-per-page capacity")]
    RepeatRangeExceedsPageCapacity,
}

fn apply_keep_together(
    score: &Score,
    mut rows: Vec<crate::RowLayout>,
    ranges: &[KeepTogetherRange],
    capacity: usize,
) -> Result<Vec<crate::RowLayout>, PrintLayoutError> {
    let measure_count = score
        .parts
        .first()
        .and_then(|part| part.staves.first())
        .map(|staff| staff.measures.len())
        .unwrap_or(0);
    for range in ranges {
        let length = range
            .last_measure
            .checked_sub(range.first_measure)
            .and_then(|length| length.checked_add(1));
        if range.first_measure > range.last_measure || range.last_measure >= measure_count {
            return Err(PrintLayoutError::InvalidKeepTogetherRange);
        }
        if length.is_none_or(|length| length > capacity) {
            return Err(PrintLayoutError::KeepTogetherExceedsSystemCapacity);
        }
        for measure_index in range.first_measure..range.last_measure {
            let has_break = score
                .parts
                .iter()
                .flat_map(|part| part.staves.iter())
                .filter_map(|staff| staff.measures.get(measure_index))
                .any(|measure| measure.system_break || measure.page_break);
            if has_break {
                return Err(PrintLayoutError::KeepTogetherConflictsWithExplicitBreak);
            }
        }

        // Split at the range boundaries before merging rows. This allows a range that
        // crosses an existing system boundary to be reflowed without pulling unrelated
        // measures into the merged system.
        let mut split_rows = Vec::with_capacity(rows.len() + 2);
        for row in rows {
            let mut cuts = vec![0, row.measure_indices.len()];
            if let Some(position) = row
                .measure_indices
                .iter()
                .position(|&index| index == range.first_measure)
            {
                cuts.push(position);
            }
            if let Some(position) = row
                .measure_indices
                .iter()
                .position(|&index| index == range.last_measure)
            {
                cuts.push(position + 1);
            }
            cuts.sort_unstable();
            cuts.dedup();
            for window in cuts.windows(2) {
                if window[0] < window[1] {
                    split_rows.push(crate::RowLayout {
                        measure_indices: row.measure_indices[window[0]..window[1]].to_vec(),
                    });
                }
            }
        }
        rows = split_rows;

        let first_row = rows
            .iter()
            .position(|row| row.measure_indices.contains(&range.first_measure));
        let last_row = rows
            .iter()
            .position(|row| row.measure_indices.contains(&range.last_measure));
        let (Some(first_row), Some(last_row)) = (first_row, last_row) else {
            return Err(PrintLayoutError::InvalidKeepTogetherRange);
        };

        if first_row != last_row {
            let merged: Vec<usize> = rows[first_row..=last_row]
                .iter()
                .flat_map(|row| row.measure_indices.iter().copied())
                .collect();
            if merged.len() > capacity {
                return Err(PrintLayoutError::KeepTogetherExceedsSystemCapacity);
            }
            rows.splice(
                first_row..=last_row,
                [crate::RowLayout {
                    measure_indices: merged,
                }],
            );
        }

        let row_index = rows
            .iter()
            .position(|row| row.measure_indices.contains(&range.first_measure))
            .ok_or(PrintLayoutError::InvalidKeepTogetherRange)?;
        let row = rows.remove(row_index);
        let start = row
            .measure_indices
            .iter()
            .position(|&index| index == range.first_measure)
            .ok_or(PrintLayoutError::InvalidKeepTogetherRange)?;
        let end = row
            .measure_indices
            .iter()
            .position(|&index| index == range.last_measure)
            .ok_or(PrintLayoutError::InvalidKeepTogetherRange)?;
        let mut replacement = Vec::new();
        if start > 0 {
            replacement.push(crate::RowLayout {
                measure_indices: row.measure_indices[..start].to_vec(),
            });
        }
        replacement.push(crate::RowLayout {
            measure_indices: row.measure_indices[start..=end].to_vec(),
        });
        if end + 1 < row.measure_indices.len() {
            replacement.push(crate::RowLayout {
                measure_indices: row.measure_indices[end + 1..].to_vec(),
            });
        }
        rows.splice(row_index..row_index, replacement);
    }
    Ok(rows)
}

fn has_first_measure_pickup(score: &Score) -> bool {
    let Some(staff) = score.parts.first().and_then(|part| part.staves.first()) else {
        return false;
    };
    let Some(measure) = staff.measures.first() else {
        return false;
    };
    let expected = measure
        .time_sig
        .as_ref()
        .unwrap_or(&score.settings.time_signature)
        .total_beats();
    let actual = measure
        .voices
        .iter()
        .map(|voice| voice.iter().map(|note| note.beats()).sum::<f64>())
        .fold(0.0, f64::max);
    actual > 1e-9 && actual + 1e-9 < expected
}

fn measure_spans(score: &Score, measure_indices: &[usize]) -> Vec<MeasureSpan> {
    let measure_count = score
        .parts
        .first()
        .and_then(|part| part.staves.first())
        .map(|staff| staff.measures.len())
        .unwrap_or(0);
    measure_indices
        .iter()
        .filter_map(|&first_measure| {
            if first_measure >= measure_count {
                return None;
            }
            let count = score
                .parts
                .iter()
                .flat_map(|part| part.staves.iter())
                .filter_map(|staff| staff.measures.get(first_measure))
                .filter_map(|measure| measure.multi_rest_count)
                .map(usize::from)
                .max()
                .unwrap_or(1)
                .max(1);
            Some(MeasureSpan {
                first_measure,
                last_measure: first_measure
                    .saturating_add(count.saturating_sub(1))
                    .min(measure_count.saturating_sub(1)),
            })
        })
        .collect()
}

fn span_bounds(span: &SpanMark) -> (usize, usize) {
    match span {
        SpanMark::Hairpin { start, end, .. }
        | SpanMark::Ottava { start, end, .. }
        | SpanMark::Pedal { start, end }
        | SpanMark::Slur { start, end }
        | SpanMark::TrillLine { start, end }
        | SpanMark::Glissando { start, end } => (
            start.measure.min(end.measure),
            start.measure.max(end.measure),
        ),
    }
}

fn span_segments(spans: &[SpanMark], measure_indices: &[usize]) -> Vec<SpanSegment> {
    let (Some(&first_measure), Some(&last_measure)) =
        (measure_indices.first(), measure_indices.last())
    else {
        return Vec::new();
    };
    spans
        .iter()
        .enumerate()
        .filter_map(|(span_index, span)| {
            let (start_measure, end_measure) = span_bounds(span);
            (start_measure <= last_measure && end_measure >= first_measure).then_some(SpanSegment {
                span_index,
                starts_here: (first_measure..=last_measure).contains(&start_measure),
                ends_here: (first_measure..=last_measure).contains(&end_measure),
            })
        })
        .collect()
}

fn measure_marks(score: &Score, measure_indices: &[usize]) -> Vec<MeasureMark> {
    let Some(staff) = score.parts.first().and_then(|part| part.staves.first()) else {
        return Vec::new();
    };
    measure_indices
        .iter()
        .filter_map(|&measure_index| {
            let measure = staff.measures.get(measure_index)?;
            let repeat_start = matches!(
                measure.barline_left,
                Barline::RepeatStart | Barline::RepeatBoth
            );
            let repeat_end = matches!(
                measure.barline_right,
                Barline::RepeatEnd | Barline::RepeatBoth
            );
            let has_mark = repeat_start
                || repeat_end
                || measure.volta.is_some()
                || measure.navigation.is_some()
                || measure.rehearsal.is_some();
            has_mark.then(|| MeasureMark {
                measure_index,
                repeat_start,
                repeat_end,
                volta_number: measure.volta.as_ref().map(|volta| volta.number),
                volta_kind: measure.volta.as_ref().map(|volta| volta.kind.clone()),
                navigation: measure.navigation.clone(),
                rehearsal: measure.rehearsal.clone(),
            })
        })
        .collect()
}

fn volta_ranges(score: &Score) -> Vec<KeepTogetherRange> {
    let Some(staff) = score.parts.first().and_then(|part| part.staves.first()) else {
        return Vec::new();
    };
    let mut ranges = Vec::new();
    let mut start = None;
    for (index, measure) in staff.measures.iter().enumerate() {
        let Some(volta) = measure.volta.as_ref() else {
            continue;
        };
        if matches!(volta.kind.as_str(), "begin" | "begin_end") {
            start = Some(index);
        }
        if matches!(volta.kind.as_str(), "end" | "begin_end")
            && let Some(first_measure) = start.take()
        {
            ranges.push(KeepTogetherRange {
                first_measure,
                last_measure: index,
            });
        }
    }
    ranges
}

fn repeat_ranges(score: &Score) -> Vec<KeepTogetherRange> {
    let Some(staff) = score.parts.first().and_then(|part| part.staves.first()) else {
        return Vec::new();
    };
    let mut ranges = Vec::new();
    let mut start = None;
    for (index, measure) in staff.measures.iter().enumerate() {
        if matches!(
            measure.barline_left,
            Barline::RepeatStart | Barline::RepeatBoth
        ) {
            start = Some(index);
        }
        if matches!(
            measure.barline_right,
            Barline::RepeatEnd | Barline::RepeatBoth
        ) {
            ranges.push(KeepTogetherRange {
                first_measure: start.take().unwrap_or(0),
                last_measure: index,
            });
        }
    }
    ranges
}

fn repeat_system_ranges(score: &Score, rows: &[crate::RowLayout]) -> Vec<(usize, usize)> {
    repeat_ranges(score)
        .into_iter()
        .filter_map(|range| {
            let first = rows
                .iter()
                .position(|row| row.measure_indices.contains(&range.first_measure))?;
            let last = rows
                .iter()
                .position(|row| row.measure_indices.contains(&range.last_measure))?;
            Some((first, last))
        })
        .collect()
}

fn page_span_segments(systems: &[SystemLayout]) -> Vec<PageSpanSegment> {
    let mut segments = Vec::new();
    for system in systems {
        for segment in &system.span_segments {
            if let Some(existing) = segments
                .iter_mut()
                .find(|existing: &&mut PageSpanSegment| existing.span_index == segment.span_index)
            {
                existing.ends_here |= segment.ends_here;
            } else {
                segments.push(PageSpanSegment {
                    span_index: segment.span_index,
                    starts_here: segment.starts_here,
                    ends_here: segment.ends_here,
                });
            }
        }
    }
    segments
}

fn page_measure_marks(systems: &[SystemLayout]) -> Vec<MeasureMark> {
    systems
        .iter()
        .flat_map(|system| system.measure_marks.iter().cloned())
        .collect()
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
    if !config.scale.is_finite() || config.scale <= 0.0 {
        return Err(PrintLayoutError::InvalidScale);
    }
    let scaled_system_height_mm = config.system_height_mm * config.scale;
    if !scaled_system_height_mm.is_finite() || scaled_system_height_mm <= 0.0 {
        return Err(PrintLayoutError::InvalidScale);
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
            (content_height_mm / scaled_system_height_mm)
                .floor()
                .max(1.0) as usize
        })
        .max(1);
    let layout = compute_layout(
        score,
        &LayoutConfig {
            measures_per_row: config.measures_per_system.max(1),
            first_row_measures: config.first_system_measures.or_else(|| {
                (matches!(
                    config.pickup_policy,
                    PickupPolicy::Auto | PickupPolicy::DetectFirstMeasure
                ) && has_first_measure_pickup(score))
                .then_some(1)
            }),
            ..LayoutConfig::default()
        },
    );

    let mut keep_together = config.keep_together.clone();
    if matches!(
        config.notation_break_policy,
        NotationBreakPolicy::KeepVoltaTogether
    ) {
        keep_together.extend(volta_ranges(score));
    }
    let rows = apply_keep_together(
        score,
        layout.rows,
        &keep_together,
        config.measures_per_system.max(1),
    )?;

    let has_explicit_page_break = rows.iter().any(|row| {
        row.measure_indices.last().is_some_and(|&measure_index| {
            score
                .parts
                .iter()
                .flat_map(|part| part.staves.iter())
                .filter_map(|staff| staff.measures.get(measure_index))
                .any(|measure| measure.page_break)
        })
    });
    let repeat_system_ranges = if matches!(
        config.notation_break_policy,
        NotationBreakPolicy::KeepRepeatsTogether
    ) {
        repeat_system_ranges(score, &rows)
    } else {
        Vec::new()
    };
    if repeat_system_ranges
        .iter()
        .any(|(first, last)| last.saturating_sub(*first).saturating_add(1) > systems_per_page)
    {
        return Err(PrintLayoutError::RepeatRangeExceedsPageCapacity);
    }
    let page_capacities = if matches!(config.final_page_policy, FinalPagePolicy::Balance)
        && !has_explicit_page_break
        && systems_per_page > 1
        && rows.len() > systems_per_page
        && repeat_system_ranges.is_empty()
    {
        let page_count = rows.len().div_ceil(systems_per_page);
        let base = rows.len() / page_count;
        let remainder = rows.len() % page_count;
        (0..page_count)
            .map(|index| base + usize::from(index < remainder))
            .collect::<Vec<_>>()
    } else {
        Vec::new()
    };

    let mut pages = Vec::new();
    let mut page_systems = Vec::new();
    let mut page_index = 0;
    for (system_index, row) in rows.iter().enumerate() {
        let repeat_starts_here = repeat_system_ranges
            .iter()
            .any(|(first, _)| *first == system_index);
        if repeat_starts_here && !page_systems.is_empty() {
            pages.push(PageLayout {
                address: PageAddress { page_index },
                page_index,
                page_number: match config.page_numbering {
                    PageNumbering::None => None,
                    PageNumbering::OneBased => Some(page_index + 1),
                },
                color_policy: config.color_policy,
                crop_mark_policy: config.crop_mark_policy,
                glyph_resources: config.glyph_resources.clone(),
                width_mm,
                height_mm,
                content_width_mm,
                content_height_mm,
                bleed_top_mm: config.bleed_top_mm,
                bleed_right_mm: config.bleed_right_mm,
                bleed_bottom_mm: config.bleed_bottom_mm,
                bleed_left_mm: config.bleed_left_mm,
                span_segments: page_span_segments(&page_systems),
                measure_marks: page_measure_marks(&page_systems),
                systems: std::mem::take(&mut page_systems),
                break_reason: BreakReason::PageCapacity,
            });
            page_index += 1;
        }
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
        let is_last_system = system_index + 1 == rows.len();
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
            measure_spans: measure_spans(score, &row.measure_indices),
            span_segments: span_segments(&layout.spans, &row.measure_indices),
            measure_marks: measure_marks(score, &row.measure_indices),
            top_mm: config.margin_top_mm
                + config.safe_top_mm
                + page_systems.len() as f32 * scaled_system_height_mm,
            height_mm: scaled_system_height_mm,
            break_reason,
        };
        page_systems.push(system);

        let page_capacity = page_capacities
            .get(page_index)
            .copied()
            .unwrap_or(systems_per_page);
        let page_is_full = page_systems.len() >= page_capacity;
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
                page_number: match config.page_numbering {
                    PageNumbering::None => None,
                    PageNumbering::OneBased => Some(page_index + 1),
                },
                color_policy: config.color_policy,
                crop_mark_policy: config.crop_mark_policy,
                glyph_resources: config.glyph_resources.clone(),
                width_mm,
                height_mm,
                content_width_mm,
                content_height_mm,
                bleed_top_mm: config.bleed_top_mm,
                bleed_right_mm: config.bleed_right_mm,
                bleed_bottom_mm: config.bleed_bottom_mm,
                bleed_left_mm: config.bleed_left_mm,
                span_segments: page_span_segments(&page_systems),
                measure_marks: page_measure_marks(&page_systems),
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
            page_number: match config.page_numbering {
                PageNumbering::None => None,
                PageNumbering::OneBased => Some(page_index + 1),
            },
            color_policy: config.color_policy,
            crop_mark_policy: config.crop_mark_policy,
            glyph_resources: config.glyph_resources.clone(),
            width_mm,
            height_mm,
            content_width_mm,
            content_height_mm,
            bleed_top_mm: config.bleed_top_mm,
            bleed_right_mm: config.bleed_right_mm,
            bleed_bottom_mm: config.bleed_bottom_mm,
            bleed_left_mm: config.bleed_left_mm,
            span_segments: page_span_segments(&page_systems),
            measure_marks: page_measure_marks(&page_systems),
            systems: page_systems,
            break_reason: BreakReason::EndOfScore,
        });
    }

    Ok(PrintLayoutResult {
        contract_version: 15,
        pages,
    })
}

#[cfg(test)]
mod tests {
    use super::*;
    use acorde_core::{Clef, Duration, Measure, Note, Part, Pitch, Score, Staff, Step};

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
    fn keep_together_range_is_not_split_across_systems() {
        let score = score_with_measures(5);
        let result = compute_print_layout(
            &score,
            &PrintConfig {
                measures_per_system: 3,
                systems_per_page: Some(8),
                keep_together: vec![KeepTogetherRange {
                    first_measure: 1,
                    last_measure: 2,
                }],
                ..PrintConfig::default()
            },
        )
        .expect("valid keep-together range");
        assert_eq!(result.pages[0].systems[0].measure_indices, vec![0]);
        assert_eq!(result.pages[0].systems[1].measure_indices, vec![1, 2]);
        assert_eq!(result.pages[0].systems[2].measure_indices, vec![3, 4]);
    }

    #[test]
    fn first_system_measure_capacity_is_preserved_in_print_layout() {
        let score = score_with_measures(5);
        let result = compute_print_layout(
            &score,
            &PrintConfig {
                measures_per_system: 3,
                first_system_measures: Some(1),
                systems_per_page: Some(8),
                ..PrintConfig::default()
            },
        )
        .expect("valid first-system capacity");
        assert_eq!(result.pages[0].systems[0].measure_indices, vec![0]);
        assert_eq!(result.pages[0].systems[1].measure_indices, vec![1, 2, 3]);
        assert_eq!(result.pages[0].systems[2].measure_indices, vec![4]);
    }

    #[test]
    fn pickup_policy_isolates_a_partial_first_measure() {
        let mut score = score_with_measures(4);
        score.parts[0].staves[0].measures[0].voices[0] =
            vec![acorde_core::Note::rest(acorde_core::Duration::Quarter)];
        let result = compute_print_layout(
            &score,
            &PrintConfig {
                measures_per_system: 3,
                pickup_policy: PickupPolicy::DetectFirstMeasure,
                systems_per_page: Some(8),
                ..PrintConfig::default()
            },
        )
        .expect("valid pickup policy");
        assert_eq!(result.pages[0].systems[0].measure_indices, vec![0]);
        assert_eq!(result.pages[0].systems[1].measure_indices, vec![1, 2, 3]);
    }

    #[test]
    fn pickup_policy_auto_isolates_a_partial_first_measure_by_default() {
        let mut score = score_with_measures(4);
        score.parts[0].staves[0].measures[0].voices[0] =
            vec![acorde_core::Note::rest(acorde_core::Duration::Quarter)];
        let result = compute_print_layout(
            &score,
            &PrintConfig {
                measures_per_system: 3,
                systems_per_page: Some(8),
                ..PrintConfig::default()
            },
        )
        .expect("valid automatic pickup policy");
        assert_eq!(result.pages[0].systems[0].measure_indices, vec![0]);
        assert_eq!(result.pages[0].systems[1].measure_indices, vec![1, 2, 3]);
    }

    #[test]
    fn system_exposes_physical_span_for_multi_rest_slot() {
        let mut score = score_with_measures(6);
        score.parts[0].staves[0].measures[1].multi_rest_count = Some(3);
        let result = compute_print_layout(&score, &PrintConfig::default())
            .expect("valid multi-rest print layout");
        assert_eq!(
            result.pages[0].systems[0].measure_spans[1],
            MeasureSpan {
                first_measure: 1,
                last_measure: 3,
            }
        );
    }

    #[test]
    fn multirest_width_drives_system_breaking_without_splitting() {
        let mut score = score_with_measures(5);
        score.parts[0].staves[0].measures[1].multi_rest_count = Some(3);
        let result = compute_print_layout(
            &score,
            &PrintConfig {
                measures_per_system: 2,
                pickup_policy: PickupPolicy::Preserve,
                systems_per_page: Some(8),
                ..PrintConfig::default()
            },
        )
        .expect("valid multi-rest pagination");
        assert_eq!(
            result.pages[0]
                .systems
                .iter()
                .map(|system| system.measure_indices.clone())
                .collect::<Vec<_>>(),
            vec![vec![0], vec![1], vec![2, 3], vec![4]]
        );
        assert_eq!(
            result.pages[0].systems[1].measure_spans[0],
            MeasureSpan {
                first_measure: 1,
                last_measure: 3,
            }
        );
    }

    #[test]
    fn system_exposes_cross_system_span_segments() {
        let mut score = score_with_measures(4);
        let mut start = Note::new(Pitch::new(Step::C, 4), Duration::Quarter);
        start.slur_start = true;
        let mut end = Note::new(Pitch::new(Step::D, 4), Duration::Quarter);
        end.slur_end = true;
        score.parts[0].staves[0].measures[0].voices[0] = vec![start];
        score.parts[0].staves[0].measures[3].voices[0] = vec![end];
        let result = compute_print_layout(
            &score,
            &PrintConfig {
                measures_per_system: 2,
                pickup_policy: PickupPolicy::Preserve,
                systems_per_page: Some(8),
                ..PrintConfig::default()
            },
        )
        .expect("valid cross-system span layout");
        assert_eq!(
            result.pages[0].systems[0].span_segments,
            vec![SpanSegment {
                span_index: 0,
                starts_here: true,
                ends_here: false,
            }]
        );
        assert_eq!(
            result.pages[0].systems[1].span_segments,
            vec![SpanSegment {
                span_index: 0,
                starts_here: false,
                ends_here: true,
            }]
        );
    }

    #[test]
    fn system_exposes_repeat_volta_navigation_and_rehearsal_marks() {
        let mut score = score_with_measures(4);
        let measures = &mut score.parts[0].staves[0].measures;
        measures[0].barline_right = Barline::RepeatEnd;
        measures[1].barline_left = Barline::RepeatStart;
        measures[2].volta = Some(acorde_core::VoltaBracket {
            number: 1,
            kind: "begin".to_string(),
        });
        measures[2].navigation = Some("ToCoda".to_string());
        measures[2].rehearsal = Some("B".to_string());
        let result = compute_print_layout(
            &score,
            &PrintConfig {
                measures_per_system: 2,
                systems_per_page: Some(8),
                ..PrintConfig::default()
            },
        )
        .expect("valid measure mark layout");
        assert_eq!(
            result.pages[0].systems[0].measure_marks,
            vec![
                MeasureMark {
                    measure_index: 0,
                    repeat_start: false,
                    repeat_end: true,
                    volta_number: None,
                    volta_kind: None,
                    navigation: None,
                    rehearsal: None,
                },
                MeasureMark {
                    measure_index: 1,
                    repeat_start: true,
                    repeat_end: false,
                    volta_number: None,
                    volta_kind: None,
                    navigation: None,
                    rehearsal: None,
                },
            ]
        );
        assert_eq!(
            result.pages[0].systems[1].measure_marks,
            vec![MeasureMark {
                measure_index: 2,
                repeat_start: false,
                repeat_end: false,
                volta_number: Some(1),
                volta_kind: Some("begin".to_string()),
                navigation: Some("ToCoda".to_string()),
                rehearsal: Some("B".to_string()),
            }]
        );
    }

    #[test]
    fn page_aggregates_cross_system_span_ownership() {
        let mut score = score_with_measures(4);
        let mut start = Note::new(Pitch::new(Step::C, 4), Duration::Quarter);
        start.slur_start = true;
        let mut end = Note::new(Pitch::new(Step::D, 4), Duration::Quarter);
        end.slur_end = true;
        score.parts[0].staves[0].measures[0].voices[0] = vec![start];
        score.parts[0].staves[0].measures[3].voices[0] = vec![end];
        let result = compute_print_layout(
            &score,
            &PrintConfig {
                measures_per_system: 2,
                pickup_policy: PickupPolicy::Preserve,
                systems_per_page: Some(1),
                ..PrintConfig::default()
            },
        )
        .expect("valid page span layout");
        assert_eq!(
            result.pages[0].span_segments,
            vec![PageSpanSegment {
                span_index: 0,
                starts_here: true,
                ends_here: false,
            }]
        );
        assert_eq!(
            result.pages[1].span_segments,
            vec![PageSpanSegment {
                span_index: 0,
                starts_here: false,
                ends_here: true,
            }]
        );
    }

    #[test]
    fn page_artifact_measure_span_borrows_system_spans() {
        let mut score = score_with_measures(4);
        let mut start = Note::new(Pitch::new(Step::C, 4), Duration::Quarter);
        start.slur_start = true;
        let mut end = Note::new(Pitch::new(Step::D, 4), Duration::Quarter);
        end.slur_end = true;
        score.parts[0].staves[0].measures[0].voices[0] = vec![start];
        score.parts[0].staves[0].measures[3].voices[0] = vec![end];
        let result = compute_print_layout(
            &score,
            &PrintConfig {
                measures_per_system: 2,
                pickup_policy: PickupPolicy::Preserve,
                systems_per_page: Some(1),
                ..PrintConfig::default()
            },
        )
        .expect("valid page artifact");
        let first = result
            .page(PageAddress { page_index: 0 })
            .expect("first page");
        assert_eq!(
            first.measure_span(),
            Some(MeasureSpan {
                first_measure: 0,
                last_measure: 1,
            })
        );
        assert!(first.has_span_continuation());
        assert!(result.page(PageAddress { page_index: 99 }).is_none());
    }

    #[test]
    fn notation_policy_keeps_volta_range_in_one_system() {
        let mut score = score_with_measures(4);
        score.parts[0].staves[0].measures[1].volta = Some(acorde_core::VoltaBracket {
            number: 1,
            kind: "begin".to_string(),
        });
        score.parts[0].staves[0].measures[2].volta = Some(acorde_core::VoltaBracket {
            number: 1,
            kind: "end".to_string(),
        });
        let result = compute_print_layout(
            &score,
            &PrintConfig {
                measures_per_system: 2,
                systems_per_page: Some(8),
                notation_break_policy: NotationBreakPolicy::KeepVoltaTogether,
                ..PrintConfig::default()
            },
        )
        .expect("valid volta-preserving layout");
        assert_eq!(result.pages[0].systems[0].measure_indices, vec![0]);
        assert_eq!(result.pages[0].systems[1].measure_indices, vec![1, 2]);
        assert_eq!(result.pages[0].systems[2].measure_indices, vec![3]);
    }

    #[test]
    fn notation_policy_keeps_repeat_section_on_one_page() {
        let mut score = score_with_measures(5);
        score.parts[0].staves[0].measures[2].barline_left = Barline::RepeatStart;
        score.parts[0].staves[0].measures[4].barline_right = Barline::RepeatEnd;
        let result = compute_print_layout(
            &score,
            &PrintConfig {
                measures_per_system: 2,
                systems_per_page: Some(2),
                notation_break_policy: NotationBreakPolicy::KeepRepeatsTogether,
                ..PrintConfig::default()
            },
        )
        .expect("valid repeat-preserving layout");
        assert_eq!(result.pages[0].systems.len(), 1);
        assert_eq!(result.pages[1].systems.len(), 2);
        assert_eq!(
            result.pages[1]
                .systems
                .iter()
                .flat_map(|system| system.measure_indices.iter().copied())
                .collect::<Vec<_>>(),
            vec![2, 3, 4]
        );
    }

    #[test]
    fn balance_policy_avoids_single_system_final_page() {
        let score = score_with_measures(5);
        let result = compute_print_layout(
            &score,
            &PrintConfig {
                measures_per_system: 1,
                systems_per_page: Some(4),
                final_page_policy: FinalPagePolicy::Balance,
                ..PrintConfig::default()
            },
        )
        .expect("valid balanced print config");
        assert_eq!(result.pages.len(), 2);
        assert_eq!(result.pages[0].systems.len(), 3);
        assert_eq!(result.pages[1].systems.len(), 2);
    }

    #[test]
    fn balance_policy_preserves_explicit_page_breaks() {
        let mut score = score_with_measures(5);
        score.parts[0].staves[0].measures[1].page_break = true;
        let result = compute_print_layout(
            &score,
            &PrintConfig {
                measures_per_system: 1,
                systems_per_page: Some(4),
                final_page_policy: FinalPagePolicy::Balance,
                ..PrintConfig::default()
            },
        )
        .expect("valid explicit-break print config");
        assert_eq!(result.pages[0].systems.len(), 2);
        assert_eq!(result.pages[1].systems.len(), 3);
    }

    #[test]
    fn keep_together_rejects_ranges_larger_than_system_capacity() {
        let score = score_with_measures(4);
        let error = compute_print_layout(
            &score,
            &PrintConfig {
                measures_per_system: 2,
                keep_together: vec![KeepTogetherRange {
                    first_measure: 0,
                    last_measure: 2,
                }],
                ..PrintConfig::default()
            },
        )
        .expect_err("range must fit in one system");
        assert_eq!(error, PrintLayoutError::KeepTogetherExceedsSystemCapacity);
    }

    #[test]
    fn keep_together_rejects_explicit_break_inside_range() {
        let mut score = score_with_measures(4);
        score.parts[0].staves[0].measures[1].system_break = true;
        let error = compute_print_layout(
            &score,
            &PrintConfig {
                measures_per_system: 3,
                keep_together: vec![KeepTogetherRange {
                    first_measure: 0,
                    last_measure: 2,
                }],
                ..PrintConfig::default()
            },
        )
        .expect_err("explicit break must win");
        assert_eq!(
            error,
            PrintLayoutError::KeepTogetherConflictsWithExplicitBreak
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
        assert_eq!(result.contract_version, 15);
        assert_eq!(page.bleed_left_mm, 3.0);
        assert_eq!(page.content_width_mm, 210.0 - 14.0 - 14.0 - 8.0 - 6.0);
        assert_eq!(page.content_height_mm, 297.0 - 16.0 - 16.0 - 5.0 - 7.0);
        assert_eq!(page.systems[0].top_mm, 21.0);
    }

    #[test]
    fn scale_changes_system_height_and_page_capacity() {
        let score = score_with_measures(10);
        let result = compute_print_layout(
            &score,
            &PrintConfig {
                scale: 2.0,
                measures_per_system: 1,
                systems_per_page: None,
                ..PrintConfig::default()
            },
        )
        .expect("valid print config");
        assert_eq!(result.pages[0].systems[0].height_mm, 48.0);
        assert_eq!(result.pages[0].systems[1].top_mm, 64.0);
        assert_eq!(result.pages.len(), 2);
    }

    #[test]
    fn rejects_non_positive_scale() {
        let score = score_with_measures(1);
        let error = compute_print_layout(
            &score,
            &PrintConfig {
                scale: 0.0,
                ..PrintConfig::default()
            },
        )
        .expect_err("invalid scale");
        assert_eq!(error, PrintLayoutError::InvalidScale);
    }

    #[test]
    fn page_numbering_is_configurable() {
        let score = score_with_measures(5);
        let numbered = compute_print_layout(
            &score,
            &PrintConfig {
                measures_per_system: 1,
                systems_per_page: Some(2),
                ..PrintConfig::default()
            },
        )
        .expect("valid print config");
        assert_eq!(numbered.pages[0].page_number, Some(1));
        assert_eq!(numbered.pages[1].page_number, Some(2));

        let unnumbered = compute_print_layout(
            &score,
            &PrintConfig {
                page_numbering: PageNumbering::None,
                measures_per_system: 1,
                systems_per_page: Some(2),
                ..PrintConfig::default()
            },
        )
        .expect("valid print config");
        assert!(
            unnumbered
                .pages
                .iter()
                .all(|page| page.page_number.is_none())
        );
    }

    #[test]
    fn print_color_and_crop_policies_are_exposed_per_page() {
        let score = score_with_measures(1);
        let result = compute_print_layout(
            &score,
            &PrintConfig {
                color_policy: PrintColorPolicy::Preserve,
                crop_mark_policy: CropMarkPolicy::BleedEdges,
                ..PrintConfig::default()
            },
        )
        .expect("valid print config");
        let page = &result.pages[0];
        assert_eq!(result.contract_version, 15);
        assert_eq!(page.color_policy, PrintColorPolicy::Preserve);
        assert_eq!(page.crop_mark_policy, CropMarkPolicy::BleedEdges);
    }

    #[test]
    fn glyph_resource_policy_is_exposed_per_page() {
        let score = score_with_measures(1);
        let result = compute_print_layout(
            &score,
            &PrintConfig {
                glyph_resources: GlyphResourcePolicy::HostProvided("music-font-v1".into()),
                ..PrintConfig::default()
            },
        )
        .expect("valid print config");
        assert_eq!(
            result.pages[0].glyph_resources,
            GlyphResourcePolicy::HostProvided("music-font-v1".into())
        );
    }
}
