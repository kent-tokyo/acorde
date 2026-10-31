//! Deterministic, pitch-based arrangement of a multi-part score onto a
//! 2-staff accordion part (treble = right hand / melody, bass = left hand /
//! everything else).
//!
//! v1 heuristic: parts are ranked by mean MIDI pitch to pick the melody;
//! everything else is merged onto the bass staff by onset-time bucketing,
//! keeping only the shortest contributing duration at each onset. Dropped
//! independent countermelodies under a dense chord are the known ceiling —
//! a proper voice-leading reduction is real music-engraving work, not a v1
//! target. A single-part score (already a 2-staff piano-style reduction, or
//! a lone melodic line) skips the multi-part ranking entirely.

use serde::{Deserialize, Serialize};
use std::collections::BTreeMap;

use super::duration::Duration;
use super::gm::instrument_range;
use super::notation::{Barline, ChordSymbol, Clef, TupletBoundary, TupletInfo};
use super::pitch::Pitch;
use super::score::{Measure, Note, NoteAddr, Part, Score, Staff};
use crate::Error;

const ACCORDION_PROGRAM: u8 = 21;
// Conservative default for the first guided chromatic-accordion preset. A later
// instrument-specific contract can narrow or widen either hand explicitly.
const GUIDED_RIGHT_HAND_RANGE: (i16, i16) = (48, 96);
const GUIDED_LEFT_HAND_RANGE: (i16, i16) = (36, 84);
/// Onset/duration quantization grid — 64th-note resolution, the finest
/// [`Duration`] unit the model supports.
const GRID: f64 = 64.0;
/// Middle-C split point used only when a single part must be divided
/// between the two staves (no second candidate part to use as the bass).
const SPLIT_MIDI: u8 = 60;

#[derive(Debug, Clone, Serialize)]
pub struct PartCandidate {
    pub part_index: usize,
    pub name: String,
    pub mean_pitch: f64,
}

#[derive(Debug, Clone, Serialize)]
pub struct AccordionAnalysis {
    pub candidates: Vec<PartCandidate>,
    /// True when the top two candidates' mean pitch is within 3 semitones —
    /// the automatic ranking is a coin flip, caller should offer a picker.
    pub ambiguous: bool,
}

#[derive(Debug, Clone, Serialize)]
pub struct ArrangeResult {
    pub score: Score,
    pub notes: Vec<String>,
}

/// First-stage, explicitly harmonized accompaniment. No chord is inferred from the melody.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "kebab-case")]
pub enum AccordionPattern {
    HeldBassChord,
    BassChordPulse,
    AlternatingBassChord,
    Arpeggio,
    RisingArpeggio,
    AlbertiBass,
    OffbeatChords,
    SwingAccompaniment,
    WalkingBass,
}

#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
pub struct PositionedAccordionChord {
    pub measure_index: usize,
    /// Quarter-note beats from the beginning of the authored measure, on a 1/64-beat grid.
    pub offset_beats: f64,
    pub chord: ChordSymbol,
}

#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
pub struct GuidedAccordionOptions {
    pub pattern: AccordionPattern,
    pub chords: Vec<PositionedAccordionChord>,
}

#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
pub struct GuidedArrangeDiagnostic {
    pub code: String,
    pub message: String,
    pub measure_index: Option<usize>,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct GuidedAccordionResult {
    /// None on any error. A caller never receives a partially generated score.
    pub score: Option<Score>,
    pub resolved_options: GuidedAccordionOptions,
    pub diagnostics: Vec<GuidedArrangeDiagnostic>,
    pub source_notes: Vec<NoteAddr>,
    pub generated_notes: Vec<NoteAddr>,
    pub contract_version: u16,
}

fn guided_error(
    options: GuidedAccordionOptions,
    code: &str,
    message: impl Into<String>,
    measure_index: Option<usize>,
) -> GuidedAccordionResult {
    GuidedAccordionResult {
        score: None,
        resolved_options: options,
        diagnostics: vec![GuidedArrangeDiagnostic {
            code: code.to_string(),
            message: message.into(),
            measure_index,
        }],
        source_notes: Vec::new(),
        generated_notes: Vec::new(),
        contract_version: 1,
    }
}

fn accordion_pitch_class(name: &str) -> Option<u8> {
    let mut characters = name.chars();
    let base: i16 = match characters.next()? {
        'C' => 0,
        'D' => 2,
        'E' => 4,
        'F' => 5,
        'G' => 7,
        'A' => 9,
        'B' => 11,
        _ => return None,
    };
    let alter = match characters.next() {
        None => 0,
        Some('#') => 1,
        Some('b') => -1,
        _ => return None,
    };
    if characters.next().is_some() {
        return None;
    }
    Some((base + alter).rem_euclid(12) as u8)
}

#[derive(Clone)]
struct RealizedAccordionChord {
    symbol: ChordSymbol,
    bass_midi: u8,
    chord_midi: Vec<u8>,
}

fn realize_accordion_chord(symbol: &ChordSymbol) -> Option<RealizedAccordionChord> {
    if !symbol.degrees.is_empty() {
        return None;
    }
    let root = accordion_pitch_class(&symbol.root)?;
    let bass = symbol
        .bass
        .as_deref()
        .map(accordion_pitch_class)
        .unwrap_or(Some(root))?;
    let intervals: &[u8] = match symbol.kind.as_str() {
        "major" => &[0, 4, 7],
        "minor" => &[0, 3, 7],
        "diminished" => &[0, 3, 6],
        "augmented" => &[0, 4, 8],
        "major-seventh" => &[0, 4, 7, 11],
        "minor-seventh" => &[0, 3, 7, 10],
        "dominant" => &[0, 4, 7, 10],
        _ => return None,
    };
    Some(RealizedAccordionChord {
        symbol: symbol.clone(),
        bass_midi: 36 + bass,
        chord_midi: intervals
            .iter()
            .map(|interval| 60 + root + interval)
            .collect(),
    })
}

pub(crate) fn preset_chord_supported(symbol: &ChordSymbol, bass_mode: AccordionBassMode) -> bool {
    realize_accordion_chord(symbol).is_some_and(|realized| {
        bass_mode == AccordionBassMode::FreeBass || realized.chord_midi.len() <= 3
    })
}

fn guided_note(pitches: &[u8], duration: Duration) -> Note {
    let mut note = Note::new(Pitch::from_midi(pitches[0], false), duration);
    note.pitches = pitches
        .iter()
        .map(|midi| Pitch::from_midi(*midi, false))
        .collect();
    note
}

fn guided_region(
    pattern: AccordionPattern,
    chord: &RealizedAccordionChord,
    start_tick: u32,
    ticks: u32,
    next_bass_midi: Option<u8>,
    pickup_phase_tick: u32,
) -> Option<Vec<Note>> {
    let mut notes = Vec::new();
    match pattern {
        AccordionPattern::HeldBassChord => {
            let mut pitches = vec![chord.bass_midi];
            pitches.extend(&chord.chord_midi);
            let mut remaining = ticks;
            while remaining > 0 {
                let duration = Duration::whole_filling_beats(f64::from(remaining) / GRID);
                let filled = (duration.beats(0) * GRID).round() as u32;
                if filled == 0 || filled > remaining {
                    return None;
                }
                let mut note = guided_note(&pitches, duration);
                note.tie_end = !notes.is_empty();
                note.tie_start = remaining > filled;
                notes.push(note);
                remaining -= filled;
            }
        }
        AccordionPattern::BassChordPulse | AccordionPattern::AlternatingBassChord => {
            if pickup_phase_tick > 0 {
                let phase_start = pickup_phase_tick.checked_add(start_tick)?;
                if !phase_start.is_multiple_of(32) || !ticks.is_multiple_of(32) {
                    return None;
                }
                let mut phase = phase_start;
                let phase_end = phase.checked_add(ticks)?;
                while phase < phase_end {
                    let run = (phase_end - phase).min(64 - phase % 64);
                    let duration = match run {
                        32 => Duration::Eighth,
                        64 => Duration::Quarter,
                        _ => return None,
                    };
                    let pitches: Vec<u8> = match (phase / 64) % 4 {
                        0 => vec![chord.bass_midi],
                        2 if pattern == AccordionPattern::AlternatingBassChord => {
                            vec![chord.bass_midi + 7]
                        }
                        2 => vec![chord.bass_midi],
                        _ => chord.chord_midi.to_vec(),
                    };
                    notes.push(guided_note(&pitches, duration));
                    phase += run;
                }
            } else {
                if !ticks.is_multiple_of(64) {
                    return None;
                }
                for pulse in 0..ticks / 64 {
                    let pitches: Vec<u8> = match (start_tick / 64 + pulse) % 4 {
                        0 => vec![chord.bass_midi],
                        2 if pattern == AccordionPattern::AlternatingBassChord => {
                            vec![chord.bass_midi + 7]
                        }
                        2 => vec![chord.bass_midi],
                        _ => chord.chord_midi.to_vec(),
                    };
                    notes.push(guided_note(&pitches, Duration::Quarter));
                }
            }
        }
        AccordionPattern::Arpeggio => {
            if !ticks.is_multiple_of(32) {
                return None;
            }
            for pulse in 0..ticks / 32 {
                let midi = match (start_tick / 32 + pulse) % 4 {
                    0 => chord.bass_midi + 12,
                    1 | 3 => chord.chord_midi[1],
                    _ => chord.chord_midi[2],
                };
                notes.push(guided_note(&[midi], Duration::Eighth));
            }
        }
        AccordionPattern::RisingArpeggio | AccordionPattern::AlbertiBass => {
            if !ticks.is_multiple_of(32) {
                return None;
            }
            for pulse in 0..ticks / 32 {
                let phase = (start_tick / 32 + pulse) % 4;
                let midi = if pattern == AccordionPattern::RisingArpeggio {
                    match phase {
                        0 => chord.bass_midi + 12,
                        1 => chord.chord_midi[1],
                        2 => chord.chord_midi[2],
                        _ => chord
                            .chord_midi
                            .get(3)
                            .copied()
                            .unwrap_or(chord.chord_midi[0] + 12),
                    }
                } else {
                    match phase {
                        0 => chord.bass_midi + 12,
                        1 | 3 => chord.chord_midi[2],
                        _ => chord.chord_midi[1],
                    }
                };
                notes.push(guided_note(&[midi], Duration::Eighth));
            }
        }
        AccordionPattern::OffbeatChords => {
            if pickup_phase_tick > 0 {
                let phase_start = pickup_phase_tick.checked_add(start_tick)?;
                if !phase_start.is_multiple_of(32) || !ticks.is_multiple_of(32) {
                    return None;
                }
                for step in 0..ticks / 32 {
                    if (phase_start / 32 + step).is_multiple_of(2) {
                        notes.push(Note::rest(Duration::Eighth));
                    } else {
                        notes.push(guided_note(&chord.chord_midi, Duration::Eighth));
                    }
                }
            } else {
                if !ticks.is_multiple_of(64) {
                    return None;
                }
                for _ in 0..ticks / 64 {
                    notes.push(Note::rest(Duration::Eighth));
                    notes.push(guided_note(&chord.chord_midi, Duration::Eighth));
                }
            }
        }
        AccordionPattern::SwingAccompaniment => {
            if !ticks.is_multiple_of(64) {
                return None;
            }
            let triplet = Some(TupletInfo {
                actual_notes: 3,
                normal_notes: 2,
            });
            for _ in 0..ticks / 64 {
                let mut bass = guided_note(&[chord.bass_midi], Duration::Quarter);
                bass.tuplet = triplet.clone();
                bass.tuplet_boundary = Some(TupletBoundary::Start);
                let mut chord_note = guided_note(&chord.chord_midi, Duration::Eighth);
                chord_note.tuplet = triplet.clone();
                chord_note.tuplet_boundary = Some(TupletBoundary::Stop);
                notes.push(bass);
                notes.push(chord_note);
            }
        }
        AccordionPattern::WalkingBass => {
            if !ticks.is_multiple_of(64) {
                return None;
            }
            let pulses = ticks / 64;
            for pulse in 0..pulses {
                let midi = if pulse + 1 == pulses {
                    next_bass_midi.map_or(chord.bass_midi, |next| {
                        if chord.bass_midi <= next {
                            next.saturating_sub(1)
                        } else {
                            next.saturating_add(1)
                        }
                    })
                } else {
                    match (start_tick / 64 + pulse) % 3 {
                        0 => chord.bass_midi,
                        1 => chord.bass_midi + (chord.chord_midi[1] - chord.chord_midi[0]),
                        _ => chord.bass_midi + 7,
                    }
                };
                notes.push(guided_note(&[midi], Duration::Quarter));
            }
        }
    }
    if let Some(first) = notes.iter_mut().find(|note| !note.is_rest) {
        first.chord_symbol = Some(chord.symbol.clone());
    }
    Some(notes)
}

/// Generate a two-staff accordion score from a monophonic melody and positioned, explicit chords.
/// Source melody notes, bar lengths, ties, repeats and endings remain authored; invalid requests
/// return only structured diagnostics. Generation is deterministic in musical content and timing.
pub fn arrange_with_explicit_chords(
    score: &Score,
    options: GuidedAccordionOptions,
) -> GuidedAccordionResult {
    if score.parts.len() != 1 || score.parts[0].staves.len() != 1 {
        return guided_error(
            options,
            "unsupported-source",
            "expected one melody part and one staff",
            None,
        );
    }
    let source_staff = &score.parts[0].staves[0];
    if source_staff.measures.is_empty() || source_staff.measures.len() > 2048 {
        return guided_error(
            options,
            "unsupported-source",
            "measure count is empty or exceeds 2048",
            None,
        );
    }
    if options.chords.is_empty() || options.chords.len() > 8192 {
        return guided_error(
            options,
            "missing-chords",
            "provide 1 to 8192 positioned chords",
            None,
        );
    }
    for (index, measure) in source_staff.measures.iter().enumerate() {
        if measure.voices.iter().skip(1).any(|voice| !voice.is_empty())
            || measure.voices[0]
                .iter()
                .any(|note| !note.is_rest && note.pitches.len() != 1)
            || measure.multi_rest_count.is_some()
        {
            return guided_error(
                options,
                "unsupported-source",
                "expected a monophonic melody without measure repeats",
                Some(index),
            );
        }
    }
    let validation = super::validate::validate(score);
    if !validation.is_valid() {
        return guided_error(
            options,
            "invalid-source",
            format!("source validation failed: {:?}", validation.errors[0]),
            None,
        );
    }
    let mut regions: Vec<Vec<(u32, RealizedAccordionChord)>> =
        vec![Vec::new(); source_staff.measures.len()];
    let mut length_ticks = Vec::with_capacity(source_staff.measures.len());
    for index in 0..source_staff.measures.len() {
        let beats = score.playback_measure_beats(index);
        let ticks = (beats * GRID).round();
        if !ticks.is_finite() || ticks <= 0.0 || (ticks / GRID - beats).abs() > 1e-9 {
            return guided_error(
                options,
                "unsupported-rhythm",
                "measure length must be on a 1/64-beat grid",
                Some(index),
            );
        }
        length_ticks.push(ticks as u32);
    }
    for placed in &options.chords {
        let index = placed.measure_index;
        let Some(&length) = length_ticks.get(index) else {
            return guided_error(
                options,
                "chord-position",
                "chord measure is outside the score",
                Some(index),
            );
        };
        let offset = (placed.offset_beats * GRID).round();
        if !offset.is_finite()
            || offset < 0.0
            || offset >= f64::from(length)
            || (offset / GRID - placed.offset_beats).abs() > 1e-9
        {
            return guided_error(
                options,
                "chord-position",
                "chord offset must be inside the measure on a 1/64-beat grid",
                Some(index),
            );
        }
        let Some(chord) = realize_accordion_chord(&placed.chord) else {
            return guided_error(
                options.clone(),
                "unsupported-chord",
                format!("cannot realize chord {}", placed.chord.display_text()),
                Some(index),
            );
        };
        regions[index].push((offset as u32, chord));
    }
    let mut left = Staff::new(Clef::Bass);
    let mut generated_notes = Vec::new();
    let mut current_chord: Option<RealizedAccordionChord> = None;
    for (index, source_measure) in source_staff.measures.iter().enumerate() {
        // A fractional first pickup occupies the end of its nominal meter. Whole-beat
        // pickups retain the established measure-start phase for preset compatibility.
        let pickup_phase_tick = if index == 0 && !length_ticks[index].is_multiple_of(64) {
            let meter = source_measure
                .time_sig
                .as_ref()
                .unwrap_or(&score.settings.time_signature);
            let meter_ticks = (f64::from(meter.numerator) * 4.0 / f64::from(meter.denominator)
                * GRID)
                .round() as u32;
            meter_ticks.saturating_sub(length_ticks[index])
        } else {
            0
        };
        regions[index].sort_by_key(|(offset, _)| *offset);
        if regions[index]
            .windows(2)
            .any(|window| window[0].0 == window[1].0)
        {
            return guided_error(
                options,
                "chord-position",
                "two chords occupy the same position",
                Some(index),
            );
        }
        if regions[index]
            .first()
            .is_none_or(|(offset, _)| *offset != 0)
        {
            let Some(chord) = current_chord.clone() else {
                return guided_error(
                    options,
                    "missing-chords",
                    "no explicit chord covers the start of this measure",
                    Some(index),
                );
            };
            regions[index].insert(0, (0, chord));
        }
        // A repeated spelling of the same harmony is an annotation, not a
        // request to restart the accompaniment or its walking-bass approach.
        regions[index].dedup_by(|later, earlier| later.1.symbol == earlier.1.symbol);
        let mut generated = Vec::new();
        for position in 0..regions[index].len() {
            let (start, chord) = &regions[index][position];
            let end = regions[index]
                .get(position + 1)
                .map_or(length_ticks[index], |(offset, _)| *offset);
            let next_bass = regions[index]
                .get(position + 1)
                .map(|(_, next)| next.bass_midi)
                .or_else(|| {
                    regions.iter().skip(index + 1).find_map(|bar| {
                        bar.iter()
                            .min_by_key(|(offset, _)| *offset)
                            .map(|(_, next)| next.bass_midi)
                    })
                });
            let Some(mut notes) = guided_region(
                options.pattern,
                chord,
                *start,
                end - start,
                next_bass,
                pickup_phase_tick,
            ) else {
                return guided_error(
                    options,
                    "unsupported-rhythm",
                    "pattern cannot fill the chord region exactly",
                    Some(index),
                );
            };
            for (offset, note) in notes.iter().enumerate() {
                if !note.is_rest {
                    generated_notes.push(NoteAddr {
                        part: 0,
                        staff: 1,
                        measure: index,
                        voice: 0,
                        note: generated.len() + offset,
                    });
                }
            }
            generated.append(&mut notes);
            current_chord = Some(chord.clone());
        }
        let mut measure = Measure::empty(
            source_measure
                .time_sig
                .as_ref()
                .map_or(score.settings.time_signature.numerator, |ts| ts.numerator),
            source_measure
                .time_sig
                .as_ref()
                .map_or(score.settings.time_signature.denominator, |ts| {
                    ts.denominator
                }),
        );
        measure.number = source_measure.number;
        measure.time_sig = source_measure.time_sig.clone();
        measure.key_sig = source_measure.key_sig.clone();
        measure.tempo = source_measure.tempo;
        measure.tempo_ramp_to = source_measure.tempo_ramp_to;
        measure.barline_left = source_measure.barline_left.clone();
        measure.barline_right = source_measure.barline_right.clone();
        measure.volta = source_measure.volta.clone();
        measure.system_break = source_measure.system_break;
        measure.page_break = source_measure.page_break;
        measure.section_break = source_measure.section_break;
        measure.actual_length = source_measure.actual_length;
        measure.voices[0] = generated;
        left.measures.push(measure);
    }
    let mut output = score.clone();
    output.id = uuid::Uuid::new_v4().to_string();
    let mut part = Part::new("Accordion", "Acc.");
    part.midi_program = ACCORDION_PROGRAM;
    part.staves = vec![source_staff.clone(), left];
    output.parts = vec![part];
    output.part_groups.clear();
    output.views.clear();
    for (staff_index, staff) in output.parts[0].staves.iter().enumerate() {
        let (min_midi, max_midi) = if staff_index == 0 {
            GUIDED_RIGHT_HAND_RANGE
        } else {
            GUIDED_LEFT_HAND_RANGE
        };
        for (measure_index, measure) in staff.measures.iter().enumerate() {
            for note in &measure.voices[0] {
                if note
                    .pitches
                    .iter()
                    .any(|pitch| pitch.to_midi() < min_midi || pitch.to_midi() > max_midi)
                {
                    return guided_error(
                        options,
                        "out-of-range",
                        format!("hand {staff_index} requires MIDI range {min_midi}..={max_midi}"),
                        Some(measure_index),
                    );
                }
            }
        }
    }
    let output_validation = super::validate::validate(&output);
    if !output_validation.is_valid() {
        return guided_error(
            options,
            "invalid-output",
            format!(
                "generated score validation failed: {:?}",
                output_validation.errors[0]
            ),
            None,
        );
    }
    let source_notes = source_staff
        .measures
        .iter()
        .enumerate()
        .flat_map(|(measure, bar)| {
            bar.voices[0]
                .iter()
                .enumerate()
                .filter(|(_, note)| !note.is_rest)
                .map(move |(note, _)| NoteAddr {
                    part: 0,
                    staff: 0,
                    measure,
                    voice: 0,
                    note,
                })
        })
        .collect();
    GuidedAccordionResult {
        score: Some(output),
        resolved_options: options,
        diagnostics: Vec::new(),
        source_notes,
        generated_notes,
        contract_version: 1,
    }
}

#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "kebab-case")]
pub enum AccordionInversion {
    Root,
    First,
    Second,
    Third,
    Nearby,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "kebab-case")]
pub enum AccordionMelodicVoice {
    Unchanged,
    ThirdBelow,
    SixthBelow,
    OctaveBelow,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "kebab-case")]
pub enum AccordionSwingPolicy {
    Straight,
    NotatedAccompanimentTwoToOne,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "kebab-case")]
pub enum AccordionBassMode {
    StandardBass,
    FreeBass,
}

#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
pub struct LayeredAccordionOptions {
    pub chords: Vec<PositionedAccordionChord>,
    /// An explicitly supplied alternative progression. No melody harmonization is inferred.
    pub alternative_chords: Option<Vec<PositionedAccordionChord>>,
    pub inversion: Option<AccordionInversion>,
    pub accompaniment: Option<AccordionPattern>,
    pub melodic_voice: AccordionMelodicVoice,
    pub swing: AccordionSwingPolicy,
    pub bass_mode: AccordionBassMode,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct LayeredAccordionResult {
    pub score: Option<Score>,
    pub resolved_options: LayeredAccordionOptions,
    pub resolved_chords: Vec<PositionedAccordionChord>,
    pub diagnostics: Vec<GuidedArrangeDiagnostic>,
    pub source_notes: Vec<NoteAddr>,
    pub generated_notes: Vec<NoteAddr>,
    /// A host should leave PlaybackOptions.swing unset for the notated 2:1 policy.
    pub notated_swing: bool,
    pub contract_version: u16,
}

fn layered_error(
    options: LayeredAccordionOptions,
    code: &str,
    message: impl Into<String>,
    measure_index: Option<usize>,
) -> LayeredAccordionResult {
    LayeredAccordionResult {
        score: None,
        resolved_options: options,
        resolved_chords: Vec::new(),
        diagnostics: vec![GuidedArrangeDiagnostic {
            code: code.to_string(),
            message: message.into(),
            measure_index,
        }],
        source_notes: Vec::new(),
        generated_notes: Vec::new(),
        notated_swing: false,
        contract_version: 1,
    }
}

fn chord_pitch_name(midi: u8) -> &'static str {
    [
        "C", "C#", "D", "D#", "E", "F", "F#", "G", "G#", "A", "A#", "B",
    ][(midi % 12) as usize]
}

fn resolve_layered_chords(
    chords: &mut [PositionedAccordionChord],
    inversion: Option<AccordionInversion>,
) -> Result<(), (usize, String)> {
    let mut previous_bass: Option<u8> = None;
    chords.sort_by(|a, b| {
        (a.measure_index, a.offset_beats)
            .partial_cmp(&(b.measure_index, b.offset_beats))
            .unwrap_or(std::cmp::Ordering::Equal)
    });
    for placed in chords {
        let Some(realized) = realize_accordion_chord(&placed.chord) else {
            return Err((
                placed.measure_index,
                format!("cannot realize chord {}", placed.chord.display_text()),
            ));
        };
        if let Some(policy) = inversion {
            let index = match policy {
                AccordionInversion::Root => 0,
                AccordionInversion::First => 1,
                AccordionInversion::Second => 2,
                AccordionInversion::Third => 3,
                AccordionInversion::Nearby => realized
                    .chord_midi
                    .iter()
                    .enumerate()
                    .min_by_key(|(_, midi)| {
                        previous_bass.map_or(0, |previous| {
                            (i16::from(**midi % 12 + 36) - i16::from(previous)).abs()
                        })
                    })
                    .map_or(0, |(index, _)| index),
            };
            let Some(&tone) = realized.chord_midi.get(index) else {
                return Err((
                    placed.measure_index,
                    "requested inversion has no chord tone".to_string(),
                ));
            };
            placed.chord.bass = Some(chord_pitch_name(tone).to_string());
            previous_bass = Some(36 + tone % 12);
        } else {
            previous_bass = Some(realized.bass_midi);
        }
    }
    Ok(())
}

fn chord_at_note(
    chords: &[PositionedAccordionChord],
    measure_index: usize,
    offset_beats: f64,
) -> Option<&ChordSymbol> {
    chords
        .iter()
        .rfind(|placed| {
            placed.measure_index < measure_index
                || (placed.measure_index == measure_index
                    && placed.offset_beats <= offset_beats + 1e-9)
        })
        .map(|placed| &placed.chord)
}

fn harmony_voice_pitch(
    source_midi: i16,
    chord: &ChordSymbol,
    voice: AccordionMelodicVoice,
) -> Option<i16> {
    if voice == AccordionMelodicVoice::OctaveBelow {
        return Some(source_midi - 12);
    }
    let realized = realize_accordion_chord(chord)?;
    let allowed: &[i16] = match voice {
        AccordionMelodicVoice::ThirdBelow => &[3, 4],
        AccordionMelodicVoice::SixthBelow => &[8, 9],
        _ => return None,
    };
    (GUIDED_RIGHT_HAND_RANGE.0..=GUIDED_RIGHT_HAND_RANGE.1)
        .rev()
        .filter(|candidate| {
            allowed.contains(&(source_midi - candidate))
                && realized
                    .chord_midi
                    .iter()
                    .any(|tone| i16::from(*tone % 12) == candidate.rem_euclid(12))
        })
        .max()
}

/// Compose explicit harmony, accompaniment and an optional added melodic voice.
/// Each layer is opt-in; the original melody staff is copied unchanged. A fixed 2:1 swing
/// policy is encoded as quarter-plus-eighth triplets in the accompaniment only.
pub fn arrange_with_layers(
    score: &Score,
    options: LayeredAccordionOptions,
) -> LayeredAccordionResult {
    let mut resolved_chords = options
        .alternative_chords
        .clone()
        .unwrap_or_else(|| options.chords.clone());
    if resolved_chords.is_empty() {
        return layered_error(
            options,
            "missing-chords",
            "supply an explicit chord progression",
            None,
        );
    }
    if let Err((measure, message)) = resolve_layered_chords(&mut resolved_chords, options.inversion)
    {
        return layered_error(options, "unsupported-chord", message, Some(measure));
    }
    let notated_swing = options.swing == AccordionSwingPolicy::NotatedAccompanimentTwoToOne;
    if notated_swing && options.accompaniment.is_none() {
        return layered_error(
            options,
            "unsupported-swing",
            "notated swing needs an accompaniment layer",
            None,
        );
    }
    let pattern = if notated_swing {
        AccordionPattern::SwingAccompaniment
    } else {
        options
            .accompaniment
            .unwrap_or(AccordionPattern::HeldBassChord)
    };
    if options.bass_mode == AccordionBassMode::StandardBass
        && (resolved_chords.iter().any(|placed| {
            realize_accordion_chord(&placed.chord).is_some_and(|chord| chord.chord_midi.len() > 3)
        }) || matches!(
            pattern,
            AccordionPattern::Arpeggio
                | AccordionPattern::RisingArpeggio
                | AccordionPattern::AlbertiBass
                | AccordionPattern::WalkingBass
        ))
    {
        return layered_error(
            options,
            "unsupported-standard-bass",
            "the selected seventh harmony or melodic bass pattern requires free bass",
            None,
        );
    }
    let base = arrange_with_explicit_chords(
        score,
        GuidedAccordionOptions {
            pattern,
            chords: resolved_chords.clone(),
        },
    );
    let Some(mut output) = base.score else {
        return LayeredAccordionResult {
            score: None,
            resolved_options: options,
            resolved_chords,
            diagnostics: base.diagnostics,
            source_notes: Vec::new(),
            generated_notes: Vec::new(),
            notated_swing,
            contract_version: 1,
        };
    };
    let mut generated_notes = base.generated_notes;
    if options.accompaniment.is_none() {
        for measure in &mut output.parts[0].staves[1].measures {
            measure.voices[0].clear();
        }
        generated_notes.clear();
    }
    if options.melodic_voice != AccordionMelodicVoice::Unchanged {
        for (measure_index, measure) in output.parts[0].staves[0].measures.iter_mut().enumerate() {
            let mut offset = 0.0;
            let mut voice = Vec::new();
            for source_note in &measure.voices[0] {
                let mut added = if source_note.is_rest {
                    Note::rest(source_note.duration.clone())
                } else {
                    Note::new(source_note.pitches[0].clone(), source_note.duration.clone())
                };
                added.dot_count = source_note.dot_count;
                added.tuplet = source_note.tuplet.clone();
                added.tuplet_boundary = source_note.tuplet_boundary;
                added.beam = source_note.beam;
                added.articulations = source_note.articulations.clone();
                added.dynamic = source_note.dynamic;
                added.is_grace = source_note.is_grace;
                added.grace_slash = source_note.grace_slash;
                added.is_cue = source_note.is_cue;
                if options.melodic_voice == AccordionMelodicVoice::OctaveBelow {
                    added.tie_start = source_note.tie_start;
                    added.tie_end = source_note.tie_end;
                }
                if !source_note.is_rest {
                    let Some(chord) = chord_at_note(&resolved_chords, measure_index, offset) else {
                        return layered_error(
                            options,
                            "missing-chords",
                            "no chord covers a melody note",
                            Some(measure_index),
                        );
                    };
                    let source_midi = source_note.pitches[0].to_midi();
                    let Some(target) =
                        harmony_voice_pitch(source_midi, chord, options.melodic_voice)
                    else {
                        return layered_error(
                            options,
                            "incompatible-harmony",
                            "no chord tone fits the requested melodic interval",
                            Some(measure_index),
                        );
                    };
                    if !(GUIDED_RIGHT_HAND_RANGE.0..=GUIDED_RIGHT_HAND_RANGE.1).contains(&target) {
                        return layered_error(
                            options,
                            "out-of-range",
                            "added melodic voice exceeds the right-hand range",
                            Some(measure_index),
                        );
                    }
                    added.pitches[0] = Pitch::from_midi(target as u8, false);
                    generated_notes.push(NoteAddr {
                        part: 0,
                        staff: 0,
                        measure: measure_index,
                        voice: 1,
                        note: voice.len(),
                    });
                }
                offset += source_note.beats();
                voice.push(added);
            }
            measure.voices[1] = voice;
        }
    }
    let validation = super::validate::validate(&output);
    if !validation.is_valid() {
        return layered_error(
            options,
            "invalid-output",
            format!(
                "layered score validation failed: {:?}",
                validation.errors[0]
            ),
            None,
        );
    }
    LayeredAccordionResult {
        score: Some(output),
        resolved_options: options,
        resolved_chords,
        diagnostics: Vec::new(),
        source_notes: base.source_notes,
        generated_notes,
        notated_swing,
        contract_version: 1,
    }
}

fn is_percussion_part(part: &Part) -> bool {
    part.midi_channel == 9 || part.staves.iter().any(|s| s.clef == Clef::Percussion)
}

fn mean_pitch(part: &Part) -> Option<f64> {
    let (sum, count) = part
        .staves
        .iter()
        .flat_map(|s| s.measures.iter())
        .flat_map(|m| m.voices.iter())
        .flat_map(|v| v.iter())
        .filter(|n| !n.is_rest && !n.is_grace)
        .flat_map(|n| n.pitches.iter())
        .fold((0i64, 0i64), |(sum, count), p| {
            (sum + p.to_midi() as i64, count + 1)
        });
    if count == 0 {
        None
    } else {
        Some(sum as f64 / count as f64)
    }
}

/// Rank non-percussion, non-silent parts by mean pitch (descending). The top
/// part is the default melody/right-hand candidate.
pub fn analyze_for_accordion(score: &Score) -> AccordionAnalysis {
    let mut candidates: Vec<PartCandidate> = score
        .parts
        .iter()
        .enumerate()
        .filter(|(_, p)| !is_percussion_part(p))
        .filter_map(|(i, p)| {
            mean_pitch(p).map(|mp| PartCandidate {
                part_index: i,
                name: p.name.clone(),
                mean_pitch: mp,
            })
        })
        .collect();
    candidates.sort_by(|a, b| {
        b.mean_pitch
            .partial_cmp(&a.mean_pitch)
            .unwrap_or(std::cmp::Ordering::Equal)
    });
    let ambiguous =
        candidates.len() >= 2 && (candidates[0].mean_pitch - candidates[1].mean_pitch).abs() < 3.0;
    AccordionAnalysis {
        candidates,
        ambiguous,
    }
}

struct SourceEvent {
    onset_ticks: i64,
    beats: f64,
    pitches: Vec<Pitch>,
}

fn events_from_parts(
    score: &Score,
    part_indices: &[usize],
    measure_idx: usize,
) -> Vec<SourceEvent> {
    let mut events = Vec::new();
    for &pi in part_indices {
        let part = &score.parts[pi];
        for staff in &part.staves {
            let Some(measure) = staff.measures.get(measure_idx) else {
                continue;
            };
            if measure.multi_rest_count.is_some() {
                continue;
            }
            for voice in &measure.voices {
                let mut onset = 0.0f64;
                for note in voice {
                    let b = note.beats();
                    if !note.is_rest && !note.is_grace && !note.pitches.is_empty() {
                        let pitches = note
                            .pitches
                            .iter()
                            .map(|p| {
                                let midi = (p.to_midi() + staff.transpose_semitones as i16)
                                    .clamp(0, 127) as u8;
                                Pitch::from_midi(midi, false)
                            })
                            .collect();
                        events.push(SourceEvent {
                            onset_ticks: (onset * GRID).round() as i64,
                            beats: b,
                            pitches,
                        });
                    }
                    onset += b;
                }
            }
        }
    }
    events
}

/// Same walk as [`events_from_parts`] but for a single part, splitting each
/// note's pitches at [`SPLIT_MIDI`] instead of by which part they came from.
/// Used only when there is no second candidate part to merge in as the bass.
fn events_from_pitch_split(
    score: &Score,
    part_index: usize,
    measure_idx: usize,
    high: bool,
) -> Vec<SourceEvent> {
    let part = &score.parts[part_index];
    let mut events = Vec::new();
    for staff in &part.staves {
        let Some(measure) = staff.measures.get(measure_idx) else {
            continue;
        };
        if measure.multi_rest_count.is_some() {
            continue;
        }
        for voice in &measure.voices {
            let mut onset = 0.0f64;
            for note in voice {
                let b = note.beats();
                if !note.is_rest && !note.is_grace {
                    let pitches: Vec<Pitch> = note
                        .pitches
                        .iter()
                        .filter_map(|p| {
                            let midi = (p.to_midi() + staff.transpose_semitones as i16)
                                .clamp(0, 127) as u8;
                            let keep = if high {
                                midi >= SPLIT_MIDI
                            } else {
                                midi < SPLIT_MIDI
                            };
                            if keep {
                                Some(Pitch::from_midi(midi, false))
                            } else {
                                None
                            }
                        })
                        .collect();
                    if !pitches.is_empty() {
                        events.push(SourceEvent {
                            onset_ticks: (onset * GRID).round() as i64,
                            beats: b,
                            pitches,
                        });
                    }
                }
                onset += b;
            }
        }
    }
    events
}

fn fill_rests(notes: &mut Vec<Note>, mut remaining: f64) {
    while remaining > 1.0 / GRID {
        let dur = Duration::whole_filling_beats(remaining);
        let filled = dur.beats(0);
        notes.push(Note::rest(dur));
        remaining -= filled;
    }
}

/// Bucket-merge precomputed `events_per_measure` onto a single staff, one
/// measure at a time, gap-filling with rests via [`Duration::whole_filling_beats`]
/// (the same greedy decomposition [`Measure::empty`] uses).
fn assemble_staff(
    clef: Clef,
    template_per_measure: Vec<Option<Measure>>,
    events_per_measure: Vec<Vec<SourceEvent>>,
    default_ts: super::notation::TimeSignature,
) -> Staff {
    let mut current_ts = default_ts;
    let mut measures = Vec::with_capacity(template_per_measure.len());

    for (mi, (template, events)) in template_per_measure
        .into_iter()
        .zip(events_per_measure)
        .enumerate()
    {
        if let Some(ts) = template.as_ref().and_then(|m| m.time_sig.as_ref()) {
            current_ts = ts.clone();
        }
        let total_beats = current_ts.total_beats();

        let mut buckets: BTreeMap<i64, Vec<SourceEvent>> = BTreeMap::new();
        for ev in events {
            buckets.entry(ev.onset_ticks).or_default().push(ev);
        }
        let keys: Vec<i64> = buckets.keys().copied().collect();

        let mut voice0: Vec<Note> = Vec::new();
        let mut cursor = 0.0f64;
        for (idx, &key) in keys.iter().enumerate() {
            let onset = key as f64 / GRID;
            if onset < cursor - 1.0 / GRID {
                continue;
            } // swallowed by a longer preceding event
            if onset > cursor {
                fill_rests(&mut voice0, onset - cursor);
                cursor = onset;
            }

            let next_onset = keys
                .get(idx + 1)
                .map(|&k| k as f64 / GRID)
                .unwrap_or(total_beats);
            let group = &buckets[&key];
            let shortest = group.iter().map(|e| e.beats).fold(f64::MAX, f64::min);
            let cap = (next_onset - onset).max(1.0 / GRID);
            let sounding = shortest.min(cap).min((total_beats - onset).max(1.0 / GRID));

            let mut pitches: Vec<Pitch> = Vec::new();
            let mut seen_midi: Vec<u8> = Vec::new();
            for ev in group {
                for p in &ev.pitches {
                    let midi = p.to_midi().clamp(0, 127) as u8;
                    if !seen_midi.contains(&midi) {
                        seen_midi.push(midi);
                        pitches.push(p.clone());
                    }
                }
            }
            if pitches.is_empty() {
                continue;
            }

            let dur = Duration::whole_filling_beats(sounding);
            let emitted = dur.beats(0);
            let mut note = Note::new(pitches[0].clone(), dur);
            note.pitches = pitches;
            voice0.push(note);
            cursor += emitted;
        }
        if total_beats - cursor > 1.0 / GRID {
            fill_rests(&mut voice0, total_beats - cursor);
        }
        if voice0.is_empty() {
            fill_rests(&mut voice0, total_beats);
        }

        let mut measure = Measure::empty(current_ts.numerator, current_ts.denominator);
        measure.number = mi as u32 + 1;
        measure.time_sig = template.as_ref().and_then(|m| m.time_sig.clone());
        measure.key_sig = template.as_ref().and_then(|m| m.key_sig.clone());
        measure.tempo = template.as_ref().and_then(|m| m.tempo);
        measure.barline_left = template
            .as_ref()
            .map(|m| m.barline_left.clone())
            .unwrap_or(Barline::Normal);
        measure.barline_right = template
            .as_ref()
            .map(|m| m.barline_right.clone())
            .unwrap_or(Barline::Normal);
        measure.voices[0] = voice0;
        measures.push(measure);
    }

    Staff {
        clef,
        measures,
        transpose_semitones: 0,
        tablature: None,
        presentation: Default::default(),
    }
}

fn template_measure_for_parts(score: &Score, part_indices: &[usize], mi: usize) -> Option<Measure> {
    part_indices
        .iter()
        .filter_map(|&pi| score.parts[pi].staves.first())
        .find_map(|s| s.measures.get(mi).cloned())
}

/// Octave-shift `score` (expected to be the freshly-merged, single-part
/// accordion score) so its mean pitch sits within the practical range for
/// the accordion GM program. Only multiples of 12 are considered — an
/// arbitrary semitone shift would break the key signature.
fn octave_fit(score: &Score) -> (Score, i8) {
    let (lo, hi) = instrument_range(ACCORDION_PROGRAM);
    let target_mid = (lo as f64 + hi as f64) / 2.0;
    let Some(mp) = score.parts.first().and_then(mean_pitch) else {
        return (score.clone(), 0);
    };

    let shift = [-24i8, -12, 0, 12, 24]
        .into_iter()
        .min_by(|&a, &b| {
            let da = (mp + a as f64 - target_mid).abs();
            let db = (mp + b as f64 - target_mid).abs();
            da.partial_cmp(&db).unwrap_or(std::cmp::Ordering::Equal)
        })
        .unwrap_or(0);

    if shift == 0 {
        (score.clone(), 0)
    } else {
        (super::score::transpose(score, shift), shift)
    }
}

/// Arrange `score` for accordion: merge onto two staves (treble = right
/// hand / melody, bass = left hand / everything else), reassign to the
/// Accordion GM program, and octave-fit to its practical range.
///
/// `right_hand_part_index` overrides the automatic mean-pitch ranking from
/// [`analyze_for_accordion`] — pass `None` to use the default (highest mean
/// pitch). Percussion parts are always excluded from both staves.
pub fn arrange_for_accordion(
    score: &Score,
    right_hand_part_index: Option<usize>,
) -> Result<ArrangeResult, Error> {
    let analysis = analyze_for_accordion(score);
    if analysis.candidates.is_empty() {
        return Err(Error::InvalidCommand(
            "no pitched, non-percussion part to arrange".to_string(),
        ));
    }

    let treble_index = match right_hand_part_index {
        Some(i) => {
            if i >= score.parts.len() || is_percussion_part(&score.parts[i]) {
                return Err(Error::PartNotFound(i));
            }
            i
        }
        None => analysis.candidates[0].part_index,
    };
    let bass_indices: Vec<usize> = analysis
        .candidates
        .iter()
        .map(|c| c.part_index)
        .filter(|&i| i != treble_index)
        .collect();

    let mut notes = Vec::new();
    let measure_count = analysis
        .candidates
        .iter()
        .map(|c| {
            score.parts[c.part_index]
                .staves
                .iter()
                .map(|s| s.measures.len())
                .max()
                .unwrap_or(0)
        })
        .max()
        .unwrap_or(0);
    let default_ts = score.settings.time_signature.clone();

    let (treble_staff, bass_staff) = if !bass_indices.is_empty() {
        let treble_template: Vec<Option<Measure>> = (0..measure_count)
            .map(|mi| template_measure_for_parts(score, &[treble_index], mi))
            .collect();
        let treble_events: Vec<Vec<SourceEvent>> = (0..measure_count)
            .map(|mi| events_from_parts(score, &[treble_index], mi))
            .collect();
        let bass_template: Vec<Option<Measure>> = (0..measure_count)
            .map(|mi| template_measure_for_parts(score, &bass_indices, mi))
            .collect();
        let bass_events: Vec<Vec<SourceEvent>> = (0..measure_count)
            .map(|mi| events_from_parts(score, &bass_indices, mi))
            .collect();
        notes.push(format!(
            "右手(高音部): {} / 左手(低音部): {}パートを統合",
            analysis
                .candidates
                .iter()
                .find(|c| c.part_index == treble_index)
                .map(|c| c.name.as_str())
                .unwrap_or(""),
            bass_indices.len()
        ));
        (
            assemble_staff(
                Clef::Treble,
                treble_template,
                treble_events,
                default_ts.clone(),
            ),
            assemble_staff(Clef::Bass, bass_template, bass_events, default_ts),
        )
    } else {
        let template: Vec<Option<Measure>> = (0..measure_count)
            .map(|mi| template_measure_for_parts(score, &[treble_index], mi))
            .collect();
        let treble_events: Vec<Vec<SourceEvent>> = (0..measure_count)
            .map(|mi| events_from_pitch_split(score, treble_index, mi, true))
            .collect();
        let bass_events: Vec<Vec<SourceEvent>> = (0..measure_count)
            .map(|mi| events_from_pitch_split(score, treble_index, mi, false))
            .collect();
        notes.push("単一パートのため中央ハ(MIDI 60)を基準に上下2段へ分割".to_string());
        (
            assemble_staff(
                Clef::Treble,
                template.clone(),
                treble_events,
                default_ts.clone(),
            ),
            assemble_staff(Clef::Bass, template, bass_events, default_ts),
        )
    };

    let mut accordion_part = Part::new("Accordion", "Acc.");
    accordion_part.midi_program = ACCORDION_PROGRAM;
    accordion_part.staves = vec![treble_staff, bass_staff];

    let mut merged = Score {
        id: uuid::Uuid::new_v4().to_string(),
        schema_version: 1,
        metadata: score.metadata.clone(),
        settings: score.settings.clone(),
        parts: vec![accordion_part],
        part_groups: Vec::new(),
        texts: score.texts.clone(),
        style_overrides: score.style_overrides.clone(),
        object_style_overrides: score.object_style_overrides.clone(),
        chord_definitions: score.chord_definitions.clone(),
        spanners: score.spanners.clone(),
        views: Vec::new(),
    };

    let (fitted, shift) = octave_fit(&merged);
    merged = fitted;
    if shift != 0 {
        notes.push(format!(
            "アコーディオンの実用音域に合わせて{}オクターブ移調",
            shift / 12
        ));
    }

    super::score::respell_score_to_key(&mut merged);

    if analysis.ambiguous && right_hand_part_index.is_none() {
        notes.push("上位2パートの平均音高が僅差のため、右手パートの選択が曖昧です".to_string());
    }

    Ok(ArrangeResult {
        score: merged,
        notes,
    })
}

#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "kebab-case")]
pub enum AccordionLossPolicy {
    RequireAll,
    PermitReduction,
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct AccordionReductionOptions {
    pub right_hand_part_index: Option<usize>,
    /// Empty means all other pitched parts in source order.
    pub left_hand_part_indices: Vec<usize>,
    pub loss_policy: AccordionLossPolicy,
    pub octave_fit: bool,
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct AccordionReductionDiagnostic {
    pub code: String,
    pub message: String,
    pub source: Option<NoteAddr>,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub source_location: Option<String>,
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct AccordionSourceMapping {
    pub source: NoteAddr,
    /// None exactly when the selected loss policy permits this source note to be omitted.
    pub target: Option<NoteAddr>,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct AccordionReductionResult {
    /// None when a requested lossless reduction or source structure cannot be represented.
    pub score: Option<Score>,
    pub resolved_options: AccordionReductionOptions,
    pub diagnostics: Vec<AccordionReductionDiagnostic>,
    pub source_map: Vec<AccordionSourceMapping>,
    pub transposition_semitones: i8,
    pub contract_version: u16,
}

fn reduction_error(
    options: AccordionReductionOptions,
    code: &str,
    message: impl Into<String>,
) -> AccordionReductionResult {
    AccordionReductionResult {
        score: None,
        resolved_options: options,
        diagnostics: vec![AccordionReductionDiagnostic {
            code: code.to_string(),
            message: message.into(),
            source: None,
            source_location: None,
        }],
        source_map: Vec::new(),
        transposition_semitones: 0,
        contract_version: 1,
    }
}

fn same_reduction_bar(a: &Measure, b: &Measure) -> bool {
    a.number == b.number
        && a.actual_length == b.actual_length
        && a.time_sig == b.time_sig
        && a.barline_left == b.barline_left
        && a.barline_right == b.barline_right
        && a.volta == b.volta
        && a.section_break == b.section_break
        && a.measure_repeat == b.measure_repeat
}

fn reduction_measure_metadata(measure: &Measure) -> serde_json::Value {
    let mut metadata = measure.clone();
    metadata.voices = [vec![], vec![], vec![], vec![]];
    metadata.source_voice_numbers = [None; 4];
    serde_json::to_value(metadata).unwrap_or(serde_json::Value::Null)
}

fn source_voice_addresses(score: &Score, part: usize, staff: usize, voice: usize) -> Vec<NoteAddr> {
    score.parts[part].staves[staff]
        .measures
        .iter()
        .enumerate()
        .flat_map(|(measure, bar)| {
            bar.voices[voice]
                .iter()
                .enumerate()
                .filter(|(_, note)| !note.is_rest)
                .map(move |(note, _)| NoteAddr {
                    part,
                    staff,
                    measure,
                    voice,
                    note,
                })
        })
        .collect()
}

/// Reduce authored parts onto two accordion staves with explicit part selection and loss policy.
/// Selected voices are copied into separate score voices, so ties, tuplets and source note IDs
/// survive without onset bucketing or note shortening. Voice capacity and unselected notes are
/// either reported as source-located omissions or rejected before returning a score.
pub fn arrange_for_accordion_with_options(
    score: &Score,
    mut options: AccordionReductionOptions,
) -> AccordionReductionResult {
    let analysis = analyze_for_accordion(score);
    let Some(right) = options.right_hand_part_index.or_else(|| {
        analysis
            .candidates
            .first()
            .map(|candidate| candidate.part_index)
    }) else {
        return reduction_error(
            options,
            "no-pitched-part",
            "no pitched source part is available",
        );
    };
    if right >= score.parts.len() || is_percussion_part(&score.parts[right]) {
        return reduction_error(
            options,
            "invalid-selection",
            "right-hand part is invalid or percussion",
        );
    }
    options.right_hand_part_index = Some(right);
    if options.left_hand_part_indices.is_empty() {
        options.left_hand_part_indices = analysis
            .candidates
            .iter()
            .map(|candidate| candidate.part_index)
            .filter(|index| *index != right)
            .collect();
    }
    let left_indices = &options.left_hand_part_indices;
    if left_indices.is_empty()
        || left_indices.iter().any(|index| {
            *index >= score.parts.len()
                || *index == right
                || is_percussion_part(&score.parts[*index])
        })
        || {
            let mut seen = std::collections::BTreeSet::new();
            left_indices.iter().any(|index| !seen.insert(index))
        }
    {
        return reduction_error(
            options,
            "invalid-selection",
            "left-hand selection must contain distinct pitched parts other than the right hand",
        );
    }
    let right_staff = match score.parts[right].staves.first() {
        Some(staff) if !staff.measures.is_empty() => staff,
        _ => {
            return reduction_error(
                options,
                "unsupported-source",
                "right-hand part has no staff or measures",
            );
        }
    };
    for &part_index in left_indices {
        let Some(staff) = score.parts[part_index].staves.first() else {
            return reduction_error(
                options,
                "unsupported-source",
                format!("left-hand part {part_index} has no staff"),
            );
        };
        if staff.measures.len() != right_staff.measures.len()
            || staff
                .measures
                .iter()
                .zip(&right_staff.measures)
                .any(|(a, b)| !same_reduction_bar(a, b))
        {
            return reduction_error(
                options,
                "misaligned-bars",
                format!(
                    "part {part_index} has different bar numbering, lengths, repeats or endings"
                ),
            );
        }
    }
    let validation = super::validate::validate(score);
    if !validation.is_valid() {
        return reduction_error(
            options,
            "invalid-source",
            format!("source validation failed: {:?}", validation.errors[0]),
        );
    }
    if !score.spanners.is_empty() || !score.object_style_overrides.is_empty() {
        return reduction_error(
            options,
            "unsupported-notation",
            "source spanners or object style overrides require address remapping",
        );
    }
    let mut diagnostics = Vec::new();
    let primary_left = &score.parts[left_indices[0]].staves[0];
    for &part_index in left_indices.iter().skip(1) {
        for (measure_index, (primary, additional)) in primary_left
            .measures
            .iter()
            .zip(&score.parts[part_index].staves[0].measures)
            .enumerate()
        {
            if reduction_measure_metadata(primary) != reduction_measure_metadata(additional) {
                diagnostics.push(AccordionReductionDiagnostic {
                    code: "unsupported-notation".to_string(),
                    message: format!("part {part_index}, measure {measure_index} has distinct measure metadata that one left staff cannot retain"),
                    source: None,
                    source_location: Some(format!("/score/part/{}/staff/1/measure/{}", part_index + 1, measure_index + 1)),
                });
            }
        }
    }
    if !score.views.is_empty() || !score.part_groups.is_empty() {
        diagnostics.push(AccordionReductionDiagnostic {
            code: "unsupported-score-view".to_string(),
            message: "source views or part groups cannot be carried into one accordion part"
                .to_string(),
            source: None,
            source_location: Some("/score/views".to_string()),
        });
    }
    let mut source_map = Vec::new();
    let mut included = std::collections::BTreeSet::new();
    let right_copy = right_staff.clone();
    for voice in 0..4 {
        for address in source_voice_addresses(score, right, 0, voice) {
            source_map.push(AccordionSourceMapping {
                target: Some(NoteAddr {
                    part: 0,
                    staff: 0,
                    ..address.clone()
                }),
                source: address,
            });
        }
    }
    included.insert((right, 0));
    let mut left_copy = score.parts[left_indices[0]].staves[0].clone();
    left_copy.clef = Clef::Bass;
    for measure in &mut left_copy.measures {
        measure.voices = [vec![], vec![], vec![], vec![]];
        measure.source_voice_numbers = [None; 4];
    }
    let mut next_voice = 0;
    for &part_index in left_indices {
        let staff = &score.parts[part_index].staves[0];
        included.insert((part_index, 0));
        for voice in 0..4 {
            if staff
                .measures
                .iter()
                .all(|measure| measure.voices[voice].is_empty())
            {
                continue;
            }
            if next_voice < 4 {
                for (measure_index, measure) in staff.measures.iter().enumerate() {
                    left_copy.measures[measure_index].voices[next_voice] =
                        measure.voices[voice].clone();
                    for (note_index, note) in measure.voices[voice].iter().enumerate() {
                        if note.is_rest {
                            continue;
                        }
                        let address = NoteAddr {
                            part: part_index,
                            staff: 0,
                            measure: measure_index,
                            voice,
                            note: note_index,
                        };
                        source_map.push(AccordionSourceMapping {
                            target: Some(NoteAddr {
                                part: 0,
                                staff: 1,
                                measure: measure_index,
                                voice: next_voice,
                                note: note_index,
                            }),
                            source: address,
                        });
                    }
                }
                next_voice += 1;
            } else {
                for address in source_voice_addresses(score, part_index, 0, voice) {
                    diagnostics.push(AccordionReductionDiagnostic {
                        code: "voice-capacity".to_string(),
                        message: "left hand has more than four source voices".to_string(),
                        source: Some(address.clone()),
                        source_location: None,
                    });
                    source_map.push(AccordionSourceMapping {
                        source: address,
                        target: None,
                    });
                }
            }
        }
    }
    for (part_index, part) in score.parts.iter().enumerate() {
        for staff_index in 0..part.staves.len() {
            if included.contains(&(part_index, staff_index)) {
                continue;
            }
            for voice in 0..4 {
                for address in source_voice_addresses(score, part_index, staff_index, voice) {
                    diagnostics.push(AccordionReductionDiagnostic {
                        code: "unselected-source".to_string(),
                        message: "source note is outside the selected hands".to_string(),
                        source: Some(address.clone()),
                        source_location: None,
                    });
                    source_map.push(AccordionSourceMapping {
                        source: address,
                        target: None,
                    });
                }
            }
        }
    }
    source_map.sort_by_key(|mapping| {
        (
            mapping.source.part,
            mapping.source.staff,
            mapping.source.measure,
            mapping.source.voice,
            mapping.source.note,
        )
    });
    if options.loss_policy == AccordionLossPolicy::RequireAll && !diagnostics.is_empty() {
        return AccordionReductionResult {
            score: None,
            resolved_options: options,
            diagnostics,
            source_map,
            transposition_semitones: 0,
            contract_version: 1,
        };
    }
    let mut output = score.clone();
    output.id = uuid::Uuid::new_v4().to_string();
    let mut accordion = Part::new("Accordion", "Acc.");
    accordion.midi_program = ACCORDION_PROGRAM;
    accordion.staves = vec![right_copy, left_copy];
    output.parts = vec![accordion];
    output.part_groups.clear();
    output.views.clear();
    let (output, shift) = if options.octave_fit {
        octave_fit(&output)
    } else {
        (output, 0)
    };
    if shift != 0 {
        diagnostics.push(AccordionReductionDiagnostic {
            code: "octave-shift".to_string(),
            message: format!("both hands transposed by {shift} semitones"),
            source: None,
            source_location: None,
        });
    }
    let output_validation = super::validate::validate(&output);
    if !output_validation.is_valid() {
        return reduction_error(
            options,
            "invalid-output",
            format!(
                "reduced score validation failed: {:?}",
                output_validation.errors[0]
            ),
        );
    }
    AccordionReductionResult {
        score: Some(output),
        resolved_options: options,
        diagnostics,
        source_map,
        transposition_semitones: shift,
        contract_version: 1,
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::model::{pitch::Step, score::Score, validate::validate};

    fn note(step: Step, octave: i8, duration: Duration) -> Note {
        Note::new(Pitch::new(step, octave), duration)
    }

    fn guided_fixture() -> Score {
        let mut score = Score::new("melody", 120, 4, 4, 0, 2);
        let first = &mut score.parts[0].staves[0].measures[0];
        first.number = 0;
        first.actual_length = super::super::score::MeasureLength::from_beats(1.0);
        first.voices[0] = vec![note(Step::C, 5, Duration::Quarter)];
        let second = &mut score.parts[0].staves[0].measures[1];
        second.voices[0] = vec![note(Step::D, 5, Duration::Whole)];
        second.barline_right = Barline::RepeatEnd;
        score
    }

    fn guided_options(pattern: AccordionPattern) -> GuidedAccordionOptions {
        let chord = |root: &str| {
            serde_json::from_value(serde_json::json!({"root": root, "kind": "major", "bass": null}))
                .unwrap()
        };
        GuidedAccordionOptions {
            pattern,
            chords: vec![
                PositionedAccordionChord {
                    measure_index: 0,
                    offset_beats: 0.0,
                    chord: chord("C"),
                },
                PositionedAccordionChord {
                    measure_index: 1,
                    offset_beats: 0.0,
                    chord: chord("G"),
                },
            ],
        }
    }

    #[test]
    fn guided_patterns_preserve_source_and_pickup_repeat_timing() {
        let score = guided_fixture();
        let before = serde_json::to_value(&score).unwrap();
        let original_events: Vec<_> = crate::to_playback_events(&score, &Default::default())
            .into_iter()
            .map(|event| (event.pitch_midi, event.time_beats, event.duration_beats))
            .collect();
        let mut bass_patterns = Vec::new();
        for pattern in [
            AccordionPattern::HeldBassChord,
            AccordionPattern::AlternatingBassChord,
            AccordionPattern::Arpeggio,
        ] {
            let result = arrange_with_explicit_chords(&score, guided_options(pattern));
            assert!(result.diagnostics.is_empty(), "{:?}", result.diagnostics);
            let arranged = result.score.expect("valid arrangement");
            assert_eq!(arranged.parts.len(), 1);
            assert_eq!(arranged.parts[0].staves.len(), 2);
            assert_eq!(arranged.parts[0].staves[0].measures[0].number, 0);
            assert_eq!(
                arranged.parts[0].staves[1].measures[0].actual_length,
                score.parts[0].staves[0].measures[0].actual_length
            );
            assert_eq!(
                arranged.parts[0].staves[1].measures[1].barline_right,
                Barline::RepeatEnd
            );
            assert_eq!(
                arranged.parts[0].staves[0].measures[0].voices[0][0].id,
                score.parts[0].staves[0].measures[0].voices[0][0].id
            );
            let melody_events: Vec<_> = crate::to_playback_events(&arranged, &Default::default())
                .into_iter()
                .filter(|event| {
                    event
                        .source
                        .as_ref()
                        .is_some_and(|address| address.staff == 0)
                })
                .map(|event| (event.pitch_midi, event.time_beats, event.duration_beats))
                .collect();
            assert_eq!(melody_events, original_events);
            bass_patterns.push(
                arranged.parts[0].staves[1].measures[1].voices[0]
                    .iter()
                    .map(|note| {
                        (
                            note.pitches.iter().map(Pitch::to_midi).collect::<Vec<_>>(),
                            note.beats(),
                        )
                    })
                    .collect::<Vec<_>>(),
            );
        }
        assert_ne!(bass_patterns[0], bass_patterns[1]);
        assert_ne!(bass_patterns[1], bass_patterns[2]);
        assert_eq!(serde_json::to_value(&score).unwrap(), before);
    }

    #[test]
    fn repeated_harmony_does_not_restart_pulse_arpeggio_or_walking_patterns() {
        let mut score = Score::new("same harmony", 60, 4, 4, 0, 1);
        score.parts[0].staves[0].measures[0].voices[0] = vec![note(Step::C, 5, Duration::Whole)];
        for pattern in [
            AccordionPattern::AlternatingBassChord,
            AccordionPattern::Arpeggio,
            AccordionPattern::RisingArpeggio,
            AccordionPattern::AlbertiBass,
            AccordionPattern::WalkingBass,
        ] {
            let mut single = guided_options(pattern);
            single.chords.truncate(1);
            let mut repeated = single.clone();
            let mut redundant = single.chords[0].clone();
            redundant.offset_beats = 2.0;
            repeated.chords.push(redundant);
            let baseline = arrange_with_explicit_chords(&score, single);
            let actual = arrange_with_explicit_chords(&score, repeated.clone());
            assert!(
                baseline.diagnostics.is_empty(),
                "{pattern:?}: {:?}",
                baseline.diagnostics
            );
            assert!(
                actual.diagnostics.is_empty(),
                "{pattern:?}: {:?}",
                actual.diagnostics
            );
            assert_eq!(actual.resolved_options, repeated);
            let left = |score: &Score| {
                score.parts[0].staves[1].measures[0].voices[0]
                    .iter()
                    .map(|note| {
                        (
                            note.pitches.iter().map(Pitch::to_midi).collect::<Vec<_>>(),
                            note.beats(),
                        )
                    })
                    .collect::<Vec<_>>()
            };
            assert_eq!(
                left(baseline.score.as_ref().unwrap()),
                left(actual.score.as_ref().unwrap()),
                "{pattern:?}"
            );
        }
    }

    #[test]
    fn changed_harmony_keeps_measure_phase_across_pickup_and_three_four() {
        let mut score = Score::new("pickup phase", 60, 3, 4, 0, 2);
        score.parts[0].staves[0].measures[0].actual_length =
            super::super::score::MeasureLength::from_beats(1.0);
        score.parts[0].staves[0].measures[0].voices[0] = vec![note(Step::C, 5, Duration::Quarter)];
        let mut dotted = note(Step::G, 5, Duration::Half);
        dotted.dot_count = 1;
        score.parts[0].staves[0].measures[1].voices[0] = vec![dotted];
        let mut options = guided_options(AccordionPattern::AlternatingBassChord);
        options.chords[1].offset_beats = 1.0;
        let result = arrange_with_explicit_chords(&score, options);
        assert!(result.diagnostics.is_empty(), "{:?}", result.diagnostics);
        let arranged = result.score.unwrap();
        let midi = |bar: usize| {
            arranged.parts[0].staves[1].measures[bar].voices[0]
                .iter()
                .map(|note| note.pitches.iter().map(Pitch::to_midi).collect::<Vec<_>>())
                .collect::<Vec<_>>()
        };
        assert_eq!(midi(0), vec![vec![36]]);
        // The inherited C starts the new bar. G begins on its second beat,
        // where the measure-anchored pattern calls for the chord, then D bass.
        assert_eq!(midi(1), vec![vec![36], vec![67, 71, 74], vec![50]]);
        assert_eq!(
            arranged.parts[0].staves[0].measures[0].actual_length,
            score.parts[0].staves[0].measures[0].actual_length
        );
    }

    #[test]
    fn four_four_arpeggios_keep_phase_after_weak_beat_chord_change() {
        let mut score = Score::new("arpeggio change", 60, 4, 4, 0, 1);
        score.parts[0].staves[0].measures[0].voices[0] = vec![note(Step::C, 5, Duration::Whole)];
        for (pattern, expected) in [
            (AccordionPattern::Arpeggio, 74),
            (AccordionPattern::RisingArpeggio, 74),
            (AccordionPattern::AlbertiBass, 71),
        ] {
            let mut options = guided_options(pattern);
            options.chords[1].measure_index = 0;
            options.chords[1].offset_beats = 1.0;
            let result = arrange_with_explicit_chords(&score, options);
            assert!(
                result.diagnostics.is_empty(),
                "{pattern:?}: {:?}",
                result.diagnostics
            );
            let arranged = result.score.unwrap();
            let left = &arranged.parts[0].staves[1].measures[0].voices[0];
            assert_eq!(left.len(), 8);
            assert_eq!(left[2].pitches[0].to_midi(), expected, "{pattern:?}");
        }
    }

    #[test]
    fn layered_and_preset_paths_use_the_same_continuous_pattern() {
        let mut score = Score::new("layered pulse", 60, 4, 4, 0, 1);
        score.parts[0].staves[0].measures[0].voices[0] = vec![note(Step::C, 5, Duration::Whole)];
        let mut guided = guided_options(AccordionPattern::AlternatingBassChord);
        guided.chords.truncate(1);
        let mut repeated = guided.chords[0].clone();
        repeated.offset_beats = 2.0;
        guided.chords.push(repeated);
        let direct = arrange_with_explicit_chords(&score, guided.clone());
        let layered = arrange_with_layers(
            &score,
            LayeredAccordionOptions {
                chords: guided.chords.clone(),
                alternative_chords: None,
                inversion: None,
                accompaniment: Some(AccordionPattern::AlternatingBassChord),
                melodic_voice: AccordionMelodicVoice::Unchanged,
                swing: AccordionSwingPolicy::Straight,
                bass_mode: AccordionBassMode::FreeBass,
            },
        );
        let preset = crate::arrange_accordion_preset(
            &score,
            crate::AccordionPresetSelection {
                chords: guided.chords,
                alternative_chords: None,
                inversion: None,
                bass_mode: AccordionBassMode::FreeBass,
            },
            "alternating-bass",
        );
        for arranged in [
            direct.score.unwrap(),
            layered.score.unwrap(),
            preset.score.unwrap(),
        ] {
            let left = &arranged.parts[0].staves[1].measures[0].voices[0];
            assert_eq!(left[2].pitches[0].to_midi(), 43);
            assert_eq!(left.len(), 4);
        }
    }

    #[test]
    fn guided_arrangement_rejects_uncovered_unsupported_and_out_of_range_input() {
        let score = guided_fixture();
        let mut options = guided_options(AccordionPattern::Arpeggio);
        options.chords.remove(0);
        let result = arrange_with_explicit_chords(&score, options);
        assert!(result.score.is_none());
        assert_eq!(result.diagnostics[0].code, "missing-chords");

        let mut options = guided_options(AccordionPattern::AlternatingBassChord);
        options.chords[1].offset_beats = 0.5;
        let result = arrange_with_explicit_chords(&score, options);
        assert!(result.score.is_none());
        assert_eq!(result.diagnostics[0].code, "unsupported-rhythm");

        let mut high = guided_fixture();
        high.parts[0].staves[0].measures[0].voices[0][0].pitches[0] = Pitch::new(Step::C, 9);
        let result =
            arrange_with_explicit_chords(&high, guided_options(AccordionPattern::HeldBassChord));
        assert!(result.score.is_none());
        assert_eq!(result.diagnostics[0].code, "out-of-range");
    }

    #[test]
    fn guided_arrangement_carries_explicit_harmony_into_internal_short_bar() {
        let mut score = Score::new("short cadence", 120, 4, 4, 0, 2);
        score.parts[0].staves[0].measures[0].voices[0] = vec![note(Step::C, 5, Duration::Whole)];
        let cadence = &mut score.parts[0].staves[0].measures[1];
        cadence.actual_length = super::super::score::MeasureLength::from_beats(3.0);
        let mut dotted = note(Step::D, 5, Duration::Half);
        dotted.dot_count = 1;
        cadence.voices[0] = vec![dotted];
        let cadence_length = cadence.actual_length;
        let mut options = guided_options(AccordionPattern::AlternatingBassChord);
        options.chords.pop();
        let first = arrange_with_explicit_chords(&score, options.clone());
        let second = arrange_with_explicit_chords(&score, options);
        assert!(first.diagnostics.is_empty(), "{:?}", first.diagnostics);
        let arranged = first.score.unwrap();
        assert_eq!(
            arranged.parts[0].staves[1].measures[1].actual_length,
            cadence_length
        );
        let left = |score: &Score| {
            crate::to_playback_events(score, &Default::default())
                .into_iter()
                .filter(|event| {
                    event
                        .source
                        .as_ref()
                        .is_some_and(|address| address.staff == 1)
                })
                .map(|event| (event.pitch_midi, event.time_beats, event.duration_beats))
                .collect::<Vec<_>>()
        };
        assert_eq!(left(&arranged), left(second.score.as_ref().unwrap()));
        let mut starts = left(&arranged)
            .iter()
            .map(|event| event.1)
            .collect::<Vec<_>>();
        starts.dedup();
        assert_eq!(starts, [0.0, 1.0, 2.0, 3.0, 4.0, 5.0, 6.0]);
    }

    fn layered_options(pattern: AccordionPattern) -> LayeredAccordionOptions {
        let guided = guided_options(pattern);
        LayeredAccordionOptions {
            chords: guided.chords,
            alternative_chords: None,
            inversion: None,
            accompaniment: Some(pattern),
            melodic_voice: AccordionMelodicVoice::Unchanged,
            swing: AccordionSwingPolicy::Straight,
            bass_mode: AccordionBassMode::FreeBass,
        }
    }

    #[test]
    fn extended_accompaniments_are_distinct_and_walking_approaches_next_bass() {
        let score = guided_fixture();
        let mut signatures = std::collections::BTreeSet::new();
        for pattern in [
            AccordionPattern::HeldBassChord,
            AccordionPattern::BassChordPulse,
            AccordionPattern::AlternatingBassChord,
            AccordionPattern::Arpeggio,
            AccordionPattern::RisingArpeggio,
            AccordionPattern::AlbertiBass,
            AccordionPattern::OffbeatChords,
            AccordionPattern::WalkingBass,
        ] {
            let result = arrange_with_layers(&score, layered_options(pattern));
            assert!(
                result.diagnostics.is_empty(),
                "{pattern:?}: {:?}",
                result.diagnostics
            );
            let arranged = result.score.unwrap();
            let signature = arranged.parts[0].staves[1].measures[1].voices[0]
                .iter()
                .map(|note| {
                    (
                        note.is_rest,
                        note.pitches.iter().map(Pitch::to_midi).collect::<Vec<_>>(),
                        note.beats().to_bits(),
                    )
                })
                .collect::<Vec<_>>();
            assert!(
                signatures.insert(format!("{signature:?}")),
                "duplicate pattern: {pattern:?}"
            );
            if pattern == AccordionPattern::WalkingBass {
                let pickup = &arranged.parts[0].staves[1].measures[0].voices[0][0];
                assert_eq!(
                    pickup.pitches[0].to_midi(),
                    42,
                    "approaches G bass (43) from below"
                );
            }
        }
    }

    #[test]
    fn explicit_seventh_harmony_and_inversion_change_only_generated_notes() {
        let score = guided_fixture();
        let original = serde_json::to_value(&score).unwrap();
        let mut options = layered_options(AccordionPattern::HeldBassChord);
        options.chords[0].chord.kind = "major-seventh".to_string();
        options.chords[1].chord.kind = "dominant".to_string();
        let result = arrange_with_layers(&score, options.clone());
        assert!(result.diagnostics.is_empty(), "{:?}", result.diagnostics);
        let arranged = result.score.unwrap();
        assert_eq!(
            serde_json::to_value(&arranged.parts[0].staves[0]).unwrap(),
            serde_json::to_value(&score.parts[0].staves[0]).unwrap(),
            "the authored melody staff must be identical in the JSON contract"
        );
        assert!(
            arranged.parts[0].staves[1].measures[0].voices[0][0]
                .pitches
                .iter()
                .any(|pitch| pitch.to_midi() == 71)
        );
        assert!(
            arranged.parts[0].staves[1].measures[1].voices[0][0]
                .pitches
                .iter()
                .any(|pitch| pitch.to_midi() == 77)
        );
        assert_eq!(
            arranged.parts[0].staves[0].measures[0].voices[0][0].id,
            score.parts[0].staves[0].measures[0].voices[0][0].id
        );

        options.alternative_chords = Some(guided_options(AccordionPattern::HeldBassChord).chords);
        options.alternative_chords.as_mut().unwrap()[0].chord.root = "F".to_string();
        options.inversion = Some(AccordionInversion::First);
        let alternative = arrange_with_layers(&score, options);
        assert!(
            alternative.diagnostics.is_empty(),
            "{:?}",
            alternative.diagnostics
        );
        assert_eq!(
            alternative.resolved_chords[0].chord.bass.as_deref(),
            Some("A")
        );
        assert_eq!(
            alternative.score.unwrap().parts[0].staves[1].measures[0].voices[0][0].pitches[0]
                .to_midi(),
            45
        );
        assert_eq!(serde_json::to_value(&score).unwrap(), original);
    }

    #[test]
    fn supplied_jazz_sevenths_have_exact_pitch_classes_and_symbols() {
        let mut score = Score::new("sevenths", 120, 4, 4, 0, 1);
        score.parts[0].staves[0].measures[0].voices[0] = vec![note(Step::C, 5, Duration::Whole)];
        for (root, kind, expected) in [
            ("C", "major-seventh", [0, 4, 7, 11]),
            ("F", "major-seventh", [5, 9, 0, 4]),
            ("A", "minor-seventh", [9, 0, 4, 7]),
            ("D", "minor-seventh", [2, 5, 9, 0]),
            ("E", "minor-seventh", [4, 7, 11, 2]),
            ("G", "dominant", [7, 11, 2, 5]),
            ("E", "dominant", [4, 8, 11, 2]),
        ] {
            let mut options = layered_options(AccordionPattern::HeldBassChord);
            options.chords.truncate(1);
            options.chords[0].chord.root = root.to_string();
            options.chords[0].chord.kind = kind.to_string();
            let result = arrange_with_layers(&score, options);
            assert!(
                result.diagnostics.is_empty(),
                "{root} {kind}: {:?}",
                result.diagnostics
            );
            let arranged = result.score.unwrap();
            let chord = &arranged.parts[0].staves[1].measures[0].voices[0][0];
            assert_eq!(
                chord
                    .pitches
                    .iter()
                    .skip(1)
                    .map(|pitch| pitch.to_midi() % 12)
                    .collect::<Vec<_>>(),
                expected.map(|value| value as i16)
            );
            assert_eq!(
                chord
                    .chord_symbol
                    .as_ref()
                    .map(|symbol| (symbol.root.as_str(), symbol.kind.as_str())),
                Some((root, kind))
            );
        }
    }

    #[test]
    fn notated_swing_is_exact_two_to_one_without_measure_drift() {
        let score = guided_fixture();
        let mut options = layered_options(AccordionPattern::BassChordPulse);
        options.swing = AccordionSwingPolicy::NotatedAccompanimentTwoToOne;
        let result = arrange_with_layers(&score, options);
        assert!(result.notated_swing);
        assert!(result.diagnostics.is_empty(), "{:?}", result.diagnostics);
        let arranged = result.score.unwrap();
        let notes = &arranged.parts[0].staves[1].measures[1].voices[0];
        assert_eq!(notes.len(), 8);
        assert_eq!(
            crate::tuplet_group_ranges(notes),
            [(0, 1), (2, 3), (4, 5), (6, 7)]
        );
        for pair in notes.chunks(2) {
            assert_eq!(pair.len(), 2);
            assert!((pair[0].beats() / pair[1].beats() - 2.0).abs() < 1e-9);
            assert_eq!(
                pair[0]
                    .tuplet
                    .as_ref()
                    .map(|tuplet| (tuplet.actual_notes, tuplet.normal_notes)),
                Some((3, 2))
            );
        }
        assert!((notes.iter().map(Note::beats).sum::<f64>() - 4.0).abs() < 1e-9);
        let starts = crate::to_playback_events(&arranged, &Default::default())
            .into_iter()
            .filter(|event| {
                event
                    .source
                    .as_ref()
                    .is_some_and(|address| address.staff == 1 && address.measure == 1)
            })
            .map(|event| event.time_beats)
            .collect::<Vec<_>>();
        assert!(
            starts
                .iter()
                .any(|time| (*time - (1.0 + 2.0 / 3.0)).abs() < 1e-9)
        );
    }

    #[test]
    fn melodic_voice_layer_is_independent_of_accompaniment() {
        let mut score = Score::new("layers", 120, 4, 4, 0, 1);
        score.parts[0].staves[0].measures[0].voices[0] = vec![note(Step::G, 4, Duration::Whole)];
        let mut options = layered_options(AccordionPattern::BassChordPulse);
        options.chords.truncate(1);
        options.melodic_voice = AccordionMelodicVoice::ThirdBelow;
        let with_voice = arrange_with_layers(&score, options.clone());
        assert!(
            with_voice.diagnostics.is_empty(),
            "{:?}",
            with_voice.diagnostics
        );
        let arranged = with_voice.score.unwrap();
        assert_eq!(
            arranged.parts[0].staves[0].measures[0].voices[1][0].pitches[0].to_midi(),
            64
        );
        options.melodic_voice = AccordionMelodicVoice::Unchanged;
        let without_voice = arrange_with_layers(&score, options.clone()).score.unwrap();
        let bass = |score: &Score| {
            score.parts[0].staves[1].measures[0].voices[0]
                .iter()
                .map(|note| {
                    (
                        note.pitches.iter().map(Pitch::to_midi).collect::<Vec<_>>(),
                        note.beats(),
                    )
                })
                .collect::<Vec<_>>()
        };
        assert_eq!(bass(&arranged), bass(&without_voice));
        options.accompaniment = None;
        options.melodic_voice = AccordionMelodicVoice::ThirdBelow;
        let without_bass = arrange_with_layers(&score, options);
        assert!(
            without_bass.score.as_ref().unwrap().parts[0].staves[1].measures[0].voices[0]
                .is_empty()
        );
        assert_eq!(
            without_bass.score.unwrap().parts[0].staves[0].measures[0].voices[1][0].pitches[0]
                .to_midi(),
            64
        );

        score.parts[0].staves[0].measures[0].voices[0][0].pitches[0] = Pitch::new(Step::C, 5);
        for (voice, expected) in [
            (AccordionMelodicVoice::SixthBelow, 64),
            (AccordionMelodicVoice::OctaveBelow, 60),
        ] {
            let mut options = layered_options(AccordionPattern::HeldBassChord);
            options.chords.truncate(1);
            options.melodic_voice = voice;
            let result = arrange_with_layers(&score, options);
            assert!(result.diagnostics.is_empty(), "{:?}", result.diagnostics);
            assert_eq!(
                result.score.unwrap().parts[0].staves[0].measures[0].voices[1][0].pitches[0]
                    .to_midi(),
                expected
            );
        }
    }

    #[test]
    fn standard_bass_rejects_seventh_realization_without_partial_score() {
        let score = guided_fixture();
        let mut options = layered_options(AccordionPattern::HeldBassChord);
        options.bass_mode = AccordionBassMode::StandardBass;
        options.chords[0].chord.kind = "major-seventh".to_string();
        let result = arrange_with_layers(&score, options);
        assert!(result.score.is_none());
        assert_eq!(result.diagnostics[0].code, "unsupported-standard-bass");
    }

    #[test]
    fn walking_bass_keeps_pickups_rests_and_supported_meters_aligned() {
        for (numerator, duration, dots) in [
            (2, Duration::Quarter, 0),
            (3, Duration::Half, 0),
            (4, Duration::Half, 1),
        ] {
            let mut score = Score::new("meter", 120, numerator, 4, 0, 2);
            score.parts[0].staves[0].measures[0].actual_length =
                super::super::score::MeasureLength::from_beats(1.0);
            score.parts[0].staves[0].measures[0].voices[0] =
                vec![note(Step::C, 5, Duration::Quarter)];
            let mut final_note = note(Step::G, 4, duration);
            final_note.dot_count = dots;
            score.parts[0].staves[0].measures[1].voices[0] =
                vec![Note::rest(Duration::Quarter), final_note];
            let result =
                arrange_with_layers(&score, layered_options(AccordionPattern::WalkingBass));
            assert!(
                result.diagnostics.is_empty(),
                "{numerator}/4: {:?}",
                result.diagnostics
            );
            let arranged = result.score.unwrap();
            assert!(
                (arranged.parts[0].staves[1].measures[0].voices[0]
                    .iter()
                    .map(Note::beats)
                    .sum::<f64>()
                    - 1.0)
                    .abs()
                    < 1e-9
            );
            assert!(
                (arranged.parts[0].staves[1].measures[1].voices[0]
                    .iter()
                    .map(Note::beats)
                    .sum::<f64>()
                    - f64::from(numerator))
                .abs()
                    < 1e-9
            );
            let melody = crate::to_playback_events(&arranged, &Default::default())
                .into_iter()
                .filter(|event| {
                    event
                        .source
                        .as_ref()
                        .is_some_and(|address| address.staff == 0)
                })
                .map(|event| event.time_beats)
                .collect::<Vec<_>>();
            assert_eq!(melody, [0.0, 2.0]);
        }
    }

    #[test]
    fn layered_score_preserves_accidental_pickup_meter_change_and_short_final_bar() {
        let mut score = Score::new("mixed bars", 120, 4, 4, 0, 3);
        let first = &mut score.parts[0].staves[0].measures[0];
        first.number = 0;
        first.actual_length = super::super::score::MeasureLength::from_beats(1.0);
        first.voices[0] = vec![Note::new(
            Pitch::with_alter(Step::C, 5, 1),
            Duration::Quarter,
        )];
        let middle = &mut score.parts[0].staves[0].measures[1];
        middle.time_sig = Some(super::super::notation::TimeSignature {
            numerator: 3,
            denominator: 4,
        });
        middle.voices[0] = vec![
            Note::rest(Duration::Quarter),
            note(Step::G, 4, Duration::Half),
        ];
        let last = &mut score.parts[0].staves[0].measures[2];
        last.time_sig = Some(super::super::notation::TimeSignature {
            numerator: 4,
            denominator: 4,
        });
        last.actual_length = super::super::score::MeasureLength::from_beats(2.0);
        last.voices[0] = vec![note(Step::F, 4, Duration::Half)];

        let input_json = serde_json::to_value(&score).unwrap();
        let input_staff = serde_json::to_value(&score.parts[0].staves[0]).unwrap();
        let mut options = layered_options(AccordionPattern::WalkingBass);
        options.chords.push(PositionedAccordionChord {
            measure_index: 2,
            offset_beats: 0.0,
            chord: serde_json::from_value(
                serde_json::json!({"root":"F","kind":"major","bass":null}),
            )
            .unwrap(),
        });
        let result = arrange_with_layers(&score, options);
        assert!(result.diagnostics.is_empty(), "{:?}", result.diagnostics);
        let arranged = result.score.unwrap();
        assert_eq!(
            serde_json::to_value(&arranged.parts[0].staves[0]).unwrap(),
            input_staff
        );
        assert_eq!(serde_json::to_value(&score).unwrap(), input_json);
        let left = &arranged.parts[0].staves[1].measures;
        assert_eq!(
            left.iter()
                .map(|bar| bar.voices[0].iter().map(Note::beats).sum::<f64>())
                .collect::<Vec<_>>(),
            [1.0, 3.0, 2.0]
        );
        assert_eq!(
            left[1].time_sig.as_ref().map(|meter| meter.numerator),
            Some(3)
        );
        assert_eq!(
            left[2].actual_length,
            super::super::score::MeasureLength::from_beats(2.0)
        );
        let melody_events = |source: &Score| {
            crate::to_playback_events(source, &Default::default())
                .into_iter()
                .filter(|event| {
                    event
                        .source
                        .as_ref()
                        .is_some_and(|address| address.staff == 0)
                })
                .map(|event| (event.pitch_midi, event.time_beats, event.duration_beats))
                .collect::<Vec<_>>()
        };
        assert_eq!(melody_events(&arranged), melody_events(&score));
    }

    fn reduction_fixture() -> Score {
        let mut score = guided_fixture();
        let mut bass = score.parts[0].clone();
        bass.name = "bass".to_string();
        bass.staves[0].clef = Clef::Bass;
        bass.staves[0].measures[0].voices[0] = vec![note(Step::C, 3, Duration::Quarter)];
        bass.staves[0].measures[1].voices[0] = vec![note(Step::G, 3, Duration::Whole)];
        let mut middle = bass.clone();
        middle.name = "countermelody".to_string();
        middle.staves[0].measures[0].voices[0] = vec![note(Step::E, 4, Duration::Quarter)];
        middle.staves[0].measures[1].voices[0] = vec![note(Step::F, 4, Duration::Whole)];
        score.parts.push(bass);
        score.parts.push(middle);
        score
    }

    fn reduction_options(policy: AccordionLossPolicy) -> AccordionReductionOptions {
        AccordionReductionOptions {
            right_hand_part_index: Some(0),
            left_hand_part_indices: vec![1, 2],
            loss_policy: policy,
            octave_fit: false,
        }
    }

    #[test]
    fn explicit_reduction_preserves_three_parts_and_source_timing() {
        let score = reduction_fixture();
        let before = serde_json::to_value(&score).unwrap();
        let result = arrange_for_accordion_with_options(
            &score,
            reduction_options(AccordionLossPolicy::RequireAll),
        );
        assert!(result.diagnostics.is_empty(), "{:?}", result.diagnostics);
        let reduced = result.score.unwrap();
        assert_eq!(reduced.parts[0].staves.len(), 2);
        assert_eq!(
            reduced.parts[0].staves[1].measures[0].actual_length,
            score.parts[1].staves[0].measures[0].actual_length
        );
        assert_eq!(
            reduced.parts[0].staves[1].measures[1].barline_right,
            Barline::RepeatEnd
        );
        assert_eq!(result.source_map.len(), 6);
        assert!(
            result
                .source_map
                .iter()
                .all(|mapping| mapping.target.is_some())
        );
        for mapping in &result.source_map {
            let source_note = &score.parts[mapping.source.part].staves[mapping.source.staff]
                .measures[mapping.source.measure]
                .voices[mapping.source.voice][mapping.source.note];
            let target = mapping.target.as_ref().unwrap();
            let copied = &reduced.parts[target.part].staves[target.staff].measures[target.measure]
                .voices[target.voice][target.note];
            assert_eq!(copied.id, source_note.id);
            assert_eq!(copied.beats(), source_note.beats());
            assert_eq!(copied.pitches, source_note.pitches);
        }
        let original_melody = crate::to_playback_events(&score, &Default::default())
            .into_iter()
            .filter(|event| event.source.as_ref().is_some_and(|addr| addr.part == 0))
            .map(|event| (event.pitch_midi, event.time_beats))
            .collect::<Vec<_>>();
        let reduced_melody = crate::to_playback_events(&reduced, &Default::default())
            .into_iter()
            .filter(|event| event.source.as_ref().is_some_and(|addr| addr.staff == 0))
            .map(|event| (event.pitch_midi, event.time_beats))
            .collect::<Vec<_>>();
        assert_eq!(original_melody, reduced_melody);
        assert_eq!(serde_json::to_value(&score).unwrap(), before);
    }

    #[test]
    fn lossless_reduction_rejects_voice_overflow_and_permitted_reduction_reports_each_note() {
        let mut score = reduction_fixture();
        for offset in 0..3 {
            let mut extra = score.parts[1].clone();
            extra.name = format!("extra {offset}");
            extra.staves[0].measures[0].voices[0][0].pitches[0] = Pitch::new(Step::D, 3 + offset);
            score.parts.push(extra);
        }
        let mut options = reduction_options(AccordionLossPolicy::RequireAll);
        options.left_hand_part_indices = vec![1, 2, 3, 4, 5];
        let rejected = arrange_for_accordion_with_options(&score, options.clone());
        assert!(rejected.score.is_none());
        assert_eq!(rejected.diagnostics.len(), 2);
        assert!(rejected.diagnostics.iter().all(|diagnostic| {
            diagnostic.code == "voice-capacity"
                && diagnostic
                    .source
                    .as_ref()
                    .is_some_and(|source| source.part == 5)
        }));
        options.loss_policy = AccordionLossPolicy::PermitReduction;
        let reduced = arrange_for_accordion_with_options(&score, options);
        assert!(reduced.score.is_some());
        assert_eq!(
            reduced
                .source_map
                .iter()
                .filter(|mapping| mapping.target.is_none())
                .count(),
            2
        );
        assert!(
            reduced
                .source_map
                .iter()
                .filter(|mapping| mapping.target.is_none())
                .all(|mapping| mapping.source.part == 5)
        );
    }

    #[test]
    fn reduction_reports_unselected_notes_and_optional_octave_shift() {
        let mut score = reduction_fixture();
        let mut options = reduction_options(AccordionLossPolicy::RequireAll);
        options.left_hand_part_indices = vec![1];
        let rejected = arrange_for_accordion_with_options(&score, options.clone());
        assert!(rejected.score.is_none());
        assert_eq!(rejected.diagnostics.len(), 2);
        assert!(rejected.diagnostics.iter().all(|diagnostic| {
            diagnostic.code == "unselected-source"
                && diagnostic
                    .source
                    .as_ref()
                    .is_some_and(|source| source.part == 2)
        }));

        options.loss_policy = AccordionLossPolicy::PermitReduction;
        let reduced = arrange_for_accordion_with_options(&score, options.clone());
        assert!(reduced.score.is_some());
        assert_eq!(
            reduced
                .source_map
                .iter()
                .filter(|mapping| mapping.target.is_none())
                .count(),
            2
        );

        for part in &mut score.parts {
            for measure in &mut part.staves[0].measures {
                for note in &mut measure.voices[0] {
                    if !note.is_rest {
                        note.pitches[0] = Pitch::new(Step::C, 7);
                    }
                }
            }
        }
        options.left_hand_part_indices = vec![1, 2];
        options.loss_policy = AccordionLossPolicy::RequireAll;
        options.octave_fit = true;
        let fitted = arrange_for_accordion_with_options(&score, options);
        assert!(fitted.score.is_some());
        assert_ne!(fitted.transposition_semitones, 0);
        assert!(
            fitted
                .diagnostics
                .iter()
                .any(|diagnostic| diagnostic.code == "octave-shift")
        );
    }

    #[test]
    fn reduction_reports_measure_annotations_that_cannot_merge() {
        let mut score = reduction_fixture();
        score.parts[2].staves[0].measures[1].tempo_text = Some("rubato".to_string());
        let options = reduction_options(AccordionLossPolicy::RequireAll);
        let rejected = arrange_for_accordion_with_options(&score, options.clone());
        assert!(rejected.score.is_none());
        assert!(rejected.diagnostics.iter().any(|diagnostic| diagnostic.code
            == "unsupported-notation"
            && diagnostic.source_location.as_deref() == Some("/score/part/3/staff/1/measure/2")));
        let mut permitted = options;
        permitted.loss_policy = AccordionLossPolicy::PermitReduction;
        let result = arrange_for_accordion_with_options(&score, permitted);
        assert!(result.score.is_some());
        assert!(
            result
                .diagnostics
                .iter()
                .any(|diagnostic| diagnostic.code == "unsupported-notation")
        );
    }

    #[test]
    fn analyze_ranks_by_mean_pitch_descending() {
        let mut score = Score::new("T", 120, 4, 4, 0, 1);
        score.parts[0].name = "Low".to_string();
        score.parts[0].staves[0].measures[0].voices[0] = vec![note(Step::C, 3, Duration::Whole)];

        let mut high = score.parts[0].clone();
        high.name = "High".to_string();
        high.staves[0].measures[0].voices[0] = vec![note(Step::C, 5, Duration::Whole)];
        score.parts.push(high);

        let analysis = analyze_for_accordion(&score);
        assert_eq!(analysis.candidates.len(), 2);
        assert_eq!(analysis.candidates[0].name, "High");
        assert_eq!(analysis.candidates[1].name, "Low");
    }

    #[test]
    fn analyze_excludes_percussion_channel() {
        let mut score = Score::new("T", 120, 4, 4, 0, 1);
        score.parts[0].midi_channel = 9;
        score.parts[0].staves[0].measures[0].voices[0] = vec![note(Step::C, 4, Duration::Whole)];
        assert!(analyze_for_accordion(&score).candidates.is_empty());
    }

    #[test]
    fn analyze_excludes_silent_part() {
        // Score::new's default measures are rest-filled, not truly empty — mean_pitch
        // must filter is_rest notes out, not just check for an empty voice.
        let score = Score::new("T", 120, 4, 4, 0, 1);
        assert!(analyze_for_accordion(&score).candidates.is_empty());
    }

    #[test]
    fn arrange_no_candidates_errors() {
        let score = Score::new("T", 120, 4, 4, 0, 1);
        assert!(arrange_for_accordion(&score, None).is_err());
    }

    #[test]
    fn arrange_rejects_out_of_range_part_index() {
        let mut score = Score::new("T", 120, 4, 4, 0, 1);
        score.parts[0].staves[0].measures[0].voices[0] = vec![note(Step::C, 4, Duration::Whole)];
        let result = arrange_for_accordion(&score, Some(99));
        assert!(matches!(result, Err(Error::PartNotFound(99))));
    }

    #[test]
    fn arrange_single_part_splits_chord_and_sets_accordion_program() {
        let mut score = Score::new("T", 120, 4, 4, 0, 1);
        let mut chord = note(Step::C, 5, Duration::Whole);
        chord.pitches.push(Pitch::new(Step::C, 3));
        score.parts[0].staves[0].measures[0].voices[0] = vec![chord];

        let result = arrange_for_accordion(&score, None).unwrap();
        assert_eq!(result.score.parts.len(), 1);
        assert_eq!(result.score.parts[0].midi_program, ACCORDION_PROGRAM);
        assert_eq!(result.score.parts[0].staves.len(), 2);
        assert_eq!(result.score.parts[0].staves[0].clef, Clef::Treble);
        assert_eq!(result.score.parts[0].staves[1].clef, Clef::Bass);
    }

    #[test]
    fn arrange_two_parts_puts_higher_mean_pitch_on_treble() {
        let mut score = Score::new("T", 120, 4, 4, 0, 2);
        score.parts[0].staves[0].measures[0].voices[0] = vec![note(Step::C, 3, Duration::Whole)];
        score.parts[0].staves[0].measures[1].voices[0] = vec![note(Step::C, 3, Duration::Whole)];

        let mut melody = score.parts[0].clone();
        melody.name = "Melody".to_string();
        melody.staves[0].measures[0].voices[0] = vec![note(Step::C, 5, Duration::Whole)];
        melody.staves[0].measures[1].voices[0] = vec![note(Step::C, 5, Duration::Whole)];
        score.parts.push(melody);

        let result = arrange_for_accordion(&score, None).unwrap();
        assert_eq!(result.score.parts[0].midi_program, ACCORDION_PROGRAM);
        let treble_note = &result.score.parts[0].staves[0].measures[0].voices[0][0];
        assert!(!treble_note.is_rest);
        assert_eq!(treble_note.pitches[0].octave, 5);
    }

    #[test]
    fn arrange_right_hand_override_picks_requested_part() {
        let mut score = Score::new("T", 120, 4, 4, 0, 1);
        score.parts[0].name = "Low".to_string();
        score.parts[0].staves[0].measures[0].voices[0] = vec![note(Step::C, 3, Duration::Whole)];

        let mut high = score.parts[0].clone();
        high.name = "High".to_string();
        high.staves[0].measures[0].voices[0] = vec![note(Step::C, 5, Duration::Whole)];
        score.parts.push(high);

        // Force the LOWER part (index 0) onto the treble staff, against the default ranking.
        let result = arrange_for_accordion(&score, Some(0)).unwrap();
        let treble_note = &result.score.parts[0].staves[0].measures[0].voices[0][0];
        assert!(!treble_note.is_rest);
        assert_eq!(treble_note.pitches[0].octave, 3);
    }

    #[test]
    fn arranged_measures_pass_beat_count_validation() {
        // Deliberately mismatched rhythmic density (2 halves vs 4 quarters vs a
        // whole note) across the two source parts — exercises the onset
        // bucketing + gap-fill path most likely to leave a measure underfull.
        let mut score = Score::new("T", 120, 4, 4, 0, 2);
        score.parts[0].staves[0].measures[0].voices[0] = vec![
            note(Step::C, 3, Duration::Half),
            note(Step::E, 3, Duration::Half),
        ];
        score.parts[0].staves[0].measures[1].voices[0] = vec![note(Step::G, 3, Duration::Whole)];

        let mut melody = score.parts[0].clone();
        melody.staves[0].measures[0].voices[0] = vec![
            note(Step::C, 5, Duration::Quarter),
            note(Step::D, 5, Duration::Quarter),
            note(Step::E, 5, Duration::Quarter),
            note(Step::F, 5, Duration::Quarter),
        ];
        melody.staves[0].measures[1].voices[0] = vec![note(Step::G, 5, Duration::Whole)];
        score.parts.push(melody);

        let result = arrange_for_accordion(&score, None).unwrap();
        let report = validate(&result.score);
        assert!(
            report.errors.is_empty(),
            "expected no beat-count errors, got {:?}",
            report.errors
        );
    }
}
