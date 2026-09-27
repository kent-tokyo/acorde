//! Turn performed MIDI notes into bars and voices.
//!
//! Bars follow the conductor track's meter changes. Note starts and ends are snapped to a
//! sixteenth grid (a thirty-second grid when the track plays finer values); overlapping notes
//! are spread over up to four voices; a note crossing a barline is split and tied; each gap
//! becomes rests. Every note keeps its onset bar and beat, however full the bar is.

use acorde_core::{Duration, KeySignature, Measure, Note, TimeSignature};

use super::RawNote;

/// One bar of the timeline: its start, its length and the meter it is in, all in source ticks.
pub(super) struct MidiBar {
    pub start: u64,
    pub len: u64,
    pub time: TimeSignature,
    /// The bar starts a meter different from the one before it.
    pub meter_change: bool,
}

/// Bars from tick 0 until `end_tick`, following meter changes (a change between barlines takes
/// effect at the next one).
pub(super) fn midi_bars(
    ppq: u64,
    initial: &TimeSignature,
    changes: &[(u64, TimeSignature)],
    end_tick: u64,
    max_bars: usize,
) -> Vec<MidiBar> {
    let mut bars = Vec::new();
    let mut time = initial.clone();
    let mut next_change = 0usize;
    let mut tick = 0u64;
    loop {
        let before = time.clone();
        while let Some((change_tick, change)) = changes.get(next_change) {
            if *change_tick > tick {
                break;
            }
            time = change.clone();
            next_change += 1;
        }
        let len = (ppq * 4 * u64::from(time.numerator)) / u64::from(time.denominator.max(1));
        let len = len.max(1);
        bars.push(MidiBar {
            start: tick,
            len,
            meter_change: !bars.is_empty() && time != before,
            time: time.clone(),
        });
        tick += len;
        if tick >= end_tick || bars.len() >= max_bars {
            break;
        }
    }
    bars
}

/// A chord in one voice: quantized start and end and its keys.
struct VoiceChord {
    start: u64,
    end: u64,
    keys: Vec<u8>,
    unpitched: bool,
}

/// The snapping grid: a sixteenth, or a thirty-second when many notes fall between sixteenths.
fn grid_ticks(raw: &[RawNote], ppq: u64) -> u64 {
    let sixteenth = (ppq / 4).max(1);
    let tolerance = sixteenth / 4;
    let off_grid = raw
        .iter()
        .filter(|note| {
            let offset = note.start % sixteenth;
            offset > tolerance && offset < sixteenth - tolerance
        })
        .count();
    if off_grid * 10 > raw.len() {
        (ppq / 8).max(1)
    } else {
        sixteenth
    }
}

fn snap(tick: u64, grid: u64) -> u64 {
    (tick + grid / 2) / grid * grid
}

/// Spread the track's notes over at most four voices of chords.
fn assign_voices(raw: &[RawNote], grid: u64) -> [Vec<VoiceChord>; 4] {
    // Chords: same snapped start and end.
    let mut chords: Vec<VoiceChord> = Vec::new();
    let mut notes: Vec<(u64, u64, u8, bool)> = raw
        .iter()
        .map(|note| {
            let start = snap(note.start, grid);
            let end = snap(note.end, grid).max(start + grid);
            (start, end, note.midi, note.channel == 9)
        })
        .collect();
    // Higher notes first at each onset, so the top line lands in the first voice.
    notes.sort_by_key(|a| (a.0, std::cmp::Reverse(a.2)));
    for (start, end, key, unpitched) in notes {
        if let Some(chord) = chords
            .iter_mut()
            .rev()
            .take_while(|chord| chord.start == start)
            .find(|chord| chord.end == end)
        {
            if !chord.keys.contains(&key) {
                chord.keys.push(key);
            }
            chord.unpitched &= unpitched;
            continue;
        }
        chords.push(VoiceChord {
            start,
            end,
            keys: vec![key],
            unpitched,
        });
    }

    let mut voices: [Vec<VoiceChord>; 4] = Default::default();
    for chord in chords {
        let free = voices
            .iter()
            .position(|voice| voice.last().is_none_or(|last| last.end <= chord.start));
        let index = match free {
            Some(index) => index,
            None => {
                // All four voices still sound: join a chord that starts together, else cut the
                // voice that frees first short at this onset.
                if let Some(voice) = voices
                    .iter_mut()
                    .find(|voice| voice.last().is_some_and(|last| last.start == chord.start))
                {
                    if let Some(last) = voice.last_mut() {
                        for key in chord.keys {
                            if !last.keys.contains(&key) {
                                last.keys.push(key);
                            }
                        }
                    }
                    continue;
                }
                let index = (0..4)
                    .min_by_key(|&index| voices[index].last().map_or(0, |last| last.end))
                    .unwrap_or(0);
                if let Some(last) = voices[index].last_mut() {
                    last.end = chord.start.max(last.start + grid);
                }
                index
            }
        };
        voices[index].push(chord);
    }
    for voice in &mut voices {
        for chord in voice.iter_mut() {
            chord.keys.sort_unstable();
        }
    }
    voices
}

/// Note values (with dots) whose length is a whole number of ticks, longest first.
fn note_values(ppq: u64) -> Vec<(Duration, u8, u64)> {
    let mut values = Vec::new();
    for value in [
        Duration::Whole,
        Duration::Half,
        Duration::Quarter,
        Duration::Eighth,
        Duration::Sixteenth,
        Duration::ThirtySecond,
        Duration::SixtyFourth,
    ] {
        let (num, den) = value.as_fraction();
        let base = ppq * 4 * u64::from(num);
        for dots in [1u8, 0] {
            let numerator = base * ((2u64 << dots) - 1);
            let denominator = u64::from(den) << dots;
            if numerator.is_multiple_of(denominator) {
                values.push((value.clone(), dots, numerator / denominator));
            }
        }
    }
    values.sort_by_key(|(_, _, ticks)| std::cmp::Reverse(*ticks));
    values
}

/// Split a span of ticks into written values, longest first.
fn split_span(mut ticks: u64, values: &[(Duration, u8, u64)]) -> Vec<(Duration, u8)> {
    let mut parts = Vec::new();
    while let Some((value, dots, length)) = values.iter().find(|(_, _, length)| *length <= ticks) {
        parts.push((value.clone(), *dots));
        ticks -= length;
    }
    parts
}

fn push_rests(voice: &mut Vec<Note>, ticks: u64, values: &[(Duration, u8, u64)]) {
    for (value, dots) in split_span(ticks, values) {
        let mut rest = Note::rest(value);
        rest.dot_count = dots;
        voice.push(rest);
    }
}

/// Bars of one track: voice 1 fills every bar; other voices appear where they play.
pub(super) fn transcribe_track(
    raw: &[RawNote],
    ppq: u64,
    bars: &[MidiBar],
    pitch: impl Fn(u8) -> acorde_core::Pitch,
) -> Vec<Measure> {
    let grid = grid_ticks(raw, ppq);
    let voices = assign_voices(raw, grid);
    let values = note_values(ppq);
    let mut measures: Vec<Measure> = bars
        .iter()
        .enumerate()
        .map(|(index, bar)| {
            let mut measure = Measure::empty(bar.time.numerator, bar.time.denominator);
            measure.number = index as u32 + 1;
            measure.voices = [vec![], vec![], vec![], vec![]];
            if index == 0 || bar.meter_change {
                measure.time_sig = Some(bar.time.clone());
            }
            measure
        })
        .collect();
    for (voice_index, chords) in voices.iter().enumerate() {
        let mut next = 0usize;
        for (bar, measure) in bars.iter().zip(measures.iter_mut()) {
            let bar_end = bar.start + bar.len;
            // Chords sounding in this bar.
            while chords.get(next).is_some_and(|chord| chord.end <= bar.start) {
                next += 1;
            }
            let sounding: Vec<&VoiceChord> = chords[next..]
                .iter()
                .take_while(|chord| chord.start < bar_end)
                .collect();
            if sounding.is_empty() && voice_index > 0 {
                continue;
            }
            let voice = &mut measure.voices[voice_index];
            let mut cursor = bar.start;
            for chord in sounding {
                let start = chord.start.max(bar.start);
                let end = chord.end.min(bar_end);
                if start > cursor {
                    push_rests(voice, start - cursor, &values);
                }
                let pieces = split_span(end - start, &values);
                let count = pieces.len();
                for (piece_index, (value, dots)) in pieces.into_iter().enumerate() {
                    let mut note = Note::new(pitch(chord.keys[0]), value);
                    note.dot_count = dots;
                    for &key in &chord.keys[1..] {
                        note.pitches.push(pitch(key));
                    }
                    note.is_unpitched = chord.unpitched;
                    // Tied across the barline and between the pieces of one long note.
                    note.tie_end = piece_index > 0 || chord.start < bar.start;
                    note.tie_start = piece_index + 1 < count || chord.end > bar_end;
                    voice.push(note);
                }
                cursor = end.max(cursor);
            }
            if cursor < bar_end {
                push_rests(voice, bar_end - cursor, &values);
            }
        }
    }
    measures
}

/// Put tempo and key changes on the bars they start (changes between barlines are left to the
/// loss report).
pub(super) fn apply_bar_changes(
    measures: &mut [Measure],
    bars: &[MidiBar],
    tempo_changes: &[(u64, u16)],
    key_changes: &[(u64, KeySignature)],
) {
    let bar_at = |tick: u64| bars.iter().position(|bar| bar.start == tick);
    for &(tick, bpm) in tempo_changes {
        if tick > 0
            && let Some(measure) = bar_at(tick).and_then(|index| measures.get_mut(index))
        {
            measure.tempo = Some(bpm);
        }
    }
    for (tick, key) in key_changes {
        if let Some(measure) = bar_at(*tick).and_then(|index| measures.get_mut(index)) {
            measure.key_sig = Some(key.clone());
        }
    }
}
