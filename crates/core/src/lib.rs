//! Platform-agnostic music score data model and command engine (undo/redo), with zero I/O
//! and zero rendering. Pairs with [`acorde-io`](https://docs.rs/acorde-io) for MusicXML/MIDI/ABC
//! parsing and [`acorde-layout`](https://docs.rs/acorde-layout) for logical score layout.

pub mod model;

pub use model::arrange::{
    AccordionAnalysis, ArrangeResult, PartCandidate, analyze_for_accordion, arrange_for_accordion,
};
pub use model::change_hint::{ChangeHint, ChangeScope};
pub use model::commands::{
    AddHairpinCmd, AddMeasureCmd, AddNoteCmd, AddPartCmd, AddPedalCmd, AddPitchCmd, AddSpannerCmd,
    AddStaffCmd, BatchCmd, Command, CommandStack, DeleteMeasureCmd, DeleteNoteCmd, DeletePartCmd,
    DeleteStaffCmd, NewScoreCmd, PasteRangeCmd, PasteVoiceCmd, RemoveScoreViewCmd,
    RemoveSpannerCmd, ReorderPartsCmd, RespellScoreCmd, RespellScoreToKeyCmd, SetArpeggioCmd,
    SetBarlineCmd, SetChordSymbolCmd, SetClefCmd, SetCrossStaffCmd, SetCueCmd, SetDurationCmd,
    SetDynamicCmd, SetExpressionTextCmd, SetFiguredBassCmd, SetFingeringCmd, SetFingeringsCmd,
    SetGlissandoCmd, SetGraceCmd, SetGuitarBendAlterCmd, SetGuitarBendCurveCmd,
    SetGuitarTechniqueCmd, SetHarmonyRangeCmd, SetHarpPedalDiagramsCmd, SetInstrumentDefinitionCmd,
    SetInstrumentIdCmd, SetKeySignatureCmd, SetLyricCmd, SetMeasureInstrumentChangeCmd,
    SetMeasureTablatureChangeCmd, SetMeasureTextCmd, SetMetadataCmd, SetMidiInstrumentCmd,
    SetMultiRestCmd, SetNavigationMarkCmd, SetNoteHeadCmd, SetNotePlacementCmd,
    SetObjectStyleOverridesCmd, SetOttavaCmd, SetPageBreakCmd, SetPartGroupCmd, SetPartNameCmd,
    SetPercussionKitCmd, SetRehearsalMarkCmd, SetScoreStyleOverridesCmd, SetScoreTextCmd,
    SetStaffPresentationCmd, SetStemCmd, SetStringNumberCmd, SetSystemBreakCmd, SetTabPositionCmd,
    SetTablatureConfigCmd, SetTechniqueTextCmd, SetTempoAtMeasureCmd, SetTempoCmd,
    SetTimeSignatureCmd, SetTransposeCmd, SetTupletCmd, SetUnpitchedCmd, SetVoltaCmd,
    ToggleArticulationCmd, ToggleSlurCmd, ToggleTieCmd, ToggleTrillLineCmd,
    TransposeStaffRegionCmd, UpdateSpannerCmd, UpsertScoreViewCmd, command_key, command_label,
};
pub use model::duration::Duration;
pub use model::engine::{EngineHistory, HistoryConflict, HistoryRelation, ScoreEngine};
pub use model::fragment::{
    SCORE_FRAGMENT_CONTRACT_VERSION, ScoreFragment, ScoreFragmentDiagnostic, ScoreFragmentMeasure,
    ScoreFragmentSelection, ScoreFragmentVoice, extract_score_fragment,
};
pub use model::gm::{drum_name, program_name};
pub use model::harmony::{detect_chord, roman_numeral};
pub use model::interval::{Interval, IntervalQuality};
pub use model::notation::{
    Articulation, Barline, BeamState, ChordBarre, ChordDefinition, ChordDefinitionMember,
    ChordDegree, ChordSymbol, Clef, CrossStaff, Dynamic, FiguredBassFigure,
    FingeringSelectionPolicy, GuitarTechnique, HairpinKind, KeySignature, Lyric, NoteHead,
    OttavaKind, StyledText, TabPosition, TablatureConfig, TextStyle, TimeSignature, TupletInfo,
};
pub use model::pitch::{Pitch, Step};
pub use model::playback::{
    MAX_PLAYBACK_COMPARISON_EVENTS, MAX_TAB_PERFORMANCE_EVENTS, MetronomeConfig,
    OFFLINE_RENDER_CONTRACT_VERSION, OfflineRenderDiagnostic, OfflineRenderDiagnosticKind,
    OfflineRenderFormat, OfflineRenderFrameEvent, OfflineRenderManifest,
    OfflineRenderMeasureAddress, OfflineRenderNavigationPolicy, OfflineRenderRequest,
    OfflineRenderResult, OfflineRenderScope, OfflineRenderSemanticEventKind,
    OfflineRenderSemanticFrameEvent, PLAYBACK_COMPARISON_CONTRACT_VERSION,
    PLAYBACK_ROUTING_CONTRACT_VERSION, PLAYBACK_TIMING_CORPUS_CONTRACT_VERSION, PlaybackAuxSend,
    PlaybackBusRoute, PlaybackEffectRoute, PlaybackEvent, PlaybackOptions, PlaybackPosition,
    PlaybackRealizationProfile, PlaybackRoutingConfig, PlaybackRoutingManifest, PlaybackTimingCase,
    PlaybackTimingCaseReport, PlaybackTimingCorpusReport, PlaybackTimingMismatch,
    PlaybackTimingReport, PlaybackTimingTolerance, TAB_PERFORMANCE_CONTRACT_VERSION,
    TAB_ROUND_TRIP_CONTRACT_VERSION, TablaturePerformanceDiagnostic, TablaturePerformanceEvent,
    TablaturePerformanceReport, TablatureRoundTripDiagnostic, TablatureRoundTripReport,
    build_offline_render_manifest, build_playback_routing_manifest, compare_playback_timing,
    compute_playback_position, evaluate_playback_timing_corpus, project_tablature_performance,
    tablature_round_trip_report, to_playback_events, to_playback_events_bounded,
};
pub use model::repeat::measure_sequence;
pub use model::scale::{Scale, ScaleKind};
pub use model::score::{
    GuitarBendPoint, HarpPedalDiagram, HarpPedalPosition, InstrumentDefinition, InstrumentRange,
    Measure, MidiAftertouch, MidiControlChange, MidiPitchBend, MidiProgramChange, NotationSpanner,
    NotationSpannerKind, Note, NoteAddr, ObjectStyleOverride, ObjectStyleTarget, Part, PartGroup,
    PartGroupSymbol, PercussionInstrument, RegionalTranspositionTarget, Score, ScoreChange,
    ScoreMetadata, ScorePatch, ScoreSettings, ScoreStats, ScoreTemplate, ScoreView,
    ScoreViewLayoutOverrides, Staff, StaffGroup, StaffKind, StaffNoteheadScheme, StaffPresentation,
    StyleImportProvenance, TablatureFretMarkStyle, TablatureRhythmDisplay, ViewStaffKindOverride,
    ViewStaffRef, ViewStyle, ViewStyleOverride, ViewStyleProperty, ViewTranspositionPolicy,
    VoltaBracket, apply_patch, assign_tablature_positions, compute_beams, diff,
    measure_beats_remaining, optimize_tablature_positions, respell_score, respell_score_to_key,
    score_duration_secs, score_duration_secs_region, score_patch, suggested_stem_up, transpose,
    transpose_checked, transpose_staff_region_checked,
};
pub use model::validate::{
    GuitarBendCurveValidationReason, InstrumentDefinitionValidationReason, InstrumentRangeKind,
    ObjectStyleValidationReason, PercussionInstrumentValidationReason, ScoreViewValidationReason,
    SpannerEndpoint, StaffPresentationValidationReason, TablatureValidationReason, ValidationError,
    ValidationReport, ValidationWarning, validate,
};

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
    #[error("score failed structural validation")]
    InvalidScore,
    #[error("engine history base score does not match the supplied score")]
    HistoryBaseMismatch,
    #[error("engine history is not a safe append-only extension")]
    HistoryNotAppendable,
    #[error("invalid playback comparison tolerance")]
    InvalidPlaybackComparison,
    #[error("invalid offline render request")]
    InvalidOfflineRenderRequest,
    #[error("invalid playback routing configuration")]
    InvalidPlaybackRouting,
    #[error("invalid playback timing corpus")]
    InvalidPlaybackTimingCorpus,
    #[error("playback comparison contains too many events ({0})")]
    PlaybackComparisonTooLarge(usize),
    #[error("tablature performance projection contains too many events ({0})")]
    TabPerformanceTooLarge(usize),
    #[error("tablature round-trip serialization failed: {0}")]
    TabRoundTripSerialization(String),
}
