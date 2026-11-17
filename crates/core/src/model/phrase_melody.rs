//! Explicitly permitted melody anticipation combined with a developed countermelody.

use serde::{Deserialize, Serialize};

use super::arrange_presets::input_fingerprint;
use super::countermelody_development::{
    CountermelodyDevelopmentError, CountermelodyDevelopmentRequest, CountermelodyDevelopmentResult,
    arrange_countermelody_development, arrange_countermelody_development_with_activity,
};
use super::duration::Duration;
use super::score::{Note, NoteAddr, Score};
use super::validate::validate;

pub const PHRASE_MELODY_CONTRACT_VERSION: u16 = 1;
const MAX_EDITED_NOTES: usize = 64;
const MAX_MEASURES: usize = 32;

/// Permission covers both the attacked note and the rest that gives it time.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
#[serde(deny_unknown_fields)]
pub struct MelodyAnticipationEdit {
    pub note: NoteAddr,
    pub preceding_rest: NoteAddr,
    /// Units of 1/32 quarter-note beat. The original note's end stays fixed.
    pub early_ticks: u8,
}

#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
#[serde(deny_unknown_fields)]
pub struct PhraseMelodyRequest {
    pub countermelody: CountermelodyDevelopmentRequest,
    /// One entry per written measure; false leaves that countermelody measure silent.
    /// Empty enables the countermelody throughout the score.
    #[serde(default)]
    pub countermelody_active_measures: Vec<bool>,
    /// Maximum leap into a phrase after a rest; None uses the legacy behavior.
    #[serde(default)]
    pub phrase_entry_max_leap_semitones: Option<u8>,
    pub start_measure: usize,
    pub end_measure_exclusive: usize,
    pub locked_measures: Vec<usize>,
    pub anticipations: Vec<MelodyAnticipationEdit>,
}

#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
pub struct ResolvedPhraseMelody {
    pub contract_version: u16,
    pub engine_version: String,
    pub source_fingerprint: String,
    pub request: PhraseMelodyRequest,
    /// Authored note identifiers in the same order as `request.anticipations`.
    pub source_note_ids: Vec<String>,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct PhraseMelodyResult {
    pub score: Score,
    pub resolved: ResolvedPhraseMelody,
    pub countermelody: CountermelodyDevelopmentResult,
}

#[derive(Debug, thiserror::Error)]
pub enum PhraseMelodyError {
    #[error("invalid or unbounded phrase melody request")]
    InvalidRequest,
    #[error("source score is invalid")]
    InvalidSource,
    #[error("unpermitted or unsupported melody edit at index {0}")]
    UnsupportedEdit(usize),
    #[error("anticipation duration cannot be written exactly at edit {0}")]
    UnwritableDuration(usize),
    #[error("countermelody development failed: {0}")]
    Countermelody(#[from] CountermelodyDevelopmentError),
    #[error("generated score is invalid")]
    InvalidOutput,
    #[error("unsupported phrase melody contract")]
    UnsupportedContract,
    #[error("phrase melody source changed")]
    SourceMismatch,
    #[error("saved melody selection changed")]
    SelectionMismatch,
}

fn written_duration(ticks: u32) -> Option<(Duration, u8)> {
    use Duration::*;
    for duration in [
        Whole,
        Half,
        Quarter,
        Eighth,
        Sixteenth,
        ThirtySecond,
        SixtyFourth,
    ] {
        for dots in 0..=2 {
            if duration.to_ticks(dots) == ticks * 15 {
                return Some((duration, dots));
            }
        }
    }
    None
}

fn plain_rest(rest: &Note) -> bool {
    if !rest.is_rest || rest.tuplet.is_some() || rest.is_grace || rest.is_cue {
        return false;
    }
    let mut blank = Note::rest(rest.duration.clone());
    blank.id = rest.id.clone();
    blank.dot_count = rest.dot_count;
    matches!(
        (serde_json::to_value(rest), serde_json::to_value(blank)),
        (Ok(actual), Ok(expected)) if actual == expected
    )
}

fn editable_note(note: &Note) -> bool {
    !note.is_rest
        && !note.is_unpitched
        && !note.is_grace
        && !note.is_cue
        && note.pitches.len() == 1
        && note.tuplet.is_none()
        && !note.tie_start
        && !note.tie_end
        && note.pitch_tie_starts.is_empty()
        && note.pitch_tie_ends.is_empty()
        && note.pitch_incoming_partial_ties.is_empty()
        && note.pitch_tie_sources.is_empty()
        && note.lyric.is_none()
        && note.additional_lyrics.is_empty()
        && !note.slur_start
        && !note.slur_end
        && note.chord_symbol.is_none()
        && note.hairpin_start.is_none()
        && !note.hairpin_end
}

pub fn arrange_phrase_melody(
    source: &Score,
    request: &PhraseMelodyRequest,
) -> Result<PhraseMelodyResult, PhraseMelodyError> {
    if !validate(source).is_valid() {
        return Err(PhraseMelodyError::InvalidSource);
    }
    let n = source
        .parts
        .first()
        .and_then(|p| p.staves.first())
        .map(|s| s.measures.len())
        .ok_or(PhraseMelodyError::InvalidSource)?;
    if n == 0
        || n > MAX_MEASURES
        || request.start_measure >= request.end_measure_exclusive
        || request.end_measure_exclusive > n
        || request.anticipations.is_empty()
        || request.anticipations.len() > MAX_EDITED_NOTES
        || (!request.countermelody_active_measures.is_empty()
            && request.countermelody_active_measures.len() != n)
        || request
            .phrase_entry_max_leap_semitones
            .is_some_and(|limit| limit > 24)
        || request.locked_measures.len() > n
        || request.locked_measures.iter().any(|&m| m >= n)
        || request.countermelody.base.melody_part_index >= source.parts.len()
    {
        return Err(PhraseMelodyError::InvalidRequest);
    }
    let mut score = source.clone();
    let mut ids = Vec::new();
    let mut seen = std::collections::BTreeSet::new();
    for (index, edit) in request.anticipations.iter().enumerate() {
        let a = &edit.note;
        let b = &edit.preceding_rest;
        if a.part != request.countermelody.base.melody_part_index
            || a.staff != 0
            || a.voice != 0
            || b.part != a.part
            || b.staff != a.staff
            || b.voice != a.voice
            || b.measure != a.measure
            || b.note.checked_add(1) != Some(a.note)
            || a.measure < request.start_measure
            || a.measure >= request.end_measure_exclusive
            || request.locked_measures.contains(&a.measure)
            || edit.early_ticks == 0
            || edit.early_ticks > 32
            || !seen.insert((a.part, a.measure, a.voice, a.note))
            || !seen.insert((b.part, b.measure, b.voice, b.note))
        {
            return Err(PhraseMelodyError::UnsupportedEdit(index));
        }
        let voice = score
            .parts
            .get_mut(a.part)
            .and_then(|p| p.staves.get_mut(a.staff))
            .and_then(|s| s.measures.get_mut(a.measure))
            .and_then(|m| m.voices.get_mut(a.voice))
            .ok_or(PhraseMelodyError::UnsupportedEdit(index))?;
        if a.note >= voice.len() {
            return Err(PhraseMelodyError::UnsupportedEdit(index));
        }
        let (left, right) = voice.split_at_mut(a.note);
        let rest = left
            .get_mut(b.note)
            .ok_or(PhraseMelodyError::UnsupportedEdit(index))?;
        let note = right
            .first_mut()
            .ok_or(PhraseMelodyError::UnsupportedEdit(index))?;
        if !plain_rest(rest) || !editable_note(note) {
            return Err(PhraseMelodyError::UnsupportedEdit(index));
        }
        let delta = u32::from(edit.early_ticks);
        let rest_ticks = rest.duration.to_ticks(rest.dot_count) / 15;
        let note_ticks = note.duration.to_ticks(note.dot_count) / 15;
        let new_rest = rest_ticks
            .checked_sub(delta)
            .filter(|&v| v > 0)
            .and_then(written_duration)
            .ok_or(PhraseMelodyError::UnwritableDuration(index))?;
        let new_note = note_ticks
            .checked_add(delta)
            .and_then(written_duration)
            .ok_or(PhraseMelodyError::UnwritableDuration(index))?;
        rest.duration = new_rest.0;
        rest.dot_count = new_rest.1;
        note.duration = new_note.0;
        note.dot_count = new_note.1;
        ids.push(note.id.clone());
    }
    if !validate(&score).is_valid() {
        return Err(PhraseMelodyError::InvalidOutput);
    }
    let developed = if request.countermelody_active_measures.is_empty()
        && request.phrase_entry_max_leap_semitones.is_none()
    {
        arrange_countermelody_development(&score, &request.countermelody)?
    } else {
        let activity = if request.countermelody_active_measures.is_empty() {
            vec![true; n]
        } else {
            request.countermelody_active_measures.clone()
        };
        arrange_countermelody_development_with_activity(
            &score,
            &request.countermelody,
            &activity,
            request.phrase_entry_max_leap_semitones,
        )?
    };
    if !validate(&developed.score).is_valid() {
        return Err(PhraseMelodyError::InvalidOutput);
    }
    let fingerprint = input_fingerprint(source).map_err(|_| PhraseMelodyError::InvalidSource)?;
    Ok(PhraseMelodyResult {
        score: developed.score.clone(),
        resolved: ResolvedPhraseMelody {
            contract_version: PHRASE_MELODY_CONTRACT_VERSION,
            engine_version: env!("CARGO_PKG_VERSION").into(),
            source_fingerprint: fingerprint,
            request: request.clone(),
            source_note_ids: ids,
        },
        countermelody: developed,
    })
}

pub fn replay_phrase_melody(
    source: &Score,
    resolved: &ResolvedPhraseMelody,
) -> Result<PhraseMelodyResult, PhraseMelodyError> {
    if resolved.contract_version != PHRASE_MELODY_CONTRACT_VERSION
        || resolved.engine_version != env!("CARGO_PKG_VERSION")
    {
        return Err(PhraseMelodyError::UnsupportedContract);
    }
    if input_fingerprint(source).ok().as_deref() != Some(resolved.source_fingerprint.as_str()) {
        return Err(PhraseMelodyError::SourceMismatch);
    }
    let result = arrange_phrase_melody(source, &resolved.request)?;
    if result.resolved != *resolved {
        return Err(PhraseMelodyError::SelectionMismatch);
    }
    Ok(result)
}
