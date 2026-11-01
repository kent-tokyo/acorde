use super::validation::validate_event_ranges;
use crate::Error;
use acorde_core::{Articulation, Score, TupletInfo};
use midly::{
    Format, Header, MetaMessage, MidiMessage, Smf, Timing, TrackEvent, TrackEventKind,
    num::{u4, u7, u15, u24, u28},
};
use std::collections::BTreeSet;

const PPQ: u32 = 480;
pub(super) const AUX_PREFIX: &[u8] = b"ACRD1";
pub(super) const AUX_MARKER_LEN: usize = AUX_PREFIX.len() + 2;
const AUX_CHANNEL_MARKERS: [[[u8; AUX_MARKER_LEN]; 16]; 16] = {
    let mut markers = [[[0; AUX_MARKER_LEN]; 16]; 16];
    let mut base = 0;
    while base < 16 {
        let mut spare = 0;
        while spare < 16 {
            let mut index = 0;
            while index < AUX_PREFIX.len() {
                markers[base][spare][index] = AUX_PREFIX[index];
                index += 1;
            }
            markers[base][spare][AUX_PREFIX.len()] = base as u8;
            markers[base][spare][AUX_PREFIX.len() + 1] = spare as u8;
            spare += 1;
        }
        base += 1;
    }
    markers
};

/// Serialize a [`Score`] to Standard MIDI File (SMF Type 1) bytes.
///
/// - PPQ = 480 ticks per quarter note
/// - Track 0: tempo + time-signature meta events
/// - Track 1..N: one track per [`Part`](acorde_core::Part)
pub fn serialize_midi(score: &Score) -> Result<Vec<u8>, Error> {
    let seq = acorde_core::measure_sequence(score);
    serialize_midi_impl(score, &seq)
}

/// Serialize a region of a [`Score`] to Standard MIDI File bytes.
///
/// Only measures whose physical index falls within the inclusive range
/// `[region.0, region.1]` are exported. The MIDI file starts at tick 0.
pub fn serialize_midi_region(score: &Score, region: (usize, usize)) -> Result<Vec<u8>, Error> {
    let (start, end) = region;
    if start > end {
        return Err(Error::Midi(format!(
            "invalid region: start={start} > end={end}"
        )));
    }
    let full_seq = acorde_core::measure_sequence(score);
    let seq: Vec<usize> = full_seq
        .into_iter()
        .filter(|&m| m >= start && m <= end)
        .collect();
    if seq.is_empty() {
        return Err(Error::Midi(format!(
            "region [{start}, {end}] contains no measures"
        )));
    }
    serialize_midi_impl(score, &seq)
}

fn serialize_midi_impl(score: &Score, seq: &[usize]) -> Result<Vec<u8>, Error> {
    validate_event_ranges(score)?;
    let header = Header::new(Format::Parallel, Timing::Metrical(u15::from(PPQ as u16)));
    let mut smf = Smf::new(header);

    let starts = measure_start_ticks(score, seq);
    let region = seq.iter().copied().min().zip(seq.iter().copied().max());
    let options = acorde_core::PlaybackOptions {
        loop_region: region,
        ..Default::default()
    };
    let pauses: Vec<(u64, u64)> = acorde_core::playback_fermata_pauses(score, &options)
        .into_iter()
        .map(|(at, length)| {
            (
                (at * f64::from(PPQ)).round() as u64,
                (length * f64::from(PPQ)).round() as u64,
            )
        })
        .scan(0u64, |cumulative, (at, length)| {
            *cumulative = cumulative.saturating_add(length);
            Some((at, *cumulative))
        })
        .collect();
    let meta_starts: Vec<u64> = starts
        .iter()
        .map(|&tick| warp_midi_tick(tick, &pauses))
        .collect();
    smf.tracks.push(build_meta_track(score, seq, &meta_starts)?);
    // Reserve every authored channel before assigning temporary channels to
    // overlapping notes. A spare in one part must not steal another part's
    // instrument or controller stream.
    let mut occupied_channels = BTreeSet::new();
    for part in &score.parts {
        occupied_channels.insert(part.midi_channel.min(15));
        occupied_channels.extend(
            part.midi_program_changes
                .iter()
                .map(|event| event.channel.min(15)),
        );
        occupied_channels.extend(
            part.midi_pitch_bends
                .iter()
                .map(|event| event.channel.min(15)),
        );
        occupied_channels.extend(
            part.midi_control_changes
                .iter()
                .map(|event| event.channel.min(15)),
        );
        occupied_channels.extend(
            part.midi_aftertouch
                .iter()
                .map(|event| event.channel.min(15)),
        );
    }
    let full_seq = acorde_core::measure_sequence(score);
    let prefix: Vec<_> = seq.first().map_or_else(Vec::new, |first| {
        full_seq
            .iter()
            .copied()
            .take_while(|idx| idx != first)
            .collect()
    });
    for part in &score.parts {
        smf.tracks.push(build_part_track(
            part,
            seq,
            &prefix,
            &starts,
            &pauses,
            &mut occupied_channels,
        )?);
    }

    let mut bytes = Vec::new();
    smf.write_std(&mut bytes)
        .map_err(|e| Error::Midi(e.to_string()))?;
    Ok(bytes)
}

/// The tick each played bar starts at: bars take their own length (a pickup's
/// `actual_length`, else the meter in force), so every voice and every meta event lines up with
/// the bar it belongs to however full its voices are.
fn measure_start_ticks(score: &Score, seq: &[usize]) -> Vec<u64> {
    let mut tick = 0u64;
    seq.iter()
        .map(|&index| {
            let start = tick;
            let beats = score.playback_measure_beats(index);
            tick += (beats * f64::from(PPQ)).round().max(0.0) as u64;
            start
        })
        .collect()
}

fn warp_midi_tick(tick: u64, pauses: &[(u64, u64)]) -> u64 {
    let count = pauses.partition_point(|(at, _)| *at <= tick);
    tick.saturating_add(count.checked_sub(1).map_or(0, |index| pauses[index].1))
}

fn score_tempo_at(score: &Score, measure_index: usize) -> Option<u16> {
    score
        .parts
        .iter()
        .flat_map(|part| &part.staves)
        .find_map(|staff| {
            staff
                .measures
                .get(measure_index)
                .and_then(|measure| measure.tempo)
        })
}

/// MIDI key signature meta: sharps (positive) or flats and whether the key is minor.
fn midi_key_signature(key: &acorde_core::KeySignature) -> MetaMessage<'static> {
    MetaMessage::KeySignature(
        key.fifths.clamp(-7, 7),
        key.mode.eq_ignore_ascii_case("minor"),
    )
}

// ── meta track ────────────────────────────────────────────────────────────────

/// Convert a delta to the MIDI variable-length quantity limit (28 bits).
#[inline]
fn checked_delta(delta: u64) -> Result<u28, Error> {
    const MAX_DELTA: u64 = 0x0FFF_FFFF;
    u32::try_from(delta)
        .ok()
        .filter(|&value| u64::from(value) <= MAX_DELTA)
        .map(u28::from)
        .ok_or_else(|| Error::Midi(format!("MIDI delta {delta} exceeds 28-bit limit")))
}

fn build_meta_track<'a>(
    score: &Score,
    seq: &[usize],
    starts: &[u64],
) -> Result<Vec<TrackEvent<'a>>, Error> {
    let ts = &score.settings.time_signature;
    let den_log2 = (ts.denominator as u32).trailing_zeros() as u8;

    let first_staff_opt = score.parts.first().and_then(|p| p.staves.first());

    // If measure 0 carries a tempo override, use it as the initial tempo so we
    // don't emit two conflicting Tempo events at tick 0.
    let initial_bpm = seq
        .first()
        .and_then(|&index| score_tempo_at(score, index))
        .map(|b| b.max(1) as u32)
        .unwrap_or_else(|| score.settings.tempo_bpm.max(1) as u32);
    let us_per_beat = 60_000_000u32 / initial_bpm;

    let mut events: Vec<TrackEvent<'static>> = vec![
        TrackEvent {
            delta: u28::from(0u32),
            kind: TrackEventKind::Meta(MetaMessage::Tempo(u24::from(us_per_beat))),
        },
        TrackEvent {
            delta: u28::from(0u32),
            kind: TrackEventKind::Meta(MetaMessage::TimeSignature(ts.numerator, den_log2, 24, 8)),
        },
        TrackEvent {
            delta: u28::from(0u32),
            kind: TrackEventKind::Meta(midi_key_signature(
                first_staff_opt
                    .and_then(|staff| seq.first().and_then(|&i| staff.measures.get(i)))
                    .and_then(|measure| measure.key_sig.as_ref())
                    .unwrap_or(&score.settings.key_signature),
            )),
        },
    ];

    // Emit per-measure Tempo meta events for measures after the first.
    // Measure 0 is already covered by the initial event above.
    let mut prev_event_tick: u64 = 0;
    for (position, &idx) in seq.iter().enumerate() {
        let cursor_tick = starts.get(position).copied().unwrap_or(0);
        if cursor_tick > 0 {
            if let Some(bpm) = score_tempo_at(score, idx) {
                let us = 60_000_000u32 / bpm.max(1) as u32;
                events.push(TrackEvent {
                    delta: checked_delta(cursor_tick - prev_event_tick)?,
                    kind: TrackEventKind::Meta(MetaMessage::Tempo(u24::from(us))),
                });
                prev_event_tick = cursor_tick;
            }
            if let Some(measure) = first_staff_opt.and_then(|staff| staff.measures.get(idx)) {
                if let Some(time_sig) = &measure.time_sig {
                    let den_log2 = time_sig.denominator.trailing_zeros() as u8;
                    events.push(TrackEvent {
                        delta: checked_delta(cursor_tick - prev_event_tick)?,
                        kind: TrackEventKind::Meta(MetaMessage::TimeSignature(
                            time_sig.numerator,
                            den_log2,
                            24,
                            8,
                        )),
                    });
                    prev_event_tick = cursor_tick;
                }
                if let Some(key) = &measure.key_sig {
                    events.push(TrackEvent {
                        delta: checked_delta(cursor_tick - prev_event_tick)?,
                        kind: TrackEventKind::Meta(midi_key_signature(key)),
                    });
                    prev_event_tick = cursor_tick;
                }
            }
        }
    }

    events.push(TrackEvent {
        delta: u28::from(0u32),
        kind: TrackEventKind::Meta(MetaMessage::EndOfTrack),
    });
    Ok(events)
}

// ── part track ────────────────────────────────────────────────────────────────

#[derive(Clone)]
struct TimedEvent {
    abs_tick: u64,
    /// 0 = NoteOff, 1 = program/controller expression, 2 = NoteOn.
    sort_key: u8,
    channel: u4,
    message: MidiMessage,
}

struct MidiNoteCandidate {
    start_tick: u64,
    sounding_ticks: u64,
    pitch: u8,
    velocity: u8,
    tie_start: bool,
    tie_end: bool,
    partial_end: bool,
    source_location: Option<acorde_core::TieSourceLocation>,
    written_onset: f64,
    staff: usize,
    voice: usize,
    measure: usize,
    note: usize,
    channel: u8,
}

struct OpenMidiTie {
    off_event: usize,
    written_onset: f64,
    staff: usize,
    voice: usize,
    measure: usize,
    note: usize,
}

struct MidiNoteEventPair {
    on_event: usize,
    off_event: usize,
    pitch: u8,
}

fn note_ticks(duration: &acorde_core::Duration, dot_count: u8, tuplet: Option<&TupletInfo>) -> u64 {
    let base = duration.to_ticks(dot_count) as u64;
    match tuplet {
        Some(t) if t.actual_notes > 0 => {
            (base * t.normal_notes as u64 / t.actual_notes as u64).max(1)
        }
        _ => base,
    }
}

fn build_part_track(
    part: &acorde_core::Part,
    seq: &[usize],
    prefix: &[usize],
    starts: &[u64],
    pauses: &[(u64, u64)],
    occupied_channels: &mut BTreeSet<u8>,
) -> Result<Vec<TrackEvent<'static>>, Error> {
    let channel = u4::from(part.midi_channel.min(15));
    let mut events: Vec<TimedEvent> = Vec::new();
    let mut notes: Vec<MidiNoteCandidate> = Vec::new();
    // Distinct programs need distinct MIDI channels while voices overlap. Allocate only
    // programs actually referenced by switches, reserving all other parts' channels.
    let mut program_channels = std::collections::BTreeMap::new();
    if part.midi_program_changes.is_empty() {
        program_channels.insert(part.midi_program, channel.as_int());
    }
    let active_before_region = |staff: &acorde_core::Staff, voice: usize| {
        let mut active = None;
        for &index in prefix {
            if let Some(measure) = staff.measures.get(index) {
                let mut switches: Vec<_> = measure
                    .channel_switches
                    .iter()
                    .filter(|switch| switch.voice == voice && switch.position_beats.is_finite())
                    .collect();
                switches.sort_by(|a, b| a.position_beats.total_cmp(&b.position_beats));
                for switch in switches {
                    if let Some(named) = part
                        .named_channels
                        .iter()
                        .find(|named| named.name == switch.channel_name)
                    {
                        active = Some(named);
                    }
                }
            }
        }
        active
    };
    let initial = part
        .staves
        .iter()
        .flat_map(|staff| (0..4).filter_map(|voice| active_before_region(staff, voice)));
    let switched = part
        .staves
        .iter()
        .flat_map(|staff| seq.iter().filter_map(|&index| staff.measures.get(index)))
        .flat_map(|measure| &measure.channel_switches)
        .filter_map(|switch| {
            part.named_channels
                .iter()
                .find(|named| named.name == switch.channel_name)
        });
    for named in initial.chain(switched) {
        if program_channels.contains_key(&named.program) {
            continue;
        }
        let preferred = named.midi_channel;
        let assigned = if preferred != 9 && !occupied_channels.contains(&preferred) {
            preferred
        } else {
            (0..16)
                .find(|candidate| *candidate != 9 && !occupied_channels.contains(candidate))
                .ok_or_else(|| {
                    Error::Midi("no free MIDI channel for a named instrument channel".into())
                })?
        };
        occupied_channels.insert(assigned);
        program_channels.insert(named.program, assigned);
        events.push(TimedEvent {
            abs_tick: 0,
            sort_key: 1,
            channel: u4::from(assigned),
            message: MidiMessage::ProgramChange {
                program: u7::from(named.program.min(127)),
            },
        });
    }

    if part.midi_program_changes.is_empty() {
        // Always emit ProgramChange at tick 0 to initialize the channel's timbre.
        events.push(TimedEvent {
            abs_tick: 0,
            sort_key: 1,
            channel,
            message: MidiMessage::ProgramChange {
                program: u7::from(part.midi_program),
            },
        });
    } else {
        for program in &part.midi_program_changes {
            events.push(TimedEvent {
                abs_tick: program.tick,
                sort_key: 1,
                channel: u4::from(program.channel.min(15)),
                message: MidiMessage::ProgramChange {
                    program: u7::from(program.program.min(127)),
                },
            });
        }
    }

    for bend in &part.midi_pitch_bends {
        events.push(TimedEvent {
            abs_tick: bend.tick,
            sort_key: 1,
            channel: u4::from(bend.channel.min(15)),
            message: MidiMessage::PitchBend {
                bend: midly::PitchBend::from_int(bend.value),
            },
        });
    }

    for control in &part.midi_control_changes {
        events.push(TimedEvent {
            abs_tick: control.tick,
            sort_key: 1,
            channel: u4::from(control.channel.min(15)),
            message: MidiMessage::Controller {
                controller: u7::from(control.controller.min(127)),
                value: u7::from(control.value.min(127)),
            },
        });
    }

    for aftertouch in &part.midi_aftertouch {
        let message = match aftertouch.key {
            Some(key) => MidiMessage::Aftertouch {
                key: u7::from(key.min(127)),
                vel: u7::from(aftertouch.value.min(127)),
            },
            None => MidiMessage::ChannelAftertouch {
                vel: u7::from(aftertouch.value.min(127)),
            },
        };
        events.push(TimedEvent {
            abs_tick: aftertouch.tick,
            sort_key: 1,
            channel: u4::from(aftertouch.channel.min(15)),
            message,
        });
    }
    for staff in &part.staves {
        for (sequence_index, &measure_index) in seq.iter().enumerate() {
            let Some(measure) = staff.measures.get(measure_index) else {
                continue;
            };
            for direction in &measure.timed_pedals {
                if !direction.position_beats.is_finite() || direction.position_beats < 0.0 {
                    continue;
                }
                events.push(TimedEvent {
                    abs_tick: starts[sequence_index]
                        .saturating_add((direction.position_beats * f64::from(PPQ)).round() as u64),
                    sort_key: 1,
                    channel,
                    message: MidiMessage::Controller {
                        controller: u7::from(64),
                        value: u7::from(if direction.down { 127 } else { 0 }),
                    },
                });
            }
        }
    }
    for (staff_index, staff) in part.staves.iter().enumerate() {
        let dynamics = acorde_core::DynamicTimeline::for_staff(staff, seq);
        for voice_idx in 0..4usize {
            let mut cursor: u64 = 0;
            let mut active_program =
                active_before_region(staff, voice_idx).map(|named| named.program);
            for (seq_position, &idx) in seq.iter().enumerate() {
                let measure = match staff.measures.get(idx) {
                    Some(m) => m,
                    None => continue,
                };
                // Each bar's voice starts at its bar, whatever the previous bar held.
                cursor = starts.get(seq_position).copied().unwrap_or(cursor);
                let transpose = if part.midi_channel == 9 {
                    0i8
                } else {
                    staff.transpose_semitones
                };
                let mut written_beats = 0.0f64;
                let mut switches: Vec<_> = measure
                    .channel_switches
                    .iter()
                    .filter(|switch| switch.voice == voice_idx && switch.position_beats.is_finite())
                    .collect();
                switches.sort_by(|a, b| a.position_beats.total_cmp(&b.position_beats));
                let mut next_switch = 0;
                for (note_index, note) in measure.voices[voice_idx].iter().enumerate() {
                    if note.is_grace || note.is_cue {
                        continue;
                    }
                    let ticks = note_ticks(&note.duration, note.dot_count, note.tuplet.as_ref());
                    let onset_beats = written_beats;
                    while let Some(switch) = switches
                        .get(next_switch)
                        .filter(|switch| switch.position_beats <= onset_beats + 1e-9)
                    {
                        if let Some(named) = part
                            .named_channels
                            .iter()
                            .find(|named| named.name == switch.channel_name)
                        {
                            active_program = Some(named.program);
                        }
                        next_switch += 1;
                    }
                    let note_channel = active_program
                        .and_then(|program| program_channels.get(&program).copied())
                        .unwrap_or(channel.as_int());
                    written_beats += note.beats();
                    if !note.is_rest {
                        let mut vel = dynamics.velocity(seq_position, onset_beats, note.dynamic);
                        let mut sounding_ticks = ticks as f64;
                        for articulation in &note.articulations {
                            match articulation {
                                Articulation::Staccato | Articulation::Staccatissimo => {
                                    sounding_ticks *= 0.5;
                                }
                                Articulation::Accent | Articulation::Marcato => {
                                    vel = vel.saturating_add(20).min(127);
                                }
                                _ => {}
                            }
                        }
                        let sounding_ticks = sounding_ticks.round() as u64;
                        for (pitch_index, pitch) in note.pitches.iter().enumerate() {
                            // An unpitched note sounds its instrument's percussion key.
                            let midi = part.percussion_key(note, pitch_index).map_or_else(
                                || (pitch.to_midi() + transpose as i16).clamp(0, 127) as u8,
                                |key| key.min(127),
                            );
                            notes.push(MidiNoteCandidate {
                                start_tick: cursor,
                                sounding_ticks,
                                pitch: midi,
                                velocity: vel,
                                tie_start: note.pitch_tie_start(pitch_index),
                                tie_end: note.pitch_tie_end(pitch_index),
                                partial_end: note.pitch_incoming_partial_ties.get(pitch_index)
                                    == Some(&true),
                                source_location: note
                                    .pitch_tie_sources
                                    .get(pitch_index)
                                    .cloned()
                                    .flatten(),
                                written_onset: onset_beats,
                                staff: staff_index,
                                voice: voice_idx,
                                measure: idx,
                                note: note_index,
                                channel: note_channel,
                            });
                        }
                    }
                    cursor += ticks;
                }
                for switch in switches.iter().skip(next_switch) {
                    if let Some(named) = part
                        .named_channels
                        .iter()
                        .find(|named| named.name == switch.channel_name)
                    {
                        active_program = Some(named.program);
                    }
                }
            }
        }
    }

    // Resolve ties on the shared part clock, since MusicXML may move a continuation to another
    // voice or staff. Independent unison notes retain their own attacks and note-offs.
    notes.sort_by_key(|note| note.start_tick);
    let mut open_ties: std::collections::HashMap<u8, Vec<OpenMidiTie>> =
        std::collections::HashMap::new();
    let mut note_pairs = Vec::new();
    for note in notes {
        if let Some(candidates) = open_ties.get_mut(&note.pitch) {
            candidates.retain(|candidate| events[candidate.off_event].abs_tick >= note.start_tick);
            if note.tie_end || note.partial_end {
                let matching = candidates
                    .iter()
                    .enumerate()
                    .filter(|(_, candidate)| {
                        let jumped = note.measure != candidate.measure.saturating_add(1)
                            && (note.measure != candidate.measure || note.note <= candidate.note);
                        let target = acorde_core::NoteAddr {
                            part: 0,
                            staff: note.staff,
                            measure: note.measure,
                            voice: note.voice,
                            note: note.note,
                        };
                        let previous = acorde_core::NoteAddr {
                            part: 0,
                            staff: candidate.staff,
                            measure: candidate.measure,
                            voice: candidate.voice,
                            note: candidate.note,
                        };
                        let source_matches = note.source_location.as_ref().is_none_or(|location| {
                            location.matches(
                                &target,
                                &previous,
                                candidate.written_onset - note.written_onset,
                            )
                        });
                        let partial_matches = note.partial_end
                            && jumped
                            && candidate.staff == note.staff
                            && candidate.voice == note.voice;
                        source_matches
                            && (note.tie_end || partial_matches)
                            && events[candidate.off_event].abs_tick == note.start_tick
                    })
                    .max_by_key(|(_, candidate)| {
                        (candidate.staff == note.staff, candidate.voice == note.voice)
                    })
                    .map(|(position, _)| position);
                if let Some(position) = matching {
                    let mut candidate = candidates.remove(position);
                    events[candidate.off_event].abs_tick = note.start_tick + note.sounding_ticks;
                    if note.tie_start {
                        candidate.staff = note.staff;
                        candidate.voice = note.voice;
                        candidate.measure = note.measure;
                        candidate.note = note.note;
                        candidate.written_onset = note.written_onset;
                        candidates.push(candidate);
                    }
                    continue;
                }
            }
        }
        let on_event = events.len();
        events.push(TimedEvent {
            abs_tick: note.start_tick,
            sort_key: 2,
            channel: u4::from(note.channel),
            message: MidiMessage::NoteOn {
                key: u7::from(note.pitch),
                vel: u7::from(note.velocity),
            },
        });
        let off_event = events.len();
        events.push(TimedEvent {
            abs_tick: note.start_tick + note.sounding_ticks,
            sort_key: 0,
            channel: u4::from(note.channel),
            message: MidiMessage::NoteOff {
                key: u7::from(note.pitch),
                vel: u7::from(0u8),
            },
        });
        note_pairs.push(MidiNoteEventPair {
            on_event,
            off_event,
            pitch: note.pitch,
        });
        if note.tie_start {
            open_ties.entry(note.pitch).or_default().push(OpenMidiTie {
                off_event: events.len() - 1,
                written_onset: note.written_onset,
                staff: note.staff,
                voice: note.voice,
                measure: note.measure,
                note: note.note,
            });
        }
    }

    // Resolve ties in written time, then place all note and controller events on the
    // same fermata-adjusted clock before allocating channels for overlapping unisons.
    for event in &mut events {
        event.abs_tick = warp_midi_tick(event.abs_tick, pauses);
    }
    let base_messages: Vec<_> = events
        .iter()
        .filter(|event| {
            event.channel == channel
                && event.sort_key == 1
                && !matches!(event.message, MidiMessage::ProgramChange { .. })
        })
        .cloned()
        .collect();
    for &alternate in program_channels
        .values()
        .filter(|&&value| value != channel.as_int())
    {
        for message in &base_messages {
            let mut copy = message.clone();
            copy.channel = u4::from(alternate);
            events.push(copy);
        }
    }
    let channel_messages = events
        .iter()
        .filter(|event| event.sort_key == 1)
        .cloned()
        .collect::<Vec<_>>();
    let mut last_off = std::collections::BTreeMap::<(u8, u8), u64>::new();
    let mut spare_channels = Vec::<(u8, u8)>::new();
    for pair in note_pairs {
        let start = events[pair.on_event].abs_tick;
        let end = events[pair.off_event].abs_tick;
        let preferred = events[pair.on_event].channel.as_int();
        let free_for_pitch = |candidate: u8| {
            last_off
                .get(&(candidate, pair.pitch))
                .is_none_or(|&off| off <= start)
        };
        let assigned = if free_for_pitch(preferred) {
            preferred
        } else if let Some(&(_, spare)) = spare_channels
            .iter()
            .find(|&&(owner, spare)| owner == preferred && free_for_pitch(spare))
        {
            spare
        } else {
            if preferred == 9 {
                return Err(Error::Midi(
                    "overlapping percussion pitch has no independent MIDI voice".into(),
                ));
            }
            let spare = (0..16)
                .find(|candidate| *candidate != 9 && !occupied_channels.contains(candidate))
                .ok_or_else(|| Error::Midi("no free MIDI channel for overlapping pitch".into()))?;
            occupied_channels.insert(spare);
            spare_channels.push((preferred, spare));
            spare
        };
        events[pair.on_event].channel = u4::from(assigned);
        events[pair.off_event].channel = u4::from(assigned);
        last_off.insert((assigned, pair.pitch), end);
    }
    for &(owner, spare) in &spare_channels {
        for message in channel_messages
            .iter()
            .filter(|message| message.channel.as_int() == owner)
        {
            let mut copy = message.clone();
            copy.channel = u4::from(spare);
            events.push(copy);
        }
    }

    // Release, update expression, then attack at the same tick.
    events.sort_by_key(|e| (e.abs_tick, e.sort_key));

    let mut track: Vec<TrackEvent<'static>> =
        Vec::with_capacity(events.len() + spare_channels.len() + 1);
    for (owner, spare) in spare_channels {
        track.push(TrackEvent {
            delta: u28::from(0u32),
            kind: TrackEventKind::Meta(MetaMessage::SequencerSpecific(
                &AUX_CHANNEL_MARKERS[owner as usize][spare as usize],
            )),
        });
    }
    let mut prev: u64 = 0;
    for ev in events {
        track.push(TrackEvent {
            delta: checked_delta(ev.abs_tick - prev)?,
            kind: TrackEventKind::Midi {
                channel: ev.channel,
                message: ev.message,
            },
        });
        prev = ev.abs_tick;
    }
    track.push(TrackEvent {
        delta: u28::from(0u32),
        kind: TrackEventKind::Meta(MetaMessage::EndOfTrack),
    });
    Ok(track)
}

#[cfg(test)]
mod tests {
    use super::*;
    use acorde_core::{Duration, MeasureLength, Note, Pitch, Score, Step};

    fn note_schedule(bytes: &[u8]) -> Vec<(u64, bool, u8, u8)> {
        let smf = Smf::parse(bytes).expect("valid MIDI");
        let mut schedule = Vec::new();
        for track in smf.tracks.iter().skip(1) {
            let mut tick = 0u64;
            for event in track {
                tick += u64::from(event.delta.as_int());
                if let TrackEventKind::Midi { message, .. } = &event.kind {
                    match message {
                        MidiMessage::NoteOn { key, vel } if vel.as_int() > 0 => {
                            schedule.push((tick, true, key.as_int(), vel.as_int()));
                        }
                        MidiMessage::NoteOff { key, .. } => {
                            schedule.push((tick, false, key.as_int(), 0));
                        }
                        _ => {}
                    }
                }
            }
        }
        schedule.sort_unstable();
        schedule
    }

    #[test]
    fn midi_delta_rejects_values_above_vlq_limit() {
        assert!(checked_delta(0x0FFF_FFFF).is_ok());
        assert!(checked_delta(0x1000_0000).is_err());
    }

    #[test]
    fn header_magic_bytes() {
        let score = Score::default();
        let bytes = serialize_midi(&score).unwrap();
        assert_eq!(&bytes[0..4], b"MThd");
    }

    #[test]
    fn single_note_roundtrip() {
        let mut score = Score::new("T", 120, 4, 4, 0, 1);
        score.parts[0].staves[0].measures[0].voices[0] =
            vec![Note::new(Pitch::new(Step::C, 4), Duration::Quarter)];
        let bytes = serialize_midi(&score).unwrap();
        let score2 = acorde_io_parse_midi(&bytes);
        let notes: Vec<_> = score2.parts[0].staves[0].measures[0].voices[0]
            .iter()
            .filter(|n| !n.is_rest)
            .collect();
        assert!(!notes.is_empty());
        assert_eq!(notes[0].pitches[0].step, Step::C);
    }

    #[test]
    fn pitch_bend_roundtrip_preserves_tick_channel_and_value() {
        let mut score = Score::new("Bend", 120, 4, 4, 0, 1);
        score.parts[0].midi_channel = 2;
        score.parts[0].midi_pitch_bends = vec![acorde_core::MidiPitchBend {
            tick: 120,
            channel: 2,
            value: 2048,
        }];
        score.parts[0].staves[0].measures[0].voices[0] =
            vec![Note::new(Pitch::new(Step::C, 4), Duration::Quarter)];
        let bytes = serialize_midi(&score).expect("MIDI serialize failed");
        let restored = acorde_io_parse_midi(&bytes);
        assert_eq!(
            restored.parts[0].midi_pitch_bends,
            score.parts[0].midi_pitch_bends
        );
    }

    #[test]
    fn pitch_bend_serializer_accepts_signed_14_bit_boundaries() {
        let mut score = Score::new("Bend bounds", 120, 4, 4, 0, 1);
        score.parts[0].midi_pitch_bends = vec![
            acorde_core::MidiPitchBend {
                tick: 0,
                channel: 0,
                value: -8192,
            },
            acorde_core::MidiPitchBend {
                tick: 10,
                channel: 0,
                value: 8191,
            },
        ];
        score.parts[0].staves[0].measures[0].voices[0] =
            vec![Note::new(Pitch::new(Step::C, 4), Duration::Quarter)];
        let bytes = serialize_midi(&score).expect("MIDI serialize failed");
        let restored = acorde_io_parse_midi(&bytes);
        assert_eq!(
            restored.parts[0].midi_pitch_bends,
            score.parts[0].midi_pitch_bends
        );
    }

    #[test]
    fn pitch_bend_serializer_rejects_out_of_range_canonical_values() {
        let mut score = Score::new("Invalid bend", 120, 4, 4, 0, 1);
        score.parts[0].midi_pitch_bends = vec![acorde_core::MidiPitchBend {
            tick: 0,
            channel: 0,
            value: 8192,
        }];
        let error = serialize_midi(&score).expect_err("invalid pitch-bend must be rejected");
        assert!(error.to_string().contains("outside -8192..=8191"));
    }

    #[test]
    fn controller_change_roundtrip_preserves_tick_channel_and_value() {
        let mut score = Score::new("CC", 120, 4, 4, 0, 1);
        score.parts[0].midi_control_changes = vec![acorde_core::MidiControlChange {
            tick: 240,
            channel: 3,
            controller: 64,
            value: 127,
        }];
        score.parts[0].staves[0].measures[0].voices[0] =
            vec![Note::new(Pitch::new(Step::C, 4), Duration::Quarter)];
        let bytes = serialize_midi(&score).expect("MIDI serialize failed");
        let restored = acorde_io_parse_midi(&bytes);
        assert_eq!(
            restored.parts[0].midi_control_changes,
            score.parts[0].midi_control_changes
        );
    }

    #[test]
    fn program_change_roundtrip_preserves_multiple_ticks_and_channels() {
        let mut score = Score::new("Programs", 120, 4, 4, 0, 1);
        score.parts[0].midi_program_changes = vec![
            acorde_core::MidiProgramChange {
                tick: 0,
                channel: 0,
                program: 0,
            },
            acorde_core::MidiProgramChange {
                tick: 960,
                channel: 0,
                program: 40,
            },
        ];
        score.parts[0].staves[0].measures[0].voices[0] =
            vec![Note::new(Pitch::new(Step::C, 4), Duration::Quarter)];
        let bytes = serialize_midi(&score).expect("MIDI serialize failed");
        let restored = acorde_io_parse_midi(&bytes);
        assert_eq!(
            restored.parts[0].midi_program_changes,
            score.parts[0].midi_program_changes
        );
    }

    #[test]
    fn aftertouch_roundtrip_preserves_key_and_channel_pressure() {
        let mut score = Score::new("Aftertouch", 120, 4, 4, 0, 1);
        score.parts[0].midi_aftertouch = vec![
            acorde_core::MidiAftertouch {
                tick: 120,
                channel: 2,
                key: Some(60),
                value: 80,
            },
            acorde_core::MidiAftertouch {
                tick: 240,
                channel: 2,
                key: None,
                value: 40,
            },
        ];
        score.parts[0].staves[0].measures[0].voices[0] =
            vec![Note::new(Pitch::new(Step::C, 4), Duration::Quarter)];
        let bytes = serialize_midi(&score).expect("MIDI serialize failed");
        let restored = acorde_io_parse_midi(&bytes);
        assert_eq!(
            restored.parts[0].midi_aftertouch,
            score.parts[0].midi_aftertouch
        );
    }

    // Helper to avoid circular dependency in tests — parse via the parent module.
    fn acorde_io_parse_midi(data: &[u8]) -> Score {
        super::super::parse_midi(data).expect("parse_midi failed")
    }

    #[test]
    fn chord_produces_bytes() {
        let mut score = Score::new("T", 120, 4, 4, 0, 1);
        let mut note = Note::new(Pitch::new(Step::C, 4), Duration::Quarter);
        note.pitches.push(Pitch::new(Step::E, 4));
        score.parts[0].staves[0].measures[0].voices[0] = vec![note];
        let bytes = serialize_midi(&score).unwrap();
        assert!(bytes.len() > 20);
    }

    #[test]
    fn tempo_120_bpm_encoded() {
        // 120 BPM = 500000 µs/beat = 0x07A120
        let score = Score::new("T", 120, 4, 4, 0, 1);
        let bytes = serialize_midi(&score).unwrap();
        assert!(bytes.windows(3).any(|w| w == [0x07, 0xA1, 0x20]));
    }

    #[test]
    fn empty_parts_produce_valid_midi() {
        let score = Score::new("T", 120, 4, 4, 0, 0);
        let bytes = serialize_midi(&score).unwrap();
        assert_eq!(&bytes[0..4], b"MThd");
    }

    #[test]
    fn program_change_present_in_track() {
        // ProgramChange for channel 0 is encoded as 0xC0 followed by program byte.
        let score = Score::default();
        let bytes = serialize_midi(&score).unwrap();
        // 0xC0 = ProgramChange on channel 0
        assert!(bytes.windows(2).any(|w| w[0] == 0xC0 && w[1] == 0));
    }

    #[test]
    fn non_zero_program_encoded() {
        // Program 40 = Violin in General MIDI. Expect 0xC0 0x28.
        let mut score = Score::default();
        score.parts[0].midi_program = 40;
        let bytes = serialize_midi(&score).unwrap();
        assert!(bytes.windows(2).any(|w| w[0] == 0xC0 && w[1] == 40));
    }

    #[test]
    fn transpose_semitones_shifts_midi_note() {
        // Written C4 (midi=60) with transpose_semitones=-2 → concert Bb3 (midi=58).
        let mut score = Score::new("T", 120, 4, 4, 0, 1);
        score.parts[0].staves[0].transpose_semitones = -2;
        score.parts[0].staves[0].measures[0].voices[0] =
            vec![Note::new(Pitch::new(Step::C, 4), Duration::Quarter)];
        let bytes = serialize_midi(&score).unwrap();
        // 0x90 = NoteOn ch0; next byte is key. 58 = 0x3A.
        assert!(bytes.windows(2).any(|w| w[0] == 0x90 && w[1] == 58));
    }

    #[test]
    fn percussion_channel_9_ignores_transpose_semitones() {
        // Channel 9 = GM percussion; NoteOn on ch9 (0x99) must use raw pitch 60, not 58.
        let mut score = Score::new("T", 120, 4, 4, 0, 1);
        score.parts[0].midi_channel = 9;
        score.parts[0].staves[0].transpose_semitones = -2;
        score.parts[0].staves[0].measures[0].voices[0] =
            vec![Note::new(Pitch::new(Step::C, 4), Duration::Quarter)];
        let bytes = serialize_midi(&score).unwrap();
        // 0x99 = NoteOn ch9; next byte must be 60 (C4 unchanged), not 58.
        assert!(bytes.windows(2).any(|w| w[0] == 0x99 && w[1] == 60));
        assert!(!bytes.windows(2).any(|w| w[0] == 0x99 && w[1] == 58));
    }

    #[test]
    fn midi_serialize_includes_tempo_change() {
        // Second measure has tempo=60 BPM. MIDI should contain 0x0F4240 (60 BPM = 1,000,000 µs).
        let mut score = Score::new("T", 120, 4, 4, 0, 2);
        score.parts[0].staves[0].measures[1].tempo = Some(60);
        let bytes = serialize_midi(&score).unwrap();
        // 60 BPM = 1,000,000 µs = 0x0F4240
        assert!(bytes.windows(3).any(|w| w == [0x0F, 0x42, 0x40]));
    }

    #[test]
    fn midi_tempo_on_other_part_governs_all_tracks() {
        let mut score = Score::new("shared tempo", 120, 4, 4, 0, 2);
        let mut second = score.parts[0].clone();
        second.id = "second".into();
        score.parts.push(second);
        score.parts[1].staves[0].measures[0].tempo = Some(92);
        score.parts[1].staves[0].measures[1].tempo = Some(60);
        let bytes = serialize_midi(&score).unwrap();
        let smf = Smf::parse(&bytes).unwrap();
        let mut tick = 0u64;
        let mut tempos = Vec::new();
        for event in &smf.tracks[0] {
            tick += u64::from(event.delta.as_int());
            if let TrackEventKind::Meta(MetaMessage::Tempo(us)) = event.kind {
                tempos.push((tick, us.as_int()));
            }
        }
        assert_eq!(tempos, vec![(0, 652_173), (1_920, 1_000_000)]);
    }

    #[test]
    fn midi_uses_irregular_length_written_on_other_part() {
        let mut score = Score::new("shared bar", 120, 4, 4, 0, 2);
        let mut second = score.parts[0].clone();
        second.id = "second".into();
        score.parts.push(second);
        score.parts[1].staves[0].measures[0].actual_length = Some(MeasureLength {
            numerator: 1,
            denominator: 4,
        });
        score.parts[0].staves[0].measures[1].voices[0] =
            vec![Note::new(Pitch::new(Step::C, 4), Duration::Quarter)];
        let schedule = note_schedule(&serialize_midi(&score).unwrap());
        assert!(schedule.contains(&(480, true, 60, 64)));
    }

    #[test]
    fn midi_cue_is_silent_and_does_not_advance_the_voice() {
        let mut score = Score::new("cue", 120, 4, 4, 0, 1);
        let mut cue = Note::new(Pitch::new(Step::C, 4), Duration::Quarter);
        cue.is_cue = true;
        score.parts[0].staves[0].measures[0].voices[0] =
            vec![cue, Note::new(Pitch::new(Step::D, 4), Duration::Quarter)];
        let schedule = note_schedule(&serialize_midi(&score).unwrap());
        assert_eq!(schedule, vec![(0, true, 62, 64), (480, false, 62, 0)]);
    }

    #[test]
    fn fermata_delays_midi_notes_and_measure_metadata_on_each_repeat() {
        use acorde_core::Barline;
        let mut score = Score::new("fermata", 120, 3, 4, 0, 2);
        let first = &mut score.parts[0].staves[0].measures[0];
        first.barline_left = Barline::RepeatStart;
        let mut held = Note::new(Pitch::new(Step::C, 4), Duration::Quarter);
        held.articulations.push(Articulation::Fermata);
        first.voices[0] = vec![held, Note::new(Pitch::new(Step::D, 4), Duration::Half)];
        score.parts[0].staves[0].measures[1].barline_right = Barline::RepeatEnd;
        score.parts[0].staves[0].measures[1].tempo = Some(90);
        let mut lower = score.parts[0].staves[0].clone();
        lower.measures[0].voices[0] = vec![
            Note::new(Pitch::new(Step::C, 3), Duration::Half),
            Note::rest(Duration::Quarter),
        ];
        lower.measures[1].voices[0] = vec![Note::rest(Duration::Half)];
        score.parts[0].staves.push(lower);
        let bytes = serialize_midi(&score).unwrap();
        let schedule = note_schedule(&bytes);
        for event in [
            (0, true, 60, 64),
            (720, false, 60, 0),
            (720, true, 62, 64),
            (1200, false, 48, 0),
        ] {
            assert!(schedule.contains(&event), "missing {event:?}: {schedule:?}");
        }
        let midi = Smf::parse(&bytes).unwrap();
        let mut tick = 0u64;
        let tempos: Vec<u64> = midi.tracks[0]
            .iter()
            .filter_map(|event| {
                tick += u64::from(event.delta.as_int());
                matches!(event.kind, TrackEventKind::Meta(MetaMessage::Tempo(_))).then_some(tick)
            })
            .collect();
        assert!(
            tempos.contains(&1680),
            "tempo did not follow first held bar: {tempos:?}"
        );
        assert!(
            schedule.contains(&(3120, true, 60, 64)),
            "repeat did not follow hold: {schedule:?}"
        );
    }

    #[test]
    fn midi_default_articulation_matches_playback_duration_and_velocity() {
        let mut score = Score::new("articulations", 120, 4, 4, 0, 1);
        let mut fermata = Note::new(Pitch::new(Step::C, 4), Duration::Quarter);
        fermata.articulations.push(Articulation::Fermata);
        let mut staccato = Note::new(Pitch::new(Step::D, 4), Duration::Quarter);
        staccato.articulations.push(Articulation::Staccato);
        let mut accent = Note::new(Pitch::new(Step::E, 4), Duration::Quarter);
        accent.articulations.push(Articulation::Accent);
        score.parts[0].staves[0].measures[0].voices[0] = vec![fermata, staccato, accent];
        let schedule = note_schedule(&serialize_midi(&score).unwrap());
        assert_eq!(
            schedule,
            vec![
                (0, true, 60, 64),
                (720, false, 60, 0),
                (720, true, 62, 64),
                (960, false, 62, 0),
                (1200, true, 64, 84),
                (1680, false, 64, 0),
            ]
        );
    }

    #[test]
    fn overlapping_unison_uses_spare_channel_with_program_and_controls() {
        let mut score = Score::new("unison", 120, 4, 4, 0, 1);
        score.parts[0].midi_program = 21;
        score.parts[0].midi_control_changes = vec![
            acorde_core::MidiControlChange {
                tick: 0,
                channel: 0,
                controller: 7,
                value: 90,
            },
            acorde_core::MidiControlChange {
                tick: 480,
                channel: 0,
                controller: 7,
                value: 70,
            },
        ];
        let held = Note::new(Pitch::new(Step::C, 4), Duration::Half);
        score.parts[0].staves[0].measures[0].voices[0] = vec![held];
        score.parts[0].staves[0].measures[0].voices[1] = vec![
            Note::rest(Duration::Quarter),
            Note::new(Pitch::new(Step::C, 4), Duration::Quarter),
        ];
        let mut other = score.parts[0].clone();
        other.id = "reserved-channel".into();
        other.midi_channel = 1;
        other.midi_control_changes.clear();
        for voice in &mut other.staves[0].measures[0].voices {
            voice.clear();
        }
        score.parts.push(other);

        let bytes = serialize_midi(&score).unwrap();
        let mut smf = Smf::parse(&bytes).unwrap();
        let mut tick = 0_u64;
        let mut notes = Vec::new();
        let mut spare_program = false;
        let mut spare_volume = false;
        let mut spare_attack_index = None;
        let mut coincident_control_index = None;
        for (index, event) in smf.tracks[1].iter().enumerate() {
            tick += u64::from(event.delta.as_int());
            if let TrackEventKind::Midi { channel, message } = event.kind {
                match message {
                    MidiMessage::NoteOn { key, vel } if vel.as_int() > 0 => {
                        notes.push((tick, true, channel.as_int(), key.as_int()));
                        if tick == 480 && channel.as_int() == 2 {
                            spare_attack_index = Some(index);
                        }
                    }
                    MidiMessage::NoteOff { key, .. } => {
                        notes.push((tick, false, channel.as_int(), key.as_int()));
                    }
                    MidiMessage::ProgramChange { program } if channel.as_int() == 2 => {
                        spare_program = program.as_int() == 21;
                    }
                    MidiMessage::Controller { controller, value }
                        if channel.as_int() == 2 && controller.as_int() == 7 =>
                    {
                        spare_volume |= value.as_int() == 90;
                        if tick == 480 && value.as_int() == 70 {
                            coincident_control_index = Some(index);
                        }
                    }
                    _ => {}
                }
            }
        }
        assert_eq!(
            notes,
            vec![
                (0, true, 0, 60),
                (480, true, 2, 60),
                (960, false, 0, 60),
                (960, false, 2, 60),
            ]
        );
        assert!(spare_program && spare_volume);
        assert!(coincident_control_index.unwrap() < spare_attack_index.unwrap());
        let restored = crate::parse_midi(&bytes).unwrap();
        assert_eq!(
            restored.parts[0].midi_control_changes,
            score.parts[0].midi_control_changes
        );
        assert_eq!(restored.parts[0].midi_program, score.parts[0].midi_program);
        for event in &mut smf.tracks[1] {
            if let TrackEventKind::Midi {
                channel,
                message: MidiMessage::Controller { controller, value },
            } = &mut event.kind
                && channel.as_int() == 2
                && controller.as_int() == 7
            {
                *value = u7::from(91);
            }
        }
        let mut edited = Vec::new();
        smf.write_std(&mut edited).unwrap();
        let independently_edited = crate::parse_midi(&edited).unwrap();
        assert!(
            independently_edited.parts[0]
                .midi_control_changes
                .iter()
                .any(|control| control.channel == 2 && control.value == 91)
        );
    }

    #[test]
    fn overlapping_unison_rejects_exhausted_channel_pool() {
        let mut score = Score::new("no spare channel", 120, 4, 4, 0, 1);
        let held = Note::new(Pitch::new(Step::C, 4), Duration::Half);
        score.parts[0].staves[0].measures[0].voices[0] = vec![held];
        score.parts[0].staves[0].measures[0].voices[1] = vec![
            Note::rest(Duration::Quarter),
            Note::new(Pitch::new(Step::C, 4), Duration::Quarter),
        ];
        score.parts[0].midi_control_changes = (1..16)
            .map(|channel| acorde_core::MidiControlChange {
                tick: 0,
                channel,
                controller: 7,
                value: 100,
            })
            .collect();
        let error = serialize_midi(&score).expect_err("no spare channel is available");
        assert!(error.to_string().contains("no free MIDI channel"));
    }

    #[cfg(feature = "musicxml")]
    #[test]
    fn just_for_today_midi_durations_match_playback_events() {
        use std::collections::{BTreeMap, VecDeque};

        let source =
            include_bytes!("../../../../tests/fixtures/openscore_lieder_just_for_today.mxl");
        let score = crate::parse_mxl(source).unwrap();
        let mut expected = BTreeMap::<(u64, u8, u64), usize>::new();
        for event in acorde_core::to_playback_events(&score, &Default::default()) {
            if event.is_metronome {
                continue;
            }
            let onset = (event.time_beats * f64::from(PPQ)).round() as u64;
            let duration = (event.duration_beats * f64::from(PPQ)).round() as u64;
            *expected
                .entry((onset, event.pitch_midi, duration))
                .or_default() += 1;
        }

        let bytes = serialize_midi(&score).unwrap();
        let smf = Smf::parse(&bytes).unwrap();
        let mut actual = BTreeMap::<(u64, u8, u64), usize>::new();
        for track in smf.tracks.iter().skip(1) {
            let mut tick = 0_u64;
            let mut active = BTreeMap::<(u8, u8), VecDeque<u64>>::new();
            for event in track {
                tick += u64::from(event.delta.as_int());
                if let TrackEventKind::Midi { channel, message } = event.kind {
                    match message {
                        MidiMessage::NoteOn { key, vel } if vel.as_int() > 0 => {
                            active
                                .entry((channel.as_int(), key.as_int()))
                                .or_default()
                                .push_back(tick);
                        }
                        MidiMessage::NoteOff { key, .. } | MidiMessage::NoteOn { key, .. } => {
                            let onset = active
                                .get_mut(&(channel.as_int(), key.as_int()))
                                .and_then(VecDeque::pop_front)
                                .expect("note-off matches a note-on on its channel");
                            *actual
                                .entry((onset, key.as_int(), tick - onset))
                                .or_default() += 1;
                        }
                        _ => {}
                    }
                }
            }
            assert!(active.values().all(VecDeque::is_empty));
        }
        assert_eq!(expected.values().sum::<usize>(), 579);
        assert_eq!(actual, expected);
        let restored = crate::parse_midi(&bytes).unwrap();
        let reexported = serialize_midi(&restored).unwrap();
        let automation_count = |data: &[u8]| {
            let midi = Smf::parse(data).unwrap();
            midi.tracks
                .iter()
                .flat_map(|track| track.iter())
                .filter(|event| {
                    matches!(
                        event.kind,
                        TrackEventKind::Midi {
                            message: MidiMessage::Controller { .. }
                                | MidiMessage::PitchBend { .. }
                                | MidiMessage::ProgramChange { .. }
                                | MidiMessage::Aftertouch { .. }
                                | MidiMessage::ChannelAftertouch { .. },
                            ..
                        }
                    )
                })
                .count()
        };
        assert_eq!(automation_count(&reexported), automation_count(&bytes));
    }

    #[test]
    fn midi_import_preserves_program_change() {
        // Serialize with midi_program=40 (Violin) then parse back — program must survive.
        let mut score = Score::new("T", 120, 4, 4, 0, 1);
        score.parts[0].midi_program = 40;
        score.parts[0].midi_channel = 2;
        score.parts[0].staves[0].measures[0].voices[0] =
            vec![Note::new(Pitch::new(Step::C, 4), Duration::Quarter)];
        let bytes = serialize_midi(&score).unwrap();
        let score2 = acorde_io_parse_midi(&bytes);
        assert_eq!(score2.parts[0].midi_program, 40);
        assert_eq!(score2.parts[0].midi_channel, 2);
    }

    // ── serialize_midi_region ─────────────────────────────────────────────────

    #[test]
    fn serialize_midi_region_all_measures_matches_full() {
        // region(0, N-1) should produce identical bytes to serialize_midi
        let mut score = Score::new("T", 120, 4, 4, 0, 3);
        score.parts[0].staves[0].measures[0].voices[0] =
            vec![Note::new(Pitch::new(Step::C, 4), Duration::Quarter)];
        let full = serialize_midi(&score).unwrap();
        let region = serialize_midi_region(&score, (0, 2)).unwrap();
        assert_eq!(full, region);
    }

    #[test]
    fn serialize_midi_region_single_measure_excludes_other() {
        // 2 measures: measure 0 has C4, measure 1 has E4.
        // region(0,0) should contain NoteOn for C4 (midi=60) but NOT for E4 (midi=64).
        let mut score = Score::new("T", 120, 4, 4, 0, 2);
        score.parts[0].staves[0].measures[0].voices[0] =
            vec![Note::new(Pitch::new(Step::C, 4), Duration::Quarter)];
        score.parts[0].staves[0].measures[1].voices[0] =
            vec![Note::new(Pitch::new(Step::E, 4), Duration::Quarter)];
        let bytes = serialize_midi_region(&score, (0, 0)).unwrap();
        // 0x90 = NoteOn ch0; check for C4 (midi=60) present and E4 (midi=64) absent
        assert!(
            bytes.windows(2).any(|w| w[0] == 0x90 && w[1] == 60),
            "C4 should be present"
        );
        assert!(
            !bytes.windows(2).any(|w| w[0] == 0x90 && w[1] == 64),
            "E4 should be absent"
        );
    }

    #[test]
    fn serialize_midi_region_invalid_range_returns_err() {
        let score = Score::new("T", 120, 4, 4, 0, 2);
        let result = serialize_midi_region(&score, (2, 0));
        assert!(result.is_err(), "start > end should return Err");
    }

    #[test]
    fn serialize_midi_region_out_of_bounds_returns_err() {
        // Score has 4 measures (indices 0-3). Region [99,99] has no measures.
        let score = Score::default();
        let result = serialize_midi_region(&score, (99, 99));
        assert!(result.is_err(), "non-existent region should return Err");
    }
}
