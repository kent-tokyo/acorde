use crate::{LayoutConfig, SpanMark, compute_layout};
use acorde_core::{Barline, PartGroupSymbol, Score};
use serde::{Deserialize, Serialize};

/// Font-independent metrics for one print glyph, expressed in millimetres.
///
/// Hosts may resolve a resource key to a real font, but layout can use these metrics without
/// loading fonts or depending on an operating system. `advance_mm` is the cursor advance;
/// the bounding box is relative to the glyph origin and is used for collision checks.
#[derive(Debug, Clone, Copy, Serialize, Deserialize, PartialEq)]
pub struct GlyphMetrics {
    pub advance_mm: f32,
    pub left_mm: f32,
    pub top_mm: f32,
    pub width_mm: f32,
    pub height_mm: f32,
}

/// A positioned print glyph with a deterministic collision priority.
#[derive(Debug, Clone, Serialize, Deserialize, PartialEq)]
pub struct GlyphPlacement {
    pub resource_key: String,
    pub metrics: GlyphMetrics,
    pub x_mm: f32,
    pub y_mm: f32,
    /// Higher-priority glyphs keep their requested position when possible.
    pub priority: u8,
}

/// The content bounds of a validated glyph placement collection, in millimetres.
#[derive(Debug, Clone, Copy, Serialize, Deserialize, PartialEq)]
pub struct GlyphExtents {
    pub left_mm: f32,
    pub top_mm: f32,
    pub right_mm: f32,
    pub bottom_mm: f32,
}

impl GlyphExtents {
    /// Return the horizontal content span in millimetres.
    pub fn width_mm(self) -> f32 {
        self.right_mm - self.left_mm
    }

    /// Return the vertical content span in millimetres.
    pub fn height_mm(self) -> f32 {
        self.bottom_mm - self.top_mm
    }
}

/// Validation failures for host-provided print glyph geometry.
#[derive(Debug, Clone, Copy, PartialEq, Eq, thiserror::Error)]
pub enum GlyphPlacementError {
    #[error("glyph placement {index} contains non-finite geometry")]
    NonFinite { index: usize },
    #[error("glyph placement {index} has a negative bounding-box extent")]
    NegativeExtent { index: usize },
    #[error("glyph spacing is non-finite or overflows")]
    NonFiniteSpacing,
    #[error("glyph placement {index} has an empty resource key")]
    EmptyResourceKey { index: usize },
    #[error("glyph placement {index} has a negative advance")]
    NegativeAdvance { index: usize },
}

/// Validate font-independent glyph geometry before collision resolution.
pub fn validate_glyph_placements(placements: &[GlyphPlacement]) -> Result<(), GlyphPlacementError> {
    for (index, placement) in placements.iter().enumerate() {
        if placement.resource_key.trim().is_empty() {
            return Err(GlyphPlacementError::EmptyResourceKey { index });
        }
        let values = [
            placement.metrics.advance_mm,
            placement.metrics.left_mm,
            placement.metrics.top_mm,
            placement.metrics.width_mm,
            placement.metrics.height_mm,
            placement.x_mm,
            placement.y_mm,
        ];
        if values.iter().any(|value| !value.is_finite()) {
            return Err(GlyphPlacementError::NonFinite { index });
        }
        if placement.metrics.width_mm < 0.0 || placement.metrics.height_mm < 0.0 {
            return Err(GlyphPlacementError::NegativeExtent { index });
        }
        if placement.metrics.advance_mm < 0.0 {
            return Err(GlyphPlacementError::NegativeAdvance { index });
        }
    }
    Ok(())
}

/// Compute content-aware bounds for glyph placements without loading a font resource.
pub fn glyph_extents(
    placements: &[GlyphPlacement],
) -> Result<Option<GlyphExtents>, GlyphPlacementError> {
    validate_glyph_placements(placements)?;
    let Some(first) = placements.first() else {
        return Ok(None);
    };
    let (first_left, first_right) = horizontal_bounds(first);
    let (first_top, first_bottom) = vertical_bounds(first);
    if [first_left, first_right, first_top, first_bottom]
        .iter()
        .any(|value| !value.is_finite())
    {
        return Err(GlyphPlacementError::NonFinite { index: 0 });
    }
    let mut extents = GlyphExtents {
        left_mm: first_left,
        top_mm: first_top,
        right_mm: first_right,
        bottom_mm: first_bottom,
    };
    for (index, placement) in placements.iter().enumerate().skip(1) {
        let (left, right) = horizontal_bounds(placement);
        let (top, bottom) = vertical_bounds(placement);
        if [left, right, top, bottom]
            .iter()
            .any(|value| !value.is_finite())
        {
            return Err(GlyphPlacementError::NonFinite { index });
        }
        extents.left_mm = extents.left_mm.min(left);
        extents.top_mm = extents.top_mm.min(top);
        extents.right_mm = extents.right_mm.max(right);
        extents.bottom_mm = extents.bottom_mm.max(bottom);
    }
    Ok(Some(extents))
}

/// Distribute additional horizontal space evenly between glyph placements.
pub fn distribute_glyph_spacing(
    placements: &mut [GlyphPlacement],
    extra_mm: f32,
) -> Result<usize, GlyphPlacementError> {
    validate_glyph_placements(placements)?;
    if !extra_mm.is_finite() {
        return Err(GlyphPlacementError::NonFiniteSpacing);
    }
    if extra_mm <= 0.0 || placements.len() < 2 {
        return Ok(0);
    }
    let mut order: Vec<usize> = (0..placements.len()).collect();
    order.sort_by(|&left, &right| {
        placements[left]
            .x_mm
            .total_cmp(&placements[right].x_mm)
            .then(left.cmp(&right))
    });
    let denominator = (order.len() - 1) as f32;
    let mut shifts = Vec::with_capacity(order.len().saturating_sub(1));
    for (rank, &index) in order.iter().enumerate().skip(1) {
        let shift = extra_mm * rank as f32 / denominator;
        if !shift.is_finite() || !(placements[index].x_mm + shift).is_finite() {
            return Err(GlyphPlacementError::NonFiniteSpacing);
        }
        shifts.push((index, shift));
    }
    let mut moved = 0;
    for (index, shift) in shifts {
        placements[index].x_mm += shift;
        if shift > f32::EPSILON {
            moved += 1;
        }
    }
    Ok(moved)
}

/// Move lower-priority glyphs vertically until their bounding boxes no longer overlap.
///
/// This is intentionally a small, backend-neutral primitive: it does not choose fonts or
/// draw anything. The stable input order breaks ties, and the return value reports how many
/// placements were moved so a host can expose a preflight diagnostic.
pub fn resolve_glyph_collisions(placements: &mut [GlyphPlacement], gap_mm: f32) -> usize {
    let gap_mm = if gap_mm.is_finite() {
        gap_mm.max(0.0)
    } else {
        0.0
    };
    let mut order: Vec<usize> = (0..placements.len()).collect();
    order.sort_by_key(|&index| (std::cmp::Reverse(placements[index].priority), index));
    let mut moved = 0;
    for position in 0..order.len() {
        let index = order[position];
        let (left, right) = horizontal_bounds(&placements[index]);
        let mut next_y = placements[index].y_mm;
        for &previous in &order[..position] {
            let (previous_left, previous_right) = horizontal_bounds(&placements[previous]);
            if right <= previous_left || previous_right <= left {
                continue;
            }
            let (previous_top, previous_bottom) = vertical_bounds(&placements[previous]);
            let current_top = next_y + placements[index].metrics.top_mm;
            let current_bottom = current_top + placements[index].metrics.height_mm;
            if current_bottom <= previous_top || previous_bottom <= current_top {
                continue;
            }
            if current_top < previous_bottom + gap_mm {
                next_y = previous_bottom + gap_mm - placements[index].metrics.top_mm;
            }
        }
        if (next_y - placements[index].y_mm).abs() > f32::EPSILON {
            placements[index].y_mm = next_y;
            moved += 1;
        }
    }
    moved
}

/// Validate glyph geometry, then apply deterministic vertical collision resolution.
pub fn resolve_glyph_collisions_checked(
    placements: &mut [GlyphPlacement],
    gap_mm: f32,
) -> Result<usize, GlyphPlacementError> {
    validate_glyph_placements(placements)?;
    if !gap_mm.is_finite() {
        return Err(GlyphPlacementError::NonFiniteSpacing);
    }
    let mut candidate = placements.to_vec();
    let moved = resolve_glyph_collisions(&mut candidate, gap_mm);
    glyph_extents(&candidate)?;
    placements.clone_from_slice(&candidate);
    Ok(moved)
}

/// Move lower-priority glyphs horizontally until their bounding boxes no longer overlap.
///
/// Higher-priority placements retain their requested coordinates. When several placements
/// overlap, stable input order breaks ties and the return value reports how many placements moved.
pub fn resolve_glyph_horizontal_collisions(
    placements: &mut [GlyphPlacement],
    gap_mm: f32,
) -> usize {
    let gap_mm = if gap_mm.is_finite() {
        gap_mm.max(0.0)
    } else {
        0.0
    };
    let mut order: Vec<usize> = (0..placements.len()).collect();
    order.sort_by_key(|&index| (std::cmp::Reverse(placements[index].priority), index));
    let mut moved = 0;
    for position in 0..order.len() {
        let index = order[position];
        let original_x = placements[index].x_mm;
        let mut next_x = original_x;
        for &previous in &order[..position] {
            let current = GlyphPlacement {
                x_mm: next_x,
                ..placements[index].clone()
            };
            let (left, right) = horizontal_bounds(&current);
            let (previous_left, previous_right) = horizontal_bounds(&placements[previous]);
            let (top, bottom) = vertical_bounds(&current);
            let (previous_top, previous_bottom) = vertical_bounds(&placements[previous]);
            if right <= previous_left
                || previous_right <= left
                || bottom <= previous_top
                || previous_bottom <= top
            {
                continue;
            }
            next_x = previous_right + gap_mm - placements[index].metrics.left_mm;
        }
        if (next_x - original_x).abs() > f32::EPSILON {
            placements[index].x_mm = next_x;
            moved += 1;
        }
    }
    moved
}

/// Validate glyph geometry, then apply deterministic horizontal collision resolution.
pub fn resolve_glyph_horizontal_collisions_checked(
    placements: &mut [GlyphPlacement],
    gap_mm: f32,
) -> Result<usize, GlyphPlacementError> {
    validate_glyph_placements(placements)?;
    if !gap_mm.is_finite() {
        return Err(GlyphPlacementError::NonFiniteSpacing);
    }
    let mut candidate = placements.to_vec();
    let moved = resolve_glyph_horizontal_collisions(&mut candidate, gap_mm);
    glyph_extents(&candidate)?;
    placements.clone_from_slice(&candidate);
    Ok(moved)
}

fn horizontal_bounds(placement: &GlyphPlacement) -> (f32, f32) {
    (
        placement.x_mm + placement.metrics.left_mm,
        placement.x_mm + placement.metrics.left_mm + placement.metrics.width_mm,
    )
}

fn vertical_bottom(placement: &GlyphPlacement) -> f32 {
    placement.y_mm + placement.metrics.top_mm + placement.metrics.height_mm
}

fn vertical_bounds(placement: &GlyphPlacement) -> (f32, f32) {
    (
        placement.y_mm + placement.metrics.top_mm,
        vertical_bottom(placement),
    )
}

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

/// Selects the score scope used by print pagination and notation metadata.
#[derive(Debug, Clone, Copy, Serialize, Deserialize, PartialEq, Eq, Default)]
pub enum PartLayoutPolicy {
    /// Keep all parts in the score-level layout contract.
    #[default]
    FullScore,
    /// Produce an extracted-part layout for the zero-based part index.
    ExtractedPart { part_index: usize },
}

/// Host-neutral publication metadata policy carried into each page artifact.
#[derive(Debug, Clone, Serialize, Deserialize, PartialEq)]
#[serde(default)]
pub struct PublicationConfig {
    /// Insert a metadata-only title page before the music pages.
    pub title_page: bool,
    /// Optional running title shown by a host on non-title pages.
    pub running_title: Option<String>,
    pub show_part_names: bool,
    pub show_measure_numbers: bool,
    /// Optional text placed in the logical page header.
    pub header_text: Option<String>,
    /// Optional text placed in the logical page footer.
    pub footer_text: Option<String>,
    /// Add the logical page number as a footer text block when numbering is enabled.
    pub page_number_in_footer: bool,
    pub header_alignment: PublicationTextAlignment,
    pub footer_alignment: PublicationTextAlignment,
    pub title_alignment: PublicationTextAlignment,
    /// Logical line-box height for publication text blocks, in millimetres.
    pub line_height_mm: f32,
}

impl Default for PublicationConfig {
    fn default() -> Self {
        Self {
            title_page: false,
            running_title: None,
            show_part_names: true,
            show_measure_numbers: true,
            header_text: None,
            footer_text: None,
            page_number_in_footer: false,
            header_alignment: PublicationTextAlignment::Left,
            footer_alignment: PublicationTextAlignment::Left,
            title_alignment: PublicationTextAlignment::Center,
            line_height_mm: 4.0,
        }
    }
}

/// Semantic role for a host-rendered publication text block.
#[derive(Debug, Clone, Copy, Serialize, Deserialize, PartialEq, Eq)]
pub enum PublicationTextRole {
    Header,
    Footer,
    Title,
    Subtitle,
    Credit,
    Copyright,
}

/// Horizontal alignment within a publication text block's physical width.
#[derive(Debug, Clone, Copy, Serialize, Deserialize, PartialEq, Eq, Default)]
pub enum PublicationTextAlignment {
    #[default]
    Left,
    Center,
    Right,
}

/// A page text block with deterministic physical placement.
#[derive(Debug, Clone, Serialize, Deserialize, PartialEq)]
pub struct PublicationTextBlock {
    pub role: PublicationTextRole,
    pub text: String,
    pub x_mm: f32,
    pub y_mm: f32,
    pub width_mm: f32,
    pub height_mm: f32,
    #[serde(default)]
    pub alignment: PublicationTextAlignment,
}

/// A part label suitable for a score header or extracted-part host renderer.
#[derive(Debug, Clone, Serialize, Deserialize, PartialEq, Eq)]
pub struct PartLabel {
    pub part_index: usize,
    pub name: String,
    pub short_name: String,
}

/// A score-level part connector for a publication host.
#[derive(Debug, Clone, Serialize, Deserialize, PartialEq)]
pub struct PartGroupMark {
    pub first_part: usize,
    pub last_part: usize,
    pub symbol: PartGroupSymbol,
    pub barlines_connect: bool,
}

/// Publication information for one logical page.
#[derive(Debug, Clone, Serialize, Deserialize, PartialEq, Default)]
#[serde(default)]
pub struct PagePublication {
    #[serde(default)]
    pub is_title_page: bool,
    pub title: String,
    pub movement_title: String,
    pub composer: String,
    pub lyricist: String,
    pub copyright: String,
    pub running_title: Option<String>,
    pub part_labels: Vec<PartLabel>,
    #[serde(default)]
    pub part_groups: Vec<PartGroupMark>,
    pub measure_numbers: Vec<u32>,
    #[serde(default)]
    pub text_blocks: Vec<PublicationTextBlock>,
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
    #[serde(default)]
    pub publication: PublicationConfig,
    #[serde(default)]
    pub part_layout: PartLayoutPolicy,
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
            publication: PublicationConfig::default(),
            part_layout: PartLayoutPolicy::FullScore,
            keep_together: Vec::new(),
        }
    }
}

/// Version of the built-in host-neutral print preset data.
pub const PRINT_PRESET_SCHEMA_VERSION: u16 = 1;
/// Version of the serialized host-neutral print layout contract.
pub const PRINT_LAYOUT_CONTRACT_VERSION: u16 = 25;

/// Reproducible starting configurations for common publication workflows.
#[derive(Debug, Clone, Copy, Serialize, Deserialize, PartialEq, Eq)]
pub enum PrintPreset {
    A4Score,
    LetterScore,
    A4Part { part_index: usize },
    LetterPart { part_index: usize },
}

impl PrintPreset {
    /// Build a configuration without consulting host defaults or installed resources.
    pub fn config(self) -> PrintConfig {
        let (paper_size, part_layout) = match self {
            Self::A4Score => (PaperSize::A4, PartLayoutPolicy::FullScore),
            Self::LetterScore => (PaperSize::Letter, PartLayoutPolicy::FullScore),
            Self::A4Part { part_index } => (
                PaperSize::A4,
                PartLayoutPolicy::ExtractedPart { part_index },
            ),
            Self::LetterPart { part_index } => (
                PaperSize::Letter,
                PartLayoutPolicy::ExtractedPart { part_index },
            ),
        };
        PrintConfig {
            paper_size,
            part_layout,
            ..PrintConfig::default()
        }
    }

    /// Build this preset with the publication title-page policy explicitly selected.
    pub fn config_with_title_page(self, title_page: bool) -> PrintConfig {
        let mut config = self.config();
        config.publication.title_page = title_page;
        config
    }

    /// Return the schema version for this preset data.
    pub const fn schema_version(self) -> u16 {
        PRINT_PRESET_SCHEMA_VERSION
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
    TitlePage,
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
    #[serde(default)]
    pub publication: PagePublication,
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

/// A host-neutral page export descriptor.
///
/// This is intentionally geometry and metadata only. Hosts may turn each descriptor into
/// SVG, PDF, or another artifact without making this crate depend on a file format or UI API.
#[derive(Debug, Clone, Serialize, Deserialize, PartialEq)]
pub struct PageArtifact {
    pub address: PageAddress,
    pub page_index: usize,
    pub page_number: Option<usize>,
    pub width_mm: f32,
    pub height_mm: f32,
    pub content_width_mm: f32,
    pub content_height_mm: f32,
    pub measure_span: Option<MeasureSpan>,
    pub diagnostics: Vec<PageArtifactDiagnostic>,
    pub layout: PageLayout,
}

/// Typed, host-neutral diagnostics attached to a page export descriptor.
#[derive(Debug, Clone, Copy, Serialize, Deserialize, PartialEq, Eq)]
pub enum PageArtifactDiagnostic {
    /// The page uses a host-owned glyph resource and must be resolved by the exporter.
    GlyphResourceRequired,
    /// Host-provided glyph extents exceed the page content area on one or more sides.
    GlyphOverflow {
        left: bool,
        top: bool,
        right: bool,
        bottom: bool,
    },
    /// A span continues across a page boundary and needs a continuation mark in the host.
    SpanContinuation {
        span_index: usize,
        starts_here: bool,
        ends_here: bool,
    },
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

    /// Build deterministic page diagnostics from optional host-computed glyph extents.
    ///
    /// Extents are expressed relative to the page content origin. This keeps overflow
    /// detection independent of fonts and renderers while allowing a host to report a
    /// clipping risk before producing an SVG, PDF, or print artifact.
    pub fn artifact_diagnostics(
        &self,
        glyph_extents: Option<GlyphExtents>,
    ) -> Vec<PageArtifactDiagnostic> {
        let mut diagnostics = Vec::new();
        if matches!(self.glyph_resources, GlyphResourcePolicy::HostProvided(_)) {
            diagnostics.push(PageArtifactDiagnostic::GlyphResourceRequired);
        }
        if let Some(extents) = glyph_extents {
            let overflow = PageArtifactDiagnostic::GlyphOverflow {
                left: extents.left_mm < 0.0,
                top: extents.top_mm < 0.0,
                right: extents.right_mm > self.content_width_mm,
                bottom: extents.bottom_mm > self.content_height_mm,
            };
            if let PageArtifactDiagnostic::GlyphOverflow {
                left,
                top,
                right,
                bottom,
            } = overflow
                && (left || top || right || bottom)
            {
                diagnostics.push(overflow);
            }
        }
        diagnostics.extend(
            self.span_segments
                .iter()
                .filter(|segment| !segment.starts_here || !segment.ends_here)
                .map(|segment| PageArtifactDiagnostic::SpanContinuation {
                    span_index: segment.span_index,
                    starts_here: segment.starts_here,
                    ends_here: segment.ends_here,
                }),
        );
        diagnostics
    }
}

/// Deterministic page/system geometry for a score.
#[derive(Debug, Clone, Serialize, Deserialize, PartialEq)]
pub struct PrintLayoutResult {
    pub contract_version: u16,
    pub pages: Vec<PageLayout>,
}

impl PrintLayoutResult {
    /// Validate page and system addresses before consuming a serialized layout.
    ///
    /// Layouts produced by [`compute_print_layout`] satisfy this contract. The explicit
    /// validation is useful for hosts that persist or transport `PrintLayoutResult` values.
    pub fn validate(&self) -> Result<(), PrintLayoutError> {
        if self.contract_version != PRINT_LAYOUT_CONTRACT_VERSION {
            return Err(PrintLayoutError::UnsupportedContractVersion {
                found: self.contract_version,
            });
        }
        let mut expected_system_index = 0;
        let mut previous_page_number = None;
        let mut numbered_pages = None;
        for (page_index, page) in self.pages.iter().enumerate() {
            if page.page_index != page_index || page.address.page_index != page_index {
                return Err(PrintLayoutError::InvalidPageAddress { page_index });
            }
            if !page.width_mm.is_finite()
                || !page.height_mm.is_finite()
                || page.width_mm <= 0.0
                || page.height_mm <= 0.0
                || !page.content_width_mm.is_finite()
                || !page.content_height_mm.is_finite()
                || page.content_width_mm <= 0.0
                || page.content_height_mm <= 0.0
                || page.content_width_mm > page.width_mm
                || page.content_height_mm > page.height_mm
                || !page.bleed_top_mm.is_finite()
                || !page.bleed_right_mm.is_finite()
                || !page.bleed_bottom_mm.is_finite()
                || !page.bleed_left_mm.is_finite()
                || page.bleed_top_mm < 0.0
                || page.bleed_right_mm < 0.0
                || page.bleed_bottom_mm < 0.0
                || page.bleed_left_mm < 0.0
            {
                return Err(PrintLayoutError::InvalidPageGeometry { page_index });
            }
            let is_title_break = page.break_reason == BreakReason::TitlePage;
            if is_title_break != page.publication.is_title_page
                || (is_title_break && (page_index != 0 || !page.systems.is_empty()))
            {
                return Err(PrintLayoutError::InvalidTitlePage { page_index });
            }
            match page.page_number {
                Some(page_number)
                    if page_number == 0
                        || numbered_pages == Some(false)
                        || page_index.checked_add(1) != Some(page_number)
                        || previous_page_number.is_some_and(|previous| page_number <= previous) =>
                {
                    return Err(PrintLayoutError::InvalidPageNumber { page_index });
                }
                Some(page_number) => {
                    numbered_pages = Some(true);
                    previous_page_number = Some(page_number);
                }
                None if numbered_pages == Some(true) => {
                    return Err(PrintLayoutError::InvalidPageNumber { page_index });
                }
                None => numbered_pages = Some(false),
            }
            for (index_on_page, system) in page.systems.iter().enumerate() {
                if system.page_index != page_index
                    || system.address.page_index != page_index
                    || system.address.index_on_page != index_on_page
                    || system.system_index != expected_system_index
                    || system.address.system_index != expected_system_index
                {
                    return Err(PrintLayoutError::InvalidSystemAddress {
                        page_index,
                        index_on_page,
                        system_index: expected_system_index,
                    });
                }
                if !system.top_mm.is_finite()
                    || system.top_mm < 0.0
                    || !system.height_mm.is_finite()
                    || system.height_mm <= 0.0
                {
                    return Err(PrintLayoutError::InvalidSystemGeometry {
                        page_index,
                        index_on_page,
                    });
                }
                expected_system_index += 1;
            }
        }
        Ok(())
    }

    /// Retrieve one page artifact by its stable address without recomputing layout.
    pub fn page(&self, address: PageAddress) -> Option<&PageLayout> {
        self.pages
            .get(address.page_index)
            .filter(|page| page.address == address)
    }

    /// Export validated page descriptors for host renderers and archival backends.
    ///
    /// The returned vector preserves physical page order. No filesystem, PDF backend, font
    /// loader, or renderer-specific object is involved; hosts can serialize or render each
    /// descriptor independently. Validation happens before any descriptor is returned.
    pub fn export_page_artifacts(&self) -> Result<Vec<PageArtifact>, PrintLayoutError> {
        self.validate()?;
        Ok(self
            .pages
            .iter()
            .map(|page| PageArtifact {
                address: page.address,
                page_index: page.page_index,
                page_number: page.page_number,
                width_mm: page.width_mm,
                height_mm: page.height_mm,
                content_width_mm: page.content_width_mm,
                content_height_mm: page.content_height_mm,
                measure_span: page.measure_span(),
                diagnostics: page.artifact_diagnostics(None),
                layout: page.clone(),
            })
            .collect())
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
    #[error("extracted part index is outside the score")]
    InvalidPartIndex,
    #[error("publication line height must be finite and greater than zero")]
    InvalidPublicationLineHeight,
    #[error("host-provided glyph resource key must not be empty")]
    InvalidGlyphResourceKey,
    #[error("unsupported print layout contract version {found}")]
    UnsupportedContractVersion { found: u16 },
    #[error("page {page_index} has an inconsistent stable address")]
    InvalidPageAddress { page_index: usize },
    #[error("page {page_index} has an invalid or non-monotonic page number")]
    InvalidPageNumber { page_index: usize },
    #[error("page {page_index} has inconsistent title-page metadata")]
    InvalidTitlePage { page_index: usize },
    #[error(
        "system {system_index} at page {page_index}, position {index_on_page} has an inconsistent stable address"
    )]
    InvalidSystemAddress {
        page_index: usize,
        index_on_page: usize,
        system_index: usize,
    },
    #[error("page {page_index} has invalid physical geometry")]
    InvalidPageGeometry { page_index: usize },
    #[error("system at page {page_index}, position {index_on_page} has invalid physical geometry")]
    InvalidSystemGeometry {
        page_index: usize,
        index_on_page: usize,
    },
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

fn page_publication(
    score: &Score,
    measure_score: &Score,
    config: &PrintConfig,
    systems: &[SystemLayout],
    is_title_page: bool,
    page_number: Option<usize>,
) -> PagePublication {
    let metadata = &score.metadata;
    let part_labels = if config.publication.show_part_names {
        let parts = match config.part_layout {
            PartLayoutPolicy::FullScore => score.parts.iter().enumerate().collect::<Vec<_>>(),
            PartLayoutPolicy::ExtractedPart { part_index } => score
                .parts
                .get(part_index)
                .into_iter()
                .enumerate()
                .map(|(index, part)| (part_index + index, part))
                .collect(),
        };
        parts
            .into_iter()
            .map(|(part_index, part)| PartLabel {
                part_index,
                name: part.name.clone(),
                short_name: part.short_name.clone(),
            })
            .collect()
    } else {
        Vec::new()
    };
    let part_groups = if matches!(config.part_layout, PartLayoutPolicy::FullScore) {
        score
            .part_groups
            .iter()
            .map(|group| PartGroupMark {
                first_part: group.first_part,
                last_part: group.last_part,
                symbol: group.symbol.clone(),
                barlines_connect: group.barlines_connect,
            })
            .collect()
    } else {
        Vec::new()
    };
    let measure_numbers = if config.publication.show_measure_numbers {
        let staff = measure_score
            .parts
            .first()
            .and_then(|part| part.staves.first());
        systems
            .iter()
            .flat_map(|system| system.measure_indices.iter().copied())
            .filter_map(|index| staff.and_then(|staff| staff.measures.get(index)))
            .map(|measure| measure.number)
            .collect()
    } else {
        Vec::new()
    };
    let (paper_width, paper_height) = config.paper_size.dimensions_mm();
    let (page_width, page_height) = if matches!(config.orientation, PageOrientation::Landscape) {
        (paper_height, paper_width)
    } else {
        (paper_width, paper_height)
    };
    let mut text_blocks = Vec::new();
    if !is_title_page
        && let Some(text) = config
            .publication
            .header_text
            .as_ref()
            .or(config.publication.running_title.as_ref())
    {
        text_blocks.push(PublicationTextBlock {
            role: PublicationTextRole::Header,
            text: text.clone(),
            x_mm: config.margin_left_mm + config.safe_left_mm,
            y_mm: config.margin_top_mm,
            width_mm: page_width
                - config.margin_left_mm
                - config.margin_right_mm
                - config.safe_left_mm
                - config.safe_right_mm,
            height_mm: config.publication.line_height_mm,
            alignment: config.publication.header_alignment,
        });
    }
    if let Some(text) = config.publication.footer_text.as_ref() {
        text_blocks.push(PublicationTextBlock {
            role: PublicationTextRole::Footer,
            text: text.clone(),
            x_mm: config.margin_left_mm + config.safe_left_mm,
            y_mm: page_height - config.margin_bottom_mm,
            width_mm: page_width
                - config.margin_left_mm
                - config.margin_right_mm
                - config.safe_left_mm
                - config.safe_right_mm,
            height_mm: config.publication.line_height_mm,
            alignment: config.publication.footer_alignment,
        });
    }
    if config.publication.page_number_in_footer {
        if let Some(page_number) = page_number {
            let (paper_width, paper_height) = config.paper_size.dimensions_mm();
            let (page_width, page_height) =
                if matches!(config.orientation, PageOrientation::Landscape) {
                    (paper_height, paper_width)
                } else {
                    (paper_width, paper_height)
                };
            text_blocks.push(PublicationTextBlock {
                role: PublicationTextRole::Footer,
                text: page_number.to_string(),
                x_mm: config.margin_left_mm + config.safe_left_mm,
                y_mm: page_height - config.margin_bottom_mm,
                width_mm: page_width
                    - config.margin_left_mm
                    - config.margin_right_mm
                    - config.safe_left_mm
                    - config.safe_right_mm,
                height_mm: config.publication.line_height_mm,
                alignment: config.publication.footer_alignment,
            });
        }
    }
    if is_title_page {
        let content_height = page_height
            - config.margin_top_mm
            - config.margin_bottom_mm
            - config.safe_top_mm
            - config.safe_bottom_mm;
        let title_x = config.margin_left_mm + config.safe_left_mm;
        let title_width = page_width
            - config.margin_left_mm
            - config.margin_right_mm
            - config.safe_left_mm
            - config.safe_right_mm;
        let title_y = config.margin_top_mm + config.safe_top_mm + content_height * 0.30;
        if !metadata.title.trim().is_empty() {
            text_blocks.push(PublicationTextBlock {
                role: PublicationTextRole::Title,
                text: metadata.title.clone(),
                x_mm: title_x,
                y_mm: title_y,
                width_mm: title_width,
                height_mm: config.publication.line_height_mm,
                alignment: config.publication.title_alignment,
            });
        }
        if !metadata.movement_title.trim().is_empty() {
            text_blocks.push(PublicationTextBlock {
                role: PublicationTextRole::Subtitle,
                text: metadata.movement_title.clone(),
                x_mm: title_x,
                y_mm: title_y + config.publication.line_height_mm * 2.5,
                width_mm: title_width,
                height_mm: config.publication.line_height_mm,
                alignment: config.publication.title_alignment,
            });
        }
        let credit = match (metadata.composer.trim(), metadata.lyricist.trim()) {
            (composer, lyricist) if !composer.is_empty() && !lyricist.is_empty() => {
                format!("{composer} / {lyricist}")
            }
            (composer, _lyricist) if !composer.is_empty() => composer.to_string(),
            (_, lyricist) => lyricist.to_string(),
        };
        if !credit.is_empty() {
            text_blocks.push(PublicationTextBlock {
                role: PublicationTextRole::Credit,
                text: credit,
                x_mm: title_x,
                y_mm: title_y + config.publication.line_height_mm * 5.0,
                width_mm: title_width,
                height_mm: config.publication.line_height_mm,
                alignment: config.publication.title_alignment,
            });
        }
        if !metadata.copyright.trim().is_empty() {
            text_blocks.push(PublicationTextBlock {
                role: PublicationTextRole::Copyright,
                text: metadata.copyright.clone(),
                x_mm: title_x,
                y_mm: page_height - config.margin_bottom_mm,
                width_mm: title_width,
                height_mm: config.publication.line_height_mm,
                alignment: config.publication.title_alignment,
            });
        }
    }
    PagePublication {
        is_title_page,
        title: metadata.title.clone(),
        movement_title: metadata.movement_title.clone(),
        composer: metadata.composer.clone(),
        lyricist: metadata.lyricist.clone(),
        copyright: metadata.copyright.clone(),
        running_title: config.publication.running_title.clone(),
        part_labels,
        part_groups,
        measure_numbers,
        text_blocks,
    }
}

fn score_for_part_layout(
    score: &Score,
    policy: PartLayoutPolicy,
) -> Result<Score, PrintLayoutError> {
    let PartLayoutPolicy::ExtractedPart { part_index } = policy else {
        return Ok(score.clone());
    };
    let Some(part) = score.parts.get(part_index) else {
        return Err(PrintLayoutError::InvalidPartIndex);
    };
    let mut selected = score.clone();
    selected.parts = vec![part.clone()];
    selected.part_groups.clear();
    Ok(selected)
}

/// Compute physical page and system placement without rendering or host integration.
pub fn compute_print_layout(
    score: &Score,
    config: &PrintConfig,
) -> Result<PrintLayoutResult, PrintLayoutError> {
    let layout_score = score_for_part_layout(score, config.part_layout)?;
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
    if !config.publication.line_height_mm.is_finite() || config.publication.line_height_mm <= 0.0 {
        return Err(PrintLayoutError::InvalidPublicationLineHeight);
    }
    if matches!(&config.glyph_resources, GlyphResourcePolicy::HostProvided(key) if key.trim().is_empty())
    {
        return Err(PrintLayoutError::InvalidGlyphResourceKey);
    }
    if !config.publication.line_height_mm.is_finite() || config.publication.line_height_mm <= 0.0 {
        return Err(PrintLayoutError::InvalidPublicationLineHeight);
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
        &layout_score,
        &LayoutConfig {
            measures_per_row: config.measures_per_system.max(1),
            first_row_measures: config.first_system_measures.or_else(|| {
                (matches!(
                    config.pickup_policy,
                    PickupPolicy::Auto | PickupPolicy::DetectFirstMeasure
                ) && has_first_measure_pickup(&layout_score))
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
        keep_together.extend(volta_ranges(&layout_score));
    }
    let rows = apply_keep_together(
        &layout_score,
        layout.rows,
        &keep_together,
        config.measures_per_system.max(1),
    )?;

    let has_explicit_page_break = rows.iter().any(|row| {
        row.measure_indices.last().is_some_and(|&measure_index| {
            layout_score
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
        repeat_system_ranges(&layout_score, &rows)
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
                publication: page_publication(
                    score,
                    &layout_score,
                    config,
                    &page_systems,
                    false,
                    match config.page_numbering {
                        PageNumbering::None => None,
                        PageNumbering::OneBased => Some(page_index + 1),
                    },
                ),
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
            layout_score
                .parts
                .iter()
                .flat_map(|part| part.staves.iter())
                .filter_map(|staff| staff.measures.get(measure_index))
                .any(|measure| measure.page_break)
        });
        let explicit_system_break = row.measure_indices.last().is_some_and(|&measure_index| {
            layout_score
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
            measure_spans: measure_spans(&layout_score, &row.measure_indices),
            span_segments: span_segments(&layout.spans, &row.measure_indices),
            measure_marks: measure_marks(&layout_score, &row.measure_indices),
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
                publication: page_publication(
                    score,
                    &layout_score,
                    config,
                    &page_systems,
                    false,
                    match config.page_numbering {
                        PageNumbering::None => None,
                        PageNumbering::OneBased => Some(page_index + 1),
                    },
                ),
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
            publication: page_publication(
                score,
                &layout_score,
                config,
                &page_systems,
                false,
                match config.page_numbering {
                    PageNumbering::None => None,
                    PageNumbering::OneBased => Some(page_index + 1),
                },
            ),
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

    if config.publication.title_page {
        for page in &mut pages {
            page.page_index += 1;
            page.address.page_index = page.page_index;
            page.page_number = match config.page_numbering {
                PageNumbering::None => None,
                PageNumbering::OneBased => Some(page.page_index + 1),
            };
            for system in &mut page.systems {
                system.page_index += 1;
                system.address.page_index = system.page_index;
            }
            page.publication = page_publication(
                score,
                &layout_score,
                config,
                &page.systems,
                false,
                page.page_number,
            );
        }
        pages.insert(
            0,
            PageLayout {
                address: PageAddress { page_index: 0 },
                page_index: 0,
                page_number: match config.page_numbering {
                    PageNumbering::None => None,
                    PageNumbering::OneBased => Some(1),
                },
                color_policy: config.color_policy,
                crop_mark_policy: config.crop_mark_policy,
                glyph_resources: config.glyph_resources.clone(),
                publication: page_publication(
                    score,
                    &layout_score,
                    config,
                    &[],
                    true,
                    match config.page_numbering {
                        PageNumbering::None => None,
                        PageNumbering::OneBased => Some(1),
                    },
                ),
                width_mm,
                height_mm,
                content_width_mm,
                content_height_mm,
                bleed_top_mm: config.bleed_top_mm,
                bleed_right_mm: config.bleed_right_mm,
                bleed_bottom_mm: config.bleed_bottom_mm,
                bleed_left_mm: config.bleed_left_mm,
                span_segments: Vec::new(),
                measure_marks: Vec::new(),
                systems: Vec::new(),
                break_reason: BreakReason::TitlePage,
            },
        );
    }

    Ok(PrintLayoutResult {
        contract_version: PRINT_LAYOUT_CONTRACT_VERSION,
        pages,
    })
}

#[cfg(test)]
mod tests {
    use super::*;
    use acorde_core::{
        Clef, Duration, Measure, Note, Part, PartGroup, PartGroupSymbol, Pitch, Score, Staff, Step,
    };

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
    fn publication_metadata_accepts_legacy_partial_json() {
        let publication: PagePublication =
            serde_json::from_str(r#"{"is_title_page":true,"title":"Legacy score"}"#)
                .expect("legacy publication metadata should deserialize");

        assert!(publication.is_title_page);
        assert_eq!(publication.title, "Legacy score");
        assert!(publication.movement_title.is_empty());
        assert!(publication.part_labels.is_empty());
        assert!(publication.part_groups.is_empty());
        assert!(publication.text_blocks.is_empty());
    }

    #[test]
    fn layout_validation_rejects_unsupported_contract_version() {
        let score = score_with_measures(1);
        let mut result =
            compute_print_layout(&score, &PrintConfig::default()).expect("valid print config");
        result.contract_version = PRINT_LAYOUT_CONTRACT_VERSION - 1;

        assert_eq!(
            result.validate(),
            Err(PrintLayoutError::UnsupportedContractVersion {
                found: PRINT_LAYOUT_CONTRACT_VERSION - 1,
            })
        );
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
        assert!(result.validate().is_ok());
    }

    #[test]
    fn export_page_artifacts_reports_host_glyph_resource_requirement() {
        let result = compute_print_layout(
            &score_with_measures(1),
            &PrintConfig {
                glyph_resources: GlyphResourcePolicy::HostProvided("licensed-font-v1".into()),
                ..PrintConfig::default()
            },
        )
        .expect("valid host resource policy");

        let artifacts = result
            .export_page_artifacts()
            .expect("host resource requirement is a diagnostic");
        assert_eq!(
            artifacts[0].diagnostics,
            vec![PageArtifactDiagnostic::GlyphResourceRequired]
        );
        assert_eq!(
            artifacts[0].layout.glyph_resources,
            GlyphResourcePolicy::HostProvided("licensed-font-v1".into())
        );
    }

    #[test]
    fn page_artifact_diagnostics_report_glyph_overflow_sides() {
        let result = compute_print_layout(&score_with_measures(1), &PrintConfig::default())
            .expect("valid print layout");
        let page = &result.pages[0];
        assert_eq!(
            page.artifact_diagnostics(Some(GlyphExtents {
                left_mm: -1.0,
                top_mm: -2.0,
                right_mm: page.content_width_mm + 3.0,
                bottom_mm: page.content_height_mm + 4.0,
            })),
            vec![PageArtifactDiagnostic::GlyphOverflow {
                left: true,
                top: true,
                right: true,
                bottom: true,
            }]
        );
        assert!(page.artifact_diagnostics(None).is_empty());
    }

    #[test]
    fn export_page_artifacts_preserves_order_dimensions_and_continuation_diagnostics() {
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
        .expect("valid print config");

        let artifacts = result
            .export_page_artifacts()
            .expect("valid page artifacts");
        assert_eq!(artifacts.len(), 2);
        assert_eq!(artifacts[0].address, PageAddress { page_index: 0 });
        assert_eq!(artifacts[1].page_index, 1);
        assert_eq!(artifacts[0].width_mm, result.pages[0].width_mm);
        assert_eq!(artifacts[0].height_mm, result.pages[0].height_mm);
        assert_eq!(
            artifacts[0].measure_span,
            Some(MeasureSpan {
                first_measure: 0,
                last_measure: 1,
            })
        );
        assert_eq!(
            artifacts[0].diagnostics,
            vec![PageArtifactDiagnostic::SpanContinuation {
                span_index: 0,
                starts_here: true,
                ends_here: false,
            }]
        );
        assert_eq!(
            artifacts[1].diagnostics,
            vec![PageArtifactDiagnostic::SpanContinuation {
                span_index: 0,
                starts_here: false,
                ends_here: true,
            }]
        );
    }

    #[test]
    fn export_page_artifacts_rejects_invalid_serialized_layout() {
        let score = score_with_measures(1);
        let mut result =
            compute_print_layout(&score, &PrintConfig::default()).expect("valid print config");
        result.pages[0].width_mm = f32::NAN;

        assert!(matches!(
            result.export_page_artifacts(),
            Err(PrintLayoutError::InvalidPageGeometry { page_index: 0 })
        ));
    }

    #[test]
    fn page_lookup_rejects_mismatched_serialized_address() {
        let score = score_with_measures(1);
        let mut result =
            compute_print_layout(&score, &PrintConfig::default()).expect("valid print config");
        result.pages[0].address = PageAddress { page_index: 7 };

        assert!(result.page(PageAddress { page_index: 0 }).is_none());
        assert_eq!(
            result.validate(),
            Err(PrintLayoutError::InvalidPageAddress { page_index: 0 })
        );
    }

    #[test]
    fn layout_validation_rejects_mismatched_system_address() {
        let score = score_with_measures(2);
        let mut result = compute_print_layout(
            &score,
            &PrintConfig {
                measures_per_system: 1,
                ..PrintConfig::default()
            },
        )
        .expect("valid print config");
        result.pages[0].systems[0].address.index_on_page = 4;

        assert_eq!(
            result.validate(),
            Err(PrintLayoutError::InvalidSystemAddress {
                page_index: 0,
                index_on_page: 0,
                system_index: 0,
            })
        );
    }

    #[test]
    fn layout_validation_rejects_non_monotonic_page_number() {
        let score = score_with_measures(2);
        let mut result = compute_print_layout(
            &score,
            &PrintConfig {
                measures_per_system: 1,
                page_numbering: PageNumbering::OneBased,
                systems_per_page: Some(1),
                ..PrintConfig::default()
            },
        )
        .expect("valid print config");
        result.pages[1].page_number = Some(1);

        assert_eq!(
            result.validate(),
            Err(PrintLayoutError::InvalidPageNumber { page_index: 1 })
        );
    }

    #[test]
    fn layout_validation_rejects_inconsistent_title_page_metadata() {
        let score = score_with_measures(1);
        let mut result =
            compute_print_layout(&score, &PrintConfig::default()).expect("valid print config");
        result.pages[0].publication.is_title_page = true;

        assert_eq!(
            result.validate(),
            Err(PrintLayoutError::InvalidTitlePage { page_index: 0 })
        );
    }

    #[test]
    fn layout_validation_rejects_non_finite_page_geometry() {
        let score = score_with_measures(1);
        let mut result =
            compute_print_layout(&score, &PrintConfig::default()).expect("valid print config");
        result.pages[0].width_mm = f32::NAN;

        assert_eq!(
            result.validate(),
            Err(PrintLayoutError::InvalidPageGeometry { page_index: 0 })
        );
    }

    #[test]
    fn layout_validation_rejects_non_positive_system_geometry() {
        let score = score_with_measures(1);
        let mut result =
            compute_print_layout(&score, &PrintConfig::default()).expect("valid print config");
        result.pages[0].systems[0].height_mm = 0.0;

        assert_eq!(
            result.validate(),
            Err(PrintLayoutError::InvalidSystemGeometry {
                page_index: 0,
                index_on_page: 0,
            })
        );
    }

    #[test]
    fn layout_validation_rejects_content_larger_than_page() {
        let score = score_with_measures(1);
        let mut result =
            compute_print_layout(&score, &PrintConfig::default()).expect("valid print config");
        result.pages[0].content_width_mm = result.pages[0].width_mm + 1.0;

        assert_eq!(
            result.validate(),
            Err(PrintLayoutError::InvalidPageGeometry { page_index: 0 })
        );
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
        assert_eq!(result.contract_version, PRINT_LAYOUT_CONTRACT_VERSION);
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
    fn rejects_invalid_publication_line_height() {
        let score = score_with_measures(1);
        let error = compute_print_layout(
            &score,
            &PrintConfig {
                publication: PublicationConfig {
                    line_height_mm: 0.0,
                    ..PublicationConfig::default()
                },
                ..PrintConfig::default()
            },
        )
        .expect_err("invalid publication line height");
        assert_eq!(error, PrintLayoutError::InvalidPublicationLineHeight);
    }

    #[test]
    fn rejects_empty_host_glyph_resource_key() {
        let score = score_with_measures(1);
        let error = compute_print_layout(
            &score,
            &PrintConfig {
                glyph_resources: GlyphResourcePolicy::HostProvided("  ".into()),
                ..PrintConfig::default()
            },
        )
        .expect_err("empty host resource key");
        assert_eq!(error, PrintLayoutError::InvalidGlyphResourceKey);
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
        assert_eq!(result.contract_version, PRINT_LAYOUT_CONTRACT_VERSION);
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

    #[test]
    fn publication_metadata_is_deterministic_and_page_scoped() {
        let mut score = score_with_measures(3);
        score.metadata.title = "Suite".into();
        score.metadata.movement_title = "I. Prelude".into();
        score.metadata.composer = "Composer".into();
        score.metadata.copyright = "© 2026 Composer".into();
        score.metadata.lyricist = "Lyricist".into();
        score.metadata.copyright = "Copyright".into();
        score.parts.push(Part::new("Strings", "Str."));
        score.part_groups.push(PartGroup {
            first_part: 0,
            last_part: 1,
            symbol: PartGroupSymbol::Bracket,
            barlines_connect: true,
        });
        for (index, measure) in score.parts[0].staves[0].measures.iter_mut().enumerate() {
            measure.number = (index + 1) as u32;
        }
        let result = compute_print_layout(
            &score,
            &PrintConfig {
                measures_per_system: 2,
                systems_per_page: Some(1),
                publication: PublicationConfig {
                    running_title: Some("Suite — Composer".into()),
                    header_text: Some("Suite".into()),
                    footer_text: Some("Copyright".into()),
                    page_number_in_footer: true,
                    header_alignment: PublicationTextAlignment::Center,
                    footer_alignment: PublicationTextAlignment::Right,
                    ..PublicationConfig::default()
                },
                ..PrintConfig::default()
            },
        )
        .expect("valid print config");
        assert_eq!(result.pages[0].publication.title, "Suite");
        assert_eq!(
            result.pages[0].publication.running_title.as_deref(),
            Some("Suite — Composer")
        );
        assert_eq!(result.pages[0].publication.measure_numbers, vec![1, 2]);
        assert_eq!(result.pages[1].publication.measure_numbers, vec![3]);
        assert_eq!(result.pages[0].publication.part_labels[0].name, "Piano");
        assert_eq!(result.pages[0].publication.part_groups.len(), 1);
        assert_eq!(
            result.pages[0].publication.part_groups[0].symbol,
            PartGroupSymbol::Bracket
        );
        assert_eq!(result.pages[0].publication.text_blocks.len(), 3);
        assert_eq!(
            result.pages[0].publication.text_blocks[0].role,
            PublicationTextRole::Header
        );
        assert_eq!(result.pages[0].publication.text_blocks[0].x_mm, 14.0);
        assert_eq!(result.pages[0].publication.text_blocks[0].width_mm, 182.0);
        assert_eq!(
            result.pages[0].publication.text_blocks[1].role,
            PublicationTextRole::Footer
        );
        assert_eq!(result.pages[0].publication.text_blocks[2].text, "1");
        assert_eq!(result.pages[0].publication.text_blocks[0].height_mm, 4.0);
        assert_eq!(
            result.pages[0].publication.text_blocks[0].alignment,
            PublicationTextAlignment::Center
        );
        assert_eq!(
            result.pages[0].publication.text_blocks[1].alignment,
            PublicationTextAlignment::Right
        );
        let artifacts = result
            .export_page_artifacts()
            .expect("publication pages export without host resources");
        assert_eq!(artifacts.len(), result.pages.len());
        assert_eq!(artifacts[0].layout.publication, result.pages[0].publication);
        assert!(
            artifacts
                .iter()
                .all(|artifact| artifact.diagnostics.is_empty())
        );
    }

    #[test]
    fn extracted_part_policy_scopes_layout_and_rejects_missing_part() {
        let mut score = score_with_measures(2);
        let mut part = Part::new("Flute", "Fl.");
        let mut staff = Staff::new(Clef::Treble);
        staff.measures = vec![Measure::empty(4, 4); 5];
        part.staves = vec![staff];
        score.parts.push(part);

        let extracted = compute_print_layout(
            &score,
            &PrintConfig {
                part_layout: PartLayoutPolicy::ExtractedPart { part_index: 1 },
                measures_per_system: 2,
                systems_per_page: Some(1),
                ..PrintConfig::default()
            },
        )
        .expect("valid extracted part");
        assert_eq!(extracted.pages[0].systems[0].measure_indices, vec![0, 1]);
        assert_eq!(extracted.pages.len(), 3);
        assert_eq!(extracted.pages[0].publication.part_labels[0].name, "Flute");

        let error = compute_print_layout(
            &score,
            &PrintConfig {
                part_layout: PartLayoutPolicy::ExtractedPart { part_index: 2 },
                ..PrintConfig::default()
            },
        )
        .expect_err("missing extracted part");
        assert_eq!(error, PrintLayoutError::InvalidPartIndex);
    }

    #[test]
    fn title_page_is_inserted_without_consuming_music_page_capacity() {
        let mut score = score_with_measures(3);
        score.metadata.title = "Suite".into();
        score.metadata.movement_title = "I. Prelude".into();
        score.metadata.composer = "Composer".into();
        score.metadata.copyright = "© 2026 Composer".into();
        let result = compute_print_layout(
            &score,
            &PrintConfig {
                systems_per_page: Some(1),
                measures_per_system: 2,
                publication: PublicationConfig {
                    title_page: true,
                    ..PublicationConfig::default()
                },
                ..PrintConfig::default()
            },
        )
        .expect("valid title page config");
        assert_eq!(result.pages.len(), 3);
        assert!(result.pages[0].systems.is_empty());
        assert!(result.pages[0].publication.is_title_page);
        assert_eq!(result.pages[0].break_reason, BreakReason::TitlePage);
        assert_eq!(result.pages[0].page_number, Some(1));
        assert_eq!(result.pages[1].page_number, Some(2));
        assert_eq!(result.pages[1].systems[0].page_index, 1);
        assert!(!result.pages[1].publication.is_title_page);
        assert!(result.validate().is_ok());
        assert_eq!(
            result.pages[0]
                .publication
                .text_blocks
                .iter()
                .map(|block| block.role)
                .collect::<Vec<_>>(),
            vec![
                PublicationTextRole::Title,
                PublicationTextRole::Subtitle,
                PublicationTextRole::Credit,
                PublicationTextRole::Copyright
            ]
        );
    }

    #[test]
    fn print_presets_are_versioned_and_select_the_expected_scope() {
        assert_eq!(PrintPreset::A4Score.schema_version(), 1);
        assert_eq!(
            PrintPreset::A4Score.config().part_layout,
            PartLayoutPolicy::FullScore
        );
        assert_eq!(
            PrintPreset::LetterPart { part_index: 2 }
                .config()
                .part_layout,
            PartLayoutPolicy::ExtractedPart { part_index: 2 }
        );
        assert_eq!(
            PrintPreset::LetterScore.config().paper_size,
            PaperSize::Letter
        );
        assert!(
            PrintPreset::A4Score
                .config_with_title_page(true)
                .publication
                .title_page
        );
        assert!(!PrintPreset::A4Score.config().publication.title_page);
        assert_eq!(PRINT_PRESET_SCHEMA_VERSION, 1);
    }

    #[test]
    fn glyph_collision_resolution_is_deterministic_and_priority_aware() {
        let metrics = GlyphMetrics {
            advance_mm: 4.0,
            left_mm: -1.0,
            top_mm: -2.0,
            width_mm: 2.0,
            height_mm: 4.0,
        };
        let mut placements = vec![
            GlyphPlacement {
                resource_key: "high".into(),
                metrics,
                x_mm: 10.0,
                y_mm: 20.0,
                priority: 10,
            },
            GlyphPlacement {
                resource_key: "low".into(),
                metrics,
                x_mm: 10.0,
                y_mm: 20.0,
                priority: 1,
            },
        ];
        let moved = resolve_glyph_collisions(&mut placements, 1.0);
        assert_eq!(moved, 1);
        assert_eq!(placements[0].y_mm, 20.0);
        assert_eq!(placements[1].y_mm, 25.0);
    }

    #[test]
    fn vertical_collision_resolution_does_not_move_non_overlapping_glyphs() {
        let metrics = GlyphMetrics {
            advance_mm: 4.0,
            left_mm: -1.0,
            top_mm: -1.0,
            width_mm: 2.0,
            height_mm: 2.0,
        };
        let mut placements = vec![
            GlyphPlacement {
                resource_key: "high".into(),
                metrics,
                x_mm: 10.0,
                y_mm: 20.0,
                priority: 10,
            },
            GlyphPlacement {
                resource_key: "low".into(),
                metrics,
                x_mm: 10.0,
                y_mm: 0.0,
                priority: 1,
            },
        ];
        assert_eq!(resolve_glyph_collisions(&mut placements, 1.0), 0);
        assert_eq!(placements[1].y_mm, 0.0);
    }

    #[test]
    fn glyph_placement_validation_rejects_non_finite_and_negative_geometry() {
        let mut placements = vec![GlyphPlacement {
            resource_key: "test".into(),
            metrics: GlyphMetrics {
                advance_mm: 1.0,
                left_mm: 0.0,
                top_mm: 0.0,
                width_mm: 1.0,
                height_mm: 1.0,
            },
            x_mm: 0.0,
            y_mm: 0.0,
            priority: 0,
        }];
        assert_eq!(validate_glyph_placements(&placements), Ok(()));
        placements[0].x_mm = f32::NAN;
        assert_eq!(
            validate_glyph_placements(&placements),
            Err(GlyphPlacementError::NonFinite { index: 0 })
        );
        placements[0].x_mm = 0.0;
        placements[0].metrics.width_mm = -1.0;
        assert_eq!(
            validate_glyph_placements(&placements),
            Err(GlyphPlacementError::NegativeExtent { index: 0 })
        );
    }

    #[test]
    fn horizontal_glyph_collision_resolution_is_priority_aware_and_skips_vertical_gaps() {
        let metrics = GlyphMetrics {
            advance_mm: 4.0,
            left_mm: -1.0,
            top_mm: -1.0,
            width_mm: 2.0,
            height_mm: 2.0,
        };
        let mut placements = vec![
            GlyphPlacement {
                resource_key: "high".into(),
                metrics,
                x_mm: 10.0,
                y_mm: 20.0,
                priority: 10,
            },
            GlyphPlacement {
                resource_key: "low".into(),
                metrics,
                x_mm: 10.0,
                y_mm: 20.0,
                priority: 1,
            },
            GlyphPlacement {
                resource_key: "far".into(),
                metrics,
                x_mm: 10.0,
                y_mm: 30.0,
                priority: 1,
            },
        ];
        assert_eq!(resolve_glyph_horizontal_collisions(&mut placements, 1.0), 1);
        assert_eq!(placements[0].x_mm, 10.0);
        assert_eq!(placements[1].x_mm, 13.0);
        assert_eq!(placements[2].x_mm, 10.0);
    }

    #[test]
    fn glyph_spacing_distribution_is_stable_and_rejects_non_finite_spacing() {
        let metrics = GlyphMetrics {
            advance_mm: 1.0,
            left_mm: 0.0,
            top_mm: 0.0,
            width_mm: 1.0,
            height_mm: 1.0,
        };
        let mut placements = vec![
            GlyphPlacement {
                resource_key: "second".into(),
                metrics,
                x_mm: 20.0,
                y_mm: 0.0,
                priority: 0,
            },
            GlyphPlacement {
                resource_key: "first".into(),
                metrics,
                x_mm: 10.0,
                y_mm: 0.0,
                priority: 0,
            },
            GlyphPlacement {
                resource_key: "third".into(),
                metrics,
                x_mm: 30.0,
                y_mm: 0.0,
                priority: 0,
            },
        ];
        assert_eq!(distribute_glyph_spacing(&mut placements, 6.0), Ok(2));
        assert_eq!(placements[0].x_mm, 23.0);
        assert_eq!(placements[1].x_mm, 10.0);
        assert_eq!(placements[2].x_mm, 36.0);
        assert_eq!(
            distribute_glyph_spacing(&mut placements, f32::NAN),
            Err(GlyphPlacementError::NonFiniteSpacing)
        );
        let before = placements.clone();
        assert_eq!(
            distribute_glyph_spacing(&mut placements, f32::MAX),
            Err(GlyphPlacementError::NonFiniteSpacing)
        );
        assert_eq!(placements, before);
    }

    #[test]
    fn glyph_placement_validation_rejects_missing_resource_and_negative_advance() {
        let mut placement = GlyphPlacement {
            resource_key: " ".into(),
            metrics: GlyphMetrics {
                advance_mm: 1.0,
                left_mm: 0.0,
                top_mm: 0.0,
                width_mm: 1.0,
                height_mm: 1.0,
            },
            x_mm: 0.0,
            y_mm: 0.0,
            priority: 0,
        };
        assert_eq!(
            validate_glyph_placements(&[placement.clone()]),
            Err(GlyphPlacementError::EmptyResourceKey { index: 0 })
        );
        placement.resource_key = "glyph".into();
        placement.metrics.advance_mm = -1.0;
        assert_eq!(
            validate_glyph_placements(&[placement]),
            Err(GlyphPlacementError::NegativeAdvance { index: 0 })
        );
    }

    #[test]
    fn checked_collision_resolvers_reject_invalid_geometry_before_mutation() {
        let mut placements = vec![GlyphPlacement {
            resource_key: String::new(),
            metrics: GlyphMetrics {
                advance_mm: 1.0,
                left_mm: 0.0,
                top_mm: 0.0,
                width_mm: 1.0,
                height_mm: 1.0,
            },
            x_mm: 0.0,
            y_mm: 0.0,
            priority: 0,
        }];
        assert_eq!(
            resolve_glyph_collisions_checked(&mut placements, 1.0),
            Err(GlyphPlacementError::EmptyResourceKey { index: 0 })
        );
        assert_eq!(
            resolve_glyph_horizontal_collisions_checked(&mut placements, 1.0),
            Err(GlyphPlacementError::EmptyResourceKey { index: 0 })
        );
        assert_eq!(placements[0].x_mm, 0.0);
        assert_eq!(placements[0].y_mm, 0.0);
    }

    #[test]
    fn checked_collision_resolvers_reject_non_finite_gap() {
        let metrics = GlyphMetrics {
            advance_mm: 1.0,
            left_mm: 0.0,
            top_mm: 0.0,
            width_mm: 1.0,
            height_mm: 1.0,
        };
        let original = vec![GlyphPlacement {
            resource_key: "glyph".into(),
            metrics,
            x_mm: 0.0,
            y_mm: 0.0,
            priority: 0,
        }];
        let mut vertical = original.clone();
        assert_eq!(
            resolve_glyph_collisions_checked(&mut vertical, f32::NAN),
            Err(GlyphPlacementError::NonFiniteSpacing)
        );
        assert_eq!(vertical, original);

        let mut horizontal = original.clone();
        assert_eq!(
            resolve_glyph_horizontal_collisions_checked(&mut horizontal, f32::INFINITY),
            Err(GlyphPlacementError::NonFiniteSpacing)
        );
        assert_eq!(horizontal, original);
    }

    #[test]
    fn checked_collision_resolvers_reject_arithmetic_overflow_without_mutation() {
        let metrics = GlyphMetrics {
            advance_mm: 1.0,
            left_mm: 0.0,
            top_mm: 0.0,
            width_mm: f32::MAX / 2.0,
            height_mm: f32::MAX / 2.0,
        };
        let original = vec![
            GlyphPlacement {
                resource_key: "high".into(),
                metrics,
                x_mm: 0.0,
                y_mm: 0.0,
                priority: 1,
            },
            GlyphPlacement {
                resource_key: "low".into(),
                metrics,
                x_mm: 0.0,
                y_mm: 0.0,
                priority: 0,
            },
        ];
        let mut vertical = original.clone();
        assert_eq!(
            resolve_glyph_collisions_checked(&mut vertical, f32::MAX),
            Err(GlyphPlacementError::NonFinite { index: 1 })
        );
        assert_eq!(vertical, original);

        let mut horizontal = original.clone();
        assert_eq!(
            resolve_glyph_horizontal_collisions_checked(&mut horizontal, f32::MAX),
            Err(GlyphPlacementError::NonFinite { index: 1 })
        );
        assert_eq!(horizontal, original);
    }

    #[test]
    fn glyph_extents_are_content_aware_and_empty_collections_are_explicit() {
        let metrics = GlyphMetrics {
            advance_mm: 1.0,
            left_mm: -1.0,
            top_mm: -2.0,
            width_mm: 3.0,
            height_mm: 4.0,
        };
        let placements = vec![
            GlyphPlacement {
                resource_key: "a".into(),
                metrics,
                x_mm: 10.0,
                y_mm: 20.0,
                priority: 0,
            },
            GlyphPlacement {
                resource_key: "b".into(),
                metrics,
                x_mm: 30.0,
                y_mm: 5.0,
                priority: 0,
            },
        ];
        assert_eq!(
            glyph_extents(&placements),
            Ok(Some(GlyphExtents {
                left_mm: 9.0,
                top_mm: 3.0,
                right_mm: 32.0,
                bottom_mm: 22.0,
            }))
        );
        let extents = glyph_extents(&placements).unwrap().unwrap();
        assert_eq!(extents.width_mm(), 23.0);
        assert_eq!(extents.height_mm(), 19.0);
        assert_eq!(glyph_extents(&[]), Ok(None));
    }

    #[test]
    fn glyph_extents_reject_derived_bound_overflow() {
        let placements = [GlyphPlacement {
            resource_key: "edge".into(),
            metrics: GlyphMetrics {
                advance_mm: 1.0,
                left_mm: 0.0,
                top_mm: 0.0,
                width_mm: f32::MAX,
                height_mm: 1.0,
            },
            x_mm: f32::MAX,
            y_mm: 0.0,
            priority: 0,
        }];
        assert_eq!(
            glyph_extents(&placements),
            Err(GlyphPlacementError::NonFinite { index: 0 })
        );
    }
}
