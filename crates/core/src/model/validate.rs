use super::gm::instrument_range;
use super::notation::GuitarTechnique;
use super::score::{
    InstrumentDefinition, InstrumentRange, NotationSpannerKind, NoteAddr, PercussionInstrument,
    Score, ScoreView, StaffKind,
};
use serde::{Deserialize, Serialize};
use std::collections::HashMap;

/// A structural error found by [`validate`].
#[derive(Debug, Clone, Serialize, Deserialize)]
pub enum ValidationError {
    /// The score has no parts to validate.
    EmptyScore,
    /// A part has no staves.
    PartWithoutStaves { part: usize },
    /// A staff has no measures.
    StaffWithoutMeasures { part: usize, staff: usize },
    /// Staves in one part do not cover the same number of measures.
    MeasureCountMismatch {
        part: usize,
        staff: usize,
        expected: usize,
        found: usize,
    },
    /// A time signature has an unsupported numerator or denominator.
    InvalidTimeSignature {
        part: usize,
        staff: usize,
        measure: usize,
        numerator: u8,
        denominator: u8,
    },
    /// Beat-count mismatch: the notes in a voice don't fill the time signature.
    BeatCount {
        part: usize,
        staff: usize,
        measure: usize,
        voice: usize,
        expected_beats: f64,
        found_beats: f64,
    },
    /// A note pitch lies outside the practical range for the part's GM instrument.
    OutOfRange {
        part_index: usize,
        staff_index: usize,
        measure_index: usize,
        note_index: usize,
        pitch_midi: u8,
        instrument_range: (u8, u8),
    },
    /// Tablature staff metadata is internally inconsistent.
    InvalidTablature {
        part: usize,
        staff: usize,
        reason: TablatureValidationReason,
    },
    /// Renderer-independent staff presentation is internally inconsistent.
    InvalidStaffPresentation {
        part: usize,
        staff: usize,
        reason: StaffPresentationValidationReason,
    },
    /// A part's stable instrument semantics are internally inconsistent.
    InvalidInstrumentDefinition {
        part: usize,
        reason: InstrumentDefinitionValidationReason,
    },
    /// An editable percussion-kit entry is internally inconsistent.
    InvalidPercussionInstrument {
        part: usize,
        instrument: usize,
        id: String,
        reason: PercussionInstrumentValidationReason,
    },
    InvalidScoreView {
        index: usize,
        id: String,
        reason: ScoreViewValidationReason,
    },
    /// A score-wide presentation default is outside the portable style range.
    InvalidScoreStyleOverride {
        property: super::score::ViewStyleProperty,
        value: f32,
    },
    /// An object-attached presentation override has an invalid target, value, or provenance.
    InvalidObjectStyleOverride {
        index: usize,
        reason: ObjectStyleValidationReason,
    },
    /// A note's explicit string is not present on its tablature staff.
    TabPositionOutOfRange {
        part: usize,
        staff: usize,
        measure: usize,
        voice: usize,
        note: usize,
        string: u8,
        lines: u8,
    },
    /// A pitch's microtonal cents component is outside the canonical -99..99 range.
    MicrotoneOutOfRange {
        part: usize,
        staff: usize,
        measure: usize,
        voice: usize,
        note: usize,
        pitch: usize,
        microtone_cents: i16,
    },
    InvalidGuitarBendCurve {
        part: usize,
        staff: usize,
        measure: usize,
        voice: usize,
        note: usize,
        reason: GuitarBendCurveValidationReason,
    },
    /// A harmony continuation points to a note address that does not exist.
    InvalidHarmonyRange {
        part: usize,
        staff: usize,
        measure: usize,
        voice: usize,
        note: usize,
        end: NoteAddr,
    },
    /// A typed notation span must have a non-empty unique stable identity.
    InvalidSpannerId { index: usize, id: String },
    /// A typed notation span has a duplicate stable identity.
    DuplicateSpannerId {
        first: usize,
        duplicate: usize,
        id: String,
    },
    /// A typed notation span endpoint does not point to a canonical note.
    InvalidSpannerEndpoint {
        index: usize,
        id: String,
        kind: NotationSpannerKind,
        endpoint: SpannerEndpoint,
        address: NoteAddr,
    },
}

/// Which endpoint of a typed notation spanner failed validation.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
pub enum SpannerEndpoint {
    Start,
    End,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub enum TablatureValidationReason {
    InvalidLineCount {
        lines: u8,
    },
    TooManyTunings {
        tuning_count: usize,
        lines: u8,
    },
    TuningOutOfMidiRange {
        index: usize,
        midi: i16,
    },
    ChangeWithoutBase {
        measure: usize,
    },
    ChangeLineCountMismatch {
        measure: usize,
        base_lines: u8,
        changed_lines: u8,
    },
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub enum StaffPresentationValidationReason {
    InvalidLineCount { lines: u8 },
    InvalidLineDistance { line_distance: f32 },
    TablatureWithoutConfig,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub enum InstrumentDefinitionValidationReason {
    EmptyId,
    InvalidStaffCount {
        staff_count: u8,
    },
    InvalidMidiChannel {
        midi_channel: u8,
    },
    InvalidRange {
        kind: InstrumentRangeKind,
        range: InstrumentRange,
    },
}

#[derive(Debug, Clone, Copy, Serialize, Deserialize)]
pub enum InstrumentRangeKind {
    Written,
    Sounding,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub enum PercussionInstrumentValidationReason {
    EmptyId,
    DuplicateId { first: usize },
    InvalidStaffPosition { staff_position: i8 },
    InvalidPreferredVoice { preferred_voice: u8 },
    InvalidTechnique { technique: String },
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub enum ScoreViewValidationReason {
    EmptyIdOrName,
    DuplicateId {
        first: usize,
    },
    EmptyPartSelection,
    InvalidPart {
        part: usize,
    },
    DuplicatePart {
        part: usize,
    },
    InvalidStaff {
        part: usize,
        staff: usize,
    },
    HiddenStaffOutsideSelection {
        part: usize,
        staff: usize,
    },
    DuplicateStaffKindOverride {
        part: usize,
        staff: usize,
    },
    StaffKindOverrideOutsideSelection {
        part: usize,
        staff: usize,
    },
    TablatureOverrideWithoutConfig {
        part: usize,
        staff: usize,
    },
    InvalidMeasuresPerRow,
    InvalidTypedStyleOverride {
        property: super::score::ViewStyleProperty,
        value: f32,
    },
    BreakOutOfRange {
        measure: usize,
    },
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub enum ObjectStyleValidationReason {
    InvalidValue {
        property: super::score::ViewStyleProperty,
        value: f32,
    },
    MissingTarget,
    InvalidProvenance,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub enum GuitarBendCurveValidationReason {
    TooManyPoints { count: usize },
    RequiresBendTechnique,
    RequiresStartAtZero,
    RequiresEndAtFullDuration,
    PositionsNotStrictlyIncreasing,
}

/// A non-fatal advisory warning found by [`validate`].
#[derive(Debug, Clone, Serialize, Deserialize)]
pub enum ValidationWarning {
    /// A measure's beat count is less than the time signature (incomplete bar).
    IncompleteBar {
        part: usize,
        staff: usize,
        measure: usize,
        expected_beats: f64,
        actual_beats: f64,
    },
    /// Two volta brackets in the same staff overlap or share the same number.
    OverlappingVolta { part: usize, staff: usize },
    /// A part has no notes across all measures.
    EmptyPart { part: usize },
    /// The same rehearsal mark text appears more than once.
    DuplicateRehearsalMark { mark: String },
}

/// Combined result of [`validate`]: errors that indicate broken structure, plus advisory warnings.
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct ValidationReport {
    pub errors: Vec<ValidationError>,
    pub warnings: Vec<ValidationWarning>,
}

impl ValidationReport {
    /// `true` when there are no errors (warnings may still be present).
    pub fn is_valid(&self) -> bool {
        self.errors.is_empty()
    }
}

/// Check every voice in every measure for structural correctness.
///
/// Checks performed:
/// - **Errors**: beat-count mismatch, out-of-range pitch.
/// - **Warnings**: incomplete bar (underfull voice), overlapping volta brackets,
///   empty parts, duplicate rehearsal marks.
///
/// Multi-rest placeholder measures and empty voices are skipped.
/// Percussion parts (MIDI channel 9) are exempt from pitch-range checks.
pub fn validate(score: &Score) -> ValidationReport {
    let mut errors = Vec::new();
    let mut warnings = Vec::new();

    let mut rehearsal_counts: HashMap<String, usize> = HashMap::new();

    if score.parts.is_empty() {
        errors.push(ValidationError::EmptyScore);
    }

    for override_ in &score.style_overrides {
        if !valid_typed_style_value(override_.value) {
            errors.push(ValidationError::InvalidScoreStyleOverride {
                property: override_.property,
                value: override_.value,
            });
        }
    }
    for (index, override_) in score.object_style_overrides.iter().enumerate() {
        let reason = if !valid_typed_style_value(override_.value) {
            Some(ObjectStyleValidationReason::InvalidValue {
                property: override_.property,
                value: override_.value,
            })
        } else if !object_style_target_exists(score, &override_.target) {
            Some(ObjectStyleValidationReason::MissingTarget)
        } else if override_.provenance.as_ref().is_some_and(|provenance| {
            provenance.format.trim().is_empty()
                || provenance.format.len() > 64
                || provenance.source_location.trim().is_empty()
                || provenance.source_location.len() > 2048
        }) {
            Some(ObjectStyleValidationReason::InvalidProvenance)
        } else {
            None
        };
        if let Some(reason) = reason {
            errors.push(ValidationError::InvalidObjectStyleOverride { index, reason });
        }
    }

    let mut view_ids: HashMap<String, usize> = HashMap::new();
    for (index, view) in score.views.iter().enumerate() {
        validate_score_view(index, view, score, &mut view_ids, &mut errors);
    }

    let mut spanner_ids: HashMap<&str, usize> = HashMap::new();
    for (index, spanner) in score.spanners.iter().enumerate() {
        if spanner.id.trim().is_empty() {
            errors.push(ValidationError::InvalidSpannerId {
                index,
                id: spanner.id.clone(),
            });
        } else if let Some(first) = spanner_ids.insert(spanner.id.as_str(), index) {
            errors.push(ValidationError::DuplicateSpannerId {
                first,
                duplicate: index,
                id: spanner.id.clone(),
            });
        }
        for (endpoint, address) in [
            (SpannerEndpoint::Start, &spanner.start),
            (SpannerEndpoint::End, &spanner.end),
        ] {
            if !note_exists(score, address) {
                errors.push(ValidationError::InvalidSpannerEndpoint {
                    index,
                    id: spanner.id.clone(),
                    kind: spanner.kind.clone(),
                    endpoint,
                    address: address.clone(),
                });
            }
        }
    }

    for (pi, part) in score.parts.iter().enumerate() {
        let range = instrument_range(part.midi_program);
        let is_percussion = part.midi_channel == 9;
        let mut part_has_notes = false;

        if part.staves.is_empty() {
            errors.push(ValidationError::PartWithoutStaves { part: pi });
            continue;
        }

        if let Some(definition) = &part.instrument {
            validate_instrument_definition(pi, definition, &mut errors);
        }
        validate_percussion_kit(pi, &part.percussion_instruments, &mut errors);

        let expected_measure_count = part.staves[0].measures.len();

        for (si, staff) in part.staves.iter().enumerate() {
            if staff.measures.is_empty() {
                errors.push(ValidationError::StaffWithoutMeasures {
                    part: pi,
                    staff: si,
                });
                continue;
            }
            if staff.measures.len() != expected_measure_count {
                errors.push(ValidationError::MeasureCountMismatch {
                    part: pi,
                    staff: si,
                    expected: expected_measure_count,
                    found: staff.measures.len(),
                });
            }

            if !(1..=64).contains(&staff.presentation.lines) {
                errors.push(ValidationError::InvalidStaffPresentation {
                    part: pi,
                    staff: si,
                    reason: StaffPresentationValidationReason::InvalidLineCount {
                        lines: staff.presentation.lines,
                    },
                });
            }
            if !staff.presentation.line_distance.is_finite()
                || !(0.1..=16.0).contains(&staff.presentation.line_distance)
            {
                errors.push(ValidationError::InvalidStaffPresentation {
                    part: pi,
                    staff: si,
                    reason: StaffPresentationValidationReason::InvalidLineDistance {
                        line_distance: staff.presentation.line_distance,
                    },
                });
            }
            if staff.presentation.kind == StaffKind::Tablature && staff.tablature.is_none() {
                errors.push(ValidationError::InvalidStaffPresentation {
                    part: pi,
                    staff: si,
                    reason: StaffPresentationValidationReason::TablatureWithoutConfig,
                });
            }

            if let Some(tab) = &staff.tablature {
                if !(1..=64).contains(&tab.lines) {
                    errors.push(ValidationError::InvalidTablature {
                        part: pi,
                        staff: si,
                        reason: TablatureValidationReason::InvalidLineCount { lines: tab.lines },
                    });
                } else if tab.tuning_midi.len() > usize::from(tab.lines) {
                    errors.push(ValidationError::InvalidTablature {
                        part: pi,
                        staff: si,
                        reason: TablatureValidationReason::TooManyTunings {
                            tuning_count: tab.tuning_midi.len(),
                            lines: tab.lines,
                        },
                    });
                }
                for (index, &midi) in tab.tuning_midi.iter().enumerate() {
                    if !(0..=127).contains(&midi) {
                        errors.push(ValidationError::InvalidTablature {
                            part: pi,
                            staff: si,
                            reason: TablatureValidationReason::TuningOutOfMidiRange { index, midi },
                        });
                    }
                }
            }

            let mut current_ts = score.settings.time_signature.clone();
            let mut volta_numbers_seen: Vec<u8> = Vec::new();

            for (mi, measure) in staff.measures.iter().enumerate() {
                if let Some(change) = &measure.tablature_change {
                    match &staff.tablature {
                        None => errors.push(ValidationError::InvalidTablature {
                            part: pi,
                            staff: si,
                            reason: TablatureValidationReason::ChangeWithoutBase { measure: mi },
                        }),
                        Some(base) if base.lines != change.lines => {
                            errors.push(ValidationError::InvalidTablature {
                                part: pi,
                                staff: si,
                                reason: TablatureValidationReason::ChangeLineCountMismatch {
                                    measure: mi,
                                    base_lines: base.lines,
                                    changed_lines: change.lines,
                                },
                            });
                        }
                        Some(_) => {}
                    }
                    if !(1..=64).contains(&change.lines) {
                        errors.push(ValidationError::InvalidTablature {
                            part: pi,
                            staff: si,
                            reason: TablatureValidationReason::InvalidLineCount {
                                lines: change.lines,
                            },
                        });
                    } else if change.tuning_midi.len() > usize::from(change.lines) {
                        errors.push(ValidationError::InvalidTablature {
                            part: pi,
                            staff: si,
                            reason: TablatureValidationReason::TooManyTunings {
                                tuning_count: change.tuning_midi.len(),
                                lines: change.lines,
                            },
                        });
                    }
                    for (index, &midi) in change.tuning_midi.iter().enumerate() {
                        if !(0..=127).contains(&midi) {
                            errors.push(ValidationError::InvalidTablature {
                                part: pi,
                                staff: si,
                                reason: TablatureValidationReason::TuningOutOfMidiRange {
                                    index,
                                    midi,
                                },
                            });
                        }
                    }
                }
                if let Some(ts) = &measure.time_sig {
                    current_ts = ts.clone();
                }
                if !valid_time_signature(&current_ts) {
                    errors.push(ValidationError::InvalidTimeSignature {
                        part: pi,
                        staff: si,
                        measure: mi,
                        numerator: current_ts.numerator,
                        denominator: current_ts.denominator,
                    });
                    continue;
                }
                if measure.multi_rest_count.is_some() {
                    continue;
                }

                // Rehearsal mark deduplication
                if let Some(ref mark) = measure.rehearsal {
                    let entry = rehearsal_counts.entry(mark.clone()).or_insert(0);
                    *entry += 1;
                }

                // Volta overlap detection
                if let Some(ref volta) = measure.volta {
                    if volta_numbers_seen.contains(&volta.number) {
                        warnings.push(ValidationWarning::OverlappingVolta {
                            part: pi,
                            staff: si,
                        });
                    } else {
                        volta_numbers_seen.push(volta.number);
                    }
                }

                let expected = current_ts.total_beats();
                for (vi, voice) in measure.voices.iter().enumerate() {
                    if voice.is_empty() {
                        continue;
                    }
                    let non_rest_count: usize = voice.iter().filter(|n| !n.is_rest).count();
                    if non_rest_count > 0 {
                        part_has_notes = true;
                    }
                    let total: f64 = voice.iter().map(|n| n.beats()).sum();
                    if total > expected + 0.02 {
                        errors.push(ValidationError::BeatCount {
                            part: pi,
                            staff: si,
                            measure: mi,
                            voice: vi,
                            expected_beats: expected,
                            found_beats: total,
                        });
                    } else if total < expected - 0.02 && non_rest_count > 0 {
                        warnings.push(ValidationWarning::IncompleteBar {
                            part: pi,
                            staff: si,
                            measure: mi,
                            expected_beats: expected,
                            actual_beats: total,
                        });
                    }

                    for (ni, note) in voice.iter().enumerate() {
                        if note.is_rest || note.is_grace {
                            continue;
                        }
                        if let Some(chord) = &note.chord_symbol
                            && let Some(end) = &chord.range_end
                            && !note_exists(score, end)
                        {
                            errors.push(ValidationError::InvalidHarmonyRange {
                                part: pi,
                                staff: si,
                                measure: mi,
                                voice: vi,
                                note: ni,
                                end: end.clone(),
                            });
                        }
                        for (pitch_index, pitch) in note.pitches.iter().enumerate() {
                            if !(-99..=99).contains(&pitch.microtone_cents) {
                                errors.push(ValidationError::MicrotoneOutOfRange {
                                    part: pi,
                                    staff: si,
                                    measure: mi,
                                    voice: vi,
                                    note: ni,
                                    pitch: pitch_index,
                                    microtone_cents: pitch.microtone_cents,
                                });
                            }
                        }
                        if !note.guitar_bend_curve.is_empty() {
                            let reason = if note.guitar_bend_curve.len() > 32 {
                                Some(GuitarBendCurveValidationReason::TooManyPoints {
                                    count: note.guitar_bend_curve.len(),
                                })
                            } else if note.guitar_technique != Some(GuitarTechnique::Bend) {
                                Some(GuitarBendCurveValidationReason::RequiresBendTechnique)
                            } else if note
                                .guitar_bend_curve
                                .first()
                                .map(|point| point.position_per_mille)
                                != Some(0)
                            {
                                Some(GuitarBendCurveValidationReason::RequiresStartAtZero)
                            } else if note
                                .guitar_bend_curve
                                .last()
                                .map(|point| point.position_per_mille)
                                != Some(1000)
                            {
                                Some(GuitarBendCurveValidationReason::RequiresEndAtFullDuration)
                            } else if note.guitar_bend_curve.windows(2).any(|points| {
                                points[0].position_per_mille >= points[1].position_per_mille
                            }) {
                                Some(
                                    GuitarBendCurveValidationReason::PositionsNotStrictlyIncreasing,
                                )
                            } else {
                                None
                            };
                            if let Some(reason) = reason {
                                errors.push(ValidationError::InvalidGuitarBendCurve {
                                    part: pi,
                                    staff: si,
                                    measure: mi,
                                    voice: vi,
                                    note: ni,
                                    reason,
                                });
                            }
                        }
                    }

                    if !is_percussion {
                        let transpose = staff.transpose_semitones;
                        for (ni, note) in voice.iter().enumerate() {
                            if note.is_rest || note.is_grace {
                                continue;
                            }
                            if let Some(tab) = &staff.tablature {
                                let positions =
                                    note.tab_position.iter().chain(note.tab_positions.iter());
                                for position in positions {
                                    if position.string == 0 || position.string > tab.lines {
                                        errors.push(ValidationError::TabPositionOutOfRange {
                                            part: pi,
                                            staff: si,
                                            measure: mi,
                                            voice: vi,
                                            note: ni,
                                            string: position.string,
                                            lines: tab.lines,
                                        });
                                    }
                                }
                            }
                            for pitch in &note.pitches {
                                let midi = (pitch.to_midi() + transpose as i16).clamp(0, 127) as u8;
                                if midi < range.0 || midi > range.1 {
                                    errors.push(ValidationError::OutOfRange {
                                        part_index: pi,
                                        staff_index: si,
                                        measure_index: mi,
                                        note_index: ni,
                                        pitch_midi: midi,
                                        instrument_range: range,
                                    });
                                }
                            }
                        }
                    }
                }
            }
        }

        if !part_has_notes {
            warnings.push(ValidationWarning::EmptyPart { part: pi });
        }
    }

    for (mark, count) in &rehearsal_counts {
        if *count > 1 {
            warnings.push(ValidationWarning::DuplicateRehearsalMark { mark: mark.clone() });
        }
    }

    ValidationReport { errors, warnings }
}

fn validate_instrument_definition(
    part: usize,
    definition: &InstrumentDefinition,
    errors: &mut Vec<ValidationError>,
) {
    if definition.id.trim().is_empty() {
        errors.push(ValidationError::InvalidInstrumentDefinition {
            part,
            reason: InstrumentDefinitionValidationReason::EmptyId,
        });
    }
    if !(1..=64).contains(&definition.staff_count) {
        errors.push(ValidationError::InvalidInstrumentDefinition {
            part,
            reason: InstrumentDefinitionValidationReason::InvalidStaffCount {
                staff_count: definition.staff_count,
            },
        });
    }
    if definition.midi_channel > 15 {
        errors.push(ValidationError::InvalidInstrumentDefinition {
            part,
            reason: InstrumentDefinitionValidationReason::InvalidMidiChannel {
                midi_channel: definition.midi_channel,
            },
        });
    }
    for (kind, range) in [
        (InstrumentRangeKind::Written, definition.written_range),
        (InstrumentRangeKind::Sounding, definition.sounding_range),
    ] {
        if let Some(range) = range
            && range.lowest > range.highest
        {
            errors.push(ValidationError::InvalidInstrumentDefinition {
                part,
                reason: InstrumentDefinitionValidationReason::InvalidRange { kind, range },
            });
        }
    }
}

fn validate_percussion_kit(
    part: usize,
    instruments: &[PercussionInstrument],
    errors: &mut Vec<ValidationError>,
) {
    let mut ids: HashMap<&str, usize> = HashMap::new();
    for (instrument_index, instrument) in instruments.iter().enumerate() {
        let invalid = |reason| ValidationError::InvalidPercussionInstrument {
            part,
            instrument: instrument_index,
            id: instrument.id.clone(),
            reason,
        };
        if instrument.id.trim().is_empty() {
            errors.push(invalid(PercussionInstrumentValidationReason::EmptyId));
        } else if let Some(first) = ids.insert(instrument.id.as_str(), instrument_index) {
            errors.push(invalid(PercussionInstrumentValidationReason::DuplicateId {
                first,
            }));
        }
        if let Some(staff_position) = instrument.staff_position
            && !(-32..=32).contains(&staff_position)
        {
            errors.push(invalid(
                PercussionInstrumentValidationReason::InvalidStaffPosition { staff_position },
            ));
        }
        if let Some(preferred_voice) = instrument.preferred_voice
            && !(1..=4).contains(&preferred_voice)
        {
            errors.push(invalid(
                PercussionInstrumentValidationReason::InvalidPreferredVoice { preferred_voice },
            ));
        }
        for technique in &instrument.techniques {
            if technique.trim().is_empty() || technique.len() > 128 {
                errors.push(invalid(
                    PercussionInstrumentValidationReason::InvalidTechnique {
                        technique: technique.clone(),
                    },
                ));
            }
        }
    }
}

fn validate_score_view(
    index: usize,
    view: &ScoreView,
    score: &Score,
    ids: &mut HashMap<String, usize>,
    errors: &mut Vec<ValidationError>,
) {
    let invalid = |reason| ValidationError::InvalidScoreView {
        index,
        id: view.id.clone(),
        reason,
    };
    if view.id.trim().is_empty() || view.name.trim().is_empty() {
        errors.push(invalid(ScoreViewValidationReason::EmptyIdOrName));
    } else if let Some(first) = ids.insert(view.id.clone(), index) {
        errors.push(invalid(ScoreViewValidationReason::DuplicateId { first }));
    }
    if view.parts.is_empty() {
        errors.push(invalid(ScoreViewValidationReason::EmptyPartSelection));
        return;
    }
    let mut selected = vec![false; score.parts.len()];
    for &part_index in &view.parts {
        if part_index >= score.parts.len() {
            errors.push(invalid(ScoreViewValidationReason::InvalidPart {
                part: part_index,
            }));
        } else if std::mem::replace(&mut selected[part_index], true) {
            errors.push(invalid(ScoreViewValidationReason::DuplicatePart {
                part: part_index,
            }));
        }
    }
    if view.layout.measures_per_row.is_some_and(|value| value == 0) {
        errors.push(invalid(ScoreViewValidationReason::InvalidMeasuresPerRow));
    }
    for override_ in &view.layout.typed_style_overrides {
        if !valid_typed_style_value(override_.value) {
            errors.push(invalid(
                ScoreViewValidationReason::InvalidTypedStyleOverride {
                    property: override_.property,
                    value: override_.value,
                },
            ));
        }
    }
    for reference in &view.layout.hidden_staves {
        let Some(part) = score.parts.get(reference.part) else {
            errors.push(invalid(ScoreViewValidationReason::InvalidPart {
                part: reference.part,
            }));
            continue;
        };
        if reference.staff >= part.staves.len() {
            errors.push(invalid(ScoreViewValidationReason::InvalidStaff {
                part: reference.part,
                staff: reference.staff,
            }));
        } else if !selected[reference.part] {
            errors.push(invalid(
                ScoreViewValidationReason::HiddenStaffOutsideSelection {
                    part: reference.part,
                    staff: reference.staff,
                },
            ));
        }
    }
    let mut overridden = HashMap::new();
    for override_ in &view.staff_kind_overrides {
        let reference = override_.staff;
        let Some(part) = score.parts.get(reference.part) else {
            errors.push(invalid(ScoreViewValidationReason::InvalidPart {
                part: reference.part,
            }));
            continue;
        };
        if reference.staff >= part.staves.len() {
            errors.push(invalid(ScoreViewValidationReason::InvalidStaff {
                part: reference.part,
                staff: reference.staff,
            }));
        } else if !selected[reference.part] {
            errors.push(invalid(
                ScoreViewValidationReason::StaffKindOverrideOutsideSelection {
                    part: reference.part,
                    staff: reference.staff,
                },
            ));
        } else if overridden
            .insert((reference.part, reference.staff), ())
            .is_some()
        {
            errors.push(invalid(
                ScoreViewValidationReason::DuplicateStaffKindOverride {
                    part: reference.part,
                    staff: reference.staff,
                },
            ));
        } else if override_.kind == StaffKind::Tablature
            && part.staves[reference.staff].tablature.is_none()
        {
            errors.push(invalid(
                ScoreViewValidationReason::TablatureOverrideWithoutConfig {
                    part: reference.part,
                    staff: reference.staff,
                },
            ));
        }
    }
    let measure_count = score.measure_count();
    for &measure in view
        .layout
        .system_breaks
        .iter()
        .chain(view.layout.page_breaks.iter())
    {
        if measure >= measure_count {
            errors.push(invalid(ScoreViewValidationReason::BreakOutOfRange {
                measure,
            }));
        }
    }
}

fn valid_typed_style_value(value: f32) -> bool {
    value.is_finite() && (0.05..=64.0).contains(&value)
}

fn object_style_target_exists(score: &Score, target: &super::score::ObjectStyleTarget) -> bool {
    use super::score::ObjectStyleTarget;
    match target {
        ObjectStyleTarget::ScoreText { text_index } => *text_index < score.texts.len(),
        ObjectStyleTarget::MeasureText {
            part,
            staff,
            measure,
            text_index,
        } => score
            .parts
            .get(*part)
            .and_then(|part| part.staves.get(*staff))
            .and_then(|staff| staff.measures.get(*measure))
            .is_some_and(|measure| *text_index < measure.texts.len()),
        ObjectStyleTarget::Note { address } => score
            .parts
            .get(address.part)
            .and_then(|part| part.staves.get(address.staff))
            .and_then(|staff| staff.measures.get(address.measure))
            .and_then(|measure| measure.voices.get(address.voice))
            .is_some_and(|voice| address.note < voice.len()),
    }
}

fn valid_time_signature(time: &super::notation::TimeSignature) -> bool {
    time.numerator > 0 && matches!(time.denominator, 1 | 2 | 4 | 8 | 16 | 32 | 64)
}

fn note_exists(score: &Score, address: &NoteAddr) -> bool {
    score
        .parts
        .get(address.part)
        .and_then(|part| part.staves.get(address.staff))
        .and_then(|staff| staff.measures.get(address.measure))
        .and_then(|measure| measure.voices.get(address.voice))
        .and_then(|voice| voice.get(address.note))
        .is_some()
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::model::{
        duration::Duration,
        notation::ChordSymbol,
        pitch::{Pitch, Step},
        score::{Note, NoteAddr, Score},
    };

    #[test]
    fn validate_clean_score_returns_empty_errors() {
        let score = Score::new("T", 120, 4, 4, 0, 1);
        assert!(validate(&score).errors.is_empty());
    }

    #[test]
    fn validate_empty_score_returns_structural_error() {
        let mut score = Score::new("T", 120, 4, 4, 0, 1);
        score.parts.clear();
        let report = validate(&score);
        assert!(
            report
                .errors
                .iter()
                .any(|error| matches!(error, ValidationError::EmptyScore))
        );
    }

    #[test]
    fn validate_detects_missing_staves_and_measures() {
        let mut score = Score::new("T", 120, 4, 4, 0, 1);
        score.parts[0].staves.clear();
        let report = validate(&score);
        assert!(
            report
                .errors
                .iter()
                .any(|error| matches!(error, ValidationError::PartWithoutStaves { part: 0 }))
        );

        score.parts[0].staves.push(crate::model::score::Staff::new(
            crate::model::notation::Clef::Treble,
        ));
        let report = validate(&score);
        assert!(report.errors.iter().any(|error| matches!(
            error,
            ValidationError::StaffWithoutMeasures { part: 0, staff: 0 }
        )));
    }

    #[test]
    fn validate_detects_staff_measure_count_mismatch() {
        let mut score = Score::template(crate::model::score::ScoreTemplate::Piano);
        score.parts[0].staves[1].measures.pop();
        let report = validate(&score);
        assert!(report.errors.iter().any(|error| matches!(
            error,
            ValidationError::MeasureCountMismatch {
                part: 0,
                staff: 1,
                expected: 4,
                found: 3
            }
        )));
    }

    #[test]
    fn validate_detects_invalid_time_signature() {
        let mut score = Score::new("T", 120, 4, 4, 0, 1);
        score.parts[0].staves[0].measures[0].time_sig =
            Some(crate::model::notation::TimeSignature {
                numerator: 0,
                denominator: 3,
            });
        let report = validate(&score);
        assert!(report.errors.iter().any(|error| matches!(
            error,
            ValidationError::InvalidTimeSignature {
                part: 0,
                staff: 0,
                measure: 0,
                numerator: 0,
                denominator: 3
            }
        )));
    }

    #[test]
    fn validate_overfull_measure_returns_error() {
        let mut score = Score::new("T", 120, 4, 4, 0, 1);
        score.parts[0].staves[0].measures[0].voices[0]
            .push(Note::new(Pitch::new(Step::C, 4), Duration::Quarter));
        let report = validate(&score);
        assert!(!report.errors.is_empty());
        assert!(matches!(
            report.errors[0],
            ValidationError::BeatCount {
                measure: 0,
                voice: 0,
                ..
            }
        ));
    }

    #[test]
    fn validate_skips_multi_rest() {
        let mut score = Score::new("T", 120, 4, 4, 0, 1);
        score.parts[0].staves[0].measures[0].multi_rest_count = Some(4);
        score.parts[0].staves[0].measures[0].voices[0].clear();
        assert!(validate(&score).errors.is_empty());
    }

    #[test]
    fn validate_rejects_tablature_view_override_without_tablature_configuration() {
        let mut score = Score::new("T", 120, 4, 4, 0, 1);
        score
            .views
            .push(ScoreView::linked_tablature_staff("tab", "Tab", 0, 0));

        assert!(validate(&score).errors.iter().any(|error| matches!(
            error,
            ValidationError::InvalidScoreView {
                reason: ScoreViewValidationReason::TablatureOverrideWithoutConfig {
                    part: 0,
                    staff: 0,
                },
                ..
            }
        )));
    }

    #[test]
    fn validate_rejects_non_finite_typed_view_style_override() {
        let mut score = Score::new("T", 120, 4, 4, 0, 1);
        let mut view = ScoreView::linked_part("part", "Part", 0);
        view.layout
            .typed_style_overrides
            .push(super::super::score::ViewStyleOverride {
                property: super::super::score::ViewStyleProperty::TextScale,
                value: f32::NAN,
            });
        score.views.push(view);
        assert!(validate(&score).errors.iter().any(|error| matches!(
            error,
            ValidationError::InvalidScoreView {
                reason: ScoreViewValidationReason::InvalidTypedStyleOverride { .. },
                ..
            }
        )));
    }

    #[test]
    fn validate_rejects_out_of_range_score_style_override() {
        let mut score = Score::new("T", 120, 4, 4, 0, 1);
        score
            .style_overrides
            .push(super::super::score::ViewStyleOverride {
                property: super::super::score::ViewStyleProperty::StaffSpace,
                value: 0.01,
            });

        assert!(validate(&score).errors.iter().any(|error| matches!(
            error,
            ValidationError::InvalidScoreStyleOverride {
                property: super::super::score::ViewStyleProperty::StaffSpace,
                value,
            } if (*value - 0.01).abs() < f32::EPSILON
        )));
    }

    #[test]
    fn validate_rejects_object_style_override_with_missing_target() {
        let mut score = Score::new("T", 120, 4, 4, 0, 1);
        score
            .object_style_overrides
            .push(super::super::score::ObjectStyleOverride {
                target: super::super::score::ObjectStyleTarget::ScoreText { text_index: 0 },
                property: super::super::score::ViewStyleProperty::TextScale,
                value: 1.1,
                provenance: None,
            });
        assert!(validate(&score).errors.iter().any(|error| matches!(
            error,
            ValidationError::InvalidObjectStyleOverride {
                reason: ObjectStyleValidationReason::MissingTarget,
                ..
            }
        )));
    }

    #[test]
    fn validate_rejects_non_normalized_guitar_bend_curve() {
        let mut score = Score::new("T", 120, 4, 4, 0, 1);
        score.parts[0].staves[0].measures[0].voices[0] =
            vec![Note::new(Pitch::new(Step::E, 4), Duration::Whole)];
        let note = &mut score.parts[0].staves[0].measures[0].voices[0][0];
        note.guitar_technique = Some(GuitarTechnique::Bend);
        note.guitar_bend_curve = vec![
            crate::GuitarBendPoint {
                position_per_mille: 100,
                alter_cents: 0,
            },
            crate::GuitarBendPoint {
                position_per_mille: 1000,
                alter_cents: 200,
            },
        ];
        assert!(validate(&score).errors.iter().any(|error| matches!(
            error,
            ValidationError::InvalidGuitarBendCurve {
                reason: GuitarBendCurveValidationReason::RequiresStartAtZero,
                ..
            }
        )));
    }

    #[test]
    fn validate_rejects_harmony_range_to_missing_note() {
        let mut score = Score::new("T", 120, 4, 4, 0, 1);
        let mut note = Note::new(Pitch::new(Step::C, 4), Duration::Whole);
        note.chord_symbol = Some(ChordSymbol {
            root: "C".to_owned(),
            kind: "major".to_owned(),
            bass: None,
            placement: None,
            extender: true,
            harmonic_degree: None,
            harmony_function: None,
            harmony_type: None,
            chord_ref: None,
            range_end: Some(NoteAddr {
                part: 0,
                staff: 0,
                measure: 0,
                voice: 0,
                note: 9,
            }),
            degrees: Vec::new(),
        });
        score.parts[0].staves[0].measures[0].voices[0] = vec![note];
        assert!(validate(&score).errors.iter().any(|error| matches!(
            error,
            ValidationError::InvalidHarmonyRange { note: 0, end, .. }
                if end.note == 9
        )));
    }

    #[test]
    fn validate_out_of_range_pitch_detected() {
        // Piano (program 0): range 21–108. C9 (midi=120) is out of range.
        let mut score = Score::new("T", 120, 4, 4, 0, 1);
        score.parts[0].midi_program = 0;
        score.parts[0].staves[0].measures[0].voices[0] =
            vec![Note::new(Pitch::new(Step::C, 9), Duration::Whole)];
        let report = validate(&score);
        assert!(report.errors.iter().any(
            |e| matches!(e, ValidationError::OutOfRange { pitch_midi, .. } if *pitch_midi == 120)
        ));
    }

    #[test]
    fn validate_percussion_channel_skips_range_check() {
        // Channel 9 = percussion; even extreme pitches should not trigger OutOfRange.
        let mut score = Score::new("T", 120, 4, 4, 0, 1);
        score.parts[0].midi_channel = 9;
        score.parts[0].midi_program = 0;
        score.parts[0].staves[0].measures[0].voices[0] =
            vec![Note::new(Pitch::new(Step::C, 9), Duration::Whole)];
        let report = validate(&score);
        assert!(
            !report
                .errors
                .iter()
                .any(|e| matches!(e, ValidationError::OutOfRange { .. }))
        );
    }

    #[test]
    fn validate_in_range_pitch_ok() {
        // Piano C4 (midi=60) is in range 21–108.
        let mut score = Score::new("T", 120, 4, 4, 0, 1);
        score.parts[0].midi_program = 0;
        score.parts[0].staves[0].measures[0].voices[0] =
            vec![Note::new(Pitch::new(Step::C, 4), Duration::Whole)];
        assert!(
            !validate(&score)
                .errors
                .iter()
                .any(|e| matches!(e, ValidationError::OutOfRange { .. }))
        );
    }

    #[test]
    fn validate_rejects_deserialized_microtone_out_of_range() {
        let mut score = Score::new("T", 120, 4, 4, 0, 1);
        let mut note = Note::new(Pitch::new(Step::C, 4), Duration::Whole);
        note.pitches[0].microtone_cents = 100;
        score.parts[0].staves[0].measures[0].voices[0] = vec![note];
        let report = validate(&score);
        assert!(report.errors.iter().any(|error| matches!(
            error,
            ValidationError::MicrotoneOutOfRange {
                microtone_cents: 100,
                ..
            }
        )));
    }

    #[test]
    fn validate_rejects_invalid_and_duplicate_typed_spanners() {
        let mut score = Score::new("T", 120, 4, 4, 0, 1);
        score.parts[0].staves[0].measures[0].voices[0] =
            vec![Note::new(Pitch::new(Step::C, 4), Duration::Whole)];
        let address = NoteAddr {
            part: 0,
            staff: 0,
            measure: 0,
            voice: 0,
            note: 0,
        };
        score.spanners = vec![
            super::super::score::NotationSpanner {
                id: String::new(),
                kind: NotationSpannerKind::Slur,
                start: address.clone(),
                end: address.clone(),
                number: Some(1),
                line_type: None,
                text: None,
                placement: None,
                ottava_size: None,
                ottava_type: None,
            },
            super::super::score::NotationSpanner {
                id: "duplicate".to_string(),
                kind: NotationSpannerKind::Pedal,
                start: address.clone(),
                end: NoteAddr { note: 9, ..address },
                number: Some(2),
                line_type: None,
                text: None,
                placement: None,
                ottava_size: None,
                ottava_type: None,
            },
            super::super::score::NotationSpanner {
                id: "duplicate".to_string(),
                kind: NotationSpannerKind::Ottava,
                start: NoteAddr {
                    part: 9,
                    staff: 0,
                    measure: 0,
                    voice: 0,
                    note: 0,
                },
                end: NoteAddr {
                    part: 0,
                    staff: 0,
                    measure: 0,
                    voice: 0,
                    note: 0,
                },
                number: None,
                line_type: None,
                text: None,
                placement: None,
                ottava_size: Some(8),
                ottava_type: None,
            },
        ];

        let report = validate(&score);
        assert!(
            report
                .errors
                .iter()
                .any(|error| matches!(error, ValidationError::InvalidSpannerId { index: 0, .. }))
        );
        assert!(report.errors.iter().any(|error| matches!(
            error,
            ValidationError::DuplicateSpannerId {
                first: 1,
                duplicate: 2,
                ..
            }
        )));
        assert_eq!(
            report
                .errors
                .iter()
                .filter(|error| matches!(error, ValidationError::InvalidSpannerEndpoint { .. }))
                .count(),
            2
        );
    }

    #[test]
    fn validate_rejects_invalid_tablature_metadata_and_positions() {
        let mut score = Score::new("Tab", 120, 4, 4, 0, 1);
        score.parts[0].staves[0].tablature = Some(super::super::notation::TablatureConfig {
            lines: 6,
            tuning_midi: vec![64, 59, 55, 50, 45, 40, 35],
            capo: 0,
        });
        let mut note = Note::new(Pitch::new(Step::E, 4), Duration::Whole);
        note.tab_position = Some(super::super::notation::TabPosition { string: 7, fret: 0 });
        note.tab_positions = vec![super::super::notation::TabPosition { string: 8, fret: 3 }];
        score.parts[0].staves[0].measures[0].voices[0] = vec![note];

        let report = validate(&score);
        assert!(report.errors.iter().any(|error| matches!(
            error,
            ValidationError::InvalidTablature {
                reason: TablatureValidationReason::TooManyTunings { .. },
                ..
            }
        )));
        assert!(
            report
                .errors
                .iter()
                .any(|error| matches!(error, ValidationError::TabPositionOutOfRange { .. }))
        );
        assert_eq!(
            report
                .errors
                .iter()
                .filter(|error| matches!(error, ValidationError::TabPositionOutOfRange { .. }))
                .count(),
            2
        );
    }

    #[test]
    fn validate_rejects_invalid_staff_presentation() {
        let mut score = Score::new("Presentation", 120, 4, 4, 0, 1);
        let presentation = &mut score.parts[0].staves[0].presentation;
        presentation.kind = StaffKind::Tablature;
        presentation.lines = 0;
        presentation.line_distance = f32::NAN;

        let report = validate(&score);
        assert!(report.errors.iter().any(|error| matches!(
            error,
            ValidationError::InvalidStaffPresentation {
                reason: StaffPresentationValidationReason::InvalidLineCount { lines: 0 },
                ..
            }
        )));
        assert!(report.errors.iter().any(|error| matches!(
            error,
            ValidationError::InvalidStaffPresentation {
                reason: StaffPresentationValidationReason::InvalidLineDistance { .. },
                ..
            }
        )));
        assert!(report.errors.iter().any(|error| matches!(
            error,
            ValidationError::InvalidStaffPresentation {
                reason: StaffPresentationValidationReason::TablatureWithoutConfig,
                ..
            }
        )));
    }

    #[test]
    fn validate_rejects_tablature_change_without_matching_base_geometry() {
        let mut score = Score::new("Tab change", 120, 4, 4, 0, 1);
        score.parts[0].staves[0].measures[0].tablature_change =
            Some(super::super::notation::TablatureConfig {
                lines: 6,
                tuning_midi: vec![40, 45, 50, 55, 59, 64],
                capo: 2,
            });
        let report = validate(&score);
        assert!(report.errors.iter().any(|error| matches!(
            error,
            ValidationError::InvalidTablature {
                reason: TablatureValidationReason::ChangeWithoutBase { measure: 0 },
                ..
            }
        )));

        score.parts[0].staves[0].tablature = Some(super::super::notation::TablatureConfig {
            lines: 6,
            tuning_midi: vec![40, 45, 50, 55, 59, 64],
            capo: 0,
        });
        score.parts[0].staves[0].measures[0]
            .tablature_change
            .as_mut()
            .expect("change exists")
            .lines = 7;
        let report = validate(&score);
        assert!(report.errors.iter().any(|error| matches!(
            error,
            ValidationError::InvalidTablature {
                reason: TablatureValidationReason::ChangeLineCountMismatch {
                    measure: 0,
                    base_lines: 6,
                    changed_lines: 7
                },
                ..
            }
        )));
    }

    #[test]
    fn validate_rejects_invalid_percussion_kit_entries() {
        let mut score = Score::new("Kit", 120, 4, 4, 0, 1);
        score.parts[0].percussion_instruments = vec![
            PercussionInstrument {
                id: "snare".to_string(),
                name: None,
                midi_unpitched: Some(38),
                staff_position: Some(40),
                notehead: None,
                preferred_voice: Some(5),
                techniques: vec!["".to_string()],
            },
            PercussionInstrument {
                id: "snare".to_string(),
                name: None,
                midi_unpitched: Some(38),
                staff_position: None,
                notehead: None,
                preferred_voice: None,
                techniques: Vec::new(),
            },
        ];

        let report = validate(&score);
        assert!(report.errors.iter().any(|error| matches!(
            error,
            ValidationError::InvalidPercussionInstrument {
                reason: PercussionInstrumentValidationReason::DuplicateId { first: 0 },
                ..
            }
        )));
        assert!(report.errors.iter().any(|error| matches!(
            error,
            ValidationError::InvalidPercussionInstrument {
                reason: PercussionInstrumentValidationReason::InvalidStaffPosition {
                    staff_position: 40
                },
                ..
            }
        )));
        assert!(report.errors.iter().any(|error| matches!(
            error,
            ValidationError::InvalidPercussionInstrument {
                reason: PercussionInstrumentValidationReason::InvalidPreferredVoice {
                    preferred_voice: 5
                },
                ..
            }
        )));
    }

    #[test]
    fn validate_rejects_invalid_instrument_definition() {
        let mut score = Score::new("Instrument", 120, 4, 4, 0, 1);
        score.parts[0].instrument = Some(InstrumentDefinition {
            id: String::new(),
            name: "Broken".to_string(),
            short_name: String::new(),
            family: None,
            transpose_semitones: 0,
            written_range: Some(InstrumentRange {
                lowest: 80,
                highest: 40,
            }),
            sounding_range: None,
            default_clefs: Vec::new(),
            staff_count: 0,
            staff_kind: StaffKind::Standard,
            midi_channel: 16,
            midi_program: 0,
            percussion_map_id: None,
        });

        let report = validate(&score);
        assert!(report.errors.iter().any(|error| matches!(
            error,
            ValidationError::InvalidInstrumentDefinition {
                reason: InstrumentDefinitionValidationReason::EmptyId,
                ..
            }
        )));
        assert!(report.errors.iter().any(|error| matches!(
            error,
            ValidationError::InvalidInstrumentDefinition {
                reason: InstrumentDefinitionValidationReason::InvalidStaffCount { staff_count: 0 },
                ..
            }
        )));
        assert!(report.errors.iter().any(|error| matches!(
            error,
            ValidationError::InvalidInstrumentDefinition {
                reason: InstrumentDefinitionValidationReason::InvalidRange {
                    kind: InstrumentRangeKind::Written,
                    ..
                },
                ..
            }
        )));
    }

    #[test]
    fn validate_empty_part_warning() {
        let score = Score::new("T", 120, 4, 4, 0, 1);
        let report = validate(&score);
        assert!(
            report
                .warnings
                .iter()
                .any(|w| matches!(w, ValidationWarning::EmptyPart { part: 0 }))
        );
    }

    #[test]
    fn validate_duplicate_rehearsal_mark_warning() {
        use crate::model::score::Score;
        let mut score = Score::new("T", 120, 4, 4, 0, 2);
        score.parts[0].staves[0].measures[0].rehearsal = Some("A".to_string());
        score.parts[0].staves[0].measures[1].rehearsal = Some("A".to_string());
        let report = validate(&score);
        assert!(report.warnings.iter().any(
            |w| matches!(w, ValidationWarning::DuplicateRehearsalMark { mark } if mark == "A")
        ));
    }
}
