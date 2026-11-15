//! Measure-addressed expressive dynamics for generated accompaniment.

use serde::{Deserialize, Serialize};

use super::accordion_sections::{
    SectionedAccordionOptions, SectionedAccordionResult, arrange_with_sections,
};
use super::arrange::GuidedArrangeDiagnostic;
use super::notation::Dynamic;
use super::score::{NoteAddr, Score};

const MAX_SECTIONS: usize = 512;

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct AccordionDynamicSection {
    /// Inclusive, zero-based physical measure indices.
    pub first_measure: usize,
    pub last_measure: usize,
    pub dynamic: Dynamic,
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct AccordionDynamicSchedule {
    pub default_dynamic: Dynamic,
    #[serde(default)]
    pub sections: Vec<AccordionDynamicSection>,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct DynamicSectionedAccordionResult {
    pub arrangement: SectionedAccordionResult,
    pub resolved_schedule: AccordionDynamicSchedule,
    /// One requested level per physical measure; empty on failure.
    pub resolved_dynamics: Vec<Dynamic>,
    /// First sounding accompaniment note of each non-silent measure.
    pub dynamic_marks: Vec<NoteAddr>,
    pub contract_version: u16,
}

pub(super) fn supported_level(dynamic: Dynamic) -> bool {
    matches!(
        dynamic,
        Dynamic::Pppp
            | Dynamic::Ppp
            | Dynamic::Pp
            | Dynamic::P
            | Dynamic::Mp
            | Dynamic::Mf
            | Dynamic::F
            | Dynamic::Ff
            | Dynamic::Fff
            | Dynamic::Ffff
    )
}

fn reject(
    mut result: DynamicSectionedAccordionResult,
    code: &str,
    message: &str,
    measure: Option<usize>,
) -> DynamicSectionedAccordionResult {
    result.arrangement.score = None;
    result.arrangement.source_notes.clear();
    result.arrangement.generated_notes.clear();
    result.arrangement.resolved_patterns.clear();
    result.arrangement.notated_swing = false;
    result.arrangement.notated_swing_measures.clear();
    result.resolved_dynamics.clear();
    result.dynamic_marks.clear();
    result
        .arrangement
        .diagnostics
        .push(GuidedArrangeDiagnostic {
            code: code.into(),
            message: message.into(),
            measure_index: measure,
        });
    result
}

/// Add a staff-local dynamic to the first generated accompaniment attack in
/// each sounding measure. The melody, notes and rhythms remain unchanged.
pub fn arrange_sections_with_dynamics(
    score: &Score,
    accompaniment: SectionedAccordionOptions,
    mut schedule: AccordionDynamicSchedule,
) -> DynamicSectionedAccordionResult {
    schedule
        .sections
        .sort_by_key(|section| (section.first_measure, section.last_measure));
    let arrangement = arrange_with_sections(score, accompaniment);
    let mut result = DynamicSectionedAccordionResult {
        arrangement,
        resolved_schedule: schedule,
        resolved_dynamics: Vec::new(),
        dynamic_marks: Vec::new(),
        contract_version: 1,
    };
    let Some(output) = result.arrangement.score.as_ref() else {
        return result;
    };
    let count = output.parts[0].staves[1].measures.len();
    if result.resolved_schedule.sections.len() > MAX_SECTIONS {
        return reject(
            result,
            "dynamic-section-input-limit",
            "at most 512 dynamic sections are supported",
            None,
        );
    }
    if !supported_level(result.resolved_schedule.default_dynamic) {
        return reject(
            result,
            "unsupported-dynamic",
            "use a sustained level from pppp through ffff",
            None,
        );
    }
    let mut dynamics = vec![result.resolved_schedule.default_dynamic; count];
    let mut previous_end = None;
    for section in &result.resolved_schedule.sections {
        let first = section.first_measure;
        if first > section.last_measure || section.last_measure >= count {
            return reject(
                result,
                "dynamic-section-range",
                "dynamic section must be an ordered inclusive range inside the score",
                Some(first),
            );
        }
        if previous_end.is_some_and(|end| first <= end) {
            return reject(
                result,
                "overlapping-dynamic-sections",
                "dynamic sections must not overlap",
                Some(first),
            );
        }
        if !supported_level(section.dynamic) {
            return reject(
                result,
                "unsupported-dynamic",
                "use a sustained level from pppp through ffff",
                Some(first),
            );
        }
        dynamics[first..=section.last_measure].fill(section.dynamic);
        previous_end = Some(section.last_measure);
    }
    let Some(output) = result.arrangement.score.as_mut() else {
        return result;
    };
    for (measure_index, (bar, dynamic)) in output.parts[0].staves[1]
        .measures
        .iter_mut()
        .zip(&dynamics)
        .enumerate()
    {
        if let Some((note_index, note)) = bar.voices[0]
            .iter_mut()
            .enumerate()
            .find(|(_, note)| !note.is_rest)
        {
            note.dynamic = Some(*dynamic);
            result.dynamic_marks.push(NoteAddr {
                part: 0,
                staff: 1,
                measure: measure_index,
                voice: 0,
                note: note_index,
            });
        }
    }
    result.resolved_dynamics = dynamics;
    result
}
