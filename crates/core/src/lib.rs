pub mod model;

pub use model::arrange::{analyze_for_accordion, arrange_for_accordion, AccordionAnalysis, ArrangeResult, PartCandidate};
pub use model::score::{
    Measure, Note, NoteAddr, Part, PartGroup, PartGroupSymbol, Score, ScoreChange, ScoreMetadata, ScoreSettings, ScoreStats,
    ScoreTemplate, Staff, VoltaBracket, ScorePatch,
    diff, score_patch, apply_patch,
    transpose, respell_score, respell_score_to_key,
    score_duration_secs, score_duration_secs_region, measure_beats_remaining,
    suggested_stem_up, compute_beams,
};
pub use model::pitch::{Pitch, Step};
pub use model::duration::Duration;
pub use model::notation::{
    Articulation, Barline, BeamState, ChordSymbol, Clef, Dynamic, GuitarTechnique,
    HairpinKind, KeySignature, Lyric, NoteHead, OttavaKind, TimeSignature, TupletInfo,
};
pub use model::commands::{
    AddHairpinCmd, AddMeasureCmd, AddNoteCmd, AddPartCmd, AddPedalCmd, AddPitchCmd,
    AddStaffCmd, BatchCmd, Command, CommandStack, DeleteMeasureCmd, DeleteNoteCmd,
    DeletePartCmd, DeleteStaffCmd, NewScoreCmd,
    RespellScoreCmd, RespellScoreToKeyCmd, SetArpeggioCmd, SetBarlineCmd, SetChordSymbolCmd, SetClefCmd, SetDynamicCmd, SetGraceCmd,
    SetKeySignatureCmd, SetLyricCmd, SetMetadataCmd, SetMidiInstrumentCmd, SetMultiRestCmd,
    SetNavigationMarkCmd, SetOttavaCmd, SetPageBreakCmd, SetPartNameCmd, SetRehearsalMarkCmd,
    SetSystemBreakCmd, SetStemCmd, SetTempoCmd, SetTempoAtMeasureCmd, SetTimeSignatureCmd, SetTransposeCmd,
    SetTupletCmd, SetVoltaCmd, ToggleArticulationCmd, ToggleSlurCmd, ToggleTieCmd,
    PasteVoiceCmd, PasteRangeCmd, command_key, command_label,
    SetTechniqueTextCmd, SetFingeringCmd, SetStringNumberCmd, SetNoteHeadCmd, SetCueCmd,
    SetGuitarTechniqueCmd,
    SetExpressionTextCmd, ToggleTrillLineCmd, SetPartGroupCmd,
};
pub use model::change_hint::{ChangeHint, ChangeScope};
pub use model::engine::{EngineHistory, ScoreEngine};
pub use model::gm::{drum_name, program_name};
pub use model::harmony::{detect_chord, roman_numeral};
pub use model::interval::{Interval, IntervalQuality};
pub use model::playback::{
    MetronomeConfig, PlaybackEvent, PlaybackOptions, PlaybackPosition,
    compute_playback_position, to_playback_events,
};
pub use model::repeat::measure_sequence;
pub use model::scale::{Scale, ScaleKind};
pub use model::validate::{ValidationError, ValidationWarning, ValidationReport, validate};

/// Current Score JSON schema version produced by this crate.
pub const SCORE_SCHEMA_VERSION: u32 = 1;

#[derive(Debug, thiserror::Error)]
pub enum Error {
    #[error("part index {0} out of range")]
    PartNotFound(usize),
    #[error("staff index {0} out of range")]
    StaffNotFound(usize),
    #[error("measure index {0} out of range")]
    MeasureNotFound(usize),
    #[error("note index {0} out of range")]
    NoteNotFound(usize),
    #[error("voice index {0} out of range")]
    VoiceOutOfRange(usize),
    #[error("{0}")]
    InvalidCommand(String),
    #[error("nothing to undo")]
    NothingToUndo,
    #[error("nothing to redo")]
    NothingToRedo,
    #[error("clipboard is empty")]
    ClipboardEmpty,
    #[error("cannot delete the last staff of a part")]
    CannotDeleteLastStaff,
    #[error("invalid patch: {0}")]
    InvalidPatch(String),
}
