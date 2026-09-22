//! Serializable, non-mutating previews for structural score transformations.

use super::commands::{Command, apply_command, command_key};
use super::duration::Duration;
use super::score::{NoteAddr, Score};
use super::validate::validate;
use crate::Error;
use serde::{Deserialize, Serialize};
use std::collections::{BTreeMap, BTreeSet};

/// Version of the structural change-plan JSON contract.
pub const STRUCTURAL_CHANGE_PLAN_CONTRACT_VERSION: u32 = 1;

/// The reason a structural command cannot safely be applied.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum StructuralChangeDiagnosticKind {
    Conflict,
    Unsupported,
    Invalid,
    ValidationFailed,
}

/// A machine-readable pre-mutation diagnostic.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct StructuralChangeDiagnostic {
    pub kind: StructuralChangeDiagnosticKind,
    pub message: String,
}

/// The resulting editable voice structure for one physical measure.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct StructuralMeasureStructure {
    pub part: usize,
    pub staff: usize,
    pub measure: usize,
    pub exists: bool,
    pub voice_note_counts: [usize; 4],
    pub source_voice_numbers: [Option<u32>; 4],
}

/// A side-effect-free preview of a structural command.
///
/// When `can_apply` is true, `affected_addresses` includes both old and new
/// note addresses for moved notes and `resulting_measures` records every lane
/// whose voice structure changes.  A false plan never exposes a partial score:
/// it contains diagnostics only.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct StructuralChangePlan {
    pub contract_version: u32,
    pub command_key: String,
    pub can_apply: bool,
    pub affected_addresses: Vec<NoteAddr>,
    pub resulting_measures: Vec<StructuralMeasureStructure>,
    pub diagnostics: Vec<StructuralChangeDiagnostic>,
}

fn is_structural_command(command: &Command) -> bool {
    matches!(
        command,
        Command::ExchangeVoices(_)
            | Command::MoveOrCopyVoiceRange(_)
            | Command::SplitMeasure(_)
            | Command::JoinMeasures(_)
            | Command::ImplodeStaves(_)
            | Command::ExplodeVoices(_)
            | Command::ExplodeChordPitches(_)
            | Command::ScaleVoiceRange(_)
    )
}

fn diagnostic_kind(message: &str) -> StructuralChangeDiagnosticKind {
    if message.contains("not yet supported") || message.contains("does not support") {
        StructuralChangeDiagnosticKind::Unsupported
    } else if message.contains("discard")
        || message.contains("destination")
        || message.contains("onto itself")
        || message.contains("conflict")
    {
        StructuralChangeDiagnosticKind::Conflict
    } else {
        StructuralChangeDiagnosticKind::Invalid
    }
}

#[derive(Clone, PartialEq, Eq)]
struct NoteRecord {
    address: NoteAddr,
    duration: Duration,
    dot_count: u8,
}

fn note_records(score: &Score) -> BTreeMap<String, Vec<NoteRecord>> {
    let mut addresses = BTreeMap::new();
    for (part, part_data) in score.parts.iter().enumerate() {
        for (staff, staff_data) in part_data.staves.iter().enumerate() {
            for (measure, measure_data) in staff_data.measures.iter().enumerate() {
                for (voice, notes) in measure_data.voices.iter().enumerate() {
                    for (note, note_data) in notes.iter().enumerate() {
                        addresses
                            .entry(note_data.id.clone())
                            .or_insert_with(Vec::new)
                            .push(NoteRecord {
                                address: NoteAddr {
                                    part,
                                    staff,
                                    measure,
                                    voice,
                                    note,
                                },
                                duration: note_data.duration.clone(),
                                dot_count: note_data.dot_count,
                            });
                    }
                }
            }
        }
    }
    addresses
}

fn sorted_unique_addresses(mut addresses: Vec<NoteAddr>) -> Vec<NoteAddr> {
    addresses.sort_by_key(|address| {
        (
            address.part,
            address.staff,
            address.measure,
            address.voice,
            address.note,
        )
    });
    addresses.dedup();
    addresses
}

fn affected_addresses(before: &Score, after: &Score) -> Vec<NoteAddr> {
    let before = note_records(before);
    let after = note_records(after);
    let ids: BTreeSet<_> = before.keys().chain(after.keys()).cloned().collect();
    let mut affected = Vec::new();
    for id in ids {
        let old = before.get(&id);
        let new = after.get(&id);
        if old != new {
            affected.extend(
                old.into_iter()
                    .flatten()
                    .map(|record| record.address.clone()),
            );
            affected.extend(
                new.into_iter()
                    .flatten()
                    .map(|record| record.address.clone()),
            );
        }
    }
    sorted_unique_addresses(affected)
}

fn measure_structure(
    score: &Score,
    part: usize,
    staff: usize,
    measure: usize,
) -> StructuralMeasureStructure {
    let current = score
        .parts
        .get(part)
        .and_then(|part_data| part_data.staves.get(staff))
        .and_then(|staff_data| staff_data.measures.get(measure));
    match current {
        Some(current) => StructuralMeasureStructure {
            part,
            staff,
            measure,
            exists: true,
            voice_note_counts: std::array::from_fn(|voice| current.voices[voice].len()),
            source_voice_numbers: current.source_voice_numbers,
        },
        None => StructuralMeasureStructure {
            part,
            staff,
            measure,
            exists: false,
            voice_note_counts: [0; 4],
            source_voice_numbers: [None; 4],
        },
    }
}

fn resulting_measures(before: &Score, after: &Score) -> Vec<StructuralMeasureStructure> {
    let mut result = Vec::new();
    let part_count = before.parts.len().max(after.parts.len());
    for part in 0..part_count {
        let staff_count = before
            .parts
            .get(part)
            .map(|part_data| part_data.staves.len())
            .unwrap_or(0)
            .max(
                after
                    .parts
                    .get(part)
                    .map(|part_data| part_data.staves.len())
                    .unwrap_or(0),
            );
        for staff in 0..staff_count {
            let measure_count = before
                .parts
                .get(part)
                .and_then(|part_data| part_data.staves.get(staff))
                .map(|staff_data| staff_data.measures.len())
                .unwrap_or(0)
                .max(
                    after
                        .parts
                        .get(part)
                        .and_then(|part_data| part_data.staves.get(staff))
                        .map(|staff_data| staff_data.measures.len())
                        .unwrap_or(0),
                );
            for measure in 0..measure_count {
                let old = measure_structure(before, part, staff, measure);
                let new = measure_structure(after, part, staff, measure);
                if old != new {
                    result.push(new);
                }
            }
        }
    }
    result
}

/// Preview a structural command without changing `score` or any command history.
///
/// Only commands that rearrange voices, ranges, or measures are accepted.  A
/// malformed structural command yields a serializable unsuccessful plan rather
/// than a partially-applied candidate score.
pub fn plan_structural_change(
    score: &Score,
    command: &Command,
) -> Result<StructuralChangePlan, Error> {
    if !is_structural_command(command) {
        return Err(Error::InvalidCommand(
            "structural change plans require a structural command".into(),
        ));
    }
    let command_key = command_key(command);
    let initial_validation = validate(score);
    if !initial_validation.is_valid() {
        return Ok(StructuralChangePlan {
            contract_version: STRUCTURAL_CHANGE_PLAN_CONTRACT_VERSION,
            command_key,
            can_apply: false,
            affected_addresses: Vec::new(),
            resulting_measures: Vec::new(),
            diagnostics: vec![StructuralChangeDiagnostic {
                kind: StructuralChangeDiagnosticKind::ValidationFailed,
                message: format!(
                    "source score failed structural validation with {} error(s)",
                    initial_validation.errors.len()
                ),
            }],
        });
    }
    let mut candidate = score.clone();
    if let Err(error) = apply_command(command, &mut candidate) {
        let message = error.to_string();
        return Ok(StructuralChangePlan {
            contract_version: STRUCTURAL_CHANGE_PLAN_CONTRACT_VERSION,
            command_key,
            can_apply: false,
            affected_addresses: Vec::new(),
            resulting_measures: Vec::new(),
            diagnostics: vec![StructuralChangeDiagnostic {
                kind: diagnostic_kind(&message),
                message,
            }],
        });
    }
    let validation = validate(&candidate);
    if !validation.is_valid() {
        return Ok(StructuralChangePlan {
            contract_version: STRUCTURAL_CHANGE_PLAN_CONTRACT_VERSION,
            command_key,
            can_apply: false,
            affected_addresses: Vec::new(),
            resulting_measures: Vec::new(),
            diagnostics: vec![StructuralChangeDiagnostic {
                kind: StructuralChangeDiagnosticKind::ValidationFailed,
                message: format!(
                    "result would fail structural validation with {} error(s)",
                    validation.errors.len()
                ),
            }],
        });
    }
    Ok(StructuralChangePlan {
        contract_version: STRUCTURAL_CHANGE_PLAN_CONTRACT_VERSION,
        command_key,
        can_apply: true,
        affected_addresses: affected_addresses(score, &candidate),
        resulting_measures: resulting_measures(score, &candidate),
        diagnostics: Vec::new(),
    })
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::{
        Duration, DurationScale, ExplodeChordPitchesCmd, ExplodeVoicesCmd, ImplodeStavesCmd, Note,
        Pitch, ScaleVoiceRangeCmd, ScoreTemplate, Step, TupletInfo,
    };

    fn piano_score() -> Score {
        let mut score = Score::template(ScoreTemplate::Piano);
        score.parts[0].staves[0].measures[0].voices[0] =
            vec![Note::new(Pitch::new(Step::C, 5), Duration::Whole)];
        score.parts[0].staves[1].measures[0].voices[0] =
            vec![Note::new(Pitch::new(Step::E, 3), Duration::Whole)];
        score
    }

    #[test]
    fn implode_plan_is_non_mutating_and_describes_resulting_voices() {
        let score = piano_score();
        let before = serde_json::to_value(&score).unwrap();
        let plan = plan_structural_change(
            &score,
            &Command::ImplodeStaves(ImplodeStavesCmd {
                part_index: 0,
                source_staves: vec![0, 1],
                target_staff: 0,
                start_measure: 0,
                end_measure: 0,
            }),
        )
        .unwrap();
        assert!(plan.can_apply);
        assert_eq!(plan.command_key, "ImplodeStaves");
        assert!(plan.affected_addresses.iter().any(|address| {
            address.part == 0
                && address.staff == 1
                && address.measure == 0
                && address.voice == 0
                && address.note == 0
        }));
        assert!(plan.affected_addresses.iter().any(|address| {
            address.part == 0
                && address.staff == 0
                && address.measure == 0
                && address.voice == 1
                && address.note == 0
        }));
        assert!(plan.resulting_measures.iter().any(|measure| {
            measure.part == 0
                && measure.staff == 0
                && measure.measure == 0
                && measure.voice_note_counts == [1, 1, 0, 0]
        }));
        assert_eq!(serde_json::to_value(&score).unwrap(), before);
    }

    #[test]
    fn explode_plan_reports_destination_conflict_without_mutation() {
        let mut score = piano_score();
        score.parts[0].staves[0].measures[0].voices[1] =
            vec![Note::new(Pitch::new(Step::G, 4), Duration::Whole)];
        let before = serde_json::to_value(&score).unwrap();
        let plan = plan_structural_change(
            &score,
            &Command::ExplodeVoices(ExplodeVoicesCmd {
                part_index: 0,
                source_staff: 0,
                target_staves: vec![0, 1],
                start_measure: 0,
                end_measure: 0,
            }),
        )
        .unwrap();
        assert!(!plan.can_apply);
        assert_eq!(
            plan.diagnostics[0].kind,
            StructuralChangeDiagnosticKind::Conflict
        );
        assert_eq!(serde_json::to_value(&score).unwrap(), before);
    }

    #[test]
    fn scale_plan_reports_tuplet_limit_without_mutation() {
        let mut score = piano_score();
        score.parts[0].staves[0].measures[0].voices[0][0].tuplet = Some(TupletInfo {
            actual_notes: 3,
            normal_notes: 2,
        });
        let before = serde_json::to_value(&score).unwrap();
        let plan = plan_structural_change(
            &score,
            &Command::ScaleVoiceRange(ScaleVoiceRangeCmd {
                part_index: 0,
                staff_index: 0,
                voice: 0,
                start_measure: 0,
                end_measure: 0,
                scale: DurationScale::Half,
            }),
        )
        .unwrap();
        assert!(!plan.can_apply);
        assert_eq!(
            plan.diagnostics[0].kind,
            StructuralChangeDiagnosticKind::Unsupported
        );
        assert_eq!(serde_json::to_value(&score).unwrap(), before);
    }

    #[test]
    fn scale_plan_marks_duration_changes_at_stable_addresses() {
        let score = piano_score();
        let plan = plan_structural_change(
            &score,
            &Command::ScaleVoiceRange(ScaleVoiceRangeCmd {
                part_index: 0,
                staff_index: 0,
                voice: 0,
                start_measure: 0,
                end_measure: 0,
                scale: DurationScale::Half,
            }),
        )
        .unwrap();
        assert!(plan.can_apply);
        assert!(plan.affected_addresses.iter().any(|address| {
            address.part == 0
                && address.staff == 0
                && address.measure == 0
                && address.voice == 0
                && address.note == 0
        }));
        assert!(plan.resulting_measures.iter().any(|measure| {
            measure.part == 0
                && measure.staff == 0
                && measure.measure == 0
                && measure.voice_note_counts == [2, 0, 0, 0]
        }));
    }

    #[test]
    fn chord_explode_plan_is_available_before_mutation() {
        let mut score = piano_score();
        score.parts[0].staves[0].measures[0].voices[0][0]
            .pitches
            .push(Pitch::new(Step::E, 4));
        score.parts[0].staves[1].measures[0].voices[0] = vec![Note::rest(Duration::Whole)];
        let plan = plan_structural_change(
            &score,
            &Command::ExplodeChordPitches(ExplodeChordPitchesCmd {
                part_index: 0,
                source_staff: 0,
                target_staves: vec![0, 1],
                start_measure: 0,
                end_measure: 0,
            }),
        )
        .unwrap();
        assert!(plan.can_apply);
        assert!(plan.affected_addresses.iter().any(|address| {
            address.part == 0
                && address.staff == 1
                && address.measure == 0
                && address.voice == 0
                && address.note == 0
        }));
    }
}
