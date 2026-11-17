//! Exact shared kick/bass scheduling with target-bound approach notes.
use super::{
    arrange_presets::input_fingerprint,
    chord_plan::{BeatFraction, CHORD_PLAN_CONTRACT_VERSION, ChordPlan},
    chordal_pad::{duration_for_ticks, emit_rest, free_channel},
    harmony_expansion::{ExpandedHarmony, HarmonyPitchClass, expand_harmony},
    notation::{Articulation, Clef, Dynamic},
    pitch::{Pitch, Step},
    playback::{PlaybackOptions, to_playback_events},
    repeat::measure_sequence,
    score::{Measure, Note, NoteAddr, Part, PercussionInstrument, Score, Staff},
};
use serde::{Deserialize, Serialize};
use std::collections::{BTreeMap, BTreeSet};
pub const RHYTHM_SECTION_CONTRACT_VERSION: u16 = 1;
const ENGINE: &str = concat!(env!("CARGO_PKG_VERSION"), "+rhythm-section.1");

#[derive(Debug, Clone, Copy, PartialEq, Eq, PartialOrd, Ord, Serialize, Deserialize)]
#[serde(deny_unknown_fields)]
pub struct RhythmAttackRef {
    pub measure: usize,
    pub slot: usize,
}
#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "kebab-case")]
pub enum BassApproachDirection {
    Below,
    Above,
}
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "kebab-case", deny_unknown_fields)]
pub enum RhythmBassTone {
    Root,
    SlashBass,
    Third,
    Fifth,
    Approach {
        target: RhythmAttackRef,
        direction: BassApproachDirection,
        semitones: u8,
    },
}
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
#[serde(deny_unknown_fields)]
pub struct RhythmKickAttack {
    pub hold_ticks: u16,
    pub accent: bool,
}
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
#[serde(deny_unknown_fields)]
pub struct RhythmBassAttack {
    pub tone: RhythmBassTone,
    pub hold_ticks: u16,
    pub accent: bool,
    pub ghost: bool,
}
/// Consecutive slots use 32 ticks per quarter note. None is an explicit rest.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
#[serde(deny_unknown_fields)]
pub struct RhythmSectionSlot {
    pub ticks: u16,
    pub kick: Option<RhythmKickAttack>,
    pub bass: Option<RhythmBassAttack>,
    /// Require both attacks at this exact slot start. Other kicks may stand alone.
    pub synchronize: bool,
}
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
#[serde(deny_unknown_fields)]
pub struct RhythmSectionBar {
    pub measure: usize,
    pub slots: Vec<RhythmSectionSlot>,
}
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "kebab-case", deny_unknown_fields)]
pub enum BassGhostSound {
    /// Short, quiet pitched approximation; no claim of a muted articulation.
    ShortQuiet,
    /// Caller attests that this separately routed program is a muted bass sound.
    /// A separate part preserves routing through MusicXML and MSCZ.
    MutedProgram {
        midi_program: u8,
        sound_identity: String,
    },
}
#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
#[serde(deny_unknown_fields)]
pub struct RhythmSectionRequest {
    pub chords: ChordPlan,
    pub bars: Vec<RhythmSectionBar>,
    pub start_measure: usize,
    pub end_measure_exclusive: usize,
    pub locked_measures: Vec<usize>,
    pub low_midi: u8,
    pub high_midi: u8,
    pub center_midi: u8,
    pub max_leap_semitones: u8,
    pub bass_program: u8,
    pub ghost_sound: BassGhostSound,
}
#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
#[serde(deny_unknown_fields)]
pub struct ResolvedBassApproach {
    pub target: RhythmAttackRef,
    pub target_pitch: Pitch,
    pub performed_resolutions: u32,
}
#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
#[serde(deny_unknown_fields)]
pub struct ResolvedRhythmBass {
    pub at: RhythmAttackRef,
    pub offset_ticks: u32,
    pub pitch: Pitch,
    pub approach: Option<ResolvedBassApproach>,
}
#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
#[serde(deny_unknown_fields)]
pub struct ResolvedRhythmSection {
    pub contract_version: u16,
    pub engine_version: String,
    pub source_fingerprint: String,
    pub request: RhythmSectionRequest,
    pub bass: Vec<ResolvedRhythmBass>,
}
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "kebab-case")]
pub enum RhythmSectionDiagnostic {
    ShortQuietGhostApproximation,
    CallerSuppliedMutedSound,
}
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct RhythmSectionAttack {
    pub at: RhythmAttackRef,
    pub offset_ticks: u32,
    pub bass: Option<NoteAddr>,
    pub kick: Option<NoteAddr>,
    pub synchronized: bool,
}
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct RhythmSectionResult {
    pub score: Score,
    pub bass_part: usize,
    pub kick_part: usize,
    pub ghost_part: Option<usize>,
    pub resolved: ResolvedRhythmSection,
    pub attacks: Vec<RhythmSectionAttack>,
    pub diagnostics: Vec<RhythmSectionDiagnostic>,
}
#[derive(Debug, Clone, PartialEq, Eq, thiserror::Error)]
pub enum RhythmSectionError {
    #[error("invalid or unbounded rhythm section request")]
    InvalidRequest,
    #[error("invalid or inconsistent source score")]
    InvalidScore,
    #[error("invalid chord position or harmony at entry {entry}")]
    Chord { entry: usize },
    #[error("invalid rhythm in measure {measure}, slot {slot}")]
    Rhythm { measure: usize, slot: usize },
    #[error("measure {measure} is locked or outside the editable range")]
    Locked { measure: usize },
    #[error("bass register cannot realize attack {at:?}")]
    Register { at: RhythmAttackRef },
    #[error("approach {at:?} does not have the promised next bass attack on every visit")]
    Approach { at: RhythmAttackRef },
    #[error("bass leap exceeds the bound from {from:?} to {to:?}")]
    Leap {
        from: RhythmAttackRef,
        to: RhythmAttackRef,
    },
    #[error("no free MIDI channel")]
    NoChannel,
    #[error("percussion MIDI channel 9 is already in use")]
    PercussionChannelInUse,
    #[error("planned measure {measure} is absent from playback")]
    UnperformedMeasure { measure: usize },
    #[error("actual playback differs from the shared rhythm plan")]
    Playback,
    #[error("resolved rhythm contract or engine version differs")]
    ReplayVersion,
    #[error("resolved rhythm source differs")]
    ReplaySource,
    #[error("resolved rhythm choices differ")]
    ReplaySelection,
}
type Error = RhythmSectionError;
fn ticks(f: BeatFraction) -> Option<u32> {
    let n = u64::from(f.numerator) * 32;
    let d = u64::from(f.denominator);
    if d == 0 || !n.is_multiple_of(d) {
        return None;
    }
    u32::try_from(n / d).ok()
}
fn lengths(score: &Score, staff: &Staff) -> Result<Vec<u32>, Error> {
    staff
        .measures
        .iter()
        .enumerate()
        .map(|(m, bar)| {
            let meter = staff.meter_at(m, &score.settings.time_signature);
            let (n, d) = bar.actual_length.map_or(
                (u32::from(meter.numerator), u32::from(meter.denominator)),
                |l| (l.numerator, l.denominator),
            );
            let n = n.checked_mul(4).ok_or(Error::InvalidScore)?;
            ticks(BeatFraction {
                numerator: n,
                denominator: d,
            })
            .filter(|t| (1..=8192).contains(t))
            .ok_or(Error::InvalidScore)
        })
        .collect()
}
fn validate(
    score: &Score,
    r: &RhythmSectionRequest,
) -> Result<(Vec<u32>, Vec<ExpandedHarmony>), Error> {
    if score.parts.is_empty()
        || score.parts.len() > 29
        || r.bars.is_empty()
        || r.bars.len() > 32
        || r.chords.entries.is_empty()
        || r.chords.entries.len() > 64
        || r.chords.contract_version != CHORD_PLAN_CONTRACT_VERSION
        || r.locked_measures.len() > 32
        || r.low_midi > r.center_midi
        || r.center_midi > r.high_midi
        || r.high_midi > 127
        || r.bass_program > 127
        || r.max_leap_semitones > 48
    {
        return Err(Error::InvalidRequest);
    }
    if let BassGhostSound::MutedProgram {
        midi_program,
        sound_identity,
    } = &r.ghost_sound
        && (*midi_program > 127 || sound_identity.trim().is_empty() || sound_identity.len() > 256)
    {
        return Err(Error::InvalidRequest);
    }
    let reference = score.parts[0].staves.first().ok_or(Error::InvalidScore)?;
    let count = reference.measures.len();
    if count == 0
        || count > 32
        || r.start_measure >= r.end_measure_exclusive
        || r.end_measure_exclusive > count
        || r.locked_measures.iter().any(|&m| m >= count)
    {
        return Err(Error::InvalidRequest);
    }
    let performed = measure_sequence(score);
    for bar in &r.bars {
        if !performed.contains(&bar.measure) {
            return Err(Error::UnperformedMeasure {
                measure: bar.measure,
            });
        }
    }
    for part in &score.parts {
        if part.midi_channel == 9
            || part
                .instrument
                .as_ref()
                .is_some_and(|i| i.midi_channel == 9)
            || part.named_channels.iter().any(|c| c.midi_channel == 9)
            || part
                .staves
                .iter()
                .flat_map(|s| &s.measures)
                .filter_map(|m| m.instrument_change.as_ref())
                .any(|i| i.midi_channel == 9)
            || part.midi_program_changes.iter().any(|c| c.channel == 9)
            || part.midi_control_changes.iter().any(|c| c.channel == 9)
            || part.midi_pitch_bends.iter().any(|c| c.channel == 9)
            || part.midi_aftertouch.iter().any(|c| c.channel == 9)
        {
            return Err(Error::PercussionChannelInUse);
        }
    }
    let mut notes = 0usize;
    for part in &score.parts {
        if part.staves.is_empty() || part.staves.len() > 4 {
            return Err(Error::InvalidScore);
        }
        for staff in &part.staves {
            if staff.measures.len() != count {
                return Err(Error::InvalidScore);
            }
            notes += staff
                .measures
                .iter()
                .flat_map(|b| &b.voices)
                .map(Vec::len)
                .sum::<usize>();
        }
    }
    if notes > 8192 || !super::validate::validate(score).is_valid() {
        return Err(Error::InvalidScore);
    }
    let lens = lengths(score, reference)?;
    for staff in score.parts.iter().flat_map(|p| &p.staves) {
        if lengths(score, staff)? != lens {
            return Err(Error::InvalidScore);
        }
    }
    let mut previous = None;
    let mut harmonies = Vec::new();
    for (i, e) in r.chords.entries.iter().enumerate() {
        let offset = ticks(e.position.offset).ok_or(Error::Chord { entry: i })?;
        let key = (e.position.measure_index, offset);
        if key.0 >= count
            || offset >= lens[key.0]
            || (i == 0 && key != (0, 0))
            || previous.is_some_and(|p| p >= key)
            || !e.offset_beats.is_finite()
            || (e.offset_beats - e.position.offset.beats()).abs() > 1e-9
        {
            return Err(Error::Chord { entry: i });
        }
        harmonies.push(expand_harmony(&e.chord).map_err(|_| Error::Chord { entry: i })?);
        previous = Some(key);
    }
    let mut seen = BTreeSet::new();
    let mut attack_count = 0;
    for bar in &r.bars {
        let m = bar.measure;
        if m >= count || !seen.insert(m) || bar.slots.is_empty() || bar.slots.len() > 64 {
            return Err(Error::InvalidRequest);
        }
        if m < r.start_measure || m >= r.end_measure_exclusive || r.locked_measures.contains(&m) {
            return Err(Error::Locked { measure: m });
        }
        let mut sum = 0u32;
        for (slot, s) in bar.slots.iter().enumerate() {
            let bad = || Error::Rhythm { measure: m, slot };
            if s.ticks == 0 || (s.synchronize && (s.kick.is_none() || s.bass.is_none())) {
                return Err(bad());
            }
            for hold in s
                .kick
                .iter()
                .map(|a| a.hold_ticks)
                .chain(s.bass.iter().map(|a| a.hold_ticks))
            {
                if hold == 0 || hold > s.ticks {
                    return Err(bad());
                }
            }
            if let Some(b) = &s.bass {
                if b.ghost && (b.hold_ticks > 8 || b.accent) {
                    return Err(bad());
                }
                if let RhythmBassTone::Approach { semitones, .. } = b.tone
                    && !(1..=2).contains(&semitones)
                {
                    return Err(bad());
                }
            }
            sum += u32::from(s.ticks);
            attack_count += usize::from(s.kick.is_some()) + usize::from(s.bass.is_some());
        }
        if sum != lens[m] {
            return Err(Error::Rhythm {
                measure: m,
                slot: bar.slots.len(),
            });
        }
    }
    if attack_count == 0 || attack_count > 2048 {
        return Err(Error::InvalidRequest);
    }
    Ok((lens, harmonies))
}
fn choose(
    pc: &HarmonyPitchClass,
    r: &RhythmSectionRequest,
    center: i16,
    at: RhythmAttackRef,
    incoming: &[(BassApproachDirection, u8)],
) -> Result<Pitch, Error> {
    (-1..=9)
        .map(|o| Pitch::with_alter(pc.step.clone(), o, pc.alter))
        .filter(|p| (i16::from(r.low_midi)..=i16::from(r.high_midi)).contains(&p.to_midi()))
        .filter(|p| {
            incoming.iter().all(|&(direction, distance)| {
                approach_pitch(p, direction, distance).is_some_and(|a| {
                    (i16::from(r.low_midi)..=i16::from(r.high_midi)).contains(&a.to_midi())
                })
            })
        })
        .min_by_key(|p| ((p.to_midi() - center).abs(), p.to_midi()))
        .ok_or(Error::Register { at })
}
fn approach_pitch(target: &Pitch, direction: BassApproachDirection, distance: u8) -> Option<Pitch> {
    let steps = [
        Step::C,
        Step::D,
        Step::E,
        Step::F,
        Step::G,
        Step::A,
        Step::B,
    ];
    let i = steps.iter().position(|s| *s == target.step)? as i16;
    let direction = if direction == BassApproachDirection::Below {
        -1
    } else {
        1
    };
    let diatonic = i + direction;
    let step = steps[diatonic.rem_euclid(7) as usize].clone();
    let octave = i16::from(target.octave) + diatonic.div_euclid(7);
    let midi = target.to_midi() + direction * i16::from(distance);
    let alter = midi - ((octave + 1) * 12 + i16::from(step.to_semitone()));
    if !(-2..=2).contains(&alter) {
        return None;
    }
    Some(Pitch::with_alter(
        step,
        i8::try_from(octave).ok()?,
        alter as i8,
    ))
}
fn resolve(
    score: &Score,
    r: &RhythmSectionRequest,
    harmonies: &[ExpandedHarmony],
) -> Result<Vec<ResolvedRhythmBass>, Error> {
    let mut slots = BTreeMap::new();
    for bar in &r.bars {
        let mut offset = 0u32;
        for (slot, s) in bar.slots.iter().enumerate() {
            if let Some(b) = &s.bass {
                slots.insert(
                    RhythmAttackRef {
                        measure: bar.measure,
                        slot,
                    },
                    (offset, b),
                );
            }
            offset += u32::from(s.ticks);
        }
    }
    let mut incoming = BTreeMap::<RhythmAttackRef, Vec<(BassApproachDirection, u8)>>::new();
    for (_, b) in slots.values() {
        if let RhythmBassTone::Approach {
            target,
            direction,
            semitones,
        } = b.tone
        {
            incoming
                .entry(target)
                .or_default()
                .push((direction, semitones));
        }
    }
    let mut pitches = BTreeMap::new();
    let mut center = i16::from(r.center_midi);
    for (&at, &(offset, b)) in &slots {
        if matches!(b.tone, RhythmBassTone::Approach { .. }) {
            continue;
        }
        let entry = r
            .chords
            .entries
            .iter()
            .rposition(|e| {
                e.position.measure_index < at.measure
                    || (e.position.measure_index == at.measure
                        && ticks(e.position.offset).is_some_and(|t| t <= offset))
            })
            .ok_or(Error::InvalidRequest)?;
        let h = &harmonies[entry];
        let pc = match b.tone {
            RhythmBassTone::Root => &h.root,
            RhythmBassTone::SlashBass => h.bass.as_ref().unwrap_or(&h.root),
            RhythmBassTone::Third => {
                &h.tones
                    .iter()
                    .find(|t| t.degree == 3)
                    .ok_or(Error::Chord { entry })?
                    .spelling
            }
            RhythmBassTone::Fifth => {
                &h.tones
                    .iter()
                    .find(|t| t.degree == 5)
                    .ok_or(Error::Chord { entry })?
                    .spelling
            }
            RhythmBassTone::Approach { .. } => return Err(Error::InvalidRequest),
        };
        let pitch = choose(
            pc,
            r,
            center,
            at,
            incoming.get(&at).map_or(&[], Vec::as_slice),
        )?;
        center = pitch.to_midi();
        pitches.insert(at, pitch);
    }
    for (&at, &(_, b)) in &slots {
        if let RhythmBassTone::Approach {
            target,
            direction,
            semitones,
        } = b.tone
        {
            let target_pitch = pitches.get(&target).ok_or(Error::Approach { at })?;
            // Chains of approaches and ghost arrivals do not define a stable harmonic target.
            if slots
                .get(&target)
                .is_none_or(|(_, b)| b.ghost || matches!(b.tone, RhythmBassTone::Approach { .. }))
            {
                return Err(Error::Approach { at });
            }
            let p =
                approach_pitch(target_pitch, direction, semitones).ok_or(Error::Register { at })?;
            if !(i16::from(r.low_midi)..=i16::from(r.high_midi)).contains(&p.to_midi()) {
                return Err(Error::Register { at });
            }
            pitches.insert(at, p);
        }
    }
    let mut performed = Vec::new();
    for m in measure_sequence(score) {
        for &at in slots.keys().filter(|a| a.measure == m) {
            performed.push(at);
        }
    }
    if performed.len() > 32768 {
        return Err(Error::InvalidRequest);
    }
    let mut resolutions = BTreeMap::<RhythmAttackRef, u32>::new();
    for (i, &at) in performed.iter().enumerate() {
        let b = slots[&at].1;
        if let RhythmBassTone::Approach { target, .. } = b.tone {
            if performed.get(i + 1) != Some(&target) {
                return Err(Error::Approach { at });
            }
            *resolutions.entry(at).or_default() += 1;
        }
        if let Some(next) = performed.get(i + 1)
            && (pitches[&at].to_midi() - pitches[next].to_midi()).unsigned_abs()
                > u16::from(r.max_leap_semitones)
        {
            return Err(Error::Leap {
                from: at,
                to: *next,
            });
        }
    }
    slots
        .into_iter()
        .map(|(at, (offset, b))| {
            let approach = if let RhythmBassTone::Approach { target, .. } = b.tone {
                Some(ResolvedBassApproach {
                    target,
                    target_pitch: pitches.get(&target).ok_or(Error::Approach { at })?.clone(),
                    performed_resolutions: *resolutions.get(&at).ok_or(Error::Approach { at })?,
                })
            } else {
                None
            };
            Ok(ResolvedRhythmBass {
                at,
                offset_ticks: offset,
                pitch: pitches.get(&at).ok_or(Error::Register { at })?.clone(),
                approach,
            })
        })
        .collect()
}
fn part(
    score: &Score,
    name: &str,
    channel: u8,
    program: u8,
    kick: bool,
    lens: &[u32],
) -> Result<Part, Error> {
    let reference = &score.parts[0].staves[0];
    let mut part = Part::new(name, name);
    part.midi_channel = channel;
    part.midi_program = program;
    let mut staff = Staff::new(if kick { Clef::Percussion } else { Clef::Bass });
    for (m, source) in reference.measures.iter().enumerate() {
        let meter = reference.meter_at(m, &score.settings.time_signature);
        let mut bar = Measure::empty(meter.numerator, meter.denominator);
        bar.number = source.number;
        bar.time_sig = source.time_sig.clone();
        bar.time_sig_hidden = source.time_sig_hidden;
        bar.time_symbol = source.time_symbol;
        bar.key_sig = source.key_sig.clone();
        bar.actual_length = source.actual_length;
        bar.barline_left = source.barline_left.clone();
        bar.barline_right = source.barline_right.clone();
        bar.volta = source.volta.clone();
        bar.navigation = source.navigation.clone();
        bar.voices[0].clear();
        if !emit_rest(lens[m], &mut bar.voices[0]) {
            return Err(Error::InvalidScore);
        }
        staff.measures.push(bar);
    }
    part.staves.push(staff);
    if kick {
        part.percussion_instruments.push(PercussionInstrument {
            id: "rhythm-section-kick".into(),
            name: Some("Bass Drum".into()),
            midi_unpitched: Some(36),
            staff_position: Some(-3),
            notehead: None,
            preferred_voice: Some(1),
            techniques: vec![],
        });
    }
    Ok(part)
}
fn emit(
    pitch: &Pitch,
    hold: u16,
    accent: bool,
    ghost: bool,
    kick: bool,
    voice: &mut Vec<Note>,
) -> Result<usize, Error> {
    let first = voice.len();
    let mut remain = u32::from(hold);
    while remain > 0 {
        let (duration, len) = duration_for_ticks(remain).ok_or(Error::InvalidRequest)?;
        let mut n = Note::new(pitch.clone(), duration);
        n.tie_end = voice.len() > first;
        n.tie_start = remain > len;
        n.dynamic = Some(if ghost { Dynamic::Pp } else { Dynamic::Mf });
        n.articulations.push(Articulation::Tenuto);
        if accent && voice.len() == first {
            n.articulations.push(Articulation::Accent);
        }
        if ghost {
            n.pitch_parentheses = vec![true];
        }
        if kick {
            n.is_unpitched = true;
            n.instrument_id = Some("rhythm-section-kick".into());
        }
        voice.push(n);
        remain -= len;
    }
    Ok(first)
}
/// Append bass and kick parts, plus a separately routed muted ghost part when requested.
pub fn arrange_rhythm_section(
    score: &Score,
    request: &RhythmSectionRequest,
) -> Result<RhythmSectionResult, Error> {
    let (lens, harmonies) = validate(score, request)?;
    let bass = resolve(score, request, &harmonies)?;
    let has_ghost = request
        .bars
        .iter()
        .flat_map(|b| &b.slots)
        .any(|s| s.bass.as_ref().is_some_and(|b| b.ghost));
    let mut output = score.clone();
    let bass_part = output.parts.len();
    let channel = free_channel(&output).ok_or(Error::NoChannel)?;
    output.parts.push(part(
        score,
        "Bass",
        channel,
        request.bass_program,
        false,
        &lens,
    )?);
    let kick_part = output.parts.len();
    output.parts.push(part(score, "Kick", 9, 0, true, &lens)?);
    let ghost_part = if has_ghost {
        if let BassGhostSound::MutedProgram { midi_program, .. } = request.ghost_sound {
            let channel = free_channel(&output).ok_or(Error::NoChannel)?;
            let p = output.parts.len();
            output.parts.push(part(
                score,
                "Muted bass ghosts",
                channel,
                midi_program,
                false,
                &lens,
            )?);
            Some(p)
        } else {
            None
        }
    } else {
        None
    };
    let pitch_by_at = bass
        .iter()
        .map(|b| (b.at, &b.pitch))
        .collect::<BTreeMap<_, _>>();
    let mut attacks = Vec::new();
    for bar in &request.bars {
        for p in [Some(bass_part), Some(kick_part), ghost_part]
            .into_iter()
            .flatten()
        {
            output.parts[p].staves[0].measures[bar.measure].voices[0].clear();
        }
        let mut offset = 0;
        for (slot, s) in bar.slots.iter().enumerate() {
            let at = RhythmAttackRef {
                measure: bar.measure,
                slot,
            };
            let mut entry = RhythmSectionAttack {
                at,
                offset_ticks: offset,
                bass: None,
                kick: None,
                synchronized: s.synchronize,
            };
            for p in [Some(bass_part), Some(kick_part), ghost_part]
                .into_iter()
                .flatten()
            {
                let voice = &mut output.parts[p].staves[0].measures[bar.measure].voices[0];
                let mut used = 0;
                if p == kick_part {
                    if let Some(k) = &s.kick {
                        let n = emit(
                            &Pitch::with_alter(Step::F, 4, 0),
                            k.hold_ticks,
                            k.accent,
                            false,
                            true,
                            voice,
                        )?;
                        used = k.hold_ticks;
                        entry.kick = Some(NoteAddr {
                            part: p,
                            staff: 0,
                            measure: bar.measure,
                            voice: 0,
                            note: n,
                        });
                    }
                } else if let Some(b) = &s.bass {
                    let target = if b.ghost {
                        ghost_part.unwrap_or(bass_part)
                    } else {
                        bass_part
                    };
                    if p == target {
                        let pitch = pitch_by_at.get(&at).ok_or(Error::InvalidRequest)?;
                        let n = emit(pitch, b.hold_ticks, b.accent, b.ghost, false, voice)?;
                        used = b.hold_ticks;
                        entry.bass = Some(NoteAddr {
                            part: p,
                            staff: 0,
                            measure: bar.measure,
                            voice: 0,
                            note: n,
                        });
                    }
                }
                if !emit_rest(u32::from(s.ticks - used), voice) {
                    return Err(Error::InvalidRequest);
                }
            }
            if s.kick.is_some() || s.bass.is_some() {
                attacks.push(entry);
            }
            offset += u32::from(s.ticks);
        }
    }
    if !super::validate::validate(&output).is_valid() {
        return Err(Error::InvalidScore);
    }
    verify_playback(
        &output, request, &attacks, &bass, bass_part, kick_part, ghost_part,
    )?;
    let diagnostics = if has_ghost {
        vec![if ghost_part.is_some() {
            RhythmSectionDiagnostic::CallerSuppliedMutedSound
        } else {
            RhythmSectionDiagnostic::ShortQuietGhostApproximation
        }]
    } else {
        vec![]
    };
    Ok(RhythmSectionResult {
        score: output,
        bass_part,
        kick_part,
        ghost_part,
        resolved: ResolvedRhythmSection {
            contract_version: RHYTHM_SECTION_CONTRACT_VERSION,
            engine_version: ENGINE.into(),
            source_fingerprint: input_fingerprint(score).map_err(|_| Error::InvalidScore)?,
            request: request.clone(),
            bass,
        },
        attacks,
        diagnostics,
    })
}
fn address(a: &NoteAddr) -> (usize, usize, usize, usize, usize) {
    (a.part, a.staff, a.measure, a.voice, a.note)
}
fn verify_playback(
    score: &Score,
    r: &RhythmSectionRequest,
    attacks: &[RhythmSectionAttack],
    bass: &[ResolvedRhythmBass],
    bass_part: usize,
    kick_part: usize,
    ghost_part: Option<usize>,
) -> Result<(), Error> {
    let events = to_playback_events(score, &PlaybackOptions::default());
    let mut by_address = BTreeMap::<(usize, usize, usize, usize, usize), Vec<_>>::new();
    for e in &events {
        if e.part_index == bass_part
            || e.part_index == kick_part
            || Some(e.part_index) == ghost_part
        {
            let a = e.source.as_ref().ok_or(Error::Playback)?;
            by_address.entry(address(a)).or_default().push(e);
        }
    }
    let lens = lengths(score, &score.parts[0].staves[0])?;
    let mut starts = BTreeMap::<usize, Vec<u32>>::new();
    let mut cursor = 0u32;
    for measure in measure_sequence(score) {
        starts.entry(measure).or_default().push(cursor);
        cursor += lens[measure];
    }
    for a in attacks {
        let slot = r
            .bars
            .iter()
            .find(|b| b.measure == a.at.measure)
            .and_then(|b| b.slots.get(a.at.slot))
            .ok_or(Error::InvalidRequest)?;
        let be = a.bass.as_ref().and_then(|a| by_address.get(&address(a)));
        let ke = a.kick.as_ref().and_then(|a| by_address.get(&address(a)));
        let expected = starts.get(&a.at.measure).ok_or(Error::Playback)?;
        for (present, events) in [(slot.bass.is_some(), be), (slot.kick.is_some(), ke)] {
            if present {
                let events = events.ok_or(Error::Playback)?;
                if events.len() != expected.len()
                    || events.iter().zip(expected).any(|(e, start)| {
                        (e.time_beats - f64::from(start + a.offset_ticks) / 32.0).abs() > 1e-9
                    })
                {
                    return Err(Error::Playback);
                }
            }
        }
        if a.synchronized {
            let (Some(b), Some(k)) = (be, ke) else {
                return Err(Error::Playback);
            };
            if b.len() != k.len()
                || b.iter()
                    .zip(k)
                    .any(|(b, k)| (b.time_beats - k.time_beats).abs() > 1e-9)
            {
                return Err(Error::Playback);
            }
        }
        if let Some(b) = &slot.bass
            && let Some(events) = be
        {
            let pitch = bass
                .iter()
                .find(|n| n.at == a.at)
                .ok_or(Error::Playback)?
                .pitch
                .to_midi();
            for e in events {
                if i16::from(e.pitch_midi) != pitch
                    || (e.duration_beats - f64::from(b.hold_ticks) / 32.0).abs() > 1e-9
                    || e.velocity
                        != if b.ghost {
                            Dynamic::Pp.to_velocity()
                        } else {
                            Dynamic::Mf.to_velocity() + if b.accent { 20 } else { 0 }
                        }
                {
                    return Err(Error::Playback);
                }
            }
        }
        if let (Some(k), Some(events)) = (&slot.kick, ke) {
            for e in events {
                if e.pitch_midi != 36
                    || (e.duration_beats - f64::from(k.hold_ticks) / 32.0).abs() > 1e-9
                {
                    return Err(Error::Playback);
                }
            }
        }
    }
    Ok(())
}
/// Rebuild and verify saved choices; versions and source fingerprints are not signatures.
pub fn replay_rhythm_section(
    score: &Score,
    resolved: &ResolvedRhythmSection,
) -> Result<RhythmSectionResult, Error> {
    if resolved.contract_version != RHYTHM_SECTION_CONTRACT_VERSION
        || resolved.engine_version != ENGINE
    {
        return Err(Error::ReplayVersion);
    }
    if input_fingerprint(score).map_err(|_| Error::InvalidScore)? != resolved.source_fingerprint {
        return Err(Error::ReplaySource);
    }
    let result = arrange_rhythm_section(score, &resolved.request)?;
    if result.resolved != *resolved {
        return Err(Error::ReplaySelection);
    }
    Ok(result)
}
