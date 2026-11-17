//! Opt-in melody activity on a performance copy of a source score.

use serde::{Deserialize, Serialize};

use super::chordal_pad::{emit_rest, tick};
use super::ensemble_roles::{
    EnsembleMeasureActivity, EnsembleRoleOptions, EnsembleRolePart, arrange_ensemble_roles,
};
use super::repeat::measure_sequence;
use super::score::{NoteAddr, Score, Staff};

const MAX_MEASURES: usize = 512;
const MAX_SECTIONS: usize = 512;

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct MelodyRoleSection {
    pub start_measure: usize,
    pub end_measure_exclusive: usize,
    pub enabled: bool,
}

#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
pub struct EnsembleMelodyRequest {
    pub ensemble: EnsembleRoleOptions,
    pub melody_part_index: usize,
    pub melody_staff_index: usize,
    /// Overrides the default active melody in half-open physical-measure ranges.
    #[serde(default)]
    pub melody_sections: Vec<MelodyRoleSection>,
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct EnsembleMelodyDiagnostic {
    pub code: String,
    pub measure_index: Option<usize>,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct EnsembleMelodyResult {
    /// Performance score; disabled melody measures contain duration-matched rests.
    pub score: Option<Score>,
    pub resolved_request: EnsembleMelodyRequest,
    pub melody_activity: Vec<bool>,
    pub role_activity: Vec<EnsembleMeasureActivity>,
    pub role_parts: Vec<EnsembleRolePart>,
    pub diagnostics: Vec<EnsembleMelodyDiagnostic>,
    pub contract_version: u16,
}

fn fail(
    mut result: EnsembleMelodyResult,
    code: &str,
    measure_index: Option<usize>,
) -> EnsembleMelodyResult {
    result.score = None;
    result.melody_activity.clear();
    result.role_activity.clear();
    result.role_parts.clear();
    result.diagnostics.push(EnsembleMelodyDiagnostic {
        code: code.into(),
        measure_index,
    });
    result
}

fn has_active_jump_tie_target(
    source: &Score,
    staff: &Staff,
    sequence: &[usize],
    activity: &[bool],
    from: &NoteAddr,
    pitch_index: usize,
) -> bool {
    let Some(voice) = staff
        .measures
        .get(from.measure)
        .and_then(|measure| measure.voices.get(from.voice))
    else {
        return false;
    };
    let Some(note) = voice.get(from.note) else {
        return false;
    };
    let source_onset = voice
        .iter()
        .take(from.note)
        .map(|note| note.beats())
        .sum::<f64>();
    if (source_onset + note.beats() - source.playback_measure_beats(from.measure)).abs() > 1e-9 {
        return false;
    }
    let Some(pitch) = note.pitches.get(pitch_index) else {
        return false;
    };
    sequence.windows(2).any(|pair| {
        pair[0] == from.measure
            && pair[1] != from.measure + 1
            && activity.get(pair[1]) == Some(&true)
            && staff.measures.get(pair[1]).is_some_and(|measure| {
                measure
                    .voices
                    .iter()
                    .enumerate()
                    .any(|(voice_index, voice)| {
                        let mut onset = 0.0;
                        for (note_index, next) in voice.iter().enumerate() {
                            if onset > 1e-9 {
                                break;
                            }
                            if !next.is_rest {
                                let target = NoteAddr {
                                    part: from.part,
                                    staff: from.staff,
                                    measure: pair[1],
                                    voice: voice_index,
                                    note: note_index,
                                };
                                if next.pitches.iter().enumerate().any(|(index, next_pitch)| {
                                    next_pitch.to_midi_cents() == pitch.to_midi_cents()
                                        && (next.pitch_tie_end(index)
                                            || next.pitch_incoming_partial_ties.get(index)
                                                == Some(&true))
                                        && next
                                            .pitch_tie_sources
                                            .get(index)
                                            .and_then(Option::as_ref)
                                            .is_none_or(|location| {
                                                location.matches(
                                                    &target,
                                                    from,
                                                    source_onset - onset,
                                                )
                                            })
                                }) {
                                    return true;
                                }
                            }
                            onset += next.beats();
                        }
                        false
                    })
            })
    })
}

fn has_active_jump_tie_source(
    source: &Score,
    staff: &Staff,
    sequence: &[usize],
    activity: &[bool],
    target: &NoteAddr,
    pitch_index: usize,
) -> bool {
    let Some(voice) = staff
        .measures
        .get(target.measure)
        .and_then(|measure| measure.voices.get(target.voice))
    else {
        return false;
    };
    let Some(note) = voice.get(target.note) else {
        return false;
    };
    let target_onset = voice
        .iter()
        .take(target.note)
        .map(|note| note.beats())
        .sum::<f64>();
    if target_onset > 1e-9 {
        return false;
    }
    let Some(pitch) = note.pitches.get(pitch_index) else {
        return false;
    };
    let explicit_source = note
        .pitch_tie_sources
        .get(pitch_index)
        .and_then(Option::as_ref);
    sequence.windows(2).any(|pair| {
        pair[1] == target.measure
            && pair[0] + 1 != target.measure
            && activity.get(pair[0]) == Some(&true)
            && staff.measures.get(pair[0]).is_some_and(|measure| {
                measure
                    .voices
                    .iter()
                    .enumerate()
                    .any(|(voice_index, voice)| {
                        let mut onset = 0.0;
                        for (note_index, previous) in voice.iter().enumerate() {
                            let end = onset + previous.beats();
                            if !previous.is_rest
                                && (end - source.playback_measure_beats(pair[0])).abs() < 1e-9
                            {
                                let previous_address = NoteAddr {
                                    part: target.part,
                                    staff: target.staff,
                                    measure: pair[0],
                                    voice: voice_index,
                                    note: note_index,
                                };
                                if previous.pitches.iter().enumerate().any(
                                    |(index, previous_pitch)| {
                                        previous_pitch.to_midi_cents() == pitch.to_midi_cents()
                                            && previous.pitch_tie_start(index)
                                            && explicit_source.is_none_or(|location| {
                                                location.matches(
                                                    target,
                                                    &previous_address,
                                                    onset - target_onset,
                                                )
                                            })
                                    },
                                ) {
                                    return true;
                                }
                            }
                            onset = end;
                        }
                        false
                    })
            })
    })
}

/// Build a performance copy whose selected original melody staff may rest in
/// specified measures. The borrowed source score and its note IDs never change.
/// Other original parts are copied verbatim. Generate counterpoint before
/// muting melody so motif extraction always sees the authored source.
pub fn arrange_ensemble_with_melody(
    source: &Score,
    mut request: EnsembleMelodyRequest,
) -> EnsembleMelodyResult {
    request
        .melody_sections
        .sort_by_key(|section| (section.start_measure, section.end_measure_exclusive));
    let mut result = EnsembleMelodyResult {
        score: None,
        resolved_request: request,
        melody_activity: Vec::new(),
        role_activity: Vec::new(),
        role_parts: Vec::new(),
        diagnostics: Vec::new(),
        contract_version: 1,
    };
    let mut request = result.resolved_request.clone();
    if request.melody_staff_index != 0 {
        return fail(result, "unsupported-melody-staff", None);
    }
    let Some(staff) = source
        .parts
        .get(request.melody_part_index)
        .and_then(|part| part.staves.get(request.melody_staff_index))
    else {
        return fail(result, "melody-staff-unavailable", None);
    };
    let count = staff.measures.len();
    if count == 0 || count > MAX_MEASURES || request.melody_sections.len() > MAX_SECTIONS {
        return fail(result, "melody-input-limit", None);
    }
    if request
        .ensemble
        .harmony
        .as_ref()
        .and_then(|options| options.melody_part_index)
        .is_some_and(|index| index != request.melody_part_index)
        || request
            .ensemble
            .countermelody
            .as_ref()
            .is_some_and(|options| options.melody_part_index != request.melody_part_index)
    {
        return fail(result, "melody-source-mismatch", None);
    }
    if let Some(harmony) = request.ensemble.harmony.as_mut()
        && harmony.min_melody_gap_semitones.is_some()
    {
        harmony.melody_part_index = Some(request.melody_part_index);
    }
    let mut activity = vec![true; count];
    let mut previous_end = 0;
    for section in &request.melody_sections {
        if section.start_measure >= section.end_measure_exclusive
            || section.end_measure_exclusive > count
            || section.start_measure < previous_end
        {
            return fail(
                result,
                "invalid-melody-section",
                Some(section.start_measure),
            );
        }
        activity[section.start_measure..section.end_measure_exclusive].fill(section.enabled);
        previous_end = section.end_measure_exclusive;
    }
    let mut arranged = arrange_ensemble_roles(source, request.ensemble);
    let Some(mut output) = arranged.score.take() else {
        let issue = arranged.diagnostics.first();
        return fail(
            result,
            issue.map_or("ensemble-failed", |issue| issue.code.as_str()),
            issue.and_then(|issue| issue.measure_index),
        );
    };
    for (measure, enabled) in activity.iter().enumerate() {
        if *enabled {
            continue;
        }
        let Some(ticks) = tick(output.playback_measure_beats(measure)) else {
            return fail(result, "unsupported-rhythm", Some(measure));
        };
        let mut rests = Vec::new();
        if !emit_rest(ticks, &mut rests) {
            return fail(result, "unsupported-rhythm", Some(measure));
        }
        let bar = &mut output.parts[request.melody_part_index].staves[request.melody_staff_index]
            .measures[measure];
        for voice in &mut bar.voices {
            if !voice.is_empty() {
                *voice = rests.clone();
            }
        }
    }
    // A muted bar breaks a written tie chain. Close the surviving note on
    // either side so playback and MusicXML do not retain a dangling tie.
    let sequence = measure_sequence(source);
    let source_staff = &source.parts[request.melody_part_index].staves[request.melody_staff_index];
    let melody = &mut output.parts[request.melody_part_index].staves[request.melody_staff_index];
    for measure in 1..count {
        if activity[measure - 1] && !activity[measure] {
            for (voice_index, voice) in melody.measures[measure - 1].voices.iter_mut().enumerate() {
                if let Some(note_index) = voice.iter().rposition(|note| !note.is_rest) {
                    let note = &mut voice[note_index];
                    let ends = note.pitch_tie_ends_or_uniform();
                    let starts = note
                        .pitch_tie_starts_or_uniform()
                        .into_iter()
                        .enumerate()
                        .map(|(pitch_index, tied)| {
                            tied && has_active_jump_tie_target(
                                source,
                                source_staff,
                                &sequence,
                                &activity,
                                &NoteAddr {
                                    part: request.melody_part_index,
                                    staff: request.melody_staff_index,
                                    measure: measure - 1,
                                    voice: voice_index,
                                    note: note_index,
                                },
                                pitch_index,
                            )
                        })
                        .collect::<Vec<_>>();
                    note.set_pitch_ties(&starts, &ends);
                }
            }
        } else if !activity[measure - 1] && activity[measure] {
            for (voice_index, voice) in melody.measures[measure].voices.iter_mut().enumerate() {
                if let Some(note_index) = voice.iter().position(|note| !note.is_rest) {
                    let note = &mut voice[note_index];
                    let starts = note.pitch_tie_starts_or_uniform();
                    let ends = note
                        .pitch_tie_ends_or_uniform()
                        .into_iter()
                        .enumerate()
                        .map(|(pitch_index, tied)| {
                            tied && has_active_jump_tie_source(
                                source,
                                source_staff,
                                &sequence,
                                &activity,
                                &NoteAddr {
                                    part: request.melody_part_index,
                                    staff: request.melody_staff_index,
                                    measure,
                                    voice: voice_index,
                                    note: note_index,
                                },
                                pitch_index,
                            )
                        })
                        .collect::<Vec<_>>();
                    note.set_pitch_ties(&starts, &ends);
                }
            }
        }
    }
    if !super::validate::validate(&output).is_valid() {
        return fail(result, "invalid-performance-score", None);
    }
    result.resolved_request.ensemble = arranged.resolved_options;
    result.melody_activity = activity;
    result.role_activity = arranged.activity;
    result.role_parts = arranged.role_parts;
    result.score = Some(output);
    result
}
