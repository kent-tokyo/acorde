//! Score-connected, spelled harmony voicings with source-bound replay.
//!
//! All positions are quarter-note fractions. Source parts are preserved and a
//! separate accompaniment part is appended. Unsupported source effects are
//! diagnosed before returning any score; context must not silently ignore them.

use super::arrange_presets::input_fingerprint;
use super::chord_plan::{BeatFraction, CHORD_PLAN_CONTRACT_VERSION, ChordPlan, note_length};
use super::chordal_pad::{duration_for_ticks, emit_rest, free_channel};
use super::harmony_expansion::expand_harmony;
use super::harmony_voicing::{
    HarmonyRootPolicy, HarmonyVoicingEdge, HarmonyVoicingError, HarmonyVoicingNode,
    HarmonyVoicingRequest, HarmonyVoicingResult, plan_harmony_voicings,
};
use super::notation::{Articulation, Clef, OttavaKind};
use super::pitch::Pitch;
use super::repeat::measure_sequence;
use super::score::{Measure, Note, NoteAddr, Part, Score, Staff};
use serde::{Deserialize, Serialize};
use std::cmp::Ordering;

pub const HARMONY_ARRANGEMENT_CONTRACT_VERSION: u16 = 1;
const ENGINE: &str = concat!(env!("CARGO_PKG_VERSION"), "+harmony-arrangement.1");

#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
#[serde(deny_unknown_fields)]
pub struct HarmonyStaffRef {
    pub part: usize,
    pub staff: usize,
}

#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
#[serde(deny_unknown_fields)]
pub struct HarmonyArrangementRequest {
    pub chords: ChordPlan,
    pub melody: HarmonyStaffRef,
    pub bass: Option<HarmonyStaffRef>,
    pub start_measure: usize,
    pub end_measure_exclusive: usize,
    pub locked_measures: Vec<usize>,
    pub root_policy: HarmonyRootPolicy,
    pub voices: u8,
    pub low_midi: u8,
    pub high_midi: u8,
    pub center_midi: u8,
    pub min_melody_gap_semitones: u8,
    pub min_bass_gap_semitones: u8,
    pub max_leap_semitones: u8,
    pub parallel_perfect_penalty: u16,
    pub midi_program: u8,
}

#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
#[serde(deny_unknown_fields)]
pub struct ResolvedHarmonyArrangement {
    pub contract_version: u16,
    pub engine_version: String,
    pub source_fingerprint: String,
    pub request: HarmonyArrangementRequest,
}

/// A written sounding interval. Later comping schedules can move the start
/// without moving `chord_entry`, which identifies the harmonic reference.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct HarmonyArrangementSpan {
    pub measure: usize,
    pub start: BeatFraction,
    pub end: BeatFraction,
    pub chord_entry: usize,
    pub node: usize,
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct HarmonyArrangementNote {
    pub address: NoteAddr,
    pub span: usize,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct HarmonyArrangementResult {
    pub score: Score,
    pub added_part: usize,
    pub resolved: ResolvedHarmonyArrangement,
    pub context: HarmonyVoicingRequest,
    pub voicings: HarmonyVoicingResult,
    pub spans: Vec<HarmonyArrangementSpan>,
    pub generated_notes: Vec<HarmonyArrangementNote>,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "kebab-case")]
pub enum HarmonyContextFeature {
    Pedal,
    PitchEffect,
    Ornament,
    Grace,
    Unpitched,
    Microtone,
}

#[derive(Debug, Clone, PartialEq, Eq, thiserror::Error)]
pub enum HarmonyArrangementError {
    #[error("invalid or unbounded harmony arrangement request")]
    InvalidRequest,
    #[error("invalid source score")]
    InvalidScore,
    #[error("staff {staff} in part {part} has inconsistent measure lengths")]
    InconsistentMeasures { part: usize, staff: usize },
    #[error("invalid harmony position at entry {entry}")]
    InvalidPlan { entry: usize },
    #[error("exact rational time cannot be represented")]
    InvalidTime,
    #[error("measure {measure} cannot be emitted on the supported notation grid")]
    UnrepresentableTime { measure: usize },
    #[error("unsupported {feature:?} context at {address:?}")]
    UnsupportedContext {
        address: NoteAddr,
        feature: HarmonyContextFeature,
    },
    #[error("no unlocked harmony intervals in the requested range")]
    EmptySelection,
    #[error("no unoccupied melodic MIDI channel")]
    NoChannel,
    #[error("no register remains between bass and melody at chord entry {entry}")]
    NoRegister { entry: usize },
    #[error(transparent)]
    Voicing(#[from] HarmonyVoicingError),
    #[error("resolved harmony arrangement contract or engine version differs")]
    ReplayVersion,
    #[error("resolved harmony arrangement source differs")]
    ReplaySource,
}

type Error = HarmonyArrangementError;

/// Normalized exact time, with numerical rather than lexical fraction ordering.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
struct Time(BeatFraction);
impl Ord for Time {
    fn cmp(&self, other: &Self) -> Ordering {
        (u64::from(self.0.numerator) * u64::from(other.0.denominator))
            .cmp(&(u64::from(other.0.numerator) * u64::from(self.0.denominator)))
    }
}
impl PartialOrd for Time {
    fn partial_cmp(&self, other: &Self) -> Option<Ordering> {
        Some(self.cmp(other))
    }
}
impl Time {
    const ZERO: Self = Self(BeatFraction::ZERO);
    fn new(n: u64, d: u64) -> Result<Self, Error> {
        BeatFraction::new(n, d).map(Self).ok_or(Error::InvalidTime)
    }
    fn from(value: BeatFraction) -> Result<Self, Error> {
        Self::new(value.numerator.into(), value.denominator.into())
    }
    fn add(self, other: Self) -> Result<Self, Error> {
        self.0.add(other.0).map(Self).ok_or(Error::InvalidTime)
    }
    fn sub(self, other: Self) -> Result<Self, Error> {
        let n = (u64::from(self.0.numerator) * u64::from(other.0.denominator))
            .checked_sub(u64::from(other.0.numerator) * u64::from(self.0.denominator))
            .ok_or(Error::InvalidTime)?;
        Self::new(
            n,
            u64::from(self.0.denominator) * u64::from(other.0.denominator),
        )
    }
    fn ticks(self, measure: usize) -> Result<u32, Error> {
        let n = u64::from(self.0.numerator) * 32;
        let d = u64::from(self.0.denominator);
        if !n.is_multiple_of(d) {
            return Err(Error::UnrepresentableTime { measure });
        }
        u32::try_from(n / d).map_err(|_| Error::InvalidTime)
    }
}

fn staff_at(score: &Score, address: HarmonyStaffRef) -> Result<&Staff, Error> {
    score
        .parts
        .get(address.part)
        .and_then(|part| part.staves.get(address.staff))
        .ok_or(Error::InvalidRequest)
}

fn lengths(score: &Score, staff: &Staff) -> Result<Vec<Time>, Error> {
    staff
        .measures
        .iter()
        .enumerate()
        .map(|(index, measure)| {
            let meter = staff.meter_at(index, &score.settings.time_signature);
            let (n, d) = measure.actual_length.map_or(
                (u64::from(meter.numerator), u64::from(meter.denominator)),
                |length| (u64::from(length.numerator), u64::from(length.denominator)),
            );
            let time = Time::new(n * 4, d)?;
            if time == Time::ZERO || time > Time::new(256, 1)? {
                return Err(Error::InvalidTime);
            }
            Ok(time)
        })
        .collect()
}

fn bounds(score: &Score, request: &HarmonyArrangementRequest) -> Result<(), Error> {
    if score.parts.is_empty()
        || score.parts.len() > 32
        || request.chords.entries.is_empty()
        || request.chords.entries.len() > 64
        || request.chords.contract_version != CHORD_PLAN_CONTRACT_VERSION
        || request.locked_measures.len() > 32
        || request.midi_program > 127
        || request.min_melody_gap_semitones > 24
        || request.min_bass_gap_semitones > 24
    {
        return Err(Error::InvalidRequest);
    }
    let reference = staff_at(score, request.melody)?;
    let count = reference.measures.len();
    if count == 0
        || count > 32
        || request.start_measure >= request.end_measure_exclusive
        || request.end_measure_exclusive > count
        || request.locked_measures.iter().any(|m| *m >= count)
        || request.bass == Some(request.melody)
    {
        return Err(Error::InvalidRequest);
    }
    if let Some(bass) = request.bass {
        staff_at(score, bass)?;
    }
    let mut notes = 0usize;
    let mut pitches = 0usize;
    for part in &score.parts {
        if part.staves.is_empty() || part.staves.len() > 8 {
            return Err(Error::InvalidRequest);
        }
        for staff in &part.staves {
            if staff.measures.len() != count {
                return Err(Error::InvalidRequest);
            }
            for voice in staff.measures.iter().flat_map(|bar| &bar.voices) {
                notes = notes.saturating_add(voice.len());
                if notes > 8192 {
                    return Err(Error::InvalidRequest);
                }
                for note in voice {
                    pitches = pitches.saturating_add(note.pitches.len());
                }
                if pitches > 32768 {
                    return Err(Error::InvalidRequest);
                }
            }
        }
    }
    Ok(())
}

#[derive(Clone)]
struct Sound {
    start: Time,
    end: Time,
    midi: u8,
}

fn context_sounds(
    score: &Score,
    selected: HarmonyStaffRef,
    starts: &[Time],
) -> Result<Vec<Sound>, Error> {
    let staff = staff_at(score, selected)?;
    let part = &score.parts[selected.part];
    let mut slots = Vec::new();
    let mut ottavas = Vec::new();
    for (measure_index, bar) in staff.measures.iter().enumerate() {
        for (voice_index, voice) in bar.voices.iter().enumerate() {
            let mut cursor = starts[measure_index];
            for (note_index, note) in voice.iter().enumerate() {
                let address = NoteAddr {
                    part: selected.part,
                    staff: selected.staff,
                    measure: measure_index,
                    voice: voice_index,
                    note: note_index,
                };
                let duration = Time::from(note_length(note).ok_or(Error::InvalidTime)?)?;
                let end = cursor.add(duration)?;
                if end > starts[measure_index + 1] {
                    return Err(Error::InvalidScore);
                }
                let index = slots.len();
                if let Some(kind) = note.ottava_start {
                    let shift = match kind {
                        OttavaKind::Va8 => 1,
                        OttavaKind::Vb8 => -1,
                        OttavaKind::Ma15 => 2,
                        OttavaKind::Mb15 => -2,
                    };
                    ottavas.push((cursor, 1, shift, index));
                }
                ottavas.push((cursor, 2, 0, index));
                if note.ottava_end {
                    ottavas.push((end, 0, 0, index));
                }
                slots.push((address, cursor, end, note));
                cursor = end;
            }
        }
    }
    ottavas.sort_by_key(|&(time, kind, _, index)| (time, kind, index));
    let mut shifts = vec![0i16; slots.len()];
    let mut active = 0;
    for (_, kind, shift, index) in ottavas {
        match kind {
            0 => active = 0,
            1 => active = shift,
            _ => shifts[index] = active * 12,
        }
    }
    let mut sounds = Vec::new();
    for (index, (address, start, mut end, note)) in slots.into_iter().enumerate() {
        let bar = &staff.measures[address.measure];
        let instrument = bar.instrument_change.as_ref().or(part.instrument.as_ref());
        let fail = |feature| Error::UnsupportedContext {
            address: address.clone(),
            feature,
        };
        if note.pedal_start || note.pedal_end || !bar.timed_pedals.is_empty() {
            return Err(fail(HarmonyContextFeature::Pedal));
        }
        if !part.midi_pitch_bends.is_empty()
            || part
                .midi_control_changes
                .iter()
                .any(|cc| matches!(cc.controller, 64 | 66 | 120 | 123))
        {
            return Err(fail(HarmonyContextFeature::PitchEffect));
        }
        if note.is_cue || note.is_rest {
            continue;
        }
        if note.is_grace {
            return Err(fail(HarmonyContextFeature::Grace));
        }
        if note.is_unpitched || instrument.map_or(part.midi_channel, |i| i.midi_channel) == 9 {
            return Err(fail(HarmonyContextFeature::Unpitched));
        }
        if !note.guitar_bend_curve.is_empty()
            || note.guitar_bend_alter_cents.is_some_and(|c| c != 0)
            || note.glissando_start
            || note.glissando_end
            || note.arpeggiate.is_some()
        {
            return Err(fail(HarmonyContextFeature::PitchEffect));
        }
        if note.trill_line_start
            || note.trill_line_end
            || note.tremolo_pair.is_some()
            || note.articulations.iter().any(|a| {
                matches!(
                    a,
                    Articulation::Trill
                        | Articulation::Mordent
                        | Articulation::InvertedMordent
                        | Articulation::Turn
                        | Articulation::InvertedTurn
                        | Articulation::Shake
                        | Articulation::Tremolo(_)
                )
            })
        {
            return Err(fail(HarmonyContextFeature::Ornament));
        }
        for articulation in &note.articulations {
            if matches!(
                articulation,
                Articulation::Staccato | Articulation::Staccatissimo
            ) {
                let duration = end.sub(start)?;
                end = start.add(Time::new(
                    u64::from(duration.0.numerator),
                    u64::from(duration.0.denominator) * 2,
                )?)?;
            }
        }
        let transpose = i16::from(staff.transpose_semitones)
            + instrument.map_or(0, |i| i16::from(i.transpose_semitones))
            + shifts[index];
        if note.pitches.is_empty() || end <= start {
            return Err(Error::InvalidScore);
        }
        for pitch in &note.pitches {
            if pitch.microtone_cents != 0 {
                return Err(fail(HarmonyContextFeature::Microtone));
            }
            let midi = pitch.to_midi() + transpose;
            if !(0..=127).contains(&midi) {
                return Err(Error::InvalidScore);
            }
            sounds.push(Sound {
                start,
                end,
                midi: midi as u8,
            });
        }
    }
    sounds.sort_by_key(|sound| (sound.start, sound.end, sound.midi));
    Ok(sounds)
}

fn coverage(sounds: &[Sound], start: Time, end: Time, root: u8, below: u8) -> Option<u8> {
    let mut cursor = start;
    let mut highest = None;
    for sound in sounds
        .iter()
        .filter(|s| s.midi < below && s.midi % 12 == root && s.start < end && s.end > start)
    {
        if sound.start > cursor {
            return None;
        }
        cursor = cursor.max(sound.end);
        highest = Some(highest.map_or(sound.midi, |pitch: u8| pitch.max(sound.midi)));
    }
    (cursor >= end).then_some(highest).flatten()
}

fn emit_spelled(pitches: &[Pitch], mut ticks: u32, voice: &mut Vec<Note>) -> Result<(), Error> {
    let first = pitches.first().ok_or(Error::InvalidRequest)?;
    let begin = voice.len();
    while ticks > 0 {
        let (duration, length) = duration_for_ticks(ticks).ok_or(Error::InvalidTime)?;
        let mut note = Note::new(first.clone(), duration);
        note.pitches = pitches.to_vec();
        note.tie_end = voice.len() > begin;
        note.tie_start = ticks > length;
        voice.push(note);
        ticks -= length;
    }
    Ok(())
}

/// Generate held voicings, reattacked at written bar boundaries, without
/// mutating source parts. Every performed transition participates in planning.
pub fn arrange_harmony_voicings(
    score: &Score,
    request: &HarmonyArrangementRequest,
) -> Result<HarmonyArrangementResult, Error> {
    bounds(score, request)?;
    if !super::validate::validate(score).is_valid() {
        return Err(Error::InvalidScore);
    }
    let reference = staff_at(score, request.melody)?;
    let bars = lengths(score, reference)?;
    let mut starts = vec![Time::ZERO];
    let mut cursor = Time::ZERO;
    for (measure, &length) in bars.iter().enumerate() {
        length.ticks(measure)?;
        cursor = cursor.add(length)?;
        starts.push(cursor);
    }
    for (part_index, part) in score.parts.iter().enumerate() {
        for (staff_index, staff) in part.staves.iter().enumerate() {
            if lengths(score, staff)? != bars {
                return Err(Error::InconsistentMeasures {
                    part: part_index,
                    staff: staff_index,
                });
            }
        }
    }
    let mut positions = Vec::new();
    for (entry, chord) in request.chords.entries.iter().enumerate() {
        let measure = chord.position.measure_index;
        let offset = Time::from(chord.position.offset)?;
        if measure >= bars.len()
            || offset >= bars[measure]
            || !chord.offset_beats.is_finite()
            || (chord.offset_beats - offset.0.beats()).abs() > 1e-9
        {
            return Err(Error::InvalidPlan { entry });
        }
        offset.ticks(measure)?;
        let position = starts[measure].add(offset)?;
        if positions.last().is_some_and(|p| *p >= position)
            || (entry == 0 && position != Time::ZERO)
        {
            return Err(Error::InvalidPlan { entry });
        }
        positions.push(position);
    }
    let melody = context_sounds(score, request.melody, &starts)?;
    let bass = request
        .bass
        .map(|staff| context_sounds(score, staff, &starts))
        .transpose()?
        .unwrap_or_default();
    let mut spans = Vec::new();
    let mut entry_nodes = vec![None; positions.len()];
    let mut nodes = Vec::new();
    let mut supported = Vec::new();
    let mut by_measure = vec![Vec::new(); bars.len()];
    for measure in request.start_measure..request.end_measure_exclusive {
        if request.locked_measures.contains(&measure) {
            continue;
        }
        for (entry, &position) in positions.iter().enumerate() {
            let start = position.max(starts[measure]);
            let end = positions
                .get(entry + 1)
                .copied()
                .unwrap_or(cursor)
                .min(starts[measure + 1]);
            if start >= end {
                continue;
            }
            let node = if let Some(node) = entry_nodes[entry] {
                node
            } else {
                let node = nodes.len();
                entry_nodes[entry] = Some(node);
                nodes.push(HarmonyVoicingNode {
                    chord: request.chords.entries[entry].chord.clone(),
                    bass_midi: None,
                    melody_ceiling_midi: None,
                    accompaniment_floor_midi: None,
                });
                supported.push(true);
                node
            };
            let harmony = expand_harmony(&nodes[node].chord)
                .map_err(|source| HarmonyVoicingError::Harmony { node, source })?;
            if let Some(pitch) = bass
                .iter()
                .filter(|s| s.start < end && s.end > start)
                .map(|s| s.midi)
                .max()
            {
                let floor = pitch
                    .checked_add(request.min_bass_gap_semitones)
                    .filter(|p| *p <= 127)
                    .ok_or(Error::NoRegister { entry })?;
                nodes[node].accompaniment_floor_midi = Some(
                    nodes[node]
                        .accompaniment_floor_midi
                        .map_or(floor, |p| p.max(floor)),
                );
            }
            let floor = nodes[node]
                .accompaniment_floor_midi
                .unwrap_or(0)
                .max(request.low_midi);
            let root_support = coverage(&bass, start, end, harmony.root.pitch_class(), floor);
            supported[node] &= root_support.is_some();
            if let Some(pitch) = root_support {
                nodes[node].bass_midi = Some(nodes[node].bass_midi.map_or(pitch, |p| p.max(pitch)));
            }
            if let Some(pitch) = melody
                .iter()
                .filter(|s| s.start < end && s.end > start)
                .map(|s| s.midi)
                .min()
            {
                let ceiling = pitch
                    .checked_sub(request.min_melody_gap_semitones)
                    .ok_or(Error::NoRegister { entry })?;
                nodes[node].melody_ceiling_midi = Some(
                    nodes[node]
                        .melody_ceiling_midi
                        .map_or(ceiling, |p| p.min(ceiling)),
                );
            }
            by_measure[measure].push(spans.len());
            spans.push(HarmonyArrangementSpan {
                measure,
                start: start.sub(starts[measure])?.0,
                end: end.sub(starts[measure])?.0,
                chord_entry: entry,
                node,
            });
        }
    }
    if nodes.is_empty() {
        return Err(Error::EmptySelection);
    }
    for (node, supported) in nodes.iter_mut().zip(supported) {
        if !supported {
            node.bass_midi = None;
        }
    }
    let mut edges = Vec::new();
    let mut previous = None;
    let mut previous_measure = None;
    let mut jumped = false;
    for measure in measure_sequence(score) {
        if measure >= bars.len() {
            return Err(Error::InvalidScore);
        }
        jumped |= previous_measure.is_some_and(|last| measure <= last);
        previous_measure = Some(measure);
        for &span in &by_measure[measure] {
            let node = spans[span].node;
            if let Some(from) = previous {
                if edges.len() >= 512 {
                    return Err(Error::InvalidRequest);
                }
                edges.push(HarmonyVoicingEdge {
                    from,
                    to: node,
                    repeat_boundary: jumped,
                });
            }
            previous = Some(node);
            jumped = false;
        }
    }
    let context = HarmonyVoicingRequest {
        nodes,
        edges,
        root_policy: request.root_policy,
        voices: request.voices,
        low_midi: request.low_midi,
        high_midi: request.high_midi,
        center_midi: request.center_midi,
        max_leap_semitones: request.max_leap_semitones,
        parallel_perfect_penalty: request.parallel_perfect_penalty,
    };
    let voicings = plan_harmony_voicings(&context)?;
    let channel = free_channel(score).ok_or(Error::NoChannel)?;
    let added_part = score.parts.len();
    let mut staff = Staff::new(if request.center_midi < 60 {
        Clef::Bass
    } else {
        Clef::Treble
    });
    let mut generated_notes = Vec::new();
    let mut generated_count = 0usize;
    for (measure, source_bar) in reference.measures.iter().enumerate() {
        let mut voice = Vec::new();
        let mut cursor = 0;
        for &span_index in &by_measure[measure] {
            let span = &spans[span_index];
            let start = Time::from(span.start)?.ticks(measure)?;
            let end = Time::from(span.end)?.ticks(measure)?;
            if !emit_rest(start - cursor, &mut voice) {
                return Err(Error::InvalidTime);
            }
            let begin = voice.len();
            let pitches = voicings.voicings[span.node]
                .tones
                .iter()
                .map(|tone| tone.pitch.clone())
                .collect::<Vec<_>>();
            emit_spelled(&pitches, end - start, &mut voice)?;
            for note in begin..voice.len() {
                generated_notes.push(HarmonyArrangementNote {
                    address: NoteAddr {
                        part: added_part,
                        staff: 0,
                        measure,
                        voice: 0,
                        note,
                    },
                    span: span_index,
                });
            }
            cursor = end;
        }
        if !emit_rest(bars[measure].ticks(measure)? - cursor, &mut voice) {
            return Err(Error::InvalidTime);
        }
        generated_count += voice.len();
        if generated_count > 8192 {
            return Err(Error::InvalidRequest);
        }
        let meter = reference.meter_at(measure, &score.settings.time_signature);
        let mut bar = Measure::empty(meter.numerator, meter.denominator);
        bar.number = source_bar.number;
        bar.time_sig = source_bar.time_sig.clone();
        bar.time_sig_hidden = source_bar.time_sig_hidden;
        bar.time_symbol = source_bar.time_symbol;
        bar.key_sig = source_bar.key_sig.clone();
        bar.actual_length = source_bar.actual_length;
        bar.barline_left = source_bar.barline_left.clone();
        bar.barline_right = source_bar.barline_right.clone();
        bar.volta = source_bar.volta.clone();
        bar.navigation = source_bar.navigation.clone();
        bar.voices[0] = voice;
        staff.measures.push(bar);
    }
    let mut part = Part::new("Harmony", "Harm.");
    part.midi_channel = channel;
    part.midi_program = request.midi_program;
    part.staves.push(staff);
    let mut output = score.clone();
    output.parts.push(part);
    if !super::validate::validate(&output).is_valid() {
        return Err(Error::InvalidScore);
    }
    let resolved = ResolvedHarmonyArrangement {
        contract_version: HARMONY_ARRANGEMENT_CONTRACT_VERSION,
        engine_version: ENGINE.into(),
        source_fingerprint: input_fingerprint(score).map_err(|_| Error::InvalidScore)?,
        request: request.clone(),
    };
    Ok(HarmonyArrangementResult {
        score: output,
        added_part,
        resolved,
        context,
        voicings,
        spans,
        generated_notes,
    })
}

pub fn replay_harmony_arrangement(
    score: &Score,
    resolved: &ResolvedHarmonyArrangement,
) -> Result<HarmonyArrangementResult, Error> {
    if resolved.contract_version != HARMONY_ARRANGEMENT_CONTRACT_VERSION
        || resolved.engine_version != ENGINE
    {
        return Err(Error::ReplayVersion);
    }
    bounds(score, &resolved.request)?;
    if input_fingerprint(score).ok().as_deref() != Some(resolved.source_fingerprint.as_str()) {
        return Err(Error::ReplaySource);
    }
    arrange_harmony_voicings(score, &resolved.request)
}
