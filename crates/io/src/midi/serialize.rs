use crate::Error;
use acorde_core::{Score, TupletInfo};
use midly::{
    Format, Header, MetaMessage, MidiMessage, Smf, Timing, TrackEvent, TrackEventKind,
    num::{u4, u7, u15, u24, u28},
};

const PPQ: u32 = 480;

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
    let header = Header::new(Format::Parallel, Timing::Metrical(u15::from(PPQ as u16)));
    let mut smf = Smf::new(header);

    smf.tracks.push(build_meta_track(score, seq));
    for part in &score.parts {
        smf.tracks.push(build_part_track(part, seq));
    }

    let mut bytes = Vec::new();
    smf.write_std(&mut bytes)
        .map_err(|e| Error::Midi(e.to_string()))?;
    Ok(bytes)
}

// ── meta track ────────────────────────────────────────────────────────────────

/// Clamp a u64 delta to the MIDI variable-length quantity limit (28 bits = 0x0FFF_FFFF).
#[inline]
fn clamp_delta(delta: u64) -> u28 {
    u28::from(delta.min(0x0FFF_FFFF) as u32)
}

fn build_meta_track<'a>(score: &Score, seq: &[usize]) -> Vec<TrackEvent<'a>> {
    let ts = &score.settings.time_signature;
    let den_log2 = (ts.denominator as u32).trailing_zeros() as u8;
    let beats_per_measure = ts.numerator as f64 * 4.0 / ts.denominator as f64;
    let ticks_per_measure = (beats_per_measure * PPQ as f64) as u64;

    let first_staff_opt = score.parts.first().and_then(|p| p.staves.first());

    // If measure 0 carries a tempo override, use it as the initial tempo so we
    // don't emit two conflicting Tempo events at tick 0.
    let initial_bpm = first_staff_opt
        .and_then(|s| {
            seq.first()
                .and_then(|&i| s.measures.get(i))
                .and_then(|m| m.tempo)
        })
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
    ];

    // Emit per-measure Tempo meta events for measures after the first.
    // Measure 0 is already covered by the initial event above.
    if let Some(first_staff) = first_staff_opt {
        let mut cursor_tick: u64 = 0;
        let mut prev_event_tick: u64 = 0;

        for &idx in seq {
            if cursor_tick > 0
                && let Some(bpm) = first_staff.measures.get(idx).and_then(|m| m.tempo)
            {
                let us = 60_000_000u32 / bpm.max(1) as u32;
                events.push(TrackEvent {
                    delta: clamp_delta(cursor_tick - prev_event_tick),
                    kind: TrackEventKind::Meta(MetaMessage::Tempo(u24::from(us))),
                });
                prev_event_tick = cursor_tick;
            }
            cursor_tick += ticks_per_measure;
        }
    }

    events.push(TrackEvent {
        delta: u28::from(0u32),
        kind: TrackEventKind::Meta(MetaMessage::EndOfTrack),
    });
    events
}

// ── part track ────────────────────────────────────────────────────────────────

struct TimedEvent {
    abs_tick: u64,
    /// 0 = NoteOff (sort before NoteOn at same tick), 1 = NoteOn
    sort_key: u8,
    channel: u4,
    message: MidiMessage,
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

fn build_part_track(part: &acorde_core::Part, seq: &[usize]) -> Vec<TrackEvent<'static>> {
    let channel = u4::from(part.midi_channel.min(15));
    let mut events: Vec<TimedEvent> = Vec::new();

    // Always emit ProgramChange at tick 0 to initialize the channel's timbre.
    events.push(TimedEvent {
        abs_tick: 0,
        sort_key: 0,
        channel,
        message: MidiMessage::ProgramChange {
            program: u7::from(part.midi_program),
        },
    });

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

    for staff in &part.staves {
        for voice_idx in 0..4usize {
            let mut cursor: u64 = 0;
            for &idx in seq {
                let measure = match staff.measures.get(idx) {
                    Some(m) => m,
                    None => continue,
                };
                let transpose = if part.midi_channel == 9 {
                    0i8
                } else {
                    staff.transpose_semitones
                };
                for note in &measure.voices[voice_idx] {
                    let ticks = note_ticks(&note.duration, note.dot_count, note.tuplet.as_ref());
                    if !note.is_rest && !note.is_grace {
                        let vel = note
                            .dynamic
                            .as_ref()
                            .map(|d| d.to_velocity())
                            .unwrap_or(64u8);
                        for pitch in &note.pitches {
                            let midi = (pitch.to_midi() + transpose as i16).clamp(0, 127) as u8;
                            events.push(TimedEvent {
                                abs_tick: cursor,
                                sort_key: 1,
                                channel,
                                message: MidiMessage::NoteOn {
                                    key: u7::from(midi),
                                    vel: u7::from(vel),
                                },
                            });
                            events.push(TimedEvent {
                                abs_tick: cursor + ticks,
                                sort_key: 0,
                                channel,
                                message: MidiMessage::NoteOff {
                                    key: u7::from(midi),
                                    vel: u7::from(0u8),
                                },
                            });
                        }
                    }
                    if !note.is_grace {
                        cursor += ticks;
                    }
                }
            }
        }
    }

    // NoteOff (sort_key=0) before NoteOn (sort_key=1) at the same tick
    events.sort_by_key(|e| (e.abs_tick, e.sort_key));

    let mut track: Vec<TrackEvent<'static>> = Vec::with_capacity(events.len() + 1);
    let mut prev: u64 = 0;
    for ev in events {
        track.push(TrackEvent {
            delta: clamp_delta(ev.abs_tick - prev),
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
    track
}

#[cfg(test)]
mod tests {
    use super::*;
    use acorde_core::{Duration, Note, Pitch, Score, Step};

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
