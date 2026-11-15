//! Source-bound whole-piece plans for harmony, bass and answering voices.
//!
//! Section boundaries are written measure indices. Each section specifies its
//! own role activity, rhythmic texture and sustained level; phrase endings
//! coordinate harmony and countermelody. Playback visits to the same written
//! measure use the same treatment in contract 1.

use serde::{Deserialize, Serialize};

use super::accordion_dynamics::supported_level;
use super::bass_line::{BassLinePattern, BassRhythmSection, BassRhythmSlot};
use super::chord_proposals::fingerprint;
use super::chordal_pad::{
    ChordalPadPattern, ChordalPadPatternSection, MAX_MEASURES, MAX_SOURCE_NOTES,
};
use super::countermelody_development::CountermelodyDevelopmentKind;
use super::ensemble_roles::{
    EnsembleRole, EnsembleRoleOptions, EnsembleRolePart, EnsembleRoleSection,
    arrange_ensemble_roles_with_bass_patterns, arrange_ensemble_roles_with_development,
};
use super::notation::Dynamic;
use super::score::{NoteAddr, Score};

pub const ENSEMBLE_FORM_CONTRACT_VERSION: u16 = 1;
pub const ENSEMBLE_FORM_ENGINE_VERSION: &str =
    concat!(env!("CARGO_PKG_VERSION"), "+ensemble-form.1");
const MAX_SECTIONS: usize = 64;

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
#[serde(deny_unknown_fields)]
pub struct FormHarmony {
    pub pattern: ChordalPadPattern,
    pub dynamic: Dynamic,
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
#[serde(deny_unknown_fields)]
pub struct FormBass {
    pub pattern: BassLinePattern,
    pub dynamic: Dynamic,
    /// Optional authored cells repeated in every bar of this section.
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub rhythm: Option<Vec<BassRhythmSlot>>,
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
#[serde(deny_unknown_fields)]
pub struct EnsembleFormSection {
    /// Caller label such as A1, B, return or coda. At most 80 UTF-8 bytes.
    pub label: String,
    pub start_measure: usize,
    pub end_measure_exclusive: usize,
    /// None rests the role for the entire section.
    pub harmony: Option<FormHarmony>,
    pub bass: Option<FormBass>,
    pub countermelody: Option<Dynamic>,
}

#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
#[serde(deny_unknown_fields)]
pub struct EnsembleFormRequest {
    /// Source harmony, instruments, registers and collision/density limits.
    /// Activity, pattern sections, phrase endings and dynamics belong to this
    /// form instead; conflicting overrides in the template are rejected.
    pub ensemble: EnsembleRoleOptions,
    /// Ordered adjacent ranges covering every written measure exactly once.
    pub sections: Vec<EnsembleFormSection>,
    /// Zero-based written measures ending phrases. Applied to both harmony
    /// and countermelody; the following active entrance restarts its contour.
    #[serde(default)]
    pub phrase_end_measures: Vec<usize>,
}

#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
#[serde(deny_unknown_fields)]
pub struct ResolvedEnsembleForm {
    pub contract_version: u16,
    pub engine_version: String,
    pub source_fingerprint: String,
    pub request: EnsembleFormRequest,
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct EnsembleFormDiagnostic {
    pub code: String,
    pub section_index: Option<usize>,
    pub measure_index: Option<usize>,
    pub role: Option<EnsembleRole>,
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct EnsembleFormMeasure {
    pub section_index: usize,
    pub harmony_attacks: usize,
    pub bass_attacks: usize,
    pub countermelody_attacks: usize,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct EnsembleFormResult {
    pub score: Option<Score>,
    pub resolved: Option<ResolvedEnsembleForm>,
    pub role_parts: Vec<EnsembleRolePart>,
    pub measures: Vec<EnsembleFormMeasure>,
    pub dynamic_marks: Vec<NoteAddr>,
    pub diagnostics: Vec<EnsembleFormDiagnostic>,
}

fn diagnostic(
    code: &str,
    section_index: Option<usize>,
    measure_index: Option<usize>,
) -> EnsembleFormDiagnostic {
    EnsembleFormDiagnostic {
        code: code.into(),
        section_index,
        measure_index,
        role: None,
    }
}

fn failure(issue: EnsembleFormDiagnostic) -> EnsembleFormResult {
    EnsembleFormResult {
        score: None,
        resolved: None,
        role_parts: Vec::new(),
        measures: Vec::new(),
        dynamic_marks: Vec::new(),
        diagnostics: vec![issue],
    }
}

fn check_source(score: &Score) -> Result<usize, EnsembleFormDiagnostic> {
    let count = score
        .parts
        .first()
        .and_then(|part| part.staves.first())
        .map_or(0, |staff| staff.measures.len());
    let notes = score
        .parts
        .iter()
        .flat_map(|part| &part.staves)
        .flat_map(|staff| &staff.measures)
        .flat_map(|bar| &bar.voices)
        .fold(0usize, |count, voice| count.saturating_add(voice.len()));
    if count == 0 || count > MAX_MEASURES || score.parts.len() > 32 || notes > MAX_SOURCE_NOTES {
        return Err(diagnostic("form-input-limit", None, None));
    }
    if score
        .parts
        .iter()
        .flat_map(|part| &part.staves)
        .any(|staff| staff.measures.len() != count)
        || !super::validate::validate(score).is_valid()
    {
        return Err(diagnostic("invalid-form-source", None, None));
    }
    Ok(count)
}

struct CompiledForm {
    ensemble: EnsembleRoleOptions,
    bass_patterns: Vec<BassLinePattern>,
    owners: Vec<usize>,
}

fn compile(
    count: usize,
    request: &EnsembleFormRequest,
) -> Result<CompiledForm, EnsembleFormDiagnostic> {
    if request.sections.is_empty()
        || request.sections.len() > MAX_SECTIONS
        || request.phrase_end_measures.len() > count
    {
        return Err(diagnostic("form-input-limit", None, None));
    }
    let template = &request.ensemble;
    if !template.sections.is_empty()
        || template.harmony_dynamic.is_some()
        || template.bass_dynamic.is_some()
        || template.countermelody_dynamic.is_some()
        || template.harmony.as_ref().is_some_and(|pad| {
            !pad.pattern_sections.is_empty() || !pad.phrase_end_measures.is_empty()
        })
        || template
            .bass
            .as_ref()
            .is_some_and(|bass| !bass.rhythm_sections.is_empty())
        || template
            .countermelody
            .as_ref()
            .is_some_and(|counter| !counter.phrase_end_measures.is_empty())
    {
        return Err(diagnostic("conflicting-form-template", None, None));
    }
    if template.harmony.is_none() && template.bass.is_none() && template.countermelody.is_none() {
        return Err(diagnostic("missing-form-roles", None, None));
    }
    if request
        .phrase_end_measures
        .iter()
        .any(|&measure| measure >= count)
        || request
            .phrase_end_measures
            .windows(2)
            .any(|pair| pair[0] >= pair[1])
    {
        return Err(diagnostic("invalid-form-phrase-end", None, None));
    }
    let mut ensemble = template.clone();
    ensemble.restart_countermelody_at_section_entry = true;
    if let Some(pad) = &mut ensemble.harmony {
        pad.phrase_end_measures = request.phrase_end_measures.clone();
    }
    if let Some(counter) = &mut ensemble.countermelody {
        counter.phrase_end_measures = request.phrase_end_measures.clone();
    }
    let mut owners = Vec::with_capacity(count);
    let mut bass_patterns = Vec::with_capacity(count);
    for (index, section) in request.sections.iter().enumerate() {
        if section.label.trim().is_empty()
            || section.label.len() > 80
            || section.start_measure != owners.len()
            || section.start_measure >= section.end_measure_exclusive
            || section.end_measure_exclusive > count
        {
            return Err(diagnostic(
                "invalid-form-section",
                Some(index),
                Some(section.start_measure),
            ));
        }
        for (role, level, present) in [
            (
                EnsembleRole::Harmony,
                section.harmony.as_ref().map(|part| part.dynamic),
                template.harmony.is_some(),
            ),
            (
                EnsembleRole::Bass,
                section.bass.as_ref().map(|part| part.dynamic),
                template.bass.is_some(),
            ),
            (
                EnsembleRole::Countermelody,
                section.countermelody,
                template.countermelody.is_some(),
            ),
        ] {
            if level.is_some_and(|level| !supported_level(level)) || level.is_some() && !present {
                let mut issue = diagnostic(
                    if present {
                        "unsupported-form-dynamic"
                    } else {
                        "missing-form-role"
                    },
                    Some(index),
                    Some(section.start_measure),
                );
                issue.role = Some(role);
                return Err(issue);
            }
            ensemble.sections.push(EnsembleRoleSection {
                role,
                start_measure: section.start_measure,
                end_measure_exclusive: section.end_measure_exclusive,
                enabled: level.is_some(),
            });
        }
        if let (Some(pad), Some(treatment)) = (&mut ensemble.harmony, &section.harmony) {
            pad.pattern_sections.push(ChordalPadPatternSection {
                start_measure: section.start_measure,
                end_measure_exclusive: section.end_measure_exclusive,
                pattern: treatment.pattern,
            });
        }
        let pattern = section
            .bass
            .as_ref()
            .map_or(BassLinePattern::RootPulse, |bass| bass.pattern);
        if let (Some(bass), Some(treatment)) = (&mut ensemble.bass, &section.bass)
            && let Some(slots) = &treatment.rhythm
        {
            bass.rhythm_sections.push(BassRhythmSection {
                start_measure: section.start_measure,
                end_measure_exclusive: section.end_measure_exclusive,
                slots: slots.clone(),
            });
        }
        owners.resize(section.end_measure_exclusive, index);
        bass_patterns.resize(section.end_measure_exclusive, pattern);
    }
    if owners.len() != count {
        return Err(diagnostic("incomplete-form", None, Some(owners.len())));
    }
    if let Some(pad) = &mut ensemble.harmony {
        // Held textures already sustain to the end. Do not introduce the
        // legacy pad's phrase-transition boundary as an extra attack there.
        pad.phrase_end_measures.retain(|measure| {
            request.sections[owners[*measure]]
                .harmony
                .as_ref()
                .is_some_and(|harmony| harmony.pattern != ChordalPadPattern::Held)
        });
    }
    Ok(CompiledForm {
        ensemble,
        bass_patterns,
        owners,
    })
}

/// Generate a complete multi-role arrangement with a coordinated written form.
/// Input parts are preserved exactly. Invalid input returns no partial score,
/// replay contract, dynamic mark or measure report.
pub fn arrange_ensemble_form(score: &Score, request: &EnsembleFormRequest) -> EnsembleFormResult {
    arrange_ensemble_form_internal(score, request, None)
}

pub(crate) fn arrange_ensemble_form_with_development(
    score: &Score,
    request: &EnsembleFormRequest,
    kind: CountermelodyDevelopmentKind,
) -> EnsembleFormResult {
    arrange_ensemble_form_internal(score, request, Some(kind))
}

fn arrange_ensemble_form_internal(
    score: &Score,
    request: &EnsembleFormRequest,
    development: Option<CountermelodyDevelopmentKind>,
) -> EnsembleFormResult {
    let count = match check_source(score) {
        Ok(count) => count,
        Err(issue) => return failure(issue),
    };
    let compiled = match compile(count, request) {
        Ok(plan) => plan,
        Err(issue) => return failure(issue),
    };
    let Some(source_fingerprint) = fingerprint(score) else {
        return failure(diagnostic("invalid-form-source", None, None));
    };
    let arranged = if let Some(kind) = development {
        arrange_ensemble_roles_with_development(
            score,
            compiled.ensemble,
            Some(&compiled.bass_patterns),
            kind,
        )
    } else {
        arrange_ensemble_roles_with_bass_patterns(
            score,
            compiled.ensemble,
            Some(&compiled.bass_patterns),
        )
    };
    let Some(mut output) = arranged.score else {
        let issue = arranged.diagnostics.first();
        let measure = issue.and_then(|issue| issue.measure_index);
        let mut problem = diagnostic(
            issue.map_or("ensemble-form-failed", |issue| issue.code.as_str()),
            measure.and_then(|measure| compiled.owners.get(measure).copied()),
            measure,
        );
        problem.role = issue.and_then(|issue| issue.role);
        return failure(problem);
    };
    let mut marks = Vec::new();
    let mut measures = Vec::with_capacity(count);
    for (measure, &section_index) in compiled.owners.iter().enumerate() {
        let section = &request.sections[section_index];
        let mut report = EnsembleFormMeasure {
            section_index,
            harmony_attacks: 0,
            bass_attacks: 0,
            countermelody_attacks: 0,
        };
        for part in &arranged.role_parts {
            let (level, attacks) = match part.role {
                EnsembleRole::Harmony => (
                    section.harmony.as_ref().map(|part| part.dynamic),
                    &mut report.harmony_attacks,
                ),
                EnsembleRole::Bass => (
                    section.bass.as_ref().map(|part| part.dynamic),
                    &mut report.bass_attacks,
                ),
                EnsembleRole::Countermelody => {
                    (section.countermelody, &mut report.countermelody_attacks)
                }
            };
            let voice = &mut output.parts[part.part_index].staves[0].measures[measure].voices[0];
            let mut marked = false;
            for (index, note) in voice.iter_mut().enumerate() {
                if !note.is_rest && !note.tie_end {
                    *attacks += 1;
                    if !marked && let Some(level) = level {
                        // Mark each sounding bar so a repeat/navigation entry
                        // cannot inherit another section's dynamic.
                        note.dynamic = Some(level);
                        marks.push(NoteAddr {
                            part: part.part_index,
                            staff: 0,
                            measure,
                            voice: 0,
                            note: index,
                        });
                        marked = true;
                    }
                }
            }
        }
        measures.push(report);
    }
    EnsembleFormResult {
        score: Some(output),
        resolved: Some(ResolvedEnsembleForm {
            contract_version: ENSEMBLE_FORM_CONTRACT_VERSION,
            engine_version: ENSEMBLE_FORM_ENGINE_VERSION.into(),
            source_fingerprint,
            request: request.clone(),
        }),
        role_parts: arranged.role_parts,
        measures,
        dynamic_marks: marks,
        diagnostics: Vec::new(),
    }
}

/// Replay only against the same source content and generator contract.
pub fn replay_ensemble_form(score: &Score, resolved: &ResolvedEnsembleForm) -> EnsembleFormResult {
    if resolved.contract_version != ENSEMBLE_FORM_CONTRACT_VERSION
        || resolved.engine_version != ENSEMBLE_FORM_ENGINE_VERSION
    {
        return failure(diagnostic("unsupported-form-contract", None, None));
    }
    if let Err(issue) = check_source(score) {
        return failure(issue);
    }
    if fingerprint(score).as_deref() != Some(resolved.source_fingerprint.as_str()) {
        return failure(diagnostic("form-source-mismatch", None, None));
    }
    arrange_ensemble_form(score, &resolved.request)
}
