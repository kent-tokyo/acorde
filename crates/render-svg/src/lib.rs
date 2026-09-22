//! Pure-Rust/WASM SVG score renderer for [`acorde_core::Score`], driven by
//! [`acorde_layout`](https://docs.rs/acorde-layout).
//!
//! ```text
//! Score model (acorde-core) → logical layout (acorde-layout) → renderer (acorde-render-svg)
//! ```
//!
//! This crate has no browser/DOM dependency — [`render_svg`] produces a plain `String` that
//! renders identically whether you're on native Rust, in a WASM host, or on a server. It
//! consumes layout decisions ([`LayoutResult`]) rather than re-deriving them: beam grouping,
//! tuplet grouping, courtesy accidentals, and row breaks all stay in `acorde-layout`. This
//! crate places glyphs at pixel coordinates and nothing more.
//!
//! Glyphs (clefs, noteheads, accidentals, rests) are original hand-authored SVG paths — no
//! vendored font, no system-font dependency. See the crate README for the rationale.
//!
//! # Example
//!
//! ```
//! use acorde_core::Score;
//! use acorde_render_svg::{render_svg, SvgRenderOptions};
//!
//! let score = Score::default();
//! let svg = render_svg(&score, &SvgRenderOptions::default()).unwrap();
//! assert!(svg.starts_with("<svg"));
//! ```

mod beams;
mod geometry;
mod glyphs;
mod render;
mod tuplets;

use acorde_core::{
    HarpPedalDiagram, NoteAddr, NoteHead, ObjectStyleOverride, Score, ScoreView, TextStyle,
    ValidationError, ViewStyle, ViewStyleOverride,
};
use acorde_layout::{
    GlyphCollisionClass, GlyphCollisionDirection, LayoutConfig, LayoutResult, compute_layout,
};
use serde::{Deserialize, Serialize};
use std::fmt;

/// Version of the browser-facing [`RenderMetadata`] contract.
pub const SVG_CONTRACT_VERSION: u32 = 21;
/// Version of the built-in glyph coverage contract.
pub const GLYPH_COVERAGE_CONTRACT_VERSION: u32 = 3;
/// Stable identifier for the renderer's font-independent vector glyph set.
pub const BUILTIN_GLYPH_RESOURCE_ID: &str = "acorde-vector-glyphs-v1";
/// Version of the deterministic tablature metric contract.
pub const TAB_METRICS_CONTRACT_VERSION: u32 = 1;
/// Fixed separation used by the deterministic SVG annotation collision pass.
pub const SVG_ANNOTATION_COLLISION_GAP_PX: f32 = 2.0;

/// Font-independent metrics for one rendered tablature fret label.
#[derive(Debug, Clone, Copy, Serialize, Deserialize, PartialEq)]
pub struct TabFretMetrics {
    pub fret: u8,
    pub digit_count: u8,
    /// Horizontal advance in staff-space units, before multiplying by staff size.
    pub advance_units: f32,
    /// Fixed gap between adjacent fret labels in staff-space units.
    pub side_gap_units: f32,
}

/// Return the stable metrics used for tablature fret labels.
pub fn tab_fret_metrics(fret: u8) -> TabFretMetrics {
    let digit_count = fret.to_string().len() as u8;
    TabFretMetrics {
        fret,
        digit_count,
        advance_units: f32::from(digit_count) * 0.48,
        side_gap_units: 0.22,
    }
}

/// Return the stable built-in resource name for a bounded unpitched notehead shape.
///
/// This maps only visual notehead intent; it does not resolve a percussion instrument or
/// invent a MIDI sound identity.
pub fn percussion_notehead_resource_id(note_head: &NoteHead) -> &'static str {
    match note_head {
        NoteHead::Normal => "acorde-percussion-notehead-normal",
        NoteHead::Diamond => "acorde-percussion-notehead-diamond",
        NoteHead::X => "acorde-percussion-notehead-x",
        NoteHead::Slash => "acorde-percussion-notehead-slash",
        NoteHead::Cross => "acorde-percussion-notehead-cross",
        NoteHead::Triangle => "acorde-percussion-notehead-triangle",
    }
}

const MAX_RENDER_ANNOTATIONS: usize = 10_000;
const MAX_ANNOTATION_TEXT_BYTES: usize = 16 * 1024;

pub(crate) fn is_valid_xml_char(character: char) -> bool {
    matches!(character, '\u{9}' | '\u{A}' | '\u{D}')
        || ('\u{20}'..='\u{D7FF}').contains(&character)
        || ('\u{E000}'..='\u{FFFD}').contains(&character)
        || ('\u{10000}'..='\u{10FFFF}').contains(&character)
}

/// Options controlling SVG output. All fields have defaults — safe to deserialize from
/// partial JSON (e.g. `"{}"` from a WASM caller that only wants defaults).
#[derive(Debug, Clone, Serialize, Deserialize)]
#[serde(default)]
pub struct SvgRenderOptions {
    /// Total SVG width in pixels. Content is distributed across this width per row.
    pub width: f32,
    /// Distance between two adjacent staff lines, in pixels. Drives every other glyph
    /// dimension (noteheads, stems, clefs, accidentals are all proportional to this).
    pub staff_size: f32,
    /// How many measures to place per system/row.
    pub measures_per_system: usize,
    /// When `true`, emit stable `data-*` hooks (`data-acorde-kind`, `data-part`, `data-staff`,
    /// `data-measure`, `data-voice`, `data-note`, `data-note-addr`) for click-to-position
    /// interaction. When `false`, those attributes are omitted.
    pub interactive: bool,
}

/// Lightweight browser-facing metadata for a rendered score.
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct RenderMetadata {
    /// Version of the metadata contract, for forward-compatible browser hosts.
    pub contract_version: u32,
    pub width: f32,
    pub height: f32,
    pub part_count: usize,
    pub staff_count: usize,
    pub measure_count: usize,
    pub note_count: usize,
    /// Resolved linked-view context when the caller used [`render_svg_view_metadata`].
    /// `None` means the source score was rendered directly.
    #[serde(default)]
    pub view: Option<RenderedViewMetadata>,
    /// Human-readable fallback text for hosts that cannot expose the SVG semantics.
    pub accessible_text: String,
    pub address_bounds: Vec<AddressBounds>,
    /// Measure-level text with its stable score location and typed presentation role.
    #[serde(default)]
    pub text_annotations: Vec<TextAnnotation>,
    /// Score-level title-page text imported from formats such as MuseScore VBox.
    #[serde(default)]
    pub score_texts: Vec<ScoreTextMetadata>,
    /// String/fret positions exposed without requiring hosts to parse SVG elements.
    #[serde(default)]
    pub tablature_positions: Vec<TablaturePositionMetadata>,
    /// Tuning and capo metadata for each rendered tablature staff.
    #[serde(default)]
    pub tablature_staves: Vec<TablatureStaffMetadata>,
    /// Measure-local tuning or capo changes for tablature hosts.
    #[serde(default)]
    pub tablature_changes: Vec<TablatureChangeMetadata>,
    /// Chord labels whose continuation range is exposed with typed note addresses.
    #[serde(default)]
    pub harmony_ranges: Vec<HarmonyRangeMetadata>,
    /// Tablature technique connections with stable start/end note addresses.
    #[serde(default)]
    pub tablature_technique_connections: Vec<TablatureTechniqueConnectionMetadata>,
    /// Note-level semantic fields needed by browser playback and editing hosts.
    #[serde(default)]
    pub note_semantics: Vec<NoteSemanticMetadata>,
    /// Typed object-level presentation overrides, preserved for browser hosts without SVG parsing.
    #[serde(default)]
    pub object_style_overrides: Vec<ObjectStyleOverride>,
    /// Harp pedal diagrams in the conventional D-C-B / E-F-G-A order.
    #[serde(default)]
    pub harp_pedal_diagrams: Vec<HarpPedalDiagramMetadata>,
}

/// Browser-facing identity and effective style for a resolved linked view.
#[derive(Debug, Clone, Serialize, Deserialize, PartialEq)]
pub struct RenderedViewMetadata {
    pub id: String,
    pub name: String,
    pub measures_per_system: usize,
    pub style: ViewStyle,
}

/// Stable note-level semantic metadata that avoids forcing browser hosts to parse SVG elements.
#[derive(Debug, Clone, Serialize, Deserialize, PartialEq)]
pub struct NoteSemanticMetadata {
    pub part: usize,
    pub staff: usize,
    pub measure: usize,
    pub voice: usize,
    pub note: usize,
    pub is_unpitched: bool,
    /// Whether the note starts or ends an authored tie.
    #[serde(default)]
    pub tie_start: bool,
    #[serde(default)]
    pub tie_end: bool,
    /// Per-note performed duration in canonical beats, including dots and tuplets.
    #[serde(default)]
    pub duration_beats: f64,
    /// Exact sounding pitch for each source pitch in hundredths of a MIDI semitone.
    #[serde(default)]
    pub pitch_midi_cents: Vec<i32>,
    /// MusicXML note placement offsets in tenths, when authored.
    #[serde(default)]
    pub offset_x: Option<f64>,
    #[serde(default)]
    pub offset_y: Option<f64>,
    #[serde(default)]
    pub relative_x: Option<f64>,
    #[serde(default)]
    pub relative_y: Option<f64>,
    #[serde(default)]
    pub dynamic: Option<String>,
    #[serde(default)]
    pub lyric: Option<String>,
    #[serde(default)]
    pub chord_label: Option<String>,
    #[serde(default)]
    pub technique_text: Option<String>,
    /// Ordered note articulations using stable kebab-case names; tremolo includes its level.
    #[serde(default)]
    pub articulations: Vec<String>,
    /// Guitar technique used by tablature playback/rendering hosts.
    #[serde(default)]
    pub guitar_technique: Option<acorde_core::GuitarTechnique>,
    /// Authored bend amount in cents, when present.
    #[serde(default)]
    pub guitar_bend_alter_cents: Option<i16>,
    /// Authored multi-point bend/hold/release curve, when present.
    #[serde(default)]
    pub guitar_bend_curve: Vec<acorde_core::GuitarBendPoint>,
    #[serde(default)]
    pub fingerings: Vec<u8>,
    #[serde(default)]
    pub instrument_id: Option<String>,
    /// One entry per pitch in source order; zero means no microtonal offset.
    #[serde(default)]
    pub microtone_cents: Vec<i16>,
}

/// A ranged harmony annotation with stable score addresses for browser editors.
#[derive(Debug, Clone, Serialize, Deserialize, PartialEq)]
pub struct HarmonyRangeMetadata {
    pub start: NoteAddr,
    pub end: NoteAddr,
    pub label: String,
}

/// A tablature position with a stable source address for browser editing hosts.
#[derive(Debug, Clone, Serialize, Deserialize, PartialEq, Eq)]
pub struct TablaturePositionMetadata {
    pub part: usize,
    pub staff: usize,
    pub measure: usize,
    pub voice: usize,
    pub note: usize,
    pub position: usize,
    pub string: u8,
    pub fret: u8,
}

/// Typed tablature configuration for a rendered staff.
#[derive(Debug, Clone, Serialize, Deserialize, PartialEq, Eq)]
pub struct TablatureStaffMetadata {
    pub part: usize,
    pub staff: usize,
    pub lines: u8,
    #[serde(default)]
    pub tuning_midi: Vec<i16>,
    pub capo: u8,
    #[serde(default)]
    pub rhythm_display: acorde_core::TablatureRhythmDisplay,
    #[serde(default)]
    pub fret_mark_style: acorde_core::TablatureFretMarkStyle,
}

/// A tablature tuning or capo configuration that takes effect at a physical measure.
#[derive(Debug, Clone, Serialize, Deserialize, PartialEq, Eq)]
pub struct TablatureChangeMetadata {
    pub part: usize,
    pub staff: usize,
    pub measure: usize,
    pub lines: u8,
    #[serde(default)]
    pub tuning_midi: Vec<i16>,
    pub capo: u8,
}

/// A typed tablature connection for browser editing and playback hosts.
#[derive(Debug, Clone, Serialize, Deserialize, PartialEq)]
pub struct TablatureTechniqueConnectionMetadata {
    pub start: NoteAddr,
    pub end: NoteAddr,
    pub technique: acorde_core::GuitarTechnique,
    pub string: u8,
    /// True when the connection joins adjacent physical measures.
    #[serde(default)]
    pub cross_measure: bool,
}

/// A measure-level styled text entry exposed to browser hosts without SVG parsing.
#[derive(Debug, Clone, Serialize, Deserialize, PartialEq)]
pub struct TextAnnotation {
    pub part: usize,
    pub staff: usize,
    pub measure: usize,
    pub style: TextStyle,
    pub text: String,
    #[serde(default)]
    pub placement: Option<String>,
    #[serde(default)]
    pub offset_x: Option<f64>,
    #[serde(default)]
    pub offset_y: Option<f64>,
    #[serde(default)]
    pub relative_x: Option<f64>,
    #[serde(default)]
    pub relative_y: Option<f64>,
}

/// A harp pedal diagram with its stable staff-local measure location.
#[derive(Debug, Clone, Serialize, Deserialize, PartialEq, Eq)]
pub struct HarpPedalDiagramMetadata {
    pub part: usize,
    pub staff: usize,
    pub measure: usize,
    pub diagram: HarpPedalDiagram,
}

/// A score-level styled text entry exposed without requiring hosts to parse SVG or interchange XML.
#[derive(Debug, Clone, Serialize, Deserialize, PartialEq)]
pub struct ScoreTextMetadata {
    pub style: TextStyle,
    pub text: String,
    #[serde(default)]
    pub placement: Option<String>,
    #[serde(default)]
    pub offset_x: Option<f64>,
    #[serde(default)]
    pub offset_y: Option<f64>,
    #[serde(default)]
    pub relative_x: Option<f64>,
    #[serde(default)]
    pub relative_y: Option<f64>,
}

/// Host-neutral capability information for the renderer's explicit microtone marker.
#[derive(Debug, Clone, Serialize, Deserialize, PartialEq, Eq)]
pub struct MicrotoneMarkerCoverage {
    /// Whether non-zero cents are emitted as a visible SVG marker.
    pub supported: bool,
    /// Stable representation name; this is intentionally not a font glyph claim.
    pub representation: String,
    /// Whether the marker preserves the exact signed cents value from the score model.
    pub exact_cents: bool,
}

impl Default for MicrotoneMarkerCoverage {
    fn default() -> Self {
        Self {
            supported: true,
            representation: "svg-text-cents".to_owned(),
            exact_cents: true,
        }
    }
}

/// Explicit coverage information for the renderer's built-in glyph resource.
///
/// Hosts can use this before rendering or selecting a print resource. Unsupported
/// notation still returns a typed [`RenderError`] during rendering.
#[derive(Debug, Clone, Serialize, Deserialize, PartialEq, Eq)]
pub struct GlyphCoverage {
    pub contract_version: u32,
    pub resource_id: String,
    pub vector_glyphs: bool,
    pub supported_clefs: Vec<String>,
    pub accidental_min: i8,
    pub accidental_max: i8,
    /// Explicit, font-independent microtone representation available to hosts.
    #[serde(default)]
    pub microtone_marker: MicrotoneMarkerCoverage,
    /// Stable visual resources available for explicitly unpitched notehead shapes.
    #[serde(default)]
    pub percussion_noteheads: Vec<String>,
}

/// Describe the deterministic, font-independent glyphs shipped by this renderer.
pub fn glyph_coverage() -> GlyphCoverage {
    GlyphCoverage {
        contract_version: GLYPH_COVERAGE_CONTRACT_VERSION,
        resource_id: BUILTIN_GLYPH_RESOURCE_ID.to_owned(),
        vector_glyphs: true,
        supported_clefs: vec![
            "treble".to_owned(),
            "bass".to_owned(),
            "alto".to_owned(),
            "tenor".to_owned(),
        ],
        accidental_min: -2,
        accidental_max: 2,
        microtone_marker: MicrotoneMarkerCoverage::default(),
        percussion_noteheads: [
            NoteHead::Normal,
            NoteHead::Diamond,
            NoteHead::X,
            NoteHead::Slash,
            NoteHead::Cross,
            NoteHead::Triangle,
        ]
        .iter()
        .map(|head| percussion_notehead_resource_id(head).to_owned())
        .collect(),
    }
}

/// The bounded notation capabilities checked by [`render_preflight`].
#[derive(Debug, Clone, Copy, Serialize, Deserialize, PartialEq, Eq)]
pub enum RenderPreflightKind {
    UnsupportedClef,
    UnsupportedAccidental,
    InvalidTabPosition,
    MeasureTextTooLarge,
    InvalidMeasureTextOffset,
    InvalidNotePlacement,
    InvalidXmlCharacter,
}

/// A source-located renderer capability warning discovered before SVG emission.
#[derive(Debug, Clone, Serialize, Deserialize, PartialEq, Eq)]
pub struct RenderPreflightIssue {
    pub kind: RenderPreflightKind,
    pub source_location: String,
    pub preserved_value: String,
}

/// Inspect renderer capability boundaries without producing SVG or partially rendering a score.
pub fn render_preflight(score: &Score) -> Vec<RenderPreflightIssue> {
    let mut issues = Vec::new();
    push_text_preflight_issue(&mut issues, "/score/title", &score.metadata.title);
    for (part_index, part) in score.parts.iter().enumerate() {
        push_text_preflight_issue(
            &mut issues,
            &format!("/score/part/{}/name", part_index + 1),
            &part.name,
        );
        push_text_preflight_issue(
            &mut issues,
            &format!("/score/part/{}/short-name", part_index + 1),
            &part.short_name,
        );
        for (staff_index, staff) in part.staves.iter().enumerate() {
            let staff_path = format!("/score/part/{}/staff/{}", part_index + 1, staff_index + 1);
            for (measure_index, measure) in staff.measures.iter().enumerate() {
                for (text_index, styled) in measure.texts.iter().enumerate() {
                    let text_path = format!(
                        "{staff_path}/measure/{}/text/{}",
                        measure_index + 1,
                        text_index + 1
                    );
                    push_text_preflight_issue(&mut issues, &text_path, &styled.text);
                    for (field, value) in [
                        ("offset_x", styled.offset_x),
                        ("offset_y", styled.offset_y),
                        ("relative_x", styled.relative_x),
                        ("relative_y", styled.relative_y),
                    ] {
                        if let Some(value) = value {
                            if !value.is_finite() || !(value as f32).is_finite() {
                                issues.push(RenderPreflightIssue {
                                    kind: RenderPreflightKind::InvalidMeasureTextOffset,
                                    source_location: format!("{text_path}/{field}"),
                                    preserved_value: value.to_string(),
                                });
                            }
                        }
                    }
                }
                for (field, value) in [
                    ("tempo-text", measure.tempo_text.as_deref()),
                    ("rehearsal", measure.rehearsal.as_deref()),
                    ("navigation", measure.navigation.as_deref()),
                    ("expression-text", measure.expression_text.as_deref()),
                ] {
                    if let Some(value) = value {
                        push_text_preflight_issue(
                            &mut issues,
                            &format!("{staff_path}/measure/{}/{}", measure_index + 1, field),
                            value,
                        );
                    }
                }
                for (voice_index, voice) in measure.voices.iter().enumerate() {
                    for (note_index, note) in voice.iter().enumerate() {
                        let note_path = format!(
                            "{staff_path}/measure/{}/voice/{}/note/{}",
                            measure_index + 1,
                            voice_index + 1,
                            note_index + 1
                        );
                        if let Some(lyric) = &note.lyric {
                            push_text_preflight_issue(
                                &mut issues,
                                &format!("{note_path}/lyric"),
                                &lyric.text,
                            );
                        }
                        if let Some(technique) = &note.technique_text {
                            push_text_preflight_issue(
                                &mut issues,
                                &format!("{note_path}/technique-text"),
                                technique,
                            );
                        }
                        if let Some(chord) = &note.chord_symbol {
                            let chord_text = chord.display_text();
                            push_text_preflight_issue(
                                &mut issues,
                                &format!("{note_path}/chord-symbol"),
                                &chord_text,
                            );
                        }
                        for (field, value) in [
                            ("default-x", note.offset_x),
                            ("default-y", note.offset_y),
                            ("relative-x", note.relative_x),
                            ("relative-y", note.relative_y),
                        ] {
                            if let Some(value) = value {
                                if !value.is_finite() || !(value as f32).is_finite() {
                                    issues.push(RenderPreflightIssue {
                                        kind: RenderPreflightKind::InvalidNotePlacement,
                                        source_location: format!("{note_path}/{field}"),
                                        preserved_value: value.to_string(),
                                    });
                                }
                            }
                        }
                        for (pitch_index, pitch) in note.pitches.iter().enumerate() {
                            if !(-2..=2).contains(&pitch.alter) {
                                issues.push(RenderPreflightIssue {
                                    kind: RenderPreflightKind::UnsupportedAccidental,
                                    source_location: format!(
                                        "{note_path}/pitch/{}",
                                        pitch_index + 1
                                    ),
                                    preserved_value: pitch.alter.to_string(),
                                });
                            }
                        }
                        if let Some(tab) = &staff.tablature {
                            let positions = if note.tab_positions.is_empty() {
                                note.tab_position.iter().collect::<Vec<_>>()
                            } else {
                                note.tab_positions.iter().collect::<Vec<_>>()
                            };
                            for position in positions {
                                if position.string == 0 || position.string > tab.lines {
                                    issues.push(RenderPreflightIssue {
                                        kind: RenderPreflightKind::InvalidTabPosition,
                                        source_location: format!("{note_path}/tab-position"),
                                        preserved_value: format!(
                                            "string={},lines={}",
                                            position.string, tab.lines
                                        ),
                                    });
                                }
                            }
                        }
                    }
                }
            }
        }
    }
    issues
}

fn push_text_preflight_issue(
    issues: &mut Vec<RenderPreflightIssue>,
    source_location: &str,
    text: &str,
) {
    if text.len() > MAX_ANNOTATION_TEXT_BYTES {
        issues.push(RenderPreflightIssue {
            kind: RenderPreflightKind::MeasureTextTooLarge,
            source_location: source_location.to_owned(),
            preserved_value: format!("bytes={}", text.len()),
        });
    }
    if let Some(character) = text
        .chars()
        .find(|&character| !is_valid_xml_char(character))
    {
        issues.push(RenderPreflightIssue {
            kind: RenderPreflightKind::InvalidXmlCharacter,
            source_location: source_location.to_owned(),
            preserved_value: format!("U+{:04X}", character as u32),
        });
    }
}

/// Approximate interactive bounds for one stable [`acorde_core::NoteAddr`]. The box is
/// centered on the note's anchor and is intended for hit testing/highlighting, not engraving.
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct AddressBounds {
    pub part: usize,
    pub staff: usize,
    pub measure: usize,
    pub voice: usize,
    pub note: usize,
    pub x: f32,
    pub y: f32,
    pub width: f32,
    pub height: f32,
}

/// A host-provided, non-semantic annotation to place on top of rendered SVG.
///
/// Coordinates are in the SVG viewport's pixel coordinate system. The renderer does not
/// interpret `id` or `text`; this keeps domain-specific analysis outside `acorde-render-svg`.
#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
pub struct SvgAnnotation {
    /// Stable identifier within one rendered score.
    pub id: String,
    pub x: f32,
    pub y: f32,
    pub text: String,
}

/// Font-independent SVG-pixel bounds relative to an annotation's `(x, y)` anchor.
///
/// A provider supplies these when it wants the renderer to route its annotation through the
/// shared collision pass. The renderer does not measure fonts or shape text on its own.
#[derive(Debug, Clone, Copy, PartialEq, Serialize, Deserialize)]
pub struct SvgAnnotationMetrics {
    pub left_px: f32,
    pub top_px: f32,
    pub width_px: f32,
    pub height_px: f32,
}

/// Collision ownership and permitted escape lane for one host annotation.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
pub struct SvgAnnotationCollisionPolicy {
    pub class: GlyphCollisionClass,
    pub direction: GlyphCollisionDirection,
    /// Higher-priority annotations retain their requested position where possible.
    pub priority: u8,
}

/// Extension point for deterministic, host-defined SVG annotations.
pub trait RenderAnnotation {
    /// Stable provider identifier. Providers are executed in lexicographic ID order.
    fn id(&self) -> &str;

    /// Return annotations using the already-computed score, layout, and render metadata.
    fn annotate(
        &self,
        score: &Score,
        layout: &LayoutResult,
        metadata: &RenderMetadata,
    ) -> Vec<SvgAnnotation>;

    /// Optionally place one annotation in the shared deterministic collision pass.
    ///
    /// Returning `None` preserves the annotation's requested coordinates. A provider must supply
    /// its own font-independent bounds through [`SvgAnnotationMetrics`]; this renderer never
    /// infers text metrics from a host font.
    fn collision_policy(
        &self,
        _annotation: &SvgAnnotation,
    ) -> Option<(SvgAnnotationMetrics, SvgAnnotationCollisionPolicy)> {
        None
    }
}

/// Errors returned while validating host-provided render annotations.
#[derive(Debug, Clone, PartialEq, Eq)]
pub enum RenderAnnotationError {
    EmptyProviderId,
    DuplicateProviderId(String),
    EmptyAnnotationId,
    DuplicateAnnotationId(String),
    NonFiniteCoordinate { id: String },
    TooManyAnnotations { count: usize },
    AnnotationTextTooLarge { id: String, size: usize },
    InvalidXmlCharacter { id: String, codepoint: u32 },
    InvalidCollisionMetrics { id: String },
    CollisionResolution { id: String },
}

impl fmt::Display for RenderAnnotationError {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        match self {
            Self::EmptyProviderId => write!(f, "render annotation provider id is empty"),
            Self::DuplicateProviderId(id) => {
                write!(f, "duplicate render annotation provider: {id}")
            }
            Self::EmptyAnnotationId => write!(f, "render annotation id is empty"),
            Self::DuplicateAnnotationId(id) => write!(f, "duplicate render annotation: {id}"),
            Self::NonFiniteCoordinate { id } => {
                write!(f, "render annotation {id} has a non-finite coordinate")
            }
            Self::TooManyAnnotations { count } => {
                write!(f, "render annotation count exceeds limit: {count}")
            }
            Self::AnnotationTextTooLarge { id, size } => {
                write!(f, "render annotation {id} text exceeds limit: {size} bytes")
            }
            Self::InvalidXmlCharacter { id, codepoint } => write!(
                f,
                "render annotation {id} contains invalid XML character U+{codepoint:04X}"
            ),
            Self::InvalidCollisionMetrics { id } => {
                write!(f, "render annotation {id} has invalid collision metrics")
            }
            Self::CollisionResolution { id } => {
                write!(f, "render annotation {id} could not be collision-resolved")
            }
        }
    }
}

impl std::error::Error for RenderAnnotationError {}

impl Default for SvgRenderOptions {
    fn default() -> Self {
        Self {
            width: 900.0,
            staff_size: 24.0,
            measures_per_system: 4,
            interactive: true,
        }
    }
}

/// Errors returned by [`render_svg`] / [`render_svg_with_layout`].
///
/// These are all system-boundary validation failures — an arbitrary [`Score`] can reference
/// notation this renderer does not (yet) support. Never fails silently: unsupported notation or
/// an accidental beyond double-sharp/double-flat is reported, not dropped or approximated.
#[derive(Debug, Clone, PartialEq, Eq)]
pub enum RenderError {
    /// The score has no staves to render (no parts, or no parts with staves).
    EmptyScore,
    /// A requested system row does not exist in the supplied layout.
    InvalidRow { row: usize },
    /// A linked score view could not be resolved to a renderable score snapshot.
    ViewResolution { view_id: String, reason: String },
    /// The score and precomputed layout do not have compatible indices.
    InvalidLayout { reason: String },
    /// Rendering dimensions or system settings are not finite and positive.
    InvalidOptions { reason: String },
    /// A staff uses a clef this renderer has no staff-position mapping for.
    UnsupportedClef,
    /// A pitch's `alter` is outside the supported range (`-2..=2`: double-flat..double-sharp).
    UnsupportedAccidental { alter: i8 },
    /// A tablature position cannot be represented by the owning staff.
    InvalidTabPosition { string: u8, lines: u8 },
    /// Tablature glyph metrics overflowed before SVG emission.
    TabMetricsOverflow,
    /// A measure-level styled text entry exceeds the bounded renderer input size.
    MeasureTextTooLarge { size: usize },
    /// A measure-level styled text offset is not finite or cannot fit SVG coordinates.
    InvalidMeasureTextOffset { field: &'static str },
    /// A note-level MusicXML placement offset is not finite or cannot fit SVG coordinates.
    InvalidNotePlacement { field: &'static str },
    /// A score text contains a character that XML 1.0 cannot represent.
    InvalidXmlCharacter { codepoint: u32 },
    /// Host-provided annotation validation failed.
    Annotation(RenderAnnotationError),
}

impl std::fmt::Display for RenderError {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        match self {
            RenderError::EmptyScore => write!(f, "score has no staves to render"),
            RenderError::InvalidRow { row } => write!(f, "layout row {row} does not exist"),
            RenderError::ViewResolution { view_id, reason } => {
                write!(f, "cannot resolve score view '{view_id}': {reason}")
            }
            RenderError::InvalidLayout { reason } => write!(f, "invalid layout: {reason}"),
            RenderError::InvalidOptions { reason } => write!(f, "invalid render options: {reason}"),
            RenderError::UnsupportedClef => write!(
                f,
                "unsupported clef (no staff-position mapping is available)"
            ),
            RenderError::UnsupportedAccidental { alter } => {
                write!(
                    f,
                    "unsupported accidental alter={alter} (supported range is -2..=2)"
                )
            }
            RenderError::InvalidTabPosition { string, lines } => write!(
                f,
                "tablature string {string} is outside the owning staff line range 1..={lines}"
            ),
            RenderError::TabMetricsOverflow => write!(f, "tablature glyph metrics overflowed"),
            RenderError::MeasureTextTooLarge { size } => write!(
                f,
                "measure-level text is too large ({size} bytes; maximum is {MAX_ANNOTATION_TEXT_BYTES})"
            ),
            RenderError::InvalidMeasureTextOffset { field } => {
                write!(f, "measure-level text offset {field} is not finite")
            }
            RenderError::InvalidNotePlacement { field } => {
                write!(f, "note placement offset {field} is not finite")
            }
            RenderError::InvalidXmlCharacter { codepoint } => write!(
                f,
                "score text contains invalid XML character U+{codepoint:04X}"
            ),
            RenderError::Annotation(error) => write!(f, "invalid render annotation: {error}"),
        }
    }
}

impl std::error::Error for RenderError {}

/// Compute layout with `options` and render `score` to an SVG string.
///
/// Equivalent to calling [`acorde_layout::compute_layout`] with a [`LayoutConfig`] built from
/// `options.measures_per_system`, then [`render_svg_with_layout`]. Use `render_svg_with_layout`
/// directly when you already have a [`LayoutResult`] (e.g. computed once and reused, or built
/// with non-default `LayoutConfig` fields such as `concert_pitch`).
pub fn render_svg(score: &Score, options: &SvgRenderOptions) -> Result<String, RenderError> {
    let config = LayoutConfig {
        measures_per_row: options.measures_per_system.max(1),
        ..Default::default()
    };
    let layout = compute_layout(score, &config);
    render_svg_with_layout(score, &layout, options)
}

/// Render one linked [`ScoreView`] without mutating the source [`Score`].
///
/// The view's selected parts, written/concert-pitch projection, and staff-kind overrides are
/// resolved by `acorde-core`. Its `measures_per_row` override takes precedence over
/// [`SvgRenderOptions::measures_per_system`]. `ViewStyleProperty::StaffSpace` scales the
/// renderer's base staff size; the remaining typed style values stay available to hosts through
/// [`ScoreView::layout`] because they describe text and page-system policy rather than SVG glyph
/// geometry.
pub fn render_svg_view(
    score: &Score,
    view_id: &str,
    options: &SvgRenderOptions,
) -> Result<String, RenderError> {
    let view = score
        .views
        .iter()
        .find(|view| view.id == view_id)
        .ok_or_else(|| RenderError::ViewResolution {
            view_id: view_id.to_owned(),
            reason: "view does not exist".into(),
        })?;
    let projected = score
        .resolve_view(view_id)
        .map_err(|error| RenderError::ViewResolution {
            view_id: view_id.to_owned(),
            reason: error.to_string(),
        })?;
    let options = options_for_view(options, view)?;
    render_svg(&projected, &options)
}

/// Return browser metadata for a resolved linked view without requiring a host to duplicate its
/// layout/style resolution rules.
pub fn render_svg_view_metadata(
    score: &Score,
    view_id: &str,
    options: &SvgRenderOptions,
) -> Result<RenderMetadata, RenderError> {
    let view = score
        .views
        .iter()
        .find(|view| view.id == view_id)
        .ok_or_else(|| RenderError::ViewResolution {
            view_id: view_id.to_owned(),
            reason: "view does not exist".into(),
        })?;
    let projected = score
        .resolve_view(view_id)
        .map_err(|error| RenderError::ViewResolution {
            view_id: view_id.to_owned(),
            reason: error.to_string(),
        })?;
    let options = options_for_view(options, view)?;
    let layout = compute_layout(
        &projected,
        &LayoutConfig {
            measures_per_row: options.measures_per_system,
            ..Default::default()
        },
    );
    let mut metadata = render_svg_metadata(&projected, &layout, &options)?;
    metadata.view = Some(RenderedViewMetadata {
        id: view.id.clone(),
        name: view.name.clone(),
        measures_per_system: options.measures_per_system,
        style: score.resolved_view_style(&view.layout),
    });
    Ok(metadata)
}

fn options_for_view(
    options: &SvgRenderOptions,
    view: &ScoreView,
) -> Result<SvgRenderOptions, RenderError> {
    validate_style_overrides(&view.layout.typed_style_overrides)?;
    let mut resolved = options.clone();
    if let Some(measures_per_row) = view.layout.measures_per_row {
        resolved.measures_per_system = measures_per_row;
    }
    resolved.staff_size *= view.layout.resolved_style().staff_space;
    Ok(resolved)
}

fn options_for_score(
    options: &SvgRenderOptions,
    score: &Score,
) -> Result<SvgRenderOptions, RenderError> {
    validate_style_overrides(&score.style_overrides)?;
    if acorde_core::validate(score)
        .errors
        .iter()
        .any(|error| matches!(error, ValidationError::InvalidObjectStyleOverride { .. }))
    {
        return Err(RenderError::InvalidOptions {
            reason: "object style overrides must target an existing object and use valid values and provenance".into(),
        });
    }
    let mut resolved = options.clone();
    resolved.staff_size *= score.resolved_view_style(&Default::default()).staff_space;
    Ok(resolved)
}

fn validate_style_overrides(overrides: &[ViewStyleOverride]) -> Result<(), RenderError> {
    if overrides
        .iter()
        .any(|override_| !override_.value.is_finite() || !(0.05..=64.0).contains(&override_.value))
    {
        return Err(RenderError::InvalidOptions {
            reason: "typed style override values must be finite and within 0.05..=64".into(),
        });
    }
    Ok(())
}

/// Render `score` to an SVG string using an already-computed [`LayoutResult`].
///
/// `layout` must have been computed from `score` (or a structurally identical score) —
/// mismatched inputs produce undefined visual output, not a panic.
pub fn render_svg_with_layout(
    score: &Score,
    layout: &LayoutResult,
    options: &SvgRenderOptions,
) -> Result<String, RenderError> {
    let options = options_for_score(options, score)?;
    render::build_svg(score, layout, &options)
}

/// Render a score and append deterministic, host-provided annotations.
///
/// Annotation providers are sorted by [`RenderAnnotation::id`]. The returned marks are sorted by
/// their stable annotation IDs and serialized with XML escaping; arbitrary SVG fragments are not
/// accepted. An empty provider list has the same output as [`render_svg_with_layout`].
pub fn render_svg_with_annotations(
    score: &Score,
    layout: &LayoutResult,
    options: &SvgRenderOptions,
    providers: &[&dyn RenderAnnotation],
) -> Result<String, RenderError> {
    let options = options_for_score(options, score)?;
    let (mut svg, metadata) = render::build_svg_with_metadata(score, layout, &options)?;
    let annotations = render::collect_annotations(score, layout, &metadata, providers)
        .map_err(RenderError::Annotation)?;
    if annotations.is_empty() {
        return Ok(svg);
    }
    let body = annotations
        .iter()
        .map(|annotation| {
            format!(
                r#"<text class="acorde-render-annotation" data-acorde-kind="render-annotation" data-acorde-annotation-id="{}" x="{}" y="{}">{}</text>"#,
                render::escape_xml(&annotation.id),
                annotation.x,
                annotation.y,
                render::escape_xml(&annotation.text),
            )
        })
        .collect::<String>();
    let marker = "</g></svg>";
    let insertion = svg
        .rfind(marker)
        .ok_or_else(|| RenderError::InvalidLayout {
            reason: "renderer output has no score root".into(),
        })?;
    svg.insert_str(insertion, &body);
    Ok(svg)
}

/// Render one system row from a precomputed layout. The returned SVG contains only that row
/// and uses the same deterministic renderer as full-score output.
pub fn render_svg_row(
    score: &Score,
    layout: &LayoutResult,
    row: usize,
    options: &SvgRenderOptions,
) -> Result<String, RenderError> {
    let mut subset = layout.clone();
    subset.rows = vec![
        layout
            .rows
            .get(row)
            .cloned()
            .ok_or(RenderError::InvalidRow { row })?,
    ];
    render_svg_with_layout(score, &subset, options)
}

/// Return SVG dimensions and approximate hit-test bounds keyed by stable score addresses.
pub fn render_svg_metadata(
    score: &Score,
    layout: &LayoutResult,
    options: &SvgRenderOptions,
) -> Result<RenderMetadata, RenderError> {
    let options = options_for_score(options, score)?;
    render::build_svg_with_metadata(score, layout, &options).map(|(_, metadata)| metadata)
}
