//! Unfold repeat visits before choosing two independent inner lines.

use serde::{Deserialize, Serialize};

use super::arrange::{PositionedAccordionChord, chord_at_note};
use super::chord_proposals::fingerprint;
use super::ensemble_form_visits::{same_sounding_event, unfolded_measure};
use super::inner_voices::{InnerVoicesError, InnerVoicesRequest, arrange_inner_voices};
use super::playback::{PlaybackOptions, to_playback_events};
use super::repeat::measure_sequence;
use super::score::{NoteAddr, Score};

pub const INNER_VOICES_PERFORMANCE_CONTRACT_VERSION: u16 = 1;
pub const INNER_VOICES_PERFORMANCE_ENGINE_VERSION: &str =
    concat!(env!("CARGO_PKG_VERSION"), "+inner-voices-performance.1");
const MAX_VISITS: usize = 512;
const MAX_SOURCE_EVENTS: usize = 250_000;

#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
#[serde(deny_unknown_fields)]
pub struct ResolvedInnerVoicesPerformance {
    pub contract_version: u16,
    pub engine_version: String,
    pub source_fingerprint: String,
    pub request: InnerVoicesRequest,
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct InnerVoiceRepeatJoin {
    pub from_visit: usize,
    pub to_visit: usize,
    pub from_written_measure: usize,
    pub to_written_measure: usize,
    pub voice: usize,
    pub leap_semitones: u8,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct InnerVoicesPerformanceResult {
    /// Compact notation, including original repeat and navigation marks.
    pub written_score: Score,
    /// Expanded notation and export score with a new choice at every visit.
    pub performance_score: Score,
    pub written_measures_by_visit: Vec<usize>,
    pub resolved: ResolvedInnerVoicesPerformance,
    pub added_part: usize,
    pub generated_notes: Vec<NoteAddr>,
    pub repeat_back_joins: Vec<InnerVoiceRepeatJoin>,
}

#[derive(Debug, thiserror::Error)]
pub enum InnerVoicesPerformanceError {
    #[error("source score cannot be safely unfolded")]
    UnsupportedSource,
    #[error("playback visit count exceeds the supported limit")]
    VisitLimit,
    #[error("unfolded source playback differs from the written source")]
    SourcePlaybackMismatch,
    #[error("chord plan has no effective chord at visit {0}")]
    MissingChord(usize),
    #[error("inner-voice generation failed: {0}")]
    Inner(#[from] InnerVoicesError),
    #[error("unsupported inner-voice performance contract")]
    UnsupportedContract,
    #[error("inner-voice performance source mismatch")]
    SourceMismatch,
    #[error("invalid generated performance score")]
    InvalidOutput,
}

fn unfold_source(source: &Score, visits: &[usize]) -> Option<Score> {
    let mut performance = source.clone();
    for (part_index, part) in performance.parts.iter_mut().enumerate() {
        for (staff_index, staff) in part.staves.iter_mut().enumerate() {
            let written_staff = source.parts.get(part_index)?.staves.get(staff_index)?;
            let mut measures = Vec::with_capacity(visits.len());
            let mut prior_meter = None;
            let mut prior_key = None;
            let mut prior_clef = staff.clef.clone();
            for (visit, &physical) in visits.iter().enumerate() {
                let written = written_staff.measures.get(physical)?;
                let mut bar = unfolded_measure(
                    source,
                    written_staff,
                    written,
                    physical,
                    visit,
                    part_index,
                    staff_index,
                )?;
                let effective_meter = bar.time_sig.clone();
                let effective_key = bar.key_sig.clone();
                let effective_clef = bar.clef.clone().unwrap_or_else(|| prior_clef.clone());
                if visit > 0 && effective_meter == prior_meter {
                    bar.time_sig = None;
                }
                if visit > 0 && effective_key == prior_key {
                    bar.key_sig = None;
                }
                if effective_clef == prior_clef {
                    bar.clef = None;
                }
                prior_clef = bar.clef_at(&effective_clef, f64::MAX);
                prior_meter = effective_meter;
                prior_key = effective_key;
                measures.push(bar);
            }
            staff.measures = measures;
        }
    }
    super::validate::validate(&performance)
        .is_valid()
        .then_some(performance)
}

fn expanded_request(
    request: &InnerVoicesRequest,
    visits: &[usize],
) -> Result<InnerVoicesRequest, InnerVoicesPerformanceError> {
    let mut expanded = request.clone();
    expanded.chords.clear();
    expanded.active_measures = if request.active_measures.is_empty() {
        Vec::new()
    } else {
        visits
            .iter()
            .map(|&physical| {
                request
                    .active_measures
                    .get(physical)
                    .copied()
                    .ok_or(InnerVoicesPerformanceError::UnsupportedSource)
            })
            .collect::<Result<Vec<_>, _>>()?
    };
    expanded.phrase_end_measures.clear();
    for (visit, &physical) in visits.iter().enumerate() {
        let chord = chord_at_note(&request.chords, physical, 0.0)
            .ok_or(InnerVoicesPerformanceError::MissingChord(visit))?;
        expanded.chords.push(PositionedAccordionChord {
            measure_index: visit,
            offset_beats: 0.0,
            chord: chord.clone(),
        });
        expanded.chords.extend(
            request
                .chords
                .iter()
                .filter(|placed| placed.measure_index == physical && placed.offset_beats > 0.0)
                .map(|placed| PositionedAccordionChord {
                    measure_index: visit,
                    offset_beats: placed.offset_beats,
                    chord: placed.chord.clone(),
                }),
        );
        if request.phrase_end_measures.binary_search(&physical).is_ok() {
            expanded.phrase_end_measures.push(visit);
        }
    }
    Ok(expanded)
}

fn first_last_pitch(
    score: &Score,
    part: usize,
    measure: usize,
    voice: usize,
) -> (Option<u8>, Option<u8>) {
    let Some(notes) = score
        .parts
        .get(part)
        .and_then(|part| part.staves.first())
        .and_then(|staff| staff.measures.get(measure))
        .and_then(|bar| bar.voices.get(voice))
    else {
        return (None, None);
    };
    let mut first = None;
    let mut last = None;
    for note in notes {
        if !note.is_rest
            && !note.tie_end
            && let Some(pitch) = note.pitches.first()
            && let Ok(midi) = u8::try_from(pitch.to_midi())
        {
            first.get_or_insert(midi);
            last = Some(midi);
        }
    }
    (first, last)
}

fn repeat_back_joins(score: &Score, part: usize, visits: &[usize]) -> Vec<InnerVoiceRepeatJoin> {
    let mut joins = Vec::new();
    for visit in 1..visits.len() {
        if visits[visit] > visits[visit - 1] {
            continue;
        }
        for voice in 0..2 {
            let (_, from) = first_last_pitch(score, part, visit - 1, voice);
            let (to, _) = first_last_pitch(score, part, visit, voice);
            if let (Some(from), Some(to)) = (from, to) {
                joins.push(InnerVoiceRepeatJoin {
                    from_visit: visit - 1,
                    to_visit: visit,
                    from_written_measure: visits[visit - 1],
                    to_written_measure: visits[visit],
                    voice,
                    leap_semitones: from.abs_diff(to),
                });
            }
        }
    }
    joins
}

/// Preserve compact notation and make a separate performance score. Repeated
/// visits may choose different inner pitches, so active transitions follow
/// each line's leap bound in the actual playback order.
pub fn arrange_inner_voices_performance(
    source: &Score,
    request: InnerVoicesRequest,
) -> Result<InnerVoicesPerformanceResult, InnerVoicesPerformanceError> {
    let written = arrange_inner_voices(source, request)?;
    if !source.spanners.is_empty()
        || !source.views.is_empty()
        || !source.object_style_overrides.is_empty()
    {
        return Err(InnerVoicesPerformanceError::UnsupportedSource);
    }
    let visits = measure_sequence(source);
    if visits.is_empty() || visits.len() > MAX_VISITS {
        return Err(InnerVoicesPerformanceError::VisitLimit);
    }
    let performance_source =
        unfold_source(source, &visits).ok_or(InnerVoicesPerformanceError::UnsupportedSource)?;
    let expected = to_playback_events(source, &PlaybackOptions::default());
    let actual = to_playback_events(&performance_source, &PlaybackOptions::default());
    if expected.len() > MAX_SOURCE_EVENTS
        || expected.len() != actual.len()
        || !expected
            .iter()
            .zip(&actual)
            .all(|(left, right)| same_sounding_event(left, right))
    {
        return Err(InnerVoicesPerformanceError::SourcePlaybackMismatch);
    }
    let expanded = expanded_request(&written.resolved.request, &visits)?;
    let generated = arrange_inner_voices(&performance_source, expanded)?;
    if !super::validate::validate(&generated.score).is_valid() {
        return Err(InnerVoicesPerformanceError::InvalidOutput);
    }
    let joins = repeat_back_joins(&generated.score, generated.added_part, &visits);
    for join in &joins {
        let max = if join.voice == 0 {
            written.resolved.request.lower.max_leap_semitones
        } else {
            written.resolved.request.upper.max_leap_semitones
        };
        if join.leap_semitones > max {
            return Err(InnerVoicesPerformanceError::InvalidOutput);
        }
    }
    Ok(InnerVoicesPerformanceResult {
        written_score: written.score,
        performance_score: generated.score,
        written_measures_by_visit: visits,
        resolved: ResolvedInnerVoicesPerformance {
            contract_version: INNER_VOICES_PERFORMANCE_CONTRACT_VERSION,
            engine_version: INNER_VOICES_PERFORMANCE_ENGINE_VERSION.into(),
            source_fingerprint: written.resolved.source_fingerprint,
            request: written.resolved.request,
        },
        added_part: generated.added_part,
        generated_notes: generated.generated_notes,
        repeat_back_joins: joins,
    })
}

pub fn replay_inner_voices_performance(
    source: &Score,
    resolved: &ResolvedInnerVoicesPerformance,
) -> Result<InnerVoicesPerformanceResult, InnerVoicesPerformanceError> {
    if resolved.contract_version != INNER_VOICES_PERFORMANCE_CONTRACT_VERSION
        || resolved.engine_version != INNER_VOICES_PERFORMANCE_ENGINE_VERSION
    {
        return Err(InnerVoicesPerformanceError::UnsupportedContract);
    }
    if fingerprint(source).as_deref() != Some(resolved.source_fingerprint.as_str()) {
        return Err(InnerVoicesPerformanceError::SourceMismatch);
    }
    let result = arrange_inner_voices_performance(source, resolved.request.clone())?;
    if result.resolved != *resolved {
        return Err(InnerVoicesPerformanceError::UnsupportedContract);
    }
    Ok(result)
}
