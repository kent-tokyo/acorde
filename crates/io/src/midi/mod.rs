mod serialize;
pub use serialize::{serialize_midi, serialize_midi_region};

use crate::{Error, MAX_INPUT_BYTES, MAX_MIDI_EVENTS};
use acorde_core::{
    Clef, Duration, Measure, MidiPitchBend, Note, Part, Pitch, Score, Staff, Step, TimeSignature,
};
use midly::{MetaMessage, MidiMessage, Smf, Timing, TrackEventKind};

const MAX_MEASURES: usize = 10_000;
const MAX_PARTS: usize = 32;

pub fn parse_midi(data: &[u8]) -> Result<Score, Error> {
    if data.len() > MAX_INPUT_BYTES {
        return Err(Error::TooLarge(data.len()));
    }
    if data.is_empty() {
        return Err(Error::Empty);
    }
    let smf = Smf::parse(data).map_err(|e| Error::Midi(format!("{e}")))?;
    let event_count = smf.tracks.iter().map(Vec::len).sum::<usize>();
    if event_count > MAX_MIDI_EVENTS {
        return Err(Error::Midi(format!(
            "input exceeds {MAX_MIDI_EVENTS} MIDI events"
        )));
    }

    let ppq = match smf.header.timing {
        Timing::Metrical(tpq) => tpq.as_int() as u64,
        Timing::Timecode(..) => 480,
    };

    let mut tempo_bpm = 120u16;
    let mut numerator = 4u8;
    let mut denominator = 4u8;
    let mut score_title = String::new();

    if let Some(track) = smf.tracks.first() {
        for event in track.iter() {
            match &event.kind {
                TrackEventKind::Meta(MetaMessage::Tempo(t)) => {
                    let us = t.as_int() as u64;
                    if let Some(quotient) = 60_000_000u64.checked_div(us) {
                        tempo_bpm = (quotient as u16).clamp(1, 999);
                    }
                }
                TrackEventKind::Meta(MetaMessage::TimeSignature(n, d, _, _)) => {
                    numerator = *n;
                    denominator = 1u8 << d;
                }
                TrackEventKind::Meta(MetaMessage::TrackName(name)) if score_title.is_empty() => {
                    score_title = String::from_utf8_lossy(name).to_string();
                }
                _ => {}
            }
        }
    }

    let ts = TimeSignature {
        numerator,
        denominator,
    };
    let beats_per_measure = ts.total_beats();

    type TrackData = (String, Vec<Note>, Option<(u8, u8)>, Vec<MidiPitchBend>);
    let mut parts_data: Vec<TrackData> = Vec::new();
    for (ti, track) in smf.tracks.iter().enumerate() {
        if parts_data.len() >= MAX_PARTS {
            break;
        }
        let raw = collect_raw_notes(track);
        if raw.is_empty() {
            continue;
        }
        let program_info = extract_program(track);
        let notes = quantize_to_notes(raw, ppq);
        let name = track_name(track).unwrap_or_else(|| format!("Track {}", ti + 1));
        parts_data.push((name, notes, program_info, collect_pitch_bends(track)));
    }

    if parts_data.is_empty() {
        return Err(Error::Empty);
    }

    let mut score = Score::default();
    score.settings.tempo_bpm = tempo_bpm;
    score.settings.time_signature = ts;
    if !score_title.is_empty() {
        score.metadata.title = score_title;
    }
    score.parts.clear();

    for (name, notes, program_info, pitch_bends) in parts_data {
        let short: String = name.chars().take(4).collect();
        let measures = build_measures(notes, numerator, denominator, beats_per_measure);
        let mut staff = Staff::new(Clef::Treble);
        staff.measures = measures;
        let mut part = Part::new(&name, &short);
        if let Some((channel, program)) = program_info {
            part.midi_channel = channel;
            part.midi_program = program;
        }
        part.midi_pitch_bends = pitch_bends;
        part.staves.push(staff);
        score.parts.push(part);
    }

    Ok(score)
}

// ── raw note collection ───────────────────────────────────────────────────────

struct RawNote {
    start: u64,
    end: u64,
    midi: u8,
}

fn collect_raw_notes(track: &[midly::TrackEvent]) -> Vec<RawNote> {
    let mut result: Vec<RawNote> = Vec::new();
    let mut abs: u64 = 0;
    let mut on: std::collections::HashMap<u8, u64> = std::collections::HashMap::new();

    for event in track {
        abs += event.delta.as_int() as u64;
        if let TrackEventKind::Midi { message, .. } = &event.kind {
            match message {
                MidiMessage::NoteOn { key, vel } if vel.as_int() > 0 => {
                    on.insert(key.as_int(), abs);
                }
                MidiMessage::NoteOn { key, .. } | MidiMessage::NoteOff { key, .. } => {
                    if let Some(start) = on.remove(&key.as_int()) {
                        result.push(RawNote {
                            start,
                            end: abs.max(start + 1),
                            midi: key.as_int(),
                        });
                    }
                }
                _ => {}
            }
        }
    }
    for (midi, start) in on {
        result.push(RawNote {
            start,
            end: abs.max(start + 1),
            midi,
        });
    }
    result.sort_by_key(|n| (n.start, n.midi));
    result
}

fn extract_program(track: &[midly::TrackEvent]) -> Option<(u8, u8)> {
    for event in track {
        if let TrackEventKind::Midi {
            channel,
            message: MidiMessage::ProgramChange { program },
        } = &event.kind
        {
            return Some((channel.as_int(), program.as_int()));
        }
    }
    None
}

fn collect_pitch_bends(track: &[midly::TrackEvent]) -> Vec<MidiPitchBend> {
    let mut tick = 0_u64;
    let mut bends = Vec::new();
    for event in track {
        tick += u64::from(event.delta.as_int());
        if let TrackEventKind::Midi {
            channel,
            message: MidiMessage::PitchBend { bend },
        } = &event.kind
        {
            bends.push(MidiPitchBend {
                tick,
                channel: channel.as_int(),
                value: bend.as_int(),
            });
        }
    }
    bends
}

fn track_name(track: &[midly::TrackEvent]) -> Option<String> {
    for event in track {
        if let TrackEventKind::Meta(MetaMessage::TrackName(name)) = &event.kind {
            let s = String::from_utf8_lossy(name).to_string();
            if !s.is_empty() {
                return Some(s);
            }
        }
    }
    None
}

// ── quantization ─────────────────────────────────────────────────────────────

fn midi_to_pitch(midi: u8) -> Pitch {
    let (step, alter) = match midi % 12 {
        0 => (Step::C, 0i8),
        1 => (Step::C, 1),
        2 => (Step::D, 0),
        3 => (Step::D, 1),
        4 => (Step::E, 0),
        5 => (Step::F, 0),
        6 => (Step::F, 1),
        7 => (Step::G, 0),
        8 => (Step::G, 1),
        9 => (Step::A, 0),
        10 => (Step::A, 1),
        11 => (Step::B, 0),
        _ => (Step::C, 0),
    };
    let octave = (midi as i16 / 12 - 1) as i8;
    Pitch::with_alter(step, octave, alter)
}

fn quantize_duration(beats: f64) -> (Duration, u8) {
    if beats >= 3.5 {
        return (Duration::Whole, 0);
    }
    if beats >= 2.5 {
        return (Duration::Half, 1);
    }
    if beats >= 1.75 {
        return (Duration::Half, 0);
    }
    if beats >= 1.25 {
        return (Duration::Quarter, 1);
    }
    if beats >= 0.875 {
        return (Duration::Quarter, 0);
    }
    if beats >= 0.625 {
        return (Duration::Eighth, 1);
    }
    if beats >= 0.4375 {
        return (Duration::Eighth, 0);
    }
    if beats >= 0.3125 {
        return (Duration::Sixteenth, 1);
    }
    if beats >= 0.21875 {
        return (Duration::Sixteenth, 0);
    }
    if beats >= 0.15625 {
        return (Duration::ThirtySecond, 1);
    }
    if beats >= 0.109375 {
        return (Duration::ThirtySecond, 0);
    }
    if beats >= 0.078125 {
        return (Duration::SixtyFourth, 1);
    }
    (Duration::SixtyFourth, 0)
}

fn quantize_to_notes(raw: Vec<RawNote>, ppq: u64) -> Vec<Note> {
    if raw.is_empty() {
        return Vec::new();
    }

    // Group same-tick notes into chords
    let mut groups: Vec<(u64, u64, Vec<u8>)> = Vec::new();
    for rn in raw {
        if let Some(last) = groups.last_mut()
            && last.0 == rn.start
        {
            last.1 = last.1.max(rn.end);
            last.2.push(rn.midi);
            continue;
        }
        groups.push((rn.start, rn.end, vec![rn.midi]));
    }

    let mut result: Vec<Note> = Vec::new();
    let mut cursor: u64 = 0;

    for (start, end, midis) in groups {
        if start > cursor {
            fill_rests(&mut result, (start - cursor) as f64 / ppq as f64);
            cursor = start;
        }
        let dur_beats = end.saturating_sub(start).max(1) as f64 / ppq as f64;
        let (dur, dots) = quantize_duration(dur_beats);
        let actual_beats = dur.beats(dots);
        let mut note = Note::new(midi_to_pitch(midis[0]), dur.clone());
        note.dot_count = dots;
        for &m in midis.iter().skip(1) {
            note.pitches.push(midi_to_pitch(m));
        }
        result.push(note);
        cursor += (actual_beats * ppq as f64).round() as u64;
    }
    result
}

fn fill_rests(notes: &mut Vec<Note>, mut gap_beats: f64) {
    while gap_beats > 0.001 {
        let dur = Duration::whole_filling_beats(gap_beats);
        let b = dur.beats(0);
        if b < 0.001 {
            break;
        }
        gap_beats -= b;
        notes.push(Note::rest(dur));
    }
}

// ── measure building ──────────────────────────────────────────────────────────

fn build_measures(
    notes: Vec<Note>,
    numerator: u8,
    denominator: u8,
    beats_per_measure: f64,
) -> Vec<Measure> {
    let mut measures: Vec<Measure> = Vec::new();
    let mut bucket: Vec<Note> = Vec::new();
    let mut used = 0.0f64;
    let mut measure_num = 1u32;

    for note in notes {
        let nb = note.beats();
        if nb < 0.001 {
            continue;
        }
        if used + nb > beats_per_measure + 0.001 {
            flush(
                &mut measures,
                &mut bucket,
                &mut used,
                &mut measure_num,
                numerator,
                denominator,
                beats_per_measure,
            );
            if measures.len() >= MAX_MEASURES {
                break;
            }
        }
        used += nb;
        bucket.push(note);
    }
    flush(
        &mut measures,
        &mut bucket,
        &mut used,
        &mut measure_num,
        numerator,
        denominator,
        beats_per_measure,
    );

    if measures.is_empty() {
        let mut m = Measure::empty(numerator, denominator);
        m.number = 1;
        measures.push(m);
    }
    measures
}

fn flush(
    measures: &mut Vec<Measure>,
    bucket: &mut Vec<Note>,
    used: &mut f64,
    measure_num: &mut u32,
    numerator: u8,
    denominator: u8,
    beats_per_measure: f64,
) {
    fill_rests(bucket, beats_per_measure - *used);
    let mut m = Measure::empty(numerator, denominator);
    m.number = *measure_num;
    m.voices[0] = std::mem::take(bucket);
    measures.push(m);
    *measure_num += 1;
    *used = 0.0;
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn empty_bytes_returns_empty_err() {
        assert!(matches!(parse_midi(&[]), Err(Error::Empty)));
    }

    #[test]
    fn garbage_bytes_returns_err() {
        assert!(parse_midi(b"not midi data!!!").is_err());
    }

    #[test]
    fn quantize_quarter_note() {
        let (dur, dots) = quantize_duration(1.0);
        assert_eq!(dur, Duration::Quarter);
        assert_eq!(dots, 0);
    }

    #[test]
    fn quantize_dotted_half() {
        let (dur, dots) = quantize_duration(3.0);
        assert_eq!(dur, Duration::Half);
        assert_eq!(dots, 1);
    }

    #[test]
    fn midi_to_pitch_middle_c() {
        let p = midi_to_pitch(60);
        assert_eq!(p.step, Step::C);
        assert_eq!(p.octave, 4);
        assert_eq!(p.alter, 0);
    }

    #[test]
    fn midi_to_pitch_a4() {
        let p = midi_to_pitch(69);
        assert_eq!(p.step, Step::A);
        assert_eq!(p.octave, 4);
    }
}
