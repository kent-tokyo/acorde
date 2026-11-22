//! Source-bound compatibility for explicitly voiced legacy accompaniment.
//!
//! Each region gives its own sounding bounds, exact MIDI pitches, and pattern
//! phase. Regions restart independently, including repeated equal harmonies.
//! The supported patterns are held chords and half-beat bass/chord pulses;
//! other legacy patterns are intentionally outside this contract.

use super::arrange_presets::input_fingerprint;
use super::chordal_pad::{emit_rest, emit_span, free_channel};
use super::notation::{ChordSymbol, Clef};
use super::score::{Measure, Note, NoteAddr, Part, Score, Staff};
use serde::{Deserialize, Serialize};

pub const LEGACY_ACCOMPANIMENT_CONTRACT_VERSION: u16 = 1;
const ENGINE: &str = concat!(env!("CARGO_PKG_VERSION"), "+legacy-accompaniment.1");
const GRID: u32 = 32;

#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "kebab-case")]
pub enum LegacyAccompanimentPattern {
    Held,
    BassChordPulse,
}

/// A half-open written interval in 1/32 quarter-note beats. Regions are
/// independent even when their symbols and pitches happen to be identical.
#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
#[serde(deny_unknown_fields)]
pub struct LegacyAccompanimentRegion {
    pub measure: usize,
    pub start_tick: u32,
    pub end_tick: u32,
    pub bass_midi: u8,
    pub chord_midi: Vec<u8>,
    pub chord: ChordSymbol,
    /// Beat index at the first attack. Use zero to restart at this region.
    pub phase_beats: u8,
}

#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
#[serde(deny_unknown_fields)]
pub struct LegacyAccompanimentRequest {
    pub contract_version: u16,
    pub pattern: LegacyAccompanimentPattern,
    pub regions: Vec<LegacyAccompanimentRegion>,
    pub midi_program: u8,
}

#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
#[serde(deny_unknown_fields)]
pub struct ResolvedLegacyAccompaniment {
    pub contract_version: u16,
    pub engine_version: String,
    pub source_fingerprint: String,
    pub request: LegacyAccompanimentRequest,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct LegacyAccompanimentResult {
    pub score: Score,
    pub added_part: usize,
    pub generated_notes: Vec<NoteAddr>,
    pub resolved: ResolvedLegacyAccompaniment,
}

#[derive(Debug, thiserror::Error)]
pub enum LegacyAccompanimentError {
    #[error("unsupported source: expected one part and one staff with 1 to 512 measures")]
    UnsupportedSource,
    #[error("source score is invalid")]
    InvalidScore,
    #[error("invalid or unbounded compatibility request")]
    InvalidRequest,
    #[error("invalid, overlapping, or unrepresentable region {region}")]
    InvalidRegion { region: usize },
    #[error("score measure {measure} is not on the 1/32-beat grid")]
    InvalidMeasure { measure: usize },
    #[error("no free MIDI channel for accompaniment")]
    NoChannel,
    #[error("source fingerprint could not be computed")]
    Fingerprint,
    #[error("resolved contract or engine version differs")]
    ReplayVersion,
    #[error("resolved source score differs")]
    ReplaySource,
}

fn ticks(beats: f64) -> Option<u32> {
    let value = beats * f64::from(GRID);
    if !value.is_finite() || value <= 0.0 || value > 1024.0 || (value - value.round()).abs() > 1e-8
    {
        return None;
    }
    Some(value.round() as u32)
}

/// Generate the explicitly selected legacy accompaniment without changing the
/// source part. This contract describes written MIDI pitches, not instrument
/// fingering or a general transcription of every site arrangement.
pub fn arrange_legacy_accompaniment(
    source: &Score,
    request: &LegacyAccompanimentRequest,
) -> Result<LegacyAccompanimentResult, LegacyAccompanimentError> {
    use LegacyAccompanimentError as E;
    if source.parts.len() != 1 || source.parts[0].staves.len() != 1 {
        return Err(E::UnsupportedSource);
    }
    let staff = &source.parts[0].staves[0];
    if staff.measures.is_empty()
        || staff.measures.len() > 512
        || staff
            .measures
            .iter()
            .flat_map(|bar| &bar.voices)
            .fold(0usize, |count, voice| count.saturating_add(voice.len()))
            > 8192
    {
        return Err(E::UnsupportedSource);
    }
    if !super::validate::validate(source).is_valid() {
        return Err(E::InvalidScore);
    }
    if request.contract_version != LEGACY_ACCOMPANIMENT_CONTRACT_VERSION
        || request.regions.is_empty()
        || request.regions.len() > 1024
        || request.midi_program > 127
    {
        return Err(E::InvalidRequest);
    }
    let mut lengths = Vec::with_capacity(staff.measures.len());
    for i in 0..staff.measures.len() {
        lengths.push(
            ticks(staff.measure_beats(i, &source.settings.time_signature))
                .ok_or(E::InvalidMeasure { measure: i })?,
        );
    }
    let mut by_measure = vec![Vec::<(usize, &LegacyAccompanimentRegion)>::new(); lengths.len()];
    let mut previous: Option<(usize, u32)> = None;
    for (index, region) in request.regions.iter().enumerate() {
        let Some(&length) = lengths.get(region.measure) else {
            return Err(E::InvalidRegion { region: index });
        };
        let mut distinct = std::collections::BTreeSet::new();
        distinct.insert(region.bass_midi);
        let unique_pitches = region
            .chord_midi
            .iter()
            .all(|pitch| distinct.insert(*pitch));
        if region.start_tick >= region.end_tick
            || region.end_tick > length
            || region.bass_midi > 127
            || region.chord_midi.is_empty()
            || region.chord_midi.len() > 8
            || region.chord_midi.iter().any(|p| *p > 127)
            || !unique_pitches
            || region.chord.root.is_empty()
            || region.chord.root.len() > 32
            || region.chord.kind.len() > 64
            || region.phase_beats > 3
            || previous.is_some_and(|(m, end)| {
                region.measure < m || (region.measure == m && region.start_tick < end)
            })
        {
            return Err(E::InvalidRegion { region: index });
        }
        by_measure[region.measure].push((index, region));
        previous = Some((region.measure, region.end_tick));
    }
    let channel = free_channel(source).ok_or(E::NoChannel)?;
    let added_part = source.parts.len();
    let mut accompaniment = Part::new("Accompaniment", "Acc.");
    accompaniment.midi_channel = channel;
    accompaniment.midi_program = request.midi_program;
    let mut left = Staff::new(Clef::Bass);
    let mut generated_notes = Vec::new();
    for (measure, source_bar) in staff.measures.iter().enumerate() {
        let mut bar = Measure::empty(
            source.settings.time_signature.numerator,
            source.settings.time_signature.denominator,
        );
        bar.number = source_bar.number;
        bar.time_sig = source_bar.time_sig.clone();
        bar.key_sig = source_bar.key_sig.clone();
        bar.actual_length = source_bar.actual_length;
        bar.barline_left = source_bar.barline_left.clone();
        bar.barline_right = source_bar.barline_right.clone();
        bar.volta = source_bar.volta.clone();
        let mut voice = Vec::<Note>::new();
        let mut cursor = 0;
        for &(index, region) in &by_measure[measure] {
            if !emit_rest(region.start_tick - cursor, &mut voice) {
                return Err(E::InvalidRegion { region: index });
            }
            let mut at = region.start_tick;
            let mut first = true;
            while at < region.end_tick {
                let (pitches, sounding, advance) = match request.pattern {
                    LegacyAccompanimentPattern::Held => {
                        let mut pitches = vec![region.bass_midi];
                        pitches.extend_from_slice(&region.chord_midi);
                        (pitches, region.end_tick - at, region.end_tick - at)
                    }
                    LegacyAccompanimentPattern::BassChordPulse => {
                        let beat = (at - region.start_tick) / GRID + u32::from(region.phase_beats);
                        let meter = staff.meter_at(measure, &source.settings.time_signature);
                        let is_bass = beat == 0 || (meter.numerator != 3 && beat.is_multiple_of(2));
                        (
                            if is_bass {
                                vec![region.bass_midi]
                            } else {
                                region.chord_midi.clone()
                            },
                            (region.end_tick - at).min(GRID / 2),
                            (region.end_tick - at).min(GRID),
                        )
                    }
                };
                let begin = voice.len();
                if !emit_span(&pitches, sounding, &mut voice) {
                    return Err(E::InvalidRegion { region: index });
                }
                if first && let Some(note) = voice.get_mut(begin) {
                    note.chord_symbol = Some(region.chord.clone());
                }
                for note in begin..voice.len() {
                    generated_notes.push(NoteAddr {
                        part: added_part,
                        staff: 0,
                        measure,
                        voice: 0,
                        note,
                    });
                }
                if !emit_rest(advance - sounding, &mut voice) {
                    return Err(E::InvalidRegion { region: index });
                }
                first = false;
                at += advance;
            }
            cursor = region.end_tick;
        }
        if !emit_rest(lengths[measure] - cursor, &mut voice) {
            return Err(E::InvalidMeasure { measure });
        }
        if generated_notes.len() > 8192 {
            return Err(E::InvalidRequest);
        }
        bar.voices[0] = voice;
        left.measures.push(bar);
    }
    accompaniment.staves.push(left);
    let mut score = source.clone();
    score.parts.push(accompaniment);
    if !super::validate::validate(&score).is_valid() {
        return Err(E::InvalidScore);
    }
    let resolved = ResolvedLegacyAccompaniment {
        contract_version: LEGACY_ACCOMPANIMENT_CONTRACT_VERSION,
        engine_version: ENGINE.into(),
        source_fingerprint: input_fingerprint(source).map_err(|_| E::Fingerprint)?,
        request: request.clone(),
    };
    Ok(LegacyAccompanimentResult {
        score,
        added_part,
        generated_notes,
        resolved,
    })
}

/// Reapply only to the same normalized source and implementation version.
pub fn replay_legacy_accompaniment(
    source: &Score,
    resolved: &ResolvedLegacyAccompaniment,
) -> Result<LegacyAccompanimentResult, LegacyAccompanimentError> {
    if resolved.contract_version != LEGACY_ACCOMPANIMENT_CONTRACT_VERSION
        || resolved.engine_version != ENGINE
    {
        return Err(LegacyAccompanimentError::ReplayVersion);
    }
    if input_fingerprint(source).map_err(|_| LegacyAccompanimentError::Fingerprint)?
        != resolved.source_fingerprint
    {
        return Err(LegacyAccompanimentError::ReplaySource);
    }
    arrange_legacy_accompaniment(source, &resolved.request)
}
