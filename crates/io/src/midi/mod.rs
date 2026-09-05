mod serialize;
mod validation;
pub use serialize::{serialize_midi, serialize_midi_region};

use crate::{Diagnostic, Error, MAX_INPUT_BYTES, MAX_MIDI_EVENTS};
use acorde_core::{
    Clef, Duration, Measure, MidiAftertouch, MidiControlChange, MidiPitchBend, MidiProgramChange,
    Note, Part, Pitch, Score, Staff, Step, TimeSignature,
};
use midly::{MetaMessage, MidiMessage, Smf, Timing, TrackEventKind};

const MAX_MEASURES: usize = 10_000;
const MAX_PARTS: usize = 32;
type MidiMetaChanges = (Vec<(u64, u16)>, Vec<(u64, TimeSignature)>);

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

    let (tempo_changes, time_signature_changes) = smf
        .tracks
        .first()
        .map(|track| collect_meta_changes(track))
        .unwrap_or_else(|| (Vec::new(), Vec::new()));

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

    type TrackData = (
        String,
        Vec<Note>,
        Option<(u8, u8)>,
        Vec<MidiPitchBend>,
        Vec<MidiControlChange>,
        Vec<MidiProgramChange>,
        Vec<MidiAftertouch>,
    );
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
        parts_data.push((
            name,
            notes,
            program_info,
            collect_pitch_bends(track, ppq),
            collect_control_changes(track, ppq),
            collect_program_changes(track, ppq),
            collect_aftertouch(track, ppq),
        ));
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

    for (name, notes, program_info, pitch_bends, control_changes, program_changes, aftertouch) in
        parts_data
    {
        let short: String = name.chars().take(4).collect();
        let mut measures = build_measures(notes, numerator, denominator, beats_per_measure);
        apply_meta_changes(
            &mut measures,
            ppq,
            numerator,
            denominator,
            &tempo_changes,
            &time_signature_changes,
        );
        let mut staff = Staff::new(Clef::Treble);
        staff.measures = measures;
        let mut part = Part::new(&name, &short);
        if let Some((channel, program)) = program_info {
            part.midi_channel = channel;
            part.midi_program = program;
        }
        part.midi_pitch_bends = pitch_bends;
        part.midi_control_changes = control_changes;
        part.midi_program_changes = program_changes;
        part.midi_aftertouch = aftertouch;
        part.staves.push(staff);
        score.parts.push(part);
    }

    Ok(score)
}

/// Report MIDI channel events that have no canonical `Score` representation.
pub fn loss_diagnostics(data: &[u8]) -> Result<Vec<Diagnostic>, Error> {
    if data.len() > MAX_INPUT_BYTES {
        return Err(Error::TooLarge(data.len()));
    }
    if data.is_empty() {
        return Err(Error::Empty);
    }
    let smf = Smf::parse(data).map_err(|e| Error::Midi(format!("{e}")))?;
    let mut diagnostics = Vec::new();
    if matches!(smf.header.timing, Timing::Timecode(..)) {
        let mut diagnostic = Diagnostic::warning(
            "midi.unsupported-smpte-timing",
            "SMPTE MIDI timing is normalized to the canonical PPQ timing boundary",
        );
        diagnostic.source_location = Some("/header/timing".to_string());
        diagnostic.preserved_value = Some("smpte".to_string());
        diagnostics.push(diagnostic);
    }
    diagnostics.extend(off_measure_meta_change_diagnostics(&smf));
    let mut tick;
    for (track_index, track) in smf.tracks.iter().enumerate() {
        tick = 0_u64;
        for (event_index, event) in track.iter().enumerate() {
            tick += u64::from(event.delta.as_int());
            let (code, reason, channel): (&str, &str, Option<u8>) = match &event.kind {
                TrackEventKind::Midi { .. } => continue,
                TrackEventKind::SysEx(_) | TrackEventKind::Escape(_) => (
                    "midi.unsupported-system-event",
                    "MIDI system event is outside the canonical Score model",
                    None,
                ),
                _ => continue,
            };
            let mut diagnostic = Diagnostic::warning(code, reason);
            diagnostic.source_location = Some(match channel {
                Some(channel) => {
                    format!(
                        "/tracks/{track_index}/events/{event_index}@tick={tick}@channel={channel}"
                    )
                }
                None => format!("/tracks/{track_index}/events/{event_index}@tick={tick}"),
            });
            diagnostics.push(diagnostic);
        }
    }
    diagnostics.extend(unmatched_note_diagnostics(&smf));
    Ok(diagnostics)
}

/// Report score pitches whose fractional cents cannot be represented by a MIDI note key alone.
/// The MIDI serializer intentionally does not invent channel-wide pitch-bend automation, so the
/// caller must decide whether to supply a bend stream or accept the rounded note-key boundary.
pub fn export_loss_diagnostics(score: &acorde_core::Score) -> Vec<Diagnostic> {
    const MAX_DIAGNOSTICS: usize = 1_024;
    let mut diagnostics = Vec::new();
    for (part_index, part) in score.parts.iter().enumerate() {
        for (staff_index, staff) in part.staves.iter().enumerate() {
            for (measure_index, measure) in staff.measures.iter().enumerate() {
                for (voice_index, voice) in measure.voices.iter().enumerate() {
                    for (note_index, note) in voice.iter().enumerate() {
                        for (pitch_index, pitch) in note.pitches.iter().enumerate() {
                            if pitch.microtone_cents == 0 {
                                continue;
                            }
                            if diagnostics.len() >= MAX_DIAGNOSTICS {
                                return diagnostics;
                            }
                            let mut diagnostic = Diagnostic::warning(
                                "midi.export-rounded-microtone",
                                "fractional pitch cents are rounded to a MIDI note key; pitch-bend automation is not synthesized",
                            );
                            diagnostic.source_location = Some(format!(
                                "/score/part/{}/staff/{}/measure/{}/voice/{}/note/{}/pitch/{}",
                                part_index + 1,
                                staff_index + 1,
                                measure_index + 1,
                                voice_index + 1,
                                note_index + 1,
                                pitch_index + 1
                            ));
                            diagnostic.preserved_value = Some(format!(
                                "midi_cents={}, rounded_midi={}",
                                pitch.to_midi_cents(),
                                pitch.to_midi()
                            ));
                            diagnostics.push(diagnostic);
                        }
                    }
                }
            }
        }
    }
    diagnostics
}

fn unmatched_note_diagnostics(smf: &Smf<'_>) -> Vec<Diagnostic> {
    let mut diagnostics = Vec::new();
    for (track_index, track) in smf.tracks.iter().enumerate() {
        let mut tick = 0_u64;
        let mut active: std::collections::HashMap<
            (u8, u8),
            std::collections::VecDeque<(u64, usize)>,
        > = std::collections::HashMap::new();
        for (event_index, event) in track.iter().enumerate() {
            tick += u64::from(event.delta.as_int());
            let TrackEventKind::Midi { channel, message } = &event.kind else {
                continue;
            };
            let key = (
                channel.as_int(),
                match message {
                    MidiMessage::NoteOn { key, .. } | MidiMessage::NoteOff { key, .. } => {
                        key.as_int()
                    }
                    _ => continue,
                },
            );
            match message {
                MidiMessage::NoteOn { vel, .. } if vel.as_int() > 0 => {
                    active
                        .entry(key)
                        .or_default()
                        .push_back((tick, event_index));
                }
                MidiMessage::NoteOn { .. } | MidiMessage::NoteOff { .. } => {
                    let matched = active
                        .get_mut(&key)
                        .and_then(|events| events.pop_front())
                        .is_some();
                    if !matched {
                        let mut diagnostic = Diagnostic::warning(
                            "midi.unmatched-note-off",
                            "MIDI note-off has no matching note-on",
                        );
                        diagnostic.source_location = Some(format!(
                            "/tracks/{track_index}/events/{event_index}@tick={tick}@channel={}",
                            key.0
                        ));
                        diagnostics.push(diagnostic);
                    }
                }
                _ => {}
            }
        }
        for ((channel, key), events) in active {
            for (start_tick, event_index) in events {
                let mut diagnostic = Diagnostic::warning(
                    "midi.unmatched-note-on",
                    "MIDI note-on has no matching note-off",
                );
                diagnostic.source_location = Some(format!(
                    "/tracks/{track_index}/events/{event_index}@tick={start_tick}@channel={channel}@key={key}"
                ));
                diagnostics.push(diagnostic);
            }
        }
    }
    diagnostics
}

// ── raw note collection ───────────────────────────────────────────────────────

struct RawNote {
    start: u64,
    end: u64,
    midi: u8,
    channel: u8,
}

fn collect_raw_notes(track: &[midly::TrackEvent]) -> Vec<RawNote> {
    let mut result: Vec<RawNote> = Vec::new();
    let mut abs: u64 = 0;
    let mut on: std::collections::HashMap<(u8, u8), std::collections::VecDeque<u64>> =
        std::collections::HashMap::new();

    for event in track {
        abs += event.delta.as_int() as u64;
        if let TrackEventKind::Midi { channel, message } = &event.kind {
            let channel = channel.as_int();
            match message {
                MidiMessage::NoteOn { key, vel } if vel.as_int() > 0 => {
                    on.entry((channel, key.as_int()))
                        .or_default()
                        .push_back(abs);
                }
                MidiMessage::NoteOn { key, .. } | MidiMessage::NoteOff { key, .. } => {
                    let key = (channel, key.as_int());
                    if let Some(start) = on.get_mut(&key).and_then(|starts| starts.pop_front()) {
                        result.push(RawNote {
                            start,
                            end: abs.max(start + 1),
                            midi: key.1,
                            channel: key.0,
                        });
                    }
                }
                _ => {}
            }
        }
    }
    for ((channel, midi), starts) in on {
        for start in starts {
            result.push(RawNote {
                start,
                end: abs.max(start + 1),
                midi,
                channel,
            });
        }
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

fn collect_pitch_bends(track: &[midly::TrackEvent], ppq: u64) -> Vec<MidiPitchBend> {
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
                tick: normalize_tick(tick, ppq),
                channel: channel.as_int(),
                value: bend.as_int(),
            });
        }
    }
    bends
}

fn collect_control_changes(track: &[midly::TrackEvent], ppq: u64) -> Vec<MidiControlChange> {
    let mut tick = 0_u64;
    let mut controls = Vec::new();
    for event in track {
        tick += u64::from(event.delta.as_int());
        if let TrackEventKind::Midi {
            channel,
            message: MidiMessage::Controller { controller, value },
        } = &event.kind
        {
            controls.push(MidiControlChange {
                tick: normalize_tick(tick, ppq),
                channel: channel.as_int(),
                controller: controller.as_int(),
                value: value.as_int(),
            });
        }
    }
    controls
}

fn collect_program_changes(track: &[midly::TrackEvent], ppq: u64) -> Vec<MidiProgramChange> {
    let mut tick = 0_u64;
    let mut programs = Vec::new();
    for event in track {
        tick += u64::from(event.delta.as_int());
        if let TrackEventKind::Midi {
            channel,
            message: MidiMessage::ProgramChange { program },
        } = &event.kind
        {
            programs.push(MidiProgramChange {
                tick: normalize_tick(tick, ppq),
                channel: channel.as_int(),
                program: program.as_int(),
            });
        }
    }
    programs
}

fn collect_aftertouch(track: &[midly::TrackEvent], ppq: u64) -> Vec<MidiAftertouch> {
    let mut tick = 0_u64;
    let mut events = Vec::new();
    for event in track {
        tick += u64::from(event.delta.as_int());
        if let TrackEventKind::Midi { channel, message } = &event.kind {
            let (key, value) = match message {
                MidiMessage::Aftertouch { key, vel } => (Some(key.as_int()), vel.as_int()),
                MidiMessage::ChannelAftertouch { vel } => (None, vel.as_int()),
                _ => continue,
            };
            events.push(MidiAftertouch {
                tick: normalize_tick(tick, ppq),
                channel: channel.as_int(),
                key,
                value,
            });
        }
    }
    events
}

fn normalize_tick(tick: u64, source_ppq: u64) -> u64 {
    if source_ppq == 0 || source_ppq == 480 {
        return tick;
    }
    tick.saturating_mul(480).saturating_add(source_ppq / 2) / source_ppq
}

fn collect_meta_changes(track: &[midly::TrackEvent]) -> MidiMetaChanges {
    let mut tick = 0_u64;
    let mut tempo_changes = Vec::new();
    let mut time_signature_changes = Vec::new();
    for event in track {
        tick += u64::from(event.delta.as_int());
        match &event.kind {
            TrackEventKind::Meta(MetaMessage::Tempo(value)) => {
                let micros = u64::from(value.as_int());
                if let Some(bpm) = 60_000_000_u64
                    .checked_div(micros)
                    .map(|bpm| (bpm as u16).clamp(1, 999))
                {
                    tempo_changes.push((tick, bpm));
                }
            }
            TrackEventKind::Meta(MetaMessage::TimeSignature(n, d, _, _)) => {
                time_signature_changes.push((
                    tick,
                    TimeSignature {
                        numerator: *n,
                        denominator: 1_u8 << d,
                    },
                ));
            }
            _ => {}
        }
    }
    (tempo_changes, time_signature_changes)
}

fn apply_meta_changes(
    measures: &mut [Measure],
    ppq: u64,
    initial_numerator: u8,
    initial_denominator: u8,
    tempo_changes: &[(u64, u16)],
    time_signature_changes: &[(u64, TimeSignature)],
) {
    let ticks_per_measure = (initial_numerator as u64)
        .saturating_mul(4)
        .saturating_mul(ppq)
        / u64::from(initial_denominator.max(1));
    if ticks_per_measure == 0 {
        return;
    }
    for &(tick, bpm) in tempo_changes {
        if tick == 0 {
            continue;
        }
        let index = (tick / ticks_per_measure) as usize;
        if tick % ticks_per_measure == 0
            && let Some(measure) = measures.get_mut(index)
        {
            measure.tempo = Some(bpm);
        }
    }
    for &(tick, ref time_signature) in time_signature_changes {
        if tick == 0 {
            continue;
        }
        let index = (tick / ticks_per_measure) as usize;
        if tick % ticks_per_measure == 0
            && let Some(measure) = measures.get_mut(index)
        {
            measure.time_sig = Some(time_signature.clone());
        }
    }
}

fn off_measure_meta_change_diagnostics(smf: &Smf<'_>) -> Vec<Diagnostic> {
    let Some(track) = smf.tracks.first() else {
        return Vec::new();
    };
    let ppq = match smf.header.timing {
        Timing::Metrical(value) => u64::from(value.as_int()),
        Timing::Timecode(..) => 480,
    };
    let (initial_numerator, initial_denominator) = track
        .iter()
        .find_map(|event| match event.kind {
            TrackEventKind::Meta(MetaMessage::TimeSignature(n, d, _, _)) => Some((n, 1_u8 << d)),
            _ => None,
        })
        .unwrap_or((4, 4));
    let ticks_per_measure = u64::from(initial_numerator)
        .saturating_mul(4)
        .saturating_mul(ppq)
        / u64::from(initial_denominator.max(1));
    if ticks_per_measure == 0 {
        return Vec::new();
    }
    let mut tick = 0_u64;
    let mut diagnostics = Vec::new();
    for (event_index, event) in track.iter().enumerate() {
        tick += u64::from(event.delta.as_int());
        let is_timing_change = matches!(
            event.kind,
            TrackEventKind::Meta(MetaMessage::Tempo(_))
                | TrackEventKind::Meta(MetaMessage::TimeSignature(_, _, _, _))
        );
        if is_timing_change && tick > 0 && !tick.is_multiple_of(ticks_per_measure) {
            let mut diagnostic = Diagnostic::warning(
                "midi.timing-change-off-measure",
                "tempo or time-signature change occurs inside a canonical measure and is not attached to a measure boundary",
            );
            diagnostic.source_location =
                Some(format!("/tracks/0/events/{event_index}@tick={tick}"));
            diagnostic.preserved_value =
                Some("raw MIDI meta event retained only at source boundary".to_string());
            diagnostics.push(diagnostic);
        }
    }
    diagnostics
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
    let mut groups: Vec<(u64, u64, Vec<u8>, bool)> = Vec::new();
    for rn in raw {
        if let Some(last) = groups.last_mut()
            && last.0 == rn.start
        {
            last.1 = last.1.max(rn.end);
            last.2.push(rn.midi);
            last.3 &= rn.channel == 9;
            continue;
        }
        groups.push((rn.start, rn.end, vec![rn.midi], rn.channel == 9));
    }

    let mut result: Vec<Note> = Vec::new();
    let mut cursor: u64 = 0;

    for (start, end, midis, is_unpitched) in groups {
        if start > cursor {
            fill_rests(&mut result, (start - cursor) as f64 / ppq as f64);
            cursor = start;
        }
        let dur_beats = end.saturating_sub(start).max(1) as f64 / ppq as f64;
        let (dur, dots) = quantize_duration(dur_beats);
        let actual_beats = dur.beats(dots);
        let mut note = Note::new(midi_to_pitch(midis[0]), dur.clone());
        note.is_unpitched = is_unpitched;
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
    use midly::{
        Format, Header, MetaMessage, MidiMessage, Smf, Timing, TrackEvent, TrackEventKind,
    };

    fn generated_boundary_corpus_case() -> Vec<u8> {
        let header = Header::new(
            Format::Parallel,
            Timing::Metrical(midly::num::u15::from(480)),
        );
        let track = vec![
            TrackEvent {
                delta: midly::num::u28::from(0),
                kind: TrackEventKind::Meta(MetaMessage::TrackName(b"generated-boundaries")),
            },
            TrackEvent {
                delta: midly::num::u28::from(0),
                kind: TrackEventKind::Meta(MetaMessage::Tempo(midly::num::u24::from(500_000))),
            },
            TrackEvent {
                delta: midly::num::u28::from(0),
                kind: TrackEventKind::Meta(MetaMessage::TimeSignature(3, 2, 24, 8)),
            },
            TrackEvent {
                delta: midly::num::u28::from(0),
                kind: TrackEventKind::Midi {
                    channel: midly::num::u4::from(0),
                    message: MidiMessage::ProgramChange {
                        program: midly::num::u7::from(40),
                    },
                },
            },
            TrackEvent {
                delta: midly::num::u28::from(0),
                kind: TrackEventKind::Midi {
                    channel: midly::num::u4::from(0),
                    message: MidiMessage::PitchBend {
                        bend: midly::PitchBend::from_int(-8192),
                    },
                },
            },
            TrackEvent {
                delta: midly::num::u28::from(0),
                kind: TrackEventKind::Midi {
                    channel: midly::num::u4::from(0),
                    message: MidiMessage::Controller {
                        controller: midly::num::u7::from(1),
                        value: midly::num::u7::from(127),
                    },
                },
            },
            TrackEvent {
                delta: midly::num::u28::from(0),
                kind: TrackEventKind::Midi {
                    channel: midly::num::u4::from(0),
                    message: MidiMessage::ChannelAftertouch {
                        vel: midly::num::u7::from(64),
                    },
                },
            },
            TrackEvent {
                delta: midly::num::u28::from(0),
                kind: TrackEventKind::Midi {
                    channel: midly::num::u4::from(0),
                    message: MidiMessage::NoteOn {
                        key: midly::num::u7::from(60),
                        vel: midly::num::u7::from(100),
                    },
                },
            },
            TrackEvent {
                delta: midly::num::u28::from(120),
                kind: TrackEventKind::Midi {
                    channel: midly::num::u4::from(0),
                    message: MidiMessage::NoteOn {
                        key: midly::num::u7::from(60),
                        vel: midly::num::u7::from(80),
                    },
                },
            },
            TrackEvent {
                delta: midly::num::u28::from(120),
                kind: TrackEventKind::Midi {
                    channel: midly::num::u4::from(0),
                    message: MidiMessage::NoteOff {
                        key: midly::num::u7::from(60),
                        vel: midly::num::u7::from(0),
                    },
                },
            },
            TrackEvent {
                delta: midly::num::u28::from(120),
                kind: TrackEventKind::Midi {
                    channel: midly::num::u4::from(0),
                    message: MidiMessage::NoteOff {
                        key: midly::num::u7::from(60),
                        vel: midly::num::u7::from(0),
                    },
                },
            },
            TrackEvent {
                delta: midly::num::u28::from(0),
                kind: TrackEventKind::Meta(MetaMessage::EndOfTrack),
            },
        ];
        let mut smf = Smf::new(header);
        smf.tracks.push(track);
        let mut bytes = Vec::new();
        smf.write_std(&mut bytes)
            .expect("generated MIDI is writable");
        bytes
    }

    #[test]
    fn generated_boundary_corpus_preserves_event_meaning() {
        let data = generated_boundary_corpus_case();
        let score = parse_midi(&data).expect("generated MIDI corpus case parses");
        assert_eq!(score.parts.len(), 1);
        let part = &score.parts[0];
        assert_eq!(part.midi_program_changes[0].program, 40);
        assert_eq!(part.midi_pitch_bends[0].value, -8192);
        assert_eq!(part.midi_control_changes[0].value, 127);
        assert_eq!(part.midi_aftertouch[0].value, 64);
        assert_eq!(part.staves[0].measures[0].voices[0].len(), 3);

        let reparsed = parse_midi(&serialize_midi(&score).expect("generated MIDI serializes"))
            .expect("serialized generated MIDI reparses");
        assert_eq!(reparsed.parts[0].midi_pitch_bends, part.midi_pitch_bends);
        assert_eq!(
            reparsed.parts[0].midi_control_changes,
            part.midi_control_changes
        );
        assert_eq!(reparsed.parts[0].midi_aftertouch, part.midi_aftertouch);
    }

    #[test]
    fn generated_boundary_corpus_reports_timing_and_pairing_boundaries() {
        let unmatched_note_off = [
            b'M', b'T', b'h', b'd', 0, 0, 0, 6, 0, 0, 0, 1, 1, 0xE0, b'M', b'T', b'r', b'k', 0, 0,
            0, 8, 0, 0x80, 60, 0, 0, 0xFF, 0x2F, 0,
        ];
        let diagnostics = loss_diagnostics(&unmatched_note_off).expect("MIDI parses");
        assert!(
            diagnostics
                .iter()
                .any(|diagnostic| diagnostic.code == "midi.unmatched-note-off")
        );

        let smpte_timing = [
            b'M', b'T', b'h', b'd', 0, 0, 0, 6, 0, 0, 0, 1, 0xE7, 40, b'M', b'T', b'r', b'k', 0, 0,
            0, 8, 0, 0x90, 60, 100, 0, 0x80, 60, 0,
        ];
        let diagnostics = loss_diagnostics(&smpte_timing).expect("MIDI parses");
        assert!(
            diagnostics
                .iter()
                .any(|diagnostic| diagnostic.code == "midi.unsupported-smpte-timing")
        );
    }

    #[test]
    fn empty_bytes_returns_empty_err() {
        assert!(matches!(parse_midi(&[]), Err(Error::Empty)));
    }

    #[test]
    fn garbage_bytes_returns_err() {
        assert!(parse_midi(b"not midi data!!!").is_err());
    }

    #[test]
    fn export_loss_report_marks_fractional_pitch_rounding() {
        let mut score = acorde_core::Score::new("microtone", 120, 4, 4, 0, 1);
        let mut note = acorde_core::Note::new(
            acorde_core::Pitch::with_microtone(acorde_core::Step::C, 4, 0, 25),
            acorde_core::Duration::Quarter,
        );
        note.is_rest = false;
        score.parts[0].staves[0].measures[0].voices[0].push(note);
        let diagnostics = export_loss_diagnostics(&score);
        assert_eq!(diagnostics.len(), 1);
        assert_eq!(diagnostics[0].code, "midi.export-rounded-microtone");
        assert!(
            diagnostics[0]
                .preserved_value
                .as_deref()
                .is_some_and(|value| value.contains("midi_cents=6025"))
        );
        assert!(
            diagnostics[0]
                .source_location
                .as_deref()
                .is_some_and(|path| path.ends_with("/pitch/1"))
        );
    }

    #[test]
    fn controller_changes_are_preserved_and_not_reported_as_loss() {
        let data = [
            b'M', b'T', b'h', b'd', 0, 0, 0, 6, 0, 0, 0, 1, 1, 0xE0, // header
            b'M', b'T', b'r', b'k', 0, 0, 0, 0x14, // track header
            0, 0xC0, 0, 0, 0xB0, 1, 100, 0, 0x90, 60, 100, 0x83, 0x60, 0x80, 60, 0, 0, 0xFF, 0x2F,
            0,
        ];
        let score = parse_midi(&data).expect("MIDI parses");
        assert_eq!(
            score.parts[0].midi_control_changes,
            vec![MidiControlChange {
                tick: 0,
                channel: 0,
                controller: 1,
                value: 100,
            }]
        );
        assert!(
            loss_diagnostics(&data)
                .expect("MIDI diagnostics")
                .is_empty()
        );
    }

    #[test]
    fn overlapping_same_pitch_notes_are_paired_fifo() {
        let data = [
            b'M', b'T', b'h', b'd', 0, 0, 0, 6, 0, 0, 0, 1, 1, 0xE0, b'M', b'T', b'r', b'k', 0, 0,
            0, 0x14, 0, 0x90, 60, 100, 0x10, 0x90, 60, 90, 0x10, 0x80, 60, 0, 0x10, 0x80, 60, 0, 0,
            0xFF, 0x2F, 0,
        ];
        let smf = Smf::parse(&data).expect("MIDI fixture parses");
        let notes = collect_raw_notes(&smf.tracks[0]);
        assert_eq!(notes.len(), 2);
        assert_eq!((notes[0].start, notes[0].end), (0, 32));
        assert_eq!((notes[1].start, notes[1].end), (16, 48));
    }

    #[test]
    fn unmatched_note_off_is_reported() {
        let data = [
            b'M', b'T', b'h', b'd', 0, 0, 0, 6, 0, 0, 0, 1, 1, 0xE0, b'M', b'T', b'r', b'k', 0, 0,
            0, 8, 0, 0x80, 60, 0, 0, 0xFF, 0x2F, 0,
        ];
        let diagnostics = loss_diagnostics(&data).expect("MIDI diagnostics");
        assert_eq!(diagnostics.len(), 1);
        assert_eq!(diagnostics[0].code, "midi.unmatched-note-off");
    }

    #[test]
    fn smpte_timing_is_reported_as_normalization() {
        let data = [
            b'M', b'T', b'h', b'd', 0, 0, 0, 6, 0, 0, 0, 1, 0xE7, 40, b'M', b'T', b'r', b'k', 0, 0,
            0, 8, 0, 0x90, 60, 100, 0, 0x80, 60, 0,
        ];
        let diagnostics = loss_diagnostics(&data).expect("MIDI diagnostics");
        assert!(
            diagnostics
                .iter()
                .any(|diagnostic| diagnostic.code == "midi.unsupported-smpte-timing")
        );
    }

    #[test]
    fn meta_changes_at_measure_boundaries_attach_to_measure_metadata() {
        let notes = vec![
            Note::new(Pitch::new(Step::C, 4), Duration::Whole),
            Note::new(Pitch::new(Step::D, 4), Duration::Whole),
        ];
        let mut measures = build_measures(notes, 4, 4, 4.0);
        let changed = TimeSignature {
            numerator: 3,
            denominator: 4,
        };
        apply_meta_changes(
            &mut measures,
            480,
            4,
            4,
            &[(1920, 90)],
            &[(1920, changed.clone())],
        );
        assert_eq!(measures[1].tempo, Some(90));
        assert_eq!(measures[1].time_sig, Some(changed));
    }

    #[test]
    fn noncanonical_ppq_event_ticks_are_normalized_to_480() {
        assert_eq!(normalize_tick(120, 240), 240);
        assert_eq!(normalize_tick(121, 240), 242);
        assert_eq!(normalize_tick(120, 480), 120);
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
    fn drum_channel_notes_are_marked_unpitched_without_inventing_instrument_id() {
        let notes = quantize_to_notes(
            vec![RawNote {
                start: 0,
                end: 480,
                midi: 38,
                channel: 9,
            }],
            480,
        );
        assert!(notes[0].is_unpitched);
        assert_eq!(notes[0].instrument_id, None);
        assert_eq!(notes[0].pitches[0].to_midi(), 38);
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
