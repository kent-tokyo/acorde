//! Platform-agnostic music score data model and command engine (undo/redo), with zero I/O
//! and zero rendering. Pairs with [`acorde-io`](https://docs.rs/acorde-io) for MusicXML/MIDI/ABC
//! parsing and [`acorde-layout`](https://docs.rs/acorde-layout) for logical score layout.

pub mod model;

pub use model::accordion_dynamics::{
    AccordionDynamicSchedule, AccordionDynamicSection, DynamicSectionedAccordionResult,
    arrange_sections_with_dynamics,
};
pub use model::accordion_response::{
    AccordionResponseFill, AccordionResponseOptions, ResponseFilledAccordionResult,
    arrange_with_response_fills,
};
pub use model::accordion_sections::{
    AccordionAccompanimentSection, AccordionMelodicSchedule, AccordionMelodicSection,
    MelodicSectionedAccordionResult, SectionedAccordionOptions, SectionedAccordionResult,
    arrange_sections_with_melodic_voices, arrange_with_sections,
};
pub use model::accordion_voicing::{
    AccordionVoicing, AccordionVoicingOptions, VoiceLedAccordionResult,
    VoiceLedSectionedAccordionResult, arrange_sections_with_voice_leading,
    arrange_with_voice_leading,
};

pub use model::arrange::{
    AccordionAnalysis, AccordionBassMode, AccordionDroneOptions, AccordionDroneResult,
    AccordionInversion, AccordionLossPolicy, AccordionMelodicVoice, AccordionPattern,
    AccordionReductionDiagnostic, AccordionReductionOptions, AccordionReductionResult,
    AccordionSourceMapping, AccordionSwingPolicy, ArrangeResult, GuidedAccordionOptions,
    GuidedAccordionResult, GuidedArrangeDiagnostic, LayeredAccordionOptions,
    LayeredAccordionResult, PartCandidate, PositionedAccordionChord, analyze_for_accordion,
    arrange_for_accordion, arrange_for_accordion_with_options, arrange_with_drone,
    arrange_with_explicit_chords, arrange_with_layers,
};
pub use model::arrange_presets::{
    AccordionLayerConfig, AccordionLayerResult, AccordionLayerShuffleOptions,
    AccordionPresetCapabilities, AccordionPresetConfig, AccordionPresetDescription,
    AccordionPresetResult, AccordionPresetSelection, AccordionPreviousConfig,
    AccordionShuffleOptions, accordion_preset_capabilities, arrange_accordion_layers,
    arrange_accordion_preset, arrange_from_resolved_layers, arrange_from_resolved_preset,
    shuffle_accordion_layers, shuffle_accordion_preset,
};
pub use model::arrangement_proposals::{
    ARRANGEMENT_PROPOSAL_CONTRACT_VERSION, ArrangementProposalCandidate,
    ArrangementProposalDiagnostic, ArrangementProposalFeatures, ArrangementProposalReplayResult,
    ArrangementProposalRequest, ArrangementProposalResult, ArrangementRegionRequest,
    ArrangementRegionResult, ResolvedArrangementProposal, regenerate_arrangement_region,
    replay_arrangement_proposal, suggest_arrangement_proposals,
};
pub use model::authored_chords::{
    AuthoredChordAnnotation, AuthoredChordDiagnostic, AuthoredChordOptions, AuthoredChordResult,
    annotate_from_authored_chords,
};
pub use model::bass_line::{
    BassLineDiagnostic, BassLineMotionReport, BassLineOptions, BassLinePattern, BassLineResult,
    arrange_bass_line,
};
pub use model::change_hint::{ChangeHint, ChangeScope};
pub use model::chord_plan::{
    BeatFraction, CHORD_PLAN_CONTRACT_VERSION, ChordPlan, ChordPlanCoverage, ChordPlanDiagnostic,
    ChordPlanEntry, ChordPlanExtractionMode, ChordPlanExtractionOptions, ChordPlanExtractionResult,
    ChordPlanOrigin, ChordPlanPosition, ChordPlanPresetResult, ChordPlanSource,
    chord_plan_to_preset_selection, extract_chord_plan,
};
pub use model::chord_proposals::{
    CHORD_PROPOSAL_CONTRACT_VERSION, CHORD_PROPOSAL_MAX_MEASURES, ChordChangedSpan,
    ChordPresetAssessment, ChordPresetAssessmentResult, ChordProposalArrangementResult,
    ChordProposalCandidate, ChordProposalDiagnostic, ChordProposalRequest, ChordProposalResult,
    ResolvedChordProposal, SourceRangeViolation, arrange_from_chord_proposal, assess_chord_presets,
    suggest_chord_plans, suggest_chord_plans_at_positions, suggest_chord_plans_with_locks,
};
pub use model::chordal_pad::{
    ChordalPadDiagnostic, ChordalPadMeterGroupSection, ChordalPadMotionReport, ChordalPadOptions,
    ChordalPadPattern, ChordalPadPatternSection, ChordalPadResolvedVoicing, ChordalPadResult,
    ChordalPadVoicing, arrange_chordal_pad,
};
pub use model::commands::{
    AddHairpinCmd, AddMeasureCmd, AddNoteCmd, AddPartCmd, AddPedalCmd, AddPitchCmd, AddSpannerCmd,
    AddStaffCmd, BatchCmd, Command, CommandStack, CycleEnharmonicSpellingCmd, DeleteMeasureCmd,
    DeleteNoteCmd, DeletePartCmd, DeleteStaffCmd, DurationScale, ExchangeVoicesCmd,
    ExplodeChordPitchesCmd, ExplodeVoicesCmd, ImplodeStavesCmd, JoinMeasuresCmd,
    MoveOrCopyVoiceRangeCmd, NewScoreCmd, PasteRangeCmd, PasteScoreFragmentCmd, PasteVoiceCmd,
    RealizeChordSymbolsCmd, RemoveScoreViewCmd, RemoveSpannerCmd, RemoveTrailingEmptyMeasuresCmd,
    ReorderPartsCmd, ResequenceRehearsalMarksCmd, RespellScoreCmd, RespellScoreToKeyCmd,
    RespellStaffRegionCmd, ScaleVoiceRangeCmd, ScoreFragmentPastePolicy, SetArpeggioCmd,
    SetBarlineCmd, SetChordSymbolCmd, SetClefCmd, SetCrossStaffCmd, SetCueCmd, SetDurationCmd,
    SetDynamicCmd, SetExpressionTextCmd, SetFiguredBassCmd, SetFingeringCmd, SetFingeringsCmd,
    SetGlissandoCmd, SetGraceCmd, SetGuitarBendAlterCmd, SetGuitarBendCurveCmd,
    SetGuitarTechniqueCmd, SetHarmonyRangeCmd, SetHarpPedalDiagramsCmd, SetHiddenCmd,
    SetInstrumentDefinitionCmd, SetInstrumentIdCmd, SetKeySignatureCmd, SetLyricCmd,
    SetMeasureInstrumentChangeCmd, SetMeasureTablatureChangeCmd, SetMeasureTextCmd, SetMetadataCmd,
    SetMidMeasureClefsCmd, SetMidiInstrumentCmd, SetMultiRestCmd, SetNavigationMarkCmd,
    SetNoteHeadCmd, SetNotePlacementCmd, SetObjectStyleOverridesCmd, SetOttavaCmd, SetPageBreakCmd,
    SetPartGroupCmd, SetPartNameCmd, SetPercussionKitCmd, SetRehearsalMarkCmd,
    SetScoreStyleOverridesCmd, SetScoreTextCmd, SetSectionBreakCmd, SetStaffPresentationCmd,
    SetStemCmd, SetStringNumberCmd, SetSystemBreakCmd, SetSystemBreakIntervalCmd,
    SetTabPositionCmd, SetTablatureConfigCmd, SetTechniqueTextCmd, SetTempoAtMeasureCmd,
    SetTempoCmd, SetTimeSignatureCmd, SetTransposeCmd, SetTupletCmd, SetUnpitchedCmd, SetVoltaCmd,
    SplitMeasureCmd, ToggleArticulationCmd, ToggleSlurCmd, ToggleTieCmd, ToggleTrillLineCmd,
    TransposeStaffRegionCmd, TupletScalePolicy, UnrollRepeatsCmd, UpdateSpannerCmd,
    UpsertScoreViewCmd, command_key, command_label,
};
pub use model::countermelody::{
    CountermelodyDiagnostic, CountermelodyOptions, CountermelodyResult, arrange_countermelody,
};
pub use model::duration::Duration;
pub use model::engine::{EngineHistory, HistoryConflict, HistoryRelation, ScoreEngine};
pub use model::ensemble_melody::{
    EnsembleMelodyDiagnostic, EnsembleMelodyRequest, EnsembleMelodyResult, MelodyRoleSection,
    arrange_ensemble_with_melody,
};
pub use model::ensemble_roles::{
    EnsembleMeasureActivity, EnsembleRole, EnsembleRoleDiagnostic, EnsembleRoleOptions,
    EnsembleRolePart, EnsembleRoleResult, EnsembleRoleSection, arrange_ensemble_roles,
};
pub use model::fragment::{
    MIN_SUPPORTED_SCORE_FRAGMENT_CONTRACT_VERSION, SCORE_FRAGMENT_CONTRACT_VERSION, ScoreFragment,
    ScoreFragmentCrossStaffTarget, ScoreFragmentDiagnostic, ScoreFragmentMeasure,
    ScoreFragmentMeasureAttributes, ScoreFragmentSelection, ScoreFragmentVoice,
    extract_score_fragment,
};
pub use model::free_bass::{
    AccordionHandLimits, FreeBassDiagnostic, FreeBassOptions, FreeBassPitchMapping, FreeBassResult,
    split_for_free_bass,
};
pub use model::gm::{drum_name, program_name};
pub use model::harmony::{detect_chord, roman_numeral, roman_numeral_figured};
pub use model::interval::{Interval, IntervalQuality};
pub use model::melodic_insert::{
    MelodicInsertion, MelodicInsertionCandidate, MelodicInsertionContext,
    MelodicInsertionContextResult, MelodicInsertionDiagnostic, MelodicInsertionKind,
    MelodicInsertionOptions, MelodicInsertionOutcome, MelodicInsertionResult,
    MelodicInsertionSelection, MelodicInsertionSource, MelodicLocalKey, MelodicSourceMapping,
    insert_melodic_notes, insert_melodic_notes_with_context,
};
pub use model::musical_diff::{
    MusicalChange, MusicalChangeKind, MusicalComparisonDiagnostic, MusicalComparisonOptions,
    MusicalComparisonResult, NoteCorrespondence, StructurePolicy, VoiceAddr, VoiceCorrespondence,
    compare_musical_positions,
};
pub use model::notation::{
    AccidentalDisplay, Articulation, Barline, BeamState, ChordBarre, ChordDefinition,
    ChordDefinitionMember, ChordDegree, ChordSymbol, Clef, CrossStaff, Dynamic, FiguredBassFigure,
    FiguredBassGroup, FingeringSelectionPolicy, GuitarTechnique, HairpinKind, KeySignature, Lyric,
    NoteHead, OttavaKind, StyledText, TabPosition, TablatureConfig, TextStyle, TimeSignature,
    TimeSymbol, TupletBoundary, TupletInfo, VerseLyric,
};
pub use model::omr::{
    MAX_OMR_ANNOTATIONS, MAX_OMR_EVENTS_PER_MEASURE, MAX_OMR_MEASURES, MAX_OMR_PAGES,
    OMR_PROPOSAL_CONTRACT_VERSION, OmrCommandPlan, OmrDiagnostic, OmrDiagnosticKind,
    OmrMeasureProposal, OmrMeasureTarget, OmrNoteAnnotation, OmrNoteAnnotationProposal,
    OmrNoteTarget, OmrPage, OmrProposal, OmrRegion, OmrRhythmicEvent, OmrUnresolvedItem,
};
pub use model::pitch::{Pitch, Step};
pub use model::playback::{
    DynamicTimeline, MAX_PLAYBACK_COMPARISON_EVENTS, MAX_TAB_PERFORMANCE_EVENTS, MetronomeConfig,
    OFFLINE_RENDER_CONTRACT_VERSION, OfflineRenderDiagnostic, OfflineRenderDiagnosticKind,
    OfflineRenderFormat, OfflineRenderFrameEvent, OfflineRenderManifest,
    OfflineRenderMeasureAddress, OfflineRenderNavigationPolicy, OfflineRenderRequest,
    OfflineRenderResult, OfflineRenderScope, OfflineRenderSemanticEventKind,
    OfflineRenderSemanticFrameEvent, PLAYBACK_COMPARISON_CONTRACT_VERSION,
    PLAYBACK_ROUTING_CONTRACT_VERSION, PLAYBACK_TIMING_CORPUS_CONTRACT_VERSION, PlaybackAuxSend,
    PlaybackBusRoute, PlaybackEffectRoute, PlaybackEvent, PlaybackOptions, PlaybackPosition,
    PlaybackRealizationProfile, PlaybackRoutingConfig, PlaybackRoutingManifest, PlaybackTimingCase,
    PlaybackTimingCaseReport, PlaybackTimingCorpusReport, PlaybackTimingMismatch,
    PlaybackTimingReport, PlaybackTimingTolerance, SharedUnisonPlaybackEvent,
    TAB_PERFORMANCE_CONTRACT_VERSION, TAB_ROUND_TRIP_CONTRACT_VERSION,
    TablaturePerformanceDiagnostic, TablaturePerformanceEvent, TablaturePerformanceReport,
    TablatureRoundTripDiagnostic, TablatureRoundTripReport, build_offline_render_manifest,
    build_playback_routing_manifest, coalesce_shared_unisons, compare_playback_timing,
    compute_playback_position, evaluate_playback_timing_corpus, playback_fermata_pauses,
    project_tablature_performance, tablature_round_trip_report, to_playback_events,
    to_playback_events_bounded,
};
pub use model::repeat::measure_sequence;
pub use model::scale::{Scale, ScaleKind};
pub use model::score::{
    GuitarBendPoint, HarpPedalDiagram, HarpPedalPosition, InstrumentDefinition, InstrumentRange,
    Measure, MeasureLength, MetronomeMark, MidMeasureClef, MidiAftertouch, MidiControlChange,
    MidiPitchBend, MidiProgramChange, NamedInstrumentChannel, NotationSpanner, NotationSpannerKind,
    Note, NoteAddr, ObjectStyleOverride, ObjectStyleTarget, Part, PartGroup, PartGroupSymbol,
    PercussionInstrument, RegionalTranspositionTarget, RespellPolicy, Score, ScoreChange,
    ScoreMetadata, ScorePatch, ScoreSettings, ScoreStats, ScoreTemplate, ScoreView,
    ScoreViewLayoutOverrides, Staff, StaffGroup, StaffKind, StaffNoteheadScheme, StaffPresentation,
    StyleImportProvenance, TablatureFretMarkStyle, TablatureRhythmDisplay, TieSourceLocation,
    TimedPedalDirection, ViewStaffKindOverride, ViewStaffRef, ViewStyle, ViewStyleOverride,
    ViewStyleProperty, ViewTranspositionPolicy, VoiceChannelSwitch, VoltaBracket, apply_patch,
    assign_tablature_positions, compute_beams, diff, measure_beats_remaining,
    optimize_tablature_positions, respell_score, respell_score_to_key, respell_staff_region,
    score_duration_secs, score_duration_secs_region, score_patch, staff_ottava_octave_shifts,
    suggested_stem_up, transpose, transpose_checked, transpose_staff_region_checked,
    tuplet_group_ranges, voice_duration_beats,
};
pub use model::structural_plan::{
    STRUCTURAL_CHANGE_PLAN_CONTRACT_VERSION, StructuralChangeDiagnostic,
    StructuralChangeDiagnosticKind, StructuralChangePlan, StructuralMeasureStructure,
    plan_structural_change,
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
