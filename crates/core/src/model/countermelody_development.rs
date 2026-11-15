//! Opt-in source-rhythm development for a chord-grounded countermelody.

use serde::{Deserialize, Serialize};

use super::arrange::{chord_at_note, preset_chord_pitch_classes};
use super::chord_proposals::fingerprint;
use super::chordal_pad::{MAX_GENERATED_NOTES, emit_rest, emit_span, melody_spans, tick};
use super::countermelody::{CountermelodyDiagnostic, CountermelodyOptions, arrange_countermelody};
use super::score::{NoteAddr, Score};

pub const COUNTERMELODY_DEVELOPMENT_CONTRACT_VERSION: u16 = 1;

#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "kebab-case")]
pub enum CountermelodyDevelopmentKind {
    /// Copy the motif's note/rest rhythm after a one-meter-unit entry delay.
    Imitation,
    /// Copy the rhythm and reverse the motif's directed intervals.
    Inversion,
    /// Double every motif note and rest duration.
    Augmentation,
    /// Halve every motif note and rest duration.
    Diminution,
    /// Enter halfway through each bar with inverted contour.
    Answer,
}

#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
#[serde(deny_unknown_fields)]
pub struct CountermelodyDevelopmentRequest {
    pub base: CountermelodyOptions,
    pub kind: CountermelodyDevelopmentKind,
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct MotifRhythmCell {
    /// Integer units of 1/32 quarter note.
    pub ticks: u32,
    pub sounding: bool,
}

#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
#[serde(deny_unknown_fields)]
pub struct ResolvedCountermelodyDevelopment {
    pub contract_version: u16,
    pub engine_version: String,
    pub source_fingerprint: String,
    pub request: CountermelodyDevelopmentRequest,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct CountermelodyDevelopmentResult {
    pub score: Score,
    pub resolved: ResolvedCountermelodyDevelopment,
    pub motif_intervals: Vec<i8>,
    pub source_rhythm: Vec<MotifRhythmCell>,
    pub transformed_rhythm: Vec<MotifRhythmCell>,
    pub generated_notes: Vec<NoteAddr>,
    pub attack_count: usize,
}

#[derive(Debug, thiserror::Error)]
pub enum CountermelodyDevelopmentError {
    #[error("invalid source score")]
    InvalidSource,
    #[error("base countermelody failed: {0:?}")]
    Base(CountermelodyDiagnostic),
    #[error("unsupported source motif rhythm")]
    UnsupportedMotifRhythm,
    #[error("developed countermelody is unplayable at measure {0}")]
    Unplayable(usize),
    #[error("developed countermelody exceeds the generated-note limit")]
    GeneratedNoteLimit,
    #[error("invalid developed score")]
    InvalidOutput,
    #[error("unsupported countermelody development contract")]
    UnsupportedContract,
    #[error("countermelody development source mismatch")]
    SourceMismatch,
}

fn source_rhythm(score: &Score, options: &CountermelodyOptions) -> Option<Vec<MotifRhythmCell>> {
    let staff = score.parts.get(options.melody_part_index)?.staves.first()?;
    let end = options.motif_end_measure_exclusive?;
    let mut cells = Vec::new();
    let mut sounding = 0;
    for measure_index in options.motif_start_measure..end {
        let measure = staff.measures.get(measure_index)?;
        for note in measure.voices.first()? {
            if note.is_grace || note.is_cue {
                continue;
            }
            if note.pitch_tie_start(0) || note.pitch_tie_end(0) || note.tuplet.is_some() {
                return None;
            }
            let beats = if note.is_plain_whole_rest() && measure.voices[0].len() == 1 {
                score.playback_measure_beats(measure_index)
            } else {
                note.beats()
            };
            let ticks = tick(beats)?;
            if ticks == 0 || cells.len() == 32 {
                return None;
            }
            if !note.is_rest {
                if note.is_unpitched || note.pitches.len() != 1 {
                    return None;
                }
                sounding += 1;
            }
            cells.push(MotifRhythmCell {
                ticks,
                sounding: !note.is_rest,
            });
            if sounding == 8 {
                break;
            }
        }
        if sounding == 8 {
            break;
        }
    }
    (sounding >= 3 && cells.iter().any(|cell| cell.sounding)).then_some(cells)
}

fn transformed_rhythm(
    cells: &[MotifRhythmCell],
    kind: CountermelodyDevelopmentKind,
) -> Option<Vec<MotifRhythmCell>> {
    cells
        .iter()
        .map(|cell| {
            let ticks = match kind {
                CountermelodyDevelopmentKind::Augmentation => cell.ticks.checked_mul(2)?,
                CountermelodyDevelopmentKind::Diminution => {
                    if !cell.ticks.is_multiple_of(2) {
                        return None;
                    }
                    cell.ticks / 2
                }
                _ => cell.ticks,
            };
            (ticks > 0).then_some(MotifRhythmCell {
                ticks,
                sounding: cell.sounding,
            })
        })
        .collect()
}

fn choose_pitch(
    tones: &[u8],
    options: &CountermelodyOptions,
    previous: Option<u8>,
    interval: i8,
    ceiling: Option<i16>,
) -> Option<u8> {
    let target = previous.map_or(i16::from(options.center_midi), |pitch| {
        i16::from(pitch) + i16::from(interval)
    });
    (options.low_midi..=options.high_midi)
        .filter(|pitch| tones.contains(&(pitch % 12)))
        .filter(|pitch| ceiling.is_none_or(|limit| i16::from(*pitch) <= limit))
        .min_by_key(|pitch| {
            (
                (i16::from(*pitch) - target).abs(),
                pitch.abs_diff(options.center_midi),
                *pitch,
            )
        })
}

/// Develop the source's first eight sounding motif notes into a separate part.
/// The rhythm restarts in every written bar, and harmony changes may split a
/// sounding cell into fresh chord-tone attacks. The source parts are retained.
pub fn arrange_countermelody_development(
    source: &Score,
    request: &CountermelodyDevelopmentRequest,
) -> Result<CountermelodyDevelopmentResult, CountermelodyDevelopmentError> {
    let source_fingerprint =
        fingerprint(source).ok_or(CountermelodyDevelopmentError::InvalidSource)?;
    let baseline = arrange_countermelody(source, request.base.clone());
    if let Some(problem) = baseline.diagnostics.first() {
        return Err(CountermelodyDevelopmentError::Base(problem.clone()));
    }
    let options = baseline.resolved_options;
    let source_rhythm = source_rhythm(source, &options)
        .ok_or(CountermelodyDevelopmentError::UnsupportedMotifRhythm)?;
    let transformed_rhythm = transformed_rhythm(&source_rhythm, request.kind)
        .ok_or(CountermelodyDevelopmentError::UnsupportedMotifRhythm)?;
    let mut score = baseline
        .score
        .ok_or(CountermelodyDevelopmentError::InvalidOutput)?;
    let added_part = baseline
        .added_part
        .ok_or(CountermelodyDevelopmentError::InvalidOutput)?;
    let staff = score
        .parts
        .get_mut(added_part)
        .and_then(|part| part.staves.first_mut())
        .ok_or(CountermelodyDevelopmentError::InvalidOutput)?;
    let count = staff.measures.len();
    let mut bar_starts = Vec::with_capacity(count + 1);
    bar_starts.push(0u32);
    for measure in 0..count {
        let ticks = tick(source.playback_measure_beats(measure))
            .ok_or(CountermelodyDevelopmentError::UnsupportedMotifRhythm)?;
        let next = bar_starts[measure]
            .checked_add(ticks)
            .ok_or(CountermelodyDevelopmentError::UnsupportedMotifRhythm)?;
        bar_starts.push(next);
    }
    let spans = melody_spans(source, options.melody_part_index, &bar_starts)
        .map_err(CountermelodyDevelopmentError::Unplayable)?;
    let mut generated_notes = Vec::new();
    let mut generated_events = 0usize;
    let mut attack_count = 0usize;
    let mut previous_pitch = None;
    let mut phrase_attack = 0usize;
    for measure in 0..count {
        let bar_ticks = bar_starts[measure + 1] - bar_starts[measure];
        let meter = source.parts[0].staves[0].meter_at(measure, &source.settings.time_signature);
        let unit = tick(4.0 / f64::from(meter.denominator))
            .ok_or(CountermelodyDevelopmentError::UnsupportedMotifRhythm)?;
        let delay = match request.kind {
            CountermelodyDevelopmentKind::Imitation => unit.min(bar_ticks),
            CountermelodyDevelopmentKind::Answer => (bar_ticks / 2).min(bar_ticks),
            _ => 0,
        };
        let mut voice = Vec::new();
        if delay > 0 && !emit_rest(delay, &mut voice) {
            return Err(CountermelodyDevelopmentError::UnsupportedMotifRhythm);
        }
        let mut cursor = delay;
        let mut cell_index = 0usize;
        let mut ending_attacks = 0usize;
        let ending = options.phrase_end_measures.binary_search(&measure).is_ok();
        while cursor < bar_ticks {
            let cell = &transformed_rhythm[cell_index % transformed_rhythm.len()];
            let stop = cursor.saturating_add(cell.ticks).min(bar_ticks);
            let mut cuts = vec![cursor];
            cuts.extend(
                options
                    .chords
                    .iter()
                    .filter(|placed| placed.measure_index == measure)
                    .filter_map(|placed| tick(placed.offset_beats))
                    .filter(|position| *position > cursor && *position < stop),
            );
            cuts.push(stop);
            for pair in cuts.windows(2) {
                let (from, to) = (pair[0], pair[1]);
                if !cell.sounding || (ending && ending_attacks > 0) {
                    if !emit_rest(to - from, &mut voice) {
                        return Err(CountermelodyDevelopmentError::UnsupportedMotifRhythm);
                    }
                } else {
                    let chord = chord_at_note(&options.chords, measure, f64::from(from) / 32.0)
                        .ok_or(CountermelodyDevelopmentError::Unplayable(measure))?;
                    let tones = preset_chord_pitch_classes(chord)
                        .ok_or(CountermelodyDevelopmentError::Unplayable(measure))?;
                    let absolute_from = f64::from(bar_starts[measure] + from) / 32.0;
                    let absolute_to = f64::from(bar_starts[measure] + to) / 32.0;
                    let ceiling = spans
                        .iter()
                        .filter(|(start, end, _)| {
                            *start < absolute_to - 1e-9 && *end > absolute_from + 1e-9
                        })
                        .map(|(_, _, midi)| *midi - i16::from(options.min_melody_gap_semitones))
                        .min();
                    let interval = if phrase_attack == 0 {
                        0
                    } else {
                        let slot = (phrase_attack - 1) % baseline.motif_intervals.len();
                        let value = baseline.motif_intervals[slot];
                        match request.kind {
                            CountermelodyDevelopmentKind::Inversion
                            | CountermelodyDevelopmentKind::Answer => -value,
                            _ => value,
                        }
                    };
                    let pitch = choose_pitch(&tones, &options, previous_pitch, interval, ceiling)
                        .ok_or(CountermelodyDevelopmentError::Unplayable(measure))?;
                    if !emit_span(&[pitch], to - from, &mut voice) {
                        return Err(CountermelodyDevelopmentError::UnsupportedMotifRhythm);
                    }
                    previous_pitch = Some(pitch);
                    phrase_attack += 1;
                    ending_attacks += 1;
                    attack_count += 1;
                }
                if generated_events.saturating_add(voice.len()) > MAX_GENERATED_NOTES {
                    return Err(CountermelodyDevelopmentError::GeneratedNoteLimit);
                }
            }
            cursor = stop;
            cell_index += 1;
        }
        generated_events += voice.len();
        for (note, event) in voice.iter().enumerate() {
            if !event.is_rest {
                generated_notes.push(NoteAddr {
                    part: added_part,
                    staff: 0,
                    measure,
                    voice: 0,
                    note,
                });
            }
        }
        staff.measures[measure].voices[0] = voice;
        if ending {
            previous_pitch = None;
            phrase_attack = 0;
        }
    }
    if !super::validate::validate(&score).is_valid() {
        return Err(CountermelodyDevelopmentError::InvalidOutput);
    }
    Ok(CountermelodyDevelopmentResult {
        score,
        resolved: ResolvedCountermelodyDevelopment {
            contract_version: COUNTERMELODY_DEVELOPMENT_CONTRACT_VERSION,
            engine_version: env!("CARGO_PKG_VERSION").into(),
            source_fingerprint,
            request: CountermelodyDevelopmentRequest {
                base: options,
                kind: request.kind,
            },
        },
        motif_intervals: baseline.motif_intervals,
        source_rhythm,
        transformed_rhythm,
        generated_notes,
        attack_count,
    })
}

pub fn replay_countermelody_development(
    source: &Score,
    resolved: &ResolvedCountermelodyDevelopment,
) -> Result<Score, CountermelodyDevelopmentError> {
    if resolved.contract_version != COUNTERMELODY_DEVELOPMENT_CONTRACT_VERSION
        || resolved.engine_version != env!("CARGO_PKG_VERSION")
    {
        return Err(CountermelodyDevelopmentError::UnsupportedContract);
    }
    if fingerprint(source).as_deref() != Some(resolved.source_fingerprint.as_str()) {
        return Err(CountermelodyDevelopmentError::SourceMismatch);
    }
    Ok(arrange_countermelody_development(source, &resolved.request)?.score)
}
