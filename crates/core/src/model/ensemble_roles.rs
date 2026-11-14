//! Compose independently scheduled harmony, bass and countermelody around a source.

use serde::{Deserialize, Serialize};

use super::accordion_dynamics::supported_level;
use super::bass_line::{BassLineOptions, arrange_bass_line};
use super::chordal_pad::{ChordalPadOptions, arrange_chordal_pad, emit_rest, tick};
use super::countermelody::{CountermelodyOptions, arrange_countermelody_with_activity};
use super::notation::Dynamic;
use super::score::{NoteAddr, Score};

const MAX_SECTIONS: usize = 512;
const MAX_MEASURES: usize = 512;

fn is_false(value: &bool) -> bool {
    !*value
}

#[derive(Debug, Clone, Copy, PartialEq, Eq, PartialOrd, Ord, Serialize, Deserialize)]
#[serde(rename_all = "kebab-case")]
pub enum EnsembleRole {
    Harmony,
    Bass,
    Countermelody,
}

/// Half-open physical-measure range. Each role defaults to active when its
/// options are supplied; sections override that role without affecting others.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct EnsembleRoleSection {
    pub role: EnsembleRole,
    pub start_measure: usize,
    pub end_measure_exclusive: usize,
    pub enabled: bool,
}

#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
pub struct EnsembleRoleOptions {
    #[serde(default)]
    pub harmony: Option<ChordalPadOptions>,
    #[serde(default)]
    pub bass: Option<BassLineOptions>,
    #[serde(default)]
    pub countermelody: Option<CountermelodyOptions>,
    /// Optional sustained level for the first sounding note of each generated
    /// role. A level carries through later notes on that role's staff.
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub harmony_dynamic: Option<Dynamic>,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub bass_dynamic: Option<Dynamic>,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub countermelody_dynamic: Option<Dynamic>,
    #[serde(default)]
    pub sections: Vec<EnsembleRoleSection>,
    /// Pause motif development while the countermelody role is inactive and
    /// restart its contour when the role enters again. Defaults to continuous
    /// contour so saved arrangement contracts retain their playback.
    #[serde(default, skip_serializing_if = "is_false")]
    pub restart_countermelody_at_section_entry: bool,
    /// Minimum semitone separation from the bass note to every simultaneous
    /// harmony tone. Zero still rejects unison and bass above harmony.
    #[serde(default)]
    pub min_bass_harmony_gap_semitones: u8,
    /// Minimum separation between every simultaneous harmony and
    /// countermelody tone. Zero still rejects unison and role crossing.
    #[serde(default)]
    pub min_harmony_countermelody_gap_semitones: u8,
    /// Maximum number of generated note attacks, counting a chord as one
    /// attack, in any physical measure. Omit to leave density unconstrained.
    #[serde(default)]
    pub max_generated_attacks_per_measure: Option<usize>,
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct EnsembleMeasureActivity {
    pub harmony: bool,
    pub bass: bool,
    pub countermelody: bool,
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct EnsembleRolePart {
    pub role: EnsembleRole,
    pub part_index: usize,
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct EnsembleRoleDiagnostic {
    pub code: String,
    pub role: Option<EnsembleRole>,
    pub measure_index: Option<usize>,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct EnsembleRoleResult {
    pub score: Option<Score>,
    pub resolved_options: EnsembleRoleOptions,
    pub activity: Vec<EnsembleMeasureActivity>,
    pub role_parts: Vec<EnsembleRolePart>,
    pub generated_notes: Vec<NoteAddr>,
    pub diagnostics: Vec<EnsembleRoleDiagnostic>,
    pub contract_version: u16,
}

fn fail(
    mut result: EnsembleRoleResult,
    code: &str,
    role: Option<EnsembleRole>,
    measure_index: Option<usize>,
) -> EnsembleRoleResult {
    result.score = None;
    result.activity.clear();
    result.role_parts.clear();
    result.generated_notes.clear();
    result.diagnostics.push(EnsembleRoleDiagnostic {
        code: code.into(),
        role,
        measure_index,
    });
    result
}

fn sounding_spans(score: &Score, part: usize, measure: usize) -> Vec<(f64, f64, i16)> {
    let mut spans = Vec::new();
    let Some(voice) = score
        .parts
        .get(part)
        .and_then(|part| part.staves.first())
        .and_then(|staff| staff.measures.get(measure))
        .and_then(|bar| bar.voices.first())
    else {
        return spans;
    };
    let mut onset = 0.0;
    for note in voice {
        let end = onset + note.beats();
        if !note.is_rest {
            spans.extend(
                note.pitches
                    .iter()
                    .map(|pitch| (onset, end, pitch.to_midi())),
            );
        }
        onset = end;
    }
    spans
}

fn attacks(score: &Score, part: usize, measure: usize) -> usize {
    score.parts[part].staves[0].measures[measure].voices[0]
        .iter()
        .filter(|note| !note.is_rest && !note.tie_end)
        .count()
}

fn vertical_collision(lower: &[(f64, f64, i16)], upper: &[(f64, f64, i16)], gap: u8) -> bool {
    let gap = i16::from(gap.max(1));
    lower.iter().any(|&(lower_start, lower_end, lower_pitch)| {
        upper.iter().any(|&(upper_start, upper_end, upper_pitch)| {
            lower_start < upper_end - 1e-9
                && lower_end > upper_start + 1e-9
                && upper_pitch - lower_pitch < gap
        })
    })
}

/// Append role parts, then apply independent measure activity. Original source
/// parts and note IDs remain unchanged. All invalid requests return no score.
pub fn arrange_ensemble_roles(
    score: &Score,
    mut options: EnsembleRoleOptions,
) -> EnsembleRoleResult {
    options.sections.sort_by_key(|section| {
        (
            section.role,
            section.start_measure,
            section.end_measure_exclusive,
        )
    });
    let mut result = EnsembleRoleResult {
        score: None,
        resolved_options: options,
        activity: Vec::new(),
        role_parts: Vec::new(),
        generated_notes: Vec::new(),
        diagnostics: Vec::new(),
        contract_version: 1,
    };
    let Some(reference) = score.parts.first().and_then(|part| part.staves.first()) else {
        return fail(result, "unsupported-source", None, None);
    };
    let count = reference.measures.len();
    if count == 0
        || count > MAX_MEASURES
        || result.resolved_options.sections.len() > MAX_SECTIONS
        || result.resolved_options.min_bass_harmony_gap_semitones > 24
        || result
            .resolved_options
            .min_harmony_countermelody_gap_semitones
            > 24
        || result
            .resolved_options
            .max_generated_attacks_per_measure
            .is_some_and(|limit| !(1..=64).contains(&limit))
    {
        return fail(result, "ensemble-input-limit", None, None);
    }
    for (role, level, present) in [
        (
            EnsembleRole::Harmony,
            result.resolved_options.harmony_dynamic,
            result.resolved_options.harmony.is_some(),
        ),
        (
            EnsembleRole::Bass,
            result.resolved_options.bass_dynamic,
            result.resolved_options.bass.is_some(),
        ),
        (
            EnsembleRole::Countermelody,
            result.resolved_options.countermelody_dynamic,
            result.resolved_options.countermelody.is_some(),
        ),
    ] {
        if level.is_some_and(|dynamic| !supported_level(dynamic)) {
            return fail(result, "unsupported-dynamic", Some(role), None);
        }
        if level.is_some() && !present {
            return fail(result, "missing-role-options", Some(role), None);
        }
    }
    if !super::validate::validate(score).is_valid() {
        return fail(result, "invalid-source", None, None);
    }
    let mut activity = vec![
        EnsembleMeasureActivity {
            harmony: result.resolved_options.harmony.is_some(),
            bass: result.resolved_options.bass.is_some(),
            countermelody: result.resolved_options.countermelody.is_some(),
        };
        count
    ];
    let mut previous_role = None;
    let mut previous_end = 0;
    for section in result.resolved_options.sections.clone() {
        if section.start_measure >= section.end_measure_exclusive
            || section.end_measure_exclusive > count
            || previous_role == Some(section.role) && section.start_measure < previous_end
        {
            return fail(
                result,
                "invalid-role-section",
                Some(section.role),
                Some(section.start_measure),
            );
        }
        if section.enabled
            && match section.role {
                EnsembleRole::Harmony => result.resolved_options.harmony.is_none(),
                EnsembleRole::Bass => result.resolved_options.bass.is_none(),
                EnsembleRole::Countermelody => result.resolved_options.countermelody.is_none(),
            }
        {
            return fail(
                result,
                "missing-role-options",
                Some(section.role),
                Some(section.start_measure),
            );
        }
        for state in &mut activity[section.start_measure..section.end_measure_exclusive] {
            match section.role {
                EnsembleRole::Harmony => state.harmony = section.enabled,
                EnsembleRole::Bass => state.bass = section.enabled,
                EnsembleRole::Countermelody => state.countermelody = section.enabled,
            }
        }
        previous_role = Some(section.role);
        previous_end = section.end_measure_exclusive;
    }
    let mut output = score.clone();
    if let Some(harmony) = result.resolved_options.harmony.clone() {
        let arranged = arrange_chordal_pad(&output, harmony);
        let Some(next) = arranged.score else {
            let issue = arranged.diagnostics.first();
            return fail(
                result,
                issue.map_or("harmony-failed", |issue| issue.code.as_str()),
                Some(EnsembleRole::Harmony),
                issue.and_then(|issue| issue.measure_index),
            );
        };
        result.role_parts.push(EnsembleRolePart {
            role: EnsembleRole::Harmony,
            part_index: output.parts.len(),
        });
        output = next;
    }
    if let Some(bass) = result.resolved_options.bass.clone() {
        let arranged = arrange_bass_line(&output, bass);
        let Some(next) = arranged.score else {
            let issue = arranged.diagnostics.first();
            return fail(
                result,
                issue.map_or("bass-failed", |issue| issue.code.as_str()),
                Some(EnsembleRole::Bass),
                issue.and_then(|issue| issue.measure_index),
            );
        };
        result.role_parts.push(EnsembleRolePart {
            role: EnsembleRole::Bass,
            part_index: output.parts.len(),
        });
        output = next;
    }
    if let Some(countermelody) = result.resolved_options.countermelody.clone() {
        let counter_activity = activity
            .iter()
            .map(|state| state.countermelody)
            .collect::<Vec<_>>();
        let arranged = arrange_countermelody_with_activity(
            &output,
            countermelody,
            result
                .resolved_options
                .restart_countermelody_at_section_entry
                .then_some(counter_activity.as_slice()),
        );
        let Some(next) = arranged.score else {
            let issue = arranged.diagnostics.first();
            return fail(
                result,
                issue.map_or("countermelody-failed", |issue| issue.code.as_str()),
                Some(EnsembleRole::Countermelody),
                issue.and_then(|issue| issue.measure_index),
            );
        };
        result.role_parts.push(EnsembleRolePart {
            role: EnsembleRole::Countermelody,
            part_index: output.parts.len(),
        });
        output = next;
    }
    for role_part in result.role_parts.clone() {
        for (measure, state) in activity.iter().enumerate() {
            let enabled = match role_part.role {
                EnsembleRole::Harmony => state.harmony,
                EnsembleRole::Bass => state.bass,
                EnsembleRole::Countermelody => state.countermelody,
            };
            if !enabled {
                let Some(ticks) = tick(output.playback_measure_beats(measure)) else {
                    return fail(
                        result,
                        "unsupported-rhythm",
                        Some(role_part.role),
                        Some(measure),
                    );
                };
                let mut rests = Vec::new();
                if !emit_rest(ticks, &mut rests) {
                    return fail(
                        result,
                        "unsupported-rhythm",
                        Some(role_part.role),
                        Some(measure),
                    );
                }
                output.parts[role_part.part_index].staves[0].measures[measure].voices[0] = rests;
            }
        }
    }
    for role_part in &result.role_parts {
        let level = match role_part.role {
            EnsembleRole::Harmony => result.resolved_options.harmony_dynamic,
            EnsembleRole::Bass => result.resolved_options.bass_dynamic,
            EnsembleRole::Countermelody => result.resolved_options.countermelody_dynamic,
        };
        if let Some(level) = level
            && let Some(note) = output.parts[role_part.part_index].staves[0]
                .measures
                .iter_mut()
                .flat_map(|measure| &mut measure.voices[0])
                .find(|note| !note.is_rest && !note.tie_end)
        {
            note.dynamic = Some(level);
        }
    }
    let harmony_part = result
        .role_parts
        .iter()
        .find(|part| part.role == EnsembleRole::Harmony)
        .map(|part| part.part_index);
    let bass_part = result
        .role_parts
        .iter()
        .find(|part| part.role == EnsembleRole::Bass)
        .map(|part| part.part_index);
    let countermelody_part = result
        .role_parts
        .iter()
        .find(|part| part.role == EnsembleRole::Countermelody)
        .map(|part| part.part_index);
    for measure in 0..count {
        if let (Some(harmony), Some(bass)) = (harmony_part, bass_part) {
            let harmony_spans = sounding_spans(&output, harmony, measure);
            let bass_spans = sounding_spans(&output, bass, measure);
            if vertical_collision(
                &bass_spans,
                &harmony_spans,
                result.resolved_options.min_bass_harmony_gap_semitones,
            ) {
                return fail(result, "bass-harmony-collision", None, Some(measure));
            }
        }
        if let (Some(harmony), Some(countermelody)) = (harmony_part, countermelody_part)
            && vertical_collision(
                &sounding_spans(&output, harmony, measure),
                &sounding_spans(&output, countermelody, measure),
                result
                    .resolved_options
                    .min_harmony_countermelody_gap_semitones,
            )
        {
            return fail(
                result,
                "harmony-countermelody-collision",
                None,
                Some(measure),
            );
        }
        if let (Some(bass), Some(countermelody)) = (bass_part, countermelody_part)
            && vertical_collision(
                &sounding_spans(&output, bass, measure),
                &sounding_spans(&output, countermelody, measure),
                1,
            )
        {
            return fail(result, "bass-countermelody-collision", None, Some(measure));
        }
        let mut attacks_in_measure = 0usize;
        for part in &result.role_parts {
            let voice = &output.parts[part.part_index].staves[0].measures[measure].voices[0];
            for (note, event) in voice.iter().enumerate() {
                if !event.is_rest {
                    result.generated_notes.push(NoteAddr {
                        part: part.part_index,
                        staff: 0,
                        measure,
                        voice: 0,
                        note,
                    });
                }
            }
            attacks_in_measure += attacks(&output, part.part_index, measure);
        }
        if result
            .resolved_options
            .max_generated_attacks_per_measure
            .is_some_and(|limit| attacks_in_measure > limit)
        {
            return fail(result, "density-limit", None, Some(measure));
        }
    }
    if !super::validate::validate(&output).is_valid() {
        return fail(result, "invalid-output", None, None);
    }
    result.activity = activity;
    result.score = Some(output);
    result
}
