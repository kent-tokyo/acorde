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
use super::notation::Dynamic;
use super::score::{NoteAddr, Score};

pub const FORM_INNER_VOICES_CONTRACT_VERSION: u16 = 1;
pub const FORM_INNER_VOICES_ENGINE_VERSION: &str =
    concat!(env!("CARGO_PKG_VERSION"), "+form-inner-voices.1");

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
        for voice in 0..2 {
            let events = &mut output.parts[inner_part].staves[0].measures[measure].voices[voice];
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
