//! Logical, pixel-free score layout engine for [`acorde-core`](https://docs.rs/acorde-core)
//! scores — computes row breaks, multi-rest collapsing, beam groups, tuplet groups, courtesy
//! accidentals, and span (hairpin/ottava/pedal/slur) resolution for score renderers (e.g. VexFlow).

mod engine;
mod print;

pub use acorde_core::NoteAddr;
pub use engine::compute_layout;
pub use print::{
    BreakReason, CropMarkPolicy, FinalPagePolicy, GlyphResourcePolicy, KeepTogetherRange,
    MeasureMark, NotationBreakPolicy, PageAddress, PageLayout, PageNumbering, PageOrientation,
    PageSpanSegment, PaperSize, PickupPolicy, PrintColorPolicy, PrintConfig, PrintLayoutError,
    PrintLayoutResult, SpanSegment, SystemAddress, SystemLayout, compute_print_layout,
};

use acorde_core::{HairpinKind, OttavaKind};
use serde::{Deserialize, Serialize};

/// Configuration for a layout pass.
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct LayoutConfig {
    /// How many visual measure-columns fit on one row/system.
    pub measures_per_row: usize,
    /// When `true`, key signatures in [`LayoutResult::concert_key_overrides`] reflect
    /// concert pitch for transposing instruments.
    #[serde(default)]
    pub concert_pitch: bool,
    /// Override for the number of measures on the first system row only.
    /// When `None`, falls back to [`measures_per_row`].
    /// Useful when the first system is shorter due to clef/key/time signature headers.
    #[serde(default)]
    pub first_row_measures: Option<usize>,
}

impl Default for LayoutConfig {
    fn default() -> Self {
        Self {
            measures_per_row: 4,
            concert_pitch: false,
            first_row_measures: None,
        }
    }
}

/// A resolved span between two note addresses.
#[derive(Debug, Clone, Serialize, Deserialize)]
pub enum SpanMark {
    Hairpin {
        kind: HairpinKind,
        start: NoteAddr,
        end: NoteAddr,
    },
    Ottava {
        kind: OttavaKind,
        start: NoteAddr,
        end: NoteAddr,
    },
    Pedal {
        start: NoteAddr,
        end: NoteAddr,
    },
    Slur {
        start: NoteAddr,
        end: NoteAddr,
    },
    TrillLine {
        start: NoteAddr,
        end: NoteAddr,
    },
    Glissando {
        start: NoteAddr,
        end: NoteAddr,
    },
}

/// One horizontal row (system) of measures.
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct RowLayout {
    /// Ordered list of physical measure indices that appear on this row.
    pub measure_indices: Vec<usize>,
}

/// Concert-pitch key signature override for a specific staff of a transposing instrument.
///
/// Populated when `LayoutConfig::concert_pitch` is `true` and the staff has a non-zero
/// `transpose_semitones`. Renderers use this to draw the correct key signature.
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct ConcertKeyOverride {
    pub part_index: usize,
    pub staff_index: usize,
    /// Key signature in fifths (−7 … +7) adjusted to concert pitch.
    pub fifths: i8,
}

/// A group of beamed notes within a single voice of a measure.
///
/// `note_indices` are 0-based positions within `score.parts[part].staves[staff]
/// .measures[measure].voices[voice]`.
///
/// Consumers (e.g. VexFlow) use this to explicitly specify beam groupings rather than
/// relying on automatic detection, which can produce incorrect results for complex rhythms.
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct BeamGroup {
    pub part: usize,
    pub staff: usize,
    pub measure: usize,
    pub voice: usize,
    /// Ordered note indices within the voice that form this beam group.
    pub note_indices: Vec<usize>,
}

/// A group of notes forming one tuplet bracket within a single voice of one measure.
///
/// `note_indices` are 0-based positions within the voice. `actual_notes` and `normal_notes`
/// mirror [`TupletInfo`] for direct use in VexFlow tuplet rendering.
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct TupletGroup {
    pub part: usize,
    pub staff: usize,
    pub measure: usize,
    pub voice: usize,
    /// Ordered note indices within the voice, in order.
    pub note_indices: Vec<usize>,
    /// Number of notes in the tuplet (e.g. 3 for a triplet).
    pub actual_notes: u8,
    /// Normal beat count displaced (e.g. 2 for a triplet fitting in 2 beats).
    pub normal_notes: u8,
}

/// A courtesy (cautionary) accidental to display in parentheses.
///
/// Emitted when the same pitch (step + octave) was chromatically altered
/// in the immediately preceding measure and the renderer needs to remind the
/// performer that the alteration no longer applies.
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct CourtesyAccidental {
    pub part: usize,
    pub staff: usize,
    pub measure: usize,
    pub voice: usize,
    pub note_index: usize,
    /// Index within `note.pitches` (0 for single-pitch notes, ≥1 for chords).
    pub pitch_index: usize,
    /// Accidental to display: 0 = natural, 1 = sharp, -1 = flat, 2 = double-sharp, -2 = double-flat.
    pub alter: i8,
}

/// A mandatory (non-courtesy) accidental that must be drawn beside a notehead.
///
/// Emitted the first time, within a measure, that a pitch (step + octave, scoped across
/// all voices of a staff — accidentals do not carry across barlines) differs from the
/// alteration established by the key signature or by an earlier note of the same
/// step+octave earlier in the same measure. This is standard music engraving, not a
/// rendering choice, so it is computed here rather than in a renderer.
///
/// When both an [`AccidentalMark`] and a [`CourtesyAccidental`] exist for the same
/// `(part, staff, measure, voice, note_index, pitch_index)`, the mandatory mark takes
/// precedence: renderers should draw it plain and suppress the courtesy parentheses.
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct AccidentalMark {
    pub part: usize,
    pub staff: usize,
    pub measure: usize,
    pub voice: usize,
    pub note_index: usize,
    /// Index within `note.pitches` (0 for single-pitch notes, ≥1 for chords).
    pub pitch_index: usize,
    /// Accidental to display: 0 = natural, 1 = sharp, -1 = flat, 2 = double-sharp, -2 = double-flat.
    pub alter: i8,
}

/// The result of a layout pass.
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct LayoutResult {
    /// Maps each visual column index to a physical measure index.
    ///
    /// Each entry `vis_slots[v]` is the physical measure index for visual column `v`.
    /// Non-multi-rest measures each contribute exactly one entry. A measure with
    /// `multi_rest_count = N` contributes `N` consecutive entries all equal to that
    /// measure's physical index.
    ///
    /// Therefore `vis_slots.len()` equals the **total number of visual columns** —
    /// which is ≥ the number of physical measures (equal when no multi-rests are
    /// present, and greater when multi-rests expand visual space).
    ///
    /// Example: 3 physical measures where measure 0 has `multi_rest_count = Some(4)`
    /// produces `vis_slots = [0, 0, 0, 0, 1, 2]` — six visual columns.
    pub vis_slots: Vec<usize>,

    /// Each row in display order; rows cover all parts simultaneously.
    pub rows: Vec<RowLayout>,

    /// Fully resolved span marks (hairpin / ottava / pedal start+end pairs).
    pub spans: Vec<SpanMark>,

    /// Per-staff concert-pitch key signature overrides.
    /// Non-empty only when `LayoutConfig::concert_pitch` is `true` and at least one staff
    /// has a non-zero `transpose_semitones`.
    #[serde(default)]
    pub concert_key_overrides: Vec<ConcertKeyOverride>,

    /// Beam groups across all parts, staves, measures, and voices.
    ///
    /// Derived from `BeamState` flags on individual notes. Each group contains at least
    /// two note indices. Groups are ordered by (part, staff, measure, voice).
    #[serde(default)]
    pub beam_groups: Vec<BeamGroup>,

    /// Tuplet groups across all parts, staves, measures, and voices.
    ///
    /// Each group represents one tuplet bracket. Notes in a group share the same
    /// `TupletInfo`. Groups are ordered by (part, staff, measure, voice).
    #[serde(default)]
    pub tuplet_groups: Vec<TupletGroup>,

    /// Courtesy (cautionary) accidentals across all parts, staves, measures, and voices.
    ///
    /// A courtesy accidental is emitted when the same pitch (step + octave) was chromatically
    /// altered in the immediately preceding measure, reminding the performer the alteration
    /// no longer applies. Ordered by (part, staff, measure, voice, note_index, pitch_index).
    #[serde(default)]
    pub courtesy_accidentals: Vec<CourtesyAccidental>,

    /// Mandatory (non-courtesy) accidentals across all parts, staves, measures, and voices.
    ///
    /// Emitted for the first chromatic alteration of a step+octave within a measure.
    /// See [`AccidentalMark`] for the precedence rule against `courtesy_accidentals`.
    #[serde(default)]
    pub accidentals: Vec<AccidentalMark>,
}
