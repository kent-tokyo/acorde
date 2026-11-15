//! Explicit instrument assignment and conservative part-performance screening.
//!
//! Limits come from the caller's actual player and instrument. Passing this
//! screen does not certify fingering, breath technique or musical quality.

use serde::{Deserialize, Serialize};

use super::chord_proposals::fingerprint;
use super::playback::{PlaybackEvent, PlaybackOptions, to_playback_events_bounded};
use super::score::{InstrumentDefinition, InstrumentRange, Score};

pub const INSTRUMENT_PLAN_CONTRACT_VERSION: u16 = 1;
pub const INSTRUMENT_PLAN_ENGINE_VERSION: &str =
    concat!(env!("CARGO_PKG_VERSION"), "+instrument-plan.1");
const EPS: f64 = 1e-8;
const MAX_EVENTS: usize = 100_000;

#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
#[serde(rename_all = "kebab-case", tag = "kind", deny_unknown_fields)]
pub enum PartTechnique {
    /// One player's hand on a keyboard. A chord is screened by simultaneous
    /// pitch count and total pitch span, including overlapping voices.
    KeyboardHand {
        max_simultaneous_pitches: u8,
        max_span_semitones: u8,
    },
    /// A monophonic line. Short gaps do not reset its continuous breath run.
    WindLine {
        max_continuous_beats: f64,
        min_breath_rest_beats: f64,
    },
}

#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
#[serde(deny_unknown_fields)]
pub struct InstrumentPartAssignment {
    pub part_index: usize,
    /// The instrument's MIDI channel must match this part's existing channel;
    /// routing changes are a separate score operation.
    pub instrument: InstrumentDefinition,
    /// Inclusive sounding range; may be narrower than the instrument's own
    /// declared range for a particular performer or difficulty level.
    pub sounding_range: InstrumentRange,
    pub technique: PartTechnique,
}

#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
#[serde(deny_unknown_fields)]
pub struct InstrumentPlanRequest {
    pub assignments: Vec<InstrumentPartAssignment>,
}

#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
#[serde(deny_unknown_fields)]
pub struct ResolvedInstrumentPlan {
    pub contract_version: u16,
    pub engine_version: String,
    pub source_fingerprint: String,
    pub request: InstrumentPlanRequest,
}

#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
pub struct InstrumentPartReport {
    pub part_index: usize,
    pub sounding_events: usize,
    pub peak_simultaneous_pitches: usize,
    pub peak_span_semitones: u8,
    pub longest_continuous_beats: f64,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct InstrumentPlanResult {
    /// Original notes and part IDs are preserved; instrument identity and MIDI
    /// program are applied to each assigned part.
    pub score: Score,
    pub resolved: ResolvedInstrumentPlan,
    pub reports: Vec<InstrumentPartReport>,
}

#[derive(Debug, thiserror::Error)]
pub enum InstrumentPlanError {
    #[error("invalid instrument plan request")]
    InvalidRequest,
    #[error("invalid source score")]
    InvalidScore,
    #[error("instrument plan playback exceeds {MAX_EVENTS} events")]
    TooManyEvents,
    #[error("part {part} has pitch {pitch} outside its sounding range at beat {beat}")]
    Range { part: usize, pitch: u8, beat: f64 },
    #[error("part {part} has {count} simultaneous pitches at beat {beat}")]
    Simultaneous {
        part: usize,
        count: usize,
        beat: f64,
    },
    #[error("part {part} has a {span}-semitone span at beat {beat}")]
    Span { part: usize, span: u8, beat: f64 },
    #[error("part {part} exceeds its breath run at beat {beat}")]
    Breath { part: usize, beat: f64 },
    #[error("unsupported instrument plan contract")]
    UnsupportedContract,
    #[error("instrument plan source mismatch")]
    SourceMismatch,
}

fn valid_assignment(score: &Score, assignment: &InstrumentPartAssignment) -> bool {
    let Some(part) = score.parts.get(assignment.part_index) else {
        return false;
    };
    let instrument = &assignment.instrument;
    let range = assignment.sounding_range;
    if instrument.id.trim().is_empty()
        || instrument.staff_count as usize != part.staves.len()
        || instrument.midi_program > 127
        || instrument.midi_channel > 15
        || instrument.midi_channel != part.midi_channel
        || range.lowest > range.highest
        || range.highest > 127
        || instrument.sounding_range.is_some_and(|declared| {
            declared.lowest > range.lowest || declared.highest < range.highest
        })
        || part
            .staves
            .iter()
            .any(|staff| staff.transpose_semitones != instrument.transpose_semitones)
    {
        return false;
    }
    match assignment.technique {
        PartTechnique::KeyboardHand {
            max_simultaneous_pitches,
            max_span_semitones,
        } => (1..=5).contains(&max_simultaneous_pitches) && max_span_semitones <= 24,
        PartTechnique::WindLine {
            max_continuous_beats,
            min_breath_rest_beats,
        } => {
            max_continuous_beats.is_finite()
                && (0.25..=256.0).contains(&max_continuous_beats)
                && min_breath_rest_beats.is_finite()
                && (0.0..=16.0).contains(&min_breath_rest_beats)
        }
    }
}

fn check_range(
    events: &[&PlaybackEvent],
    assignment: &InstrumentPartAssignment,
) -> Result<(), InstrumentPlanError> {
    for event in events {
        let pitch = event.pitch_midi;
        let cents = event.pitch_midi_cents;
        if cents < i32::from(assignment.sounding_range.lowest) * 100
            || cents > i32::from(assignment.sounding_range.highest) * 100
        {
            return Err(InstrumentPlanError::Range {
                part: assignment.part_index,
                pitch,
                beat: event.time_beats,
            });
        }
    }
    Ok(())
}

fn check_keyboard(
    events: &[&PlaybackEvent],
    part: usize,
    max_count: u8,
    max_span: u8,
    report: &mut InstrumentPartReport,
) -> Result<(), InstrumentPlanError> {
    let mut active: Vec<(f64, i32)> = Vec::new();
    let mut position = 0;
    while position < events.len() {
        let beat = events[position].time_beats;
        active.retain(|(end, _)| *end > beat + EPS);
        while position < events.len() && (events[position].time_beats - beat).abs() < EPS {
            let event = events[position];
            active.push((
                event.time_beats + event.duration_beats,
                event.pitch_midi_cents,
            ));
            position += 1;
        }
        let mut pitches = active.iter().map(|(_, pitch)| *pitch).collect::<Vec<_>>();
        pitches.sort_unstable();
        pitches.dedup();
        report.peak_simultaneous_pitches = report.peak_simultaneous_pitches.max(pitches.len());
        if pitches.len() > usize::from(max_count) {
            return Err(InstrumentPlanError::Simultaneous {
                part,
                count: pitches.len(),
                beat,
            });
        }
        let span_cents = match (pitches.first(), pitches.last()) {
            (Some(low), Some(high)) => high - low,
            _ => 0,
        };
        let span = ((span_cents + 99) / 100) as u8;
        report.peak_span_semitones = report.peak_span_semitones.max(span);
        if span_cents > i32::from(max_span) * 100 {
            return Err(InstrumentPlanError::Span { part, span, beat });
        }
    }
    Ok(())
}

fn check_wind(
    events: &[&PlaybackEvent],
    part: usize,
    max_run: f64,
    min_rest: f64,
    report: &mut InstrumentPartReport,
) -> Result<(), InstrumentPlanError> {
    let mut run_start = None;
    let mut previous_end = 0.0_f64;
    for event in events {
        let start = event.time_beats;
        let end = start + event.duration_beats;
        if start < previous_end - EPS {
            return Err(InstrumentPlanError::Simultaneous {
                part,
                count: 2,
                beat: start,
            });
        }
        if run_start.is_none() || start - previous_end + EPS >= min_rest {
            run_start = Some(start);
        }
        let duration = end - run_start.unwrap_or(start);
        report.longest_continuous_beats = report.longest_continuous_beats.max(duration);
        if duration > max_run + EPS {
            return Err(InstrumentPlanError::Breath { part, beat: start });
        }
        previous_end = end;
    }
    report.peak_simultaneous_pitches = usize::from(!events.is_empty());
    Ok(())
}

/// Assign instrument identity to selected parts only after every requested
/// physical envelope passes in playback order, including written repeats.
/// Original notes, timing and part IDs are unchanged on success; errors return
/// no partial score.
pub fn plan_instruments(
    source: &Score,
    request: &InstrumentPlanRequest,
) -> Result<InstrumentPlanResult, InstrumentPlanError> {
    if !super::validate::validate(source).is_valid() {
        return Err(InstrumentPlanError::InvalidScore);
    }
    if request.assignments.is_empty()
        || request.assignments.len() > 32
        || request
            .assignments
            .iter()
            .any(|assignment| !valid_assignment(source, assignment))
    {
        return Err(InstrumentPlanError::InvalidRequest);
    }
    let mut seen = std::collections::BTreeSet::new();
    if request
        .assignments
        .iter()
        .any(|assignment| !seen.insert(assignment.part_index))
    {
        return Err(InstrumentPlanError::InvalidRequest);
    }
    let source_fingerprint = fingerprint(source).ok_or(InstrumentPlanError::InvalidScore)?;
    let playback = to_playback_events_bounded(source, &PlaybackOptions::default())
        .map_err(|_| InstrumentPlanError::TooManyEvents)?;
    if playback.len() > MAX_EVENTS {
        return Err(InstrumentPlanError::TooManyEvents);
    }
    let mut reports = Vec::with_capacity(request.assignments.len());
    for assignment in &request.assignments {
        let events = playback
            .iter()
            .filter(|event| event.part_index == assignment.part_index && !event.is_metronome)
            .collect::<Vec<_>>();
        check_range(&events, assignment)?;
        let mut report = InstrumentPartReport {
            part_index: assignment.part_index,
            sounding_events: events.len(),
            peak_simultaneous_pitches: 0,
            peak_span_semitones: 0,
            longest_continuous_beats: 0.0,
        };
        match assignment.technique {
            PartTechnique::KeyboardHand {
                max_simultaneous_pitches,
                max_span_semitones,
            } => check_keyboard(
                &events,
                assignment.part_index,
                max_simultaneous_pitches,
                max_span_semitones,
                &mut report,
            )?,
            PartTechnique::WindLine {
                max_continuous_beats,
                min_breath_rest_beats,
            } => check_wind(
                &events,
                assignment.part_index,
                max_continuous_beats,
                min_breath_rest_beats,
                &mut report,
            )?,
        }
        reports.push(report);
    }
    let mut score = source.clone();
    for assignment in &request.assignments {
        let part = &mut score.parts[assignment.part_index];
        if !assignment.instrument.name.is_empty() {
            part.name = assignment.instrument.name.clone();
        }
        if !assignment.instrument.short_name.is_empty() {
            part.short_name = assignment.instrument.short_name.clone();
        }
        part.midi_program = assignment.instrument.midi_program;
        part.instrument = Some(assignment.instrument.clone());
    }
    if !super::validate::validate(&score).is_valid() {
        return Err(InstrumentPlanError::InvalidRequest);
    }
    Ok(InstrumentPlanResult {
        score,
        resolved: ResolvedInstrumentPlan {
            contract_version: INSTRUMENT_PLAN_CONTRACT_VERSION,
            engine_version: INSTRUMENT_PLAN_ENGINE_VERSION.into(),
            source_fingerprint,
            request: request.clone(),
        },
        reports,
    })
}

/// Recheck an instrument assignment against the exact original score.
pub fn replay_instrument_plan(
    source: &Score,
    resolved: &ResolvedInstrumentPlan,
) -> Result<InstrumentPlanResult, InstrumentPlanError> {
    if resolved.contract_version != INSTRUMENT_PLAN_CONTRACT_VERSION
        || resolved.engine_version != INSTRUMENT_PLAN_ENGINE_VERSION
    {
        return Err(InstrumentPlanError::UnsupportedContract);
    }
    if fingerprint(source).as_deref() != Some(resolved.source_fingerprint.as_str()) {
        return Err(InstrumentPlanError::SourceMismatch);
    }
    plan_instruments(source, &resolved.request)
}
