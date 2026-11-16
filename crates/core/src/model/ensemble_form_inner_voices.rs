//! Replace a form's chordal-pad role with two independently timed inner lines.

use serde::{Deserialize, Serialize};

use super::accordion_dynamics::supported_level;
use super::chord_proposals::fingerprint;
use super::ensemble_form::EnsembleFormMeasure;
use super::ensemble_form_development::{
    EnsembleFormDevelopmentError, EnsembleFormDevelopmentRequest, arrange_ensemble_form_development,
};
use super::ensemble_roles::{EnsembleRole, EnsembleRolePart};
use super::inner_voices::{InnerVoicesError, InnerVoicesRequest, arrange_inner_voices};
use super::inner_voices_performance::{
    InnerVoiceRepeatJoin, InnerVoicesPerformanceError, arrange_inner_voices_performance,
};
use super::notation::Dynamic;
use super::score::{NoteAddr, Score};

pub const FORM_INNER_VOICES_CONTRACT_VERSION: u16 = 1;
pub const FORM_INNER_VOICES_ENGINE_VERSION: &str =
    concat!(env!("CARGO_PKG_VERSION"), "+form-inner-voices.1");
pub const FORM_INNER_VOICES_PERFORMANCE_CONTRACT_VERSION: u16 = 1;
pub const FORM_INNER_VOICES_PERFORMANCE_ENGINE_VERSION: &str = concat!(
    env!("CARGO_PKG_VERSION"),
    "+form-inner-voices-performance.1"
);

#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
#[serde(deny_unknown_fields)]
pub struct FormInnerVoicesRequest {
    /// Bass and countermelody use the existing form development engine. Its
    /// harmony role must be absent; the inner lines take that role instead.
    pub form: EnsembleFormDevelopmentRequest,
    pub inner: InnerVoicesRequest,
    /// One optional sustained level per form section; None rests both lines.
    pub section_levels: Vec<Option<Dynamic>>,
}

#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
#[serde(deny_unknown_fields)]
pub struct ResolvedFormInnerVoices {
    pub contract_version: u16,
    pub engine_version: String,
    pub source_fingerprint: String,
    pub request: FormInnerVoicesRequest,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct FormInnerVoicesResult {
    pub score: Score,
    pub resolved: ResolvedFormInnerVoices,
    /// Bass and countermelody parts produced by the underlying form.
    pub role_parts: Vec<EnsembleRolePart>,
    pub inner_part: usize,
    /// `harmony_attacks` counts both inner lines in this result.
    pub measures: Vec<EnsembleFormMeasure>,
    pub dynamic_marks: Vec<NoteAddr>,
}

#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
#[serde(deny_unknown_fields)]
pub struct ResolvedFormInnerVoicesPerformance {
    pub contract_version: u16,
    pub engine_version: String,
    pub source_fingerprint: String,
    pub request: FormInnerVoicesRequest,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct FormInnerVoicesPerformanceResult {
    /// The compact score retains authored repeats and form sections.
    pub written_score: Score,
    /// Exportable score in playback order, with visit-specific inner pitches.
    pub performance_score: Score,
    pub written_measures_by_visit: Vec<usize>,
    pub resolved: ResolvedFormInnerVoicesPerformance,
    pub role_parts: Vec<EnsembleRolePart>,
    pub inner_part: usize,
    /// One report per playback visit, in performance-score order.
    pub performance_measures: Vec<EnsembleFormMeasure>,
    pub inner_dynamic_marks: Vec<NoteAddr>,
    pub repeat_back_joins: Vec<InnerVoiceRepeatJoin>,
}

#[derive(Debug, thiserror::Error)]
pub enum FormInnerVoicesError {
    #[error("inner-voice form request is inconsistent")]
    InvalidRequest,
    #[error("form realization failed: {0}")]
    Form(#[from] EnsembleFormDevelopmentError),
    #[error("inner-voice realization failed: {0}")]
    Inner(#[from] InnerVoicesError),
    #[error("inner voice collides with {role:?} at measure {measure}")]
    Collision { role: EnsembleRole, measure: usize },
    #[error("generated attack density exceeded at measure {0}")]
    Density(usize),
    #[error("unsupported inner-voice form contract")]
    UnsupportedContract,
    #[error("inner-voice form source mismatch")]
    SourceMismatch,
    #[error("invalid generated score")]
    InvalidOutput,
}

#[derive(Debug, thiserror::Error)]
pub enum FormInnerVoicesPerformanceError {
    #[error("written inner-voice form failed: {0}")]
    Written(#[from] FormInnerVoicesError),
    #[error("form realization failed: {0}")]
    Form(#[from] EnsembleFormDevelopmentError),
    #[error("inner-voice performance failed: {0}")]
    Performance(#[from] InnerVoicesPerformanceError),
    #[error("inner voice collides with {role:?} at playback visit {visit}")]
    Collision { role: EnsembleRole, visit: usize },
    #[error("generated attack density exceeded at playback visit {0}")]
    Density(usize),
    #[error("unsupported inner-voice form performance contract")]
    UnsupportedContract,
    #[error("inner-voice form performance source mismatch")]
    SourceMismatch,
    #[error("invalid generated performance score")]
    InvalidOutput,
}

type Span = (f64, f64, i16);

fn spans(score: &Score, part_index: usize, measure: usize) -> Vec<Span> {
    let mut spans = Vec::new();
    if let Some(bar) = score
        .parts
        .get(part_index)
        .and_then(|part| part.staves.first())
        .and_then(|staff| staff.measures.get(measure))
    {
        for voice in &bar.voices {
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
        }
    }
    spans
}

fn crosses(lower: &[Span], upper: &[Span], gap: u8) -> bool {
    let gap = i16::from(gap.max(1));
    lower.iter().any(|(lower_start, lower_end, lower_pitch)| {
        upper.iter().any(|(upper_start, upper_end, upper_pitch)| {
            *lower_start < *upper_end - 1e-9
                && *lower_end > *upper_start + 1e-9
                && upper_pitch - lower_pitch < gap
        })
    })
}

/// Compose a sectioned form with two inner lines in place of its pad. The
/// density and vertical-order checks cover the combined output, not only the
/// base form. Failure returns no partial score.
pub fn arrange_form_inner_voices(
    source: &Score,
    request: &FormInnerVoicesRequest,
) -> Result<FormInnerVoicesResult, FormInnerVoicesError> {
    let form = &request.form.form;
    let count = source
        .parts
        .first()
        .and_then(|part| part.staves.first())
        .map_or(0, |staff| staff.measures.len());
    if count == 0
        || form.ensemble.harmony.is_some()
        || form.sections.is_empty()
        || form
            .sections
            .iter()
            .any(|section| section.harmony.is_some())
        || request.section_levels.len() != form.sections.len()
        || !request.section_levels.iter().any(Option::is_some)
        || request
            .section_levels
            .iter()
            .flatten()
            .any(|level| !supported_level(*level))
        || !request.inner.active_measures.is_empty()
        || !request.inner.phrase_end_measures.is_empty()
        || form
            .ensemble
            .bass
            .as_ref()
            .is_some_and(|bass| bass.chords != request.inner.chords)
        || form
            .ensemble
            .countermelody
            .as_ref()
            .is_some_and(|counter| counter.chords != request.inner.chords)
        || form
            .ensemble
            .countermelody
            .as_ref()
            .is_some_and(|counter| counter.melody_part_index != request.inner.melody_part_index)
    {
        return Err(FormInnerVoicesError::InvalidRequest);
    }
    let mut activity = vec![false; count];
    for (section, level) in form.sections.iter().zip(&request.section_levels) {
        if section.start_measure >= section.end_measure_exclusive
            || section.end_measure_exclusive > count
        {
            return Err(FormInnerVoicesError::InvalidRequest);
        }
        activity[section.start_measure..section.end_measure_exclusive].fill(level.is_some());
    }
    let base = arrange_ensemble_form_development(source, &request.form)?;
    let mut inner_request = request.inner.clone();
    inner_request.active_measures = activity;
    inner_request.phrase_end_measures = form.phrase_end_measures.clone();
    let inner = arrange_inner_voices(&base.score, inner_request)?;
    let mut output = inner.score;
    let inner_part = inner.added_part;
    let mut marks = base.dynamic_marks;
    let mut measures = base.measures;
    for (measure, report) in measures.iter_mut().enumerate() {
        let level = request.section_levels[report.section_index];
        let inner_spans = spans(&output, inner_part, measure);
        for part in &base.role_parts {
            let peer = spans(&output, part.part_index, measure);
            let collision = match part.role {
                EnsembleRole::Bass => crosses(
                    &peer,
                    &inner_spans,
                    form.ensemble.min_bass_harmony_gap_semitones,
                ),
                EnsembleRole::Countermelody => crosses(
                    &inner_spans,
                    &peer,
                    form.ensemble.min_harmony_countermelody_gap_semitones,
                ),
                EnsembleRole::Harmony => true,
            };
            if collision {
                return Err(FormInnerVoicesError::Collision {
                    role: part.role,
                    measure,
                });
            }
        }
        let mut first_attack = None;
        for (voice, events) in output.parts[inner_part].staves[0].measures[measure]
            .voices
            .iter_mut()
            .take(2)
            .enumerate()
        {
            let mut onset = 0.0;
            for (note, event) in events.iter_mut().enumerate() {
                if !event.is_rest && !event.tie_end {
                    report.harmony_attacks += 1;
                    if first_attack.as_ref().is_none_or(|(best, _)| onset < *best) {
                        first_attack = Some((
                            onset,
                            NoteAddr {
                                part: inner_part,
                                staff: 0,
                                measure,
                                voice,
                                note,
                            },
                        ));
                    }
                }
                onset += event.beats();
            }
        }
        if let (Some(level), Some((_, address))) = (level, first_attack) {
            output.parts[inner_part].staves[0].measures[measure].voices[address.voice]
                [address.note]
                .dynamic = Some(level);
            marks.push(address);
        }
        let total = report.harmony_attacks + report.bass_attacks + report.countermelody_attacks;
        if form
            .ensemble
            .max_generated_attacks_per_measure
            .is_some_and(|limit| total > limit)
        {
            return Err(FormInnerVoicesError::Density(measure));
        }
    }
    if !super::validate::validate(&output).is_valid() {
        return Err(FormInnerVoicesError::InvalidOutput);
    }
    let source_fingerprint = fingerprint(source).ok_or(FormInnerVoicesError::InvalidRequest)?;
    Ok(FormInnerVoicesResult {
        score: output,
        resolved: ResolvedFormInnerVoices {
            contract_version: FORM_INNER_VOICES_CONTRACT_VERSION,
            engine_version: FORM_INNER_VOICES_ENGINE_VERSION.into(),
            source_fingerprint,
            request: request.clone(),
        },
        role_parts: base.role_parts,
        inner_part,
        measures,
        dynamic_marks: marks,
    })
}

pub fn replay_form_inner_voices(
    source: &Score,
    resolved: &ResolvedFormInnerVoices,
) -> Result<FormInnerVoicesResult, FormInnerVoicesError> {
    if resolved.contract_version != FORM_INNER_VOICES_CONTRACT_VERSION
        || resolved.engine_version != FORM_INNER_VOICES_ENGINE_VERSION
    {
        return Err(FormInnerVoicesError::UnsupportedContract);
    }
    if fingerprint(source).as_deref() != Some(resolved.source_fingerprint.as_str()) {
        return Err(FormInnerVoicesError::SourceMismatch);
    }
    arrange_form_inner_voices(source, &resolved.request)
}

fn attack_count(score: &Score, part: usize, visit: usize) -> Option<usize> {
    let bar = score.parts.get(part)?.staves.first()?.measures.get(visit)?;
    Some(
        bar.voices
            .iter()
            .flat_map(|voice| voice.iter())
            .filter(|note| !note.is_rest && !note.tie_end)
            .count(),
    )
}

fn mark_inner_visit(
    score: &mut Score,
    part: usize,
    visit: usize,
    level: Option<Dynamic>,
) -> Result<(usize, Option<NoteAddr>), FormInnerVoicesPerformanceError> {
    let bar = score
        .parts
        .get_mut(part)
        .and_then(|part| part.staves.first_mut())
        .and_then(|staff| staff.measures.get_mut(visit))
        .ok_or(FormInnerVoicesPerformanceError::InvalidOutput)?;
    let mut attacks = 0;
    let mut first = None;
    for voice in 0..2 {
        let events = bar
            .voices
            .get(voice)
            .ok_or(FormInnerVoicesPerformanceError::InvalidOutput)?;
        let mut onset = 0.0;
        for (note, event) in events.iter().enumerate() {
            if !event.is_rest && !event.tie_end {
                attacks += 1;
                if first.as_ref().is_none_or(|(best, _)| onset < *best) {
                    first = Some((
                        onset,
                        NoteAddr {
                            part,
                            staff: 0,
                            measure: visit,
                            voice,
                            note,
                        },
                    ));
                }
            }
            onset += event.beats();
        }
    }
    if let (Some(level), Some((_, address))) = (level, first.as_ref()) {
        let event = bar
            .voices
            .get_mut(address.voice)
            .and_then(|voice| voice.get_mut(address.note))
            .ok_or(FormInnerVoicesPerformanceError::InvalidOutput)?;
        event.dynamic = Some(level);
    }
    Ok((attacks, level.and(first.map(|(_, address)| address))))
}

/// Preserve the sectioned written form and generate an export score that
/// chooses each inner-voice entrance in playback order. Every visit is checked
/// against the form's role spacing, dynamic and total density constraints.
pub fn arrange_form_inner_voices_performance(
    source: &Score,
    request: &FormInnerVoicesRequest,
) -> Result<FormInnerVoicesPerformanceResult, FormInnerVoicesPerformanceError> {
    let written = arrange_form_inner_voices(source, request)?;
    let base = arrange_ensemble_form_development(source, &request.form)?;
    let mut inner_request = request.inner.clone();
    inner_request.active_measures = written
        .measures
        .iter()
        .map(|measure| {
            request
                .section_levels
                .get(measure.section_index)
                .is_some_and(Option::is_some)
        })
        .collect();
    inner_request.phrase_end_measures = request.form.form.phrase_end_measures.clone();
    let performance = arrange_inner_voices_performance(&base.score, inner_request)?;
    let mut output = performance.performance_score;
    if output.parts.len() != written.score.parts.len() {
        return Err(FormInnerVoicesPerformanceError::InvalidOutput);
    }
    for (part, compact) in output.parts.iter_mut().zip(&written.score.parts) {
        part.id.clone_from(&compact.id);
    }
    let inner_part = performance.added_part;
    let mut reports = Vec::with_capacity(performance.written_measures_by_visit.len());
    let mut marks = Vec::new();
    for (visit, &physical) in performance.written_measures_by_visit.iter().enumerate() {
        let mut report = base
            .measures
            .get(physical)
            .cloned()
            .ok_or(FormInnerVoicesPerformanceError::InvalidOutput)?;
        let level = *request
            .section_levels
            .get(report.section_index)
            .ok_or(FormInnerVoicesPerformanceError::InvalidOutput)?;
        let inner_spans = spans(&output, inner_part, visit);
        for role_part in &base.role_parts {
            let peer = spans(&output, role_part.part_index, visit);
            let collision = match role_part.role {
                EnsembleRole::Bass => crosses(
                    &peer,
                    &inner_spans,
                    request.form.form.ensemble.min_bass_harmony_gap_semitones,
                ),
                EnsembleRole::Countermelody => crosses(
                    &inner_spans,
                    &peer,
                    request
                        .form
                        .form
                        .ensemble
                        .min_harmony_countermelody_gap_semitones,
                ),
                EnsembleRole::Harmony => true,
            };
            if collision {
                return Err(FormInnerVoicesPerformanceError::Collision {
                    role: role_part.role,
                    visit,
                });
            }
            let count = attack_count(&output, role_part.part_index, visit)
                .ok_or(FormInnerVoicesPerformanceError::InvalidOutput)?;
            match role_part.role {
                EnsembleRole::Bass => report.bass_attacks = count,
                EnsembleRole::Countermelody => report.countermelody_attacks = count,
                EnsembleRole::Harmony => {}
            }
        }
        let (attacks, mark) = mark_inner_visit(&mut output, inner_part, visit, level)?;
        report.harmony_attacks = attacks;
        if let Some(mark) = mark {
            marks.push(mark);
        }
        let total = report.harmony_attacks + report.bass_attacks + report.countermelody_attacks;
        if request
            .form
            .form
            .ensemble
            .max_generated_attacks_per_measure
            .is_some_and(|limit| total > limit)
        {
            return Err(FormInnerVoicesPerformanceError::Density(visit));
        }
        reports.push(report);
    }
    if !super::validate::validate(&output).is_valid() {
        return Err(FormInnerVoicesPerformanceError::InvalidOutput);
    }
    let source_fingerprint = written.resolved.source_fingerprint;
    Ok(FormInnerVoicesPerformanceResult {
        written_score: written.score,
        performance_score: output,
        written_measures_by_visit: performance.written_measures_by_visit,
        resolved: ResolvedFormInnerVoicesPerformance {
            contract_version: FORM_INNER_VOICES_PERFORMANCE_CONTRACT_VERSION,
            engine_version: FORM_INNER_VOICES_PERFORMANCE_ENGINE_VERSION.into(),
            source_fingerprint,
            request: request.clone(),
        },
        role_parts: base.role_parts,
        inner_part,
        performance_measures: reports,
        inner_dynamic_marks: marks,
        repeat_back_joins: performance.repeat_back_joins,
    })
}

pub fn replay_form_inner_voices_performance(
    source: &Score,
    resolved: &ResolvedFormInnerVoicesPerformance,
) -> Result<FormInnerVoicesPerformanceResult, FormInnerVoicesPerformanceError> {
    if resolved.contract_version != FORM_INNER_VOICES_PERFORMANCE_CONTRACT_VERSION
        || resolved.engine_version != FORM_INNER_VOICES_PERFORMANCE_ENGINE_VERSION
    {
        return Err(FormInnerVoicesPerformanceError::UnsupportedContract);
    }
    if fingerprint(source).as_deref() != Some(resolved.source_fingerprint.as_str()) {
        return Err(FormInnerVoicesPerformanceError::SourceMismatch);
    }
    let result = arrange_form_inner_voices_performance(source, &resolved.request)?;
    if result.resolved != *resolved {
        return Err(FormInnerVoicesPerformanceError::UnsupportedContract);
    }
    Ok(result)
}
