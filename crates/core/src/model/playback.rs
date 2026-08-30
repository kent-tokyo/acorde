use super::duration::Duration;
use super::notation::Articulation;
use super::repeat::measure_sequence;
use super::score::Score;
use serde::{Deserialize, Serialize};

fn default_fermata_multiplier() -> f64 {
    1.5
}
fn default_swing_unit() -> Duration {
    Duration::Eighth
}
fn default_metronome_channel() -> u8 {
    9
}
fn default_accent_pitch() -> u8 {
    76
}
fn default_beat_pitch() -> u8 {
    77
}
fn default_accent_velocity() -> u8 {
    100
}
fn default_beat_velocity() -> u8 {
    70
}

/// Click-track injected into [`to_playback_events`] output.
///
/// Metronome events are tagged with `PlaybackEvent.is_metronome = true` and can be
/// routed separately by checking `channel` (default 9 = GM drums).
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct MetronomeConfig {
    /// MIDI channel for the click track. Default 9 (GM drum channel).
    #[serde(default = "default_metronome_channel")]
    pub channel: u8,
    /// MIDI note for the accented first beat. Default 76 (High Wood Block).
    #[serde(default = "default_accent_pitch")]
    pub accent_pitch: u8,
    /// MIDI note for regular beats. Default 77 (Low Wood Block).
    #[serde(default = "default_beat_pitch")]
    pub beat_pitch: u8,
    /// Velocity for the accented first beat. Default 100.
    #[serde(default = "default_accent_velocity")]
    pub accent_velocity: u8,
    /// Velocity for regular beats. Default 70.
    #[serde(default = "default_beat_velocity")]
    pub beat_velocity: u8,
}

impl Default for MetronomeConfig {
    fn default() -> Self {
        Self {
            channel: 9,
            accent_pitch: 76,
            beat_pitch: 77,
            accent_velocity: 100,
            beat_velocity: 70,
        }
    }
}

/// Options for [`to_playback_events`].
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct PlaybackOptions {
    /// Replaces the score's tempo when `Some`; `None` uses `score.settings.tempo_bpm`.
    pub bpm_override: Option<u16>,
    /// Part indices to silence. Events from these parts are omitted entirely.
    pub muted_parts: Vec<usize>,
    /// Restrict playback to measures in the inclusive range `[start, end]`.
    /// Physical measure indices (0-based). `None` plays the full sequence.
    /// Events within the region start at `time_beats = 0`.
    #[serde(default)]
    pub loop_region: Option<(usize, usize)>,
    /// Duration multiplier for notes with `Fermata` articulation. Default 1.5.
    #[serde(default = "default_fermata_multiplier")]
    pub fermata_multiplier: f64,
    /// Swing ratio for pairs of plain notes of [`swing_unit`] duration. `None` = straight.
    /// `0.67` ≈ triplet swing (2:1). Valid range: (0.5, 1.0).
    /// Applied only to notes matching `swing_unit`, no tuplet, no dot.
    #[serde(default)]
    pub swing: Option<f64>,
    /// Note duration that swing is applied to. Defaults to `Duration::Eighth`.
    /// Set to `Duration::Sixteenth` for Latin/funk 16th-note swing.
    #[serde(default = "default_swing_unit")]
    pub swing_unit: Duration,
    /// When `Some`, injects metronome click events into the event list.
    /// Clicks are tagged with `PlaybackEvent.is_metronome = true`.
    #[serde(default)]
    pub metronome: Option<MetronomeConfig>,
}

impl Default for PlaybackOptions {
    fn default() -> Self {
        Self {
            bpm_override: None,
            muted_parts: Vec::new(),
            loop_region: None,
            fermata_multiplier: 1.5,
            swing: None,
            swing_unit: Duration::Eighth,
            metronome: None,
        }
    }
}

/// A single sounding event suitable for audio playback engines (e.g. Web Audio, Tone.js).
///
/// Grace notes and rests are excluded. Chords are expanded to one event per pitch.
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct PlaybackEvent {
    /// Absolute beat position from the start of the score.
    pub time_beats: f64,
    /// Absolute time in seconds from the start of the score.
    pub time_secs: f64,
    /// MIDI pitch number (0–127).
    pub pitch_midi: u8,
    /// MIDI velocity (1–127). Derived from [`Dynamic`](crate::Dynamic); defaults to 64.
    /// Boosted by +20 for Accent / Marcato articulations (clamped to 127).
    pub velocity: u8,
    /// Sounding duration in beats. Halved for Staccato / Staccatissimo.
    pub duration_beats: f64,
    /// Sounding duration in seconds.
    pub duration_secs: f64,
    /// True when the note has `pedal_start` set (sustain pedal down).
    pub pedal: bool,
    /// Index of the part this event originates from (useful for per-channel MIDI routing).
    pub part_index: usize,
    /// MIDI channel of the originating part (`part.midi_channel`). For Tone.js channel routing.
    pub channel: u8,
    /// `true` for metronome click events injected via [`MetronomeConfig`].
    #[serde(default)]
    pub is_metronome: bool,
}

/// Convert a [`Score`] into a flat, time-ordered list of [`PlaybackEvent`]s.
///
/// All parts, staves, and voices are included unless excluded via [`PlaybackOptions`].
/// Repeat sections and volta brackets are expanded using [`measure_sequence`].
/// Events are sorted by `time_beats`.
pub fn to_playback_events(score: &Score, options: &PlaybackOptions) -> Vec<PlaybackEvent> {
    let bpm = options
        .bpm_override
        .unwrap_or(score.settings.tempo_bpm)
        .max(1) as f64;
    let full_seq = measure_sequence(score);
    let seq: Vec<usize> = if let Some((lo, hi)) = options.loop_region {
        full_seq
            .into_iter()
            .filter(|&idx| idx >= lo && idx <= hi)
            .collect()
    } else {
        full_seq
    };
    let mut events: Vec<PlaybackEvent> = Vec::new();

    for (part_index, part) in score.parts.iter().enumerate() {
        if options.muted_parts.contains(&part_index) {
            continue;
        }
        for staff in &part.staves {
            for voice_idx in 0..4usize {
                let mut time_beats = 0.0f64;
                let mut time_secs_cursor = 0.0f64;
                let mut current_bpm = bpm;
                for &idx in &seq {
                    let measure = match staff.measures.get(idx) {
                        Some(m) => m,
                        None => continue,
                    };
                    if let Some(b) = measure.tempo {
                        current_bpm = b.max(1) as f64;
                    }
                    let measure_beats = measure
                        .time_sig
                        .as_ref()
                        .unwrap_or(&score.settings.time_signature)
                        .total_beats();
                    let measure_start_beats = time_beats;
                    let measure_start_secs = time_secs_cursor;
                    let mut local_beats = 0.0f64;
                    let mut local_secs = 0.0f64;
                    let mut swing_first = true;
                    for note in &measure.voices[voice_idx] {
                        if note.is_grace {
                            continue;
                        }
                        let dur = match options.swing {
                            Some(ratio)
                                if note.tuplet.is_none()
                                    && note.dot_count == 0
                                    && note.duration == options.swing_unit =>
                            {
                                let pair = note.beats() * 2.0;
                                let d = if swing_first {
                                    ratio * pair
                                } else {
                                    (1.0 - ratio) * pair
                                };
                                swing_first = !swing_first;
                                d
                            }
                            Some(_) => {
                                swing_first = true;
                                note.beats()
                            }
                            None => note.beats(),
                        };
                        if !note.is_rest {
                            let mut velocity = note
                                .dynamic
                                .as_ref()
                                .map(|d| d.to_velocity())
                                .unwrap_or(64u8);
                            let mut sounding_dur = dur;
                            for art in &note.articulations {
                                match art {
                                    Articulation::Staccato | Articulation::Staccatissimo => {
                                        sounding_dur *= 0.5;
                                    }
                                    Articulation::Accent | Articulation::Marcato => {
                                        velocity = velocity.saturating_add(20).min(127);
                                    }
                                    Articulation::Fermata => {
                                        sounding_dur *= options.fermata_multiplier;
                                    }
                                    _ => {}
                                }
                            }
                            let pedal = note.pedal_start;
                            let transpose = if part.midi_channel == 9 {
                                0i8
                            } else {
                                staff.transpose_semitones
                            };
                            for pitch in &note.pitches {
                                let midi = (pitch.to_midi() + transpose as i16).clamp(0, 127) as u8;
                                events.push(PlaybackEvent {
                                    time_beats: measure_start_beats + local_beats,
                                    time_secs: measure_start_secs + local_secs,
                                    pitch_midi: midi,
                                    velocity,
                                    duration_beats: sounding_dur,
                                    duration_secs: sounding_dur / current_bpm * 60.0,
                                    pedal,
                                    part_index,
                                    channel: part.midi_channel,
                                    is_metronome: false,
                                });
                            }
                        }
                        local_beats += dur;
                        local_secs += dur / current_bpm * 60.0;
                    }
                    // A voice may omit rests, but the next measure still starts
                    // at the notated bar boundary. This also keeps sparse voices
                    // aligned with compute_playback_position and other voices.
                    time_beats = measure_start_beats + measure_beats;
                    time_secs_cursor = measure_start_secs + measure_beats / current_bpm * 60.0;
                }
            }
        }
    }

    if let Some(ref metro) = options.metronome {
        let mut cursor_secs = 0.0f64;
        let mut cursor_beats = 0.0f64;
        let mut metro_bpm = bpm;
        for &idx in &seq {
            let first_staff = score.parts.first().and_then(|p| p.staves.first());
            if let Some(t) = first_staff
                .and_then(|s| s.measures.get(idx))
                .and_then(|m| m.tempo)
            {
                metro_bpm = t.max(1) as f64;
            }
            let ts = first_staff
                .and_then(|s| s.measures.get(idx))
                .and_then(|m| m.time_sig.as_ref())
                .unwrap_or(&score.settings.time_signature);
            let beat_unit = ts.beat_unit_beats();
            let num_beats = (ts.total_beats() / beat_unit).round() as u32;
            for b in 0..num_beats {
                let is_accent = b == 0;
                let beat_offset_secs = b as f64 * beat_unit / metro_bpm * 60.0;
                events.push(PlaybackEvent {
                    time_beats: cursor_beats + b as f64 * beat_unit,
                    time_secs: cursor_secs + beat_offset_secs,
                    pitch_midi: if is_accent {
                        metro.accent_pitch
                    } else {
                        metro.beat_pitch
                    },
                    velocity: if is_accent {
                        metro.accent_velocity
                    } else {
                        metro.beat_velocity
                    },
                    duration_beats: beat_unit * 0.1,
                    duration_secs: beat_unit * 0.1 / metro_bpm * 60.0,
                    pedal: false,
                    part_index: usize::MAX,
                    channel: metro.channel,
                    is_metronome: true,
                });
            }
            let measure_beats = ts.total_beats();
            cursor_secs += measure_beats / metro_bpm * 60.0;
            cursor_beats += measure_beats;
        }
    }

    events.sort_by(|a, b| {
        a.time_beats
            .partial_cmp(&b.time_beats)
            .unwrap_or(std::cmp::Ordering::Equal)
    });
    events
}

// ── PlaybackPosition + compute_playback_position ──────────────────────────────

/// Score position at a specific elapsed time.
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct PlaybackPosition {
    /// Physical measure index (0-based), same coordinate space as [`PlaybackEvent`] fields.
    pub measure_index: usize,
    /// Beat offset within the measure (`0.0 … time_sig.total_beats()`).
    pub beat: f64,
}

struct MeasureSegment {
    measure_idx: usize,
    start_secs: f64,
    duration_secs: f64,
    beats: f64,
    bpm: f64,
}

fn build_measure_segments(score: &Score, options: &PlaybackOptions) -> Vec<MeasureSegment> {
    let init_bpm = options
        .bpm_override
        .unwrap_or(score.settings.tempo_bpm)
        .max(1) as f64;
    let full_seq = measure_sequence(score);
    let seq: Vec<usize> = if let Some((lo, hi)) = options.loop_region {
        full_seq
            .into_iter()
            .filter(|&i| i >= lo && i <= hi)
            .collect()
    } else {
        full_seq
    };

    let mut segments = Vec::with_capacity(seq.len());
    let mut cursor_secs = 0.0f64;
    let mut current_bpm = init_bpm;

    for idx in seq {
        let first_measure = score
            .parts
            .first()
            .and_then(|p| p.staves.first())
            .and_then(|s| s.measures.get(idx));
        if let Some(t) = first_measure.and_then(|m| m.tempo) {
            current_bpm = t.max(1) as f64;
        }
        let ts = first_measure
            .and_then(|m| m.time_sig.as_ref())
            .unwrap_or(&score.settings.time_signature);
        let beats = ts.total_beats();
        let duration_secs = beats / current_bpm * 60.0;

        segments.push(MeasureSegment {
            measure_idx: idx,
            start_secs: cursor_secs,
            duration_secs,
            beats,
            bpm: current_bpm,
        });
        cursor_secs += duration_secs;
    }
    segments
}

/// Map `elapsed_secs` to a position within the score.
///
/// Returns `None` if `elapsed_secs` is negative or past the end of the last measure.
/// Pass the same [`PlaybackOptions`] used for [`to_playback_events`] so that `loop_region`
/// and tempo overrides are applied consistently.
pub fn compute_playback_position(
    score: &Score,
    options: &PlaybackOptions,
    elapsed_secs: f64,
) -> Option<PlaybackPosition> {
    if elapsed_secs < 0.0 {
        return None;
    }
    let segments = build_measure_segments(score, options);
    for seg in &segments {
        if elapsed_secs < seg.start_secs + seg.duration_secs + 1e-9 {
            let beat = ((elapsed_secs - seg.start_secs) * seg.bpm / 60.0).clamp(0.0, seg.beats);
            return Some(PlaybackPosition {
                measure_index: seg.measure_idx,
                beat,
            });
        }
    }
    None
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::model::{
        duration::Duration,
        pitch::{Pitch, Step},
        score::{Note, Score},
    };

    fn opts(bpm: Option<u16>) -> PlaybackOptions {
        PlaybackOptions {
            bpm_override: bpm,
            ..Default::default()
        }
    }

    #[test]
    fn empty_score_no_events() {
        let score = Score::new("T", 120, 4, 4, 0, 1);
        assert!(to_playback_events(&score, &opts(None)).is_empty());
    }

    #[test]
    fn single_note_at_beat_zero() {
        let mut score = Score::new("T", 120, 4, 4, 0, 1);
        score.parts[0].staves[0].measures[0].voices[0] =
            vec![Note::new(Pitch::new(Step::C, 4), Duration::Quarter)];
        let events = to_playback_events(&score, &opts(None));
        assert_eq!(events.len(), 1);
        assert!((events[0].time_beats).abs() < 1e-9);
        assert_eq!(events[0].pitch_midi, 60);
        assert_eq!(events[0].velocity, 64);
        assert!((events[0].duration_beats - 1.0).abs() < 1e-9);
        assert_eq!(events[0].part_index, 0);
    }

    #[test]
    fn chord_expands_to_multiple_events() {
        let mut score = Score::new("T", 120, 4, 4, 0, 1);
        let mut note = Note::new(Pitch::new(Step::C, 4), Duration::Quarter);
        note.pitches.push(Pitch::new(Step::E, 4));
        note.pitches.push(Pitch::new(Step::G, 4));
        score.parts[0].staves[0].measures[0].voices[0] = vec![note];
        let events = to_playback_events(&score, &opts(None));
        assert_eq!(events.len(), 3);
        assert!(events.iter().all(|e| e.time_beats.abs() < 1e-9));
    }

    #[test]
    fn grace_notes_excluded() {
        let mut score = Score::new("T", 120, 4, 4, 0, 1);
        let mut grace = Note::new(Pitch::new(Step::D, 4), Duration::Eighth);
        grace.is_grace = true;
        let regular = Note::new(Pitch::new(Step::C, 4), Duration::Quarter);
        score.parts[0].staves[0].measures[0].voices[0] = vec![grace, regular];
        let events = to_playback_events(&score, &opts(None));
        assert_eq!(events.len(), 1);
        assert_eq!(events[0].pitch_midi, 60);
    }

    #[test]
    fn second_note_has_correct_time() {
        let mut score = Score::new("T", 120, 4, 4, 0, 1);
        score.parts[0].staves[0].measures[0].voices[0] = vec![
            Note::new(Pitch::new(Step::C, 4), Duration::Quarter),
            Note::new(Pitch::new(Step::D, 4), Duration::Quarter),
        ];
        let events = to_playback_events(&score, &opts(None));
        assert_eq!(events.len(), 2);
        assert!((events[0].time_beats).abs() < 1e-9);
        assert!((events[1].time_beats - 1.0).abs() < 1e-9);
    }

    #[test]
    fn sparse_voice_keeps_measure_boundaries() {
        let mut score = Score::new("T", 120, 4, 4, 0, 2);
        let note = || Note::new(Pitch::new(Step::C, 4), Duration::Quarter);
        score.parts[0].staves[0].measures[0].voices[1] = vec![note()];
        score.parts[0].staves[0].measures[1].voices[1] = vec![note()];

        let events = to_playback_events(&score, &opts(None));
        assert_eq!(events.len(), 2);
        assert!((events[0].time_beats).abs() < 1e-9);
        assert!((events[1].time_beats - 4.0).abs() < 1e-9);
        assert!((events[1].time_secs - 2.0).abs() < 1e-9);
    }

    #[test]
    fn time_secs_120_bpm_quarter_note_is_half_second() {
        let mut score = Score::new("T", 120, 4, 4, 0, 1);
        score.parts[0].staves[0].measures[0].voices[0] =
            vec![Note::new(Pitch::new(Step::C, 4), Duration::Quarter)];
        let events = to_playback_events(&score, &opts(None));
        assert!((events[0].time_secs).abs() < 1e-9);
        assert!((events[0].duration_secs - 0.5).abs() < 1e-9);
    }

    #[test]
    fn bpm_override_changes_time_secs() {
        let mut score = Score::new("T", 120, 4, 4, 0, 1);
        score.parts[0].staves[0].measures[0].voices[0] = vec![
            Note::new(Pitch::new(Step::C, 4), Duration::Quarter),
            Note::new(Pitch::new(Step::D, 4), Duration::Quarter),
        ];
        let events = to_playback_events(&score, &opts(Some(60)));
        assert!((events[0].time_secs).abs() < 1e-9);
        assert!((events[1].time_secs - 1.0).abs() < 1e-9);
    }

    #[test]
    fn transpose_semitones_shifts_midi_output() {
        let mut score = Score::new("T", 120, 4, 4, 0, 1);
        score.parts[0].staves[0].transpose_semitones = -2;
        score.parts[0].staves[0].measures[0].voices[0] =
            vec![Note::new(Pitch::new(Step::C, 4), Duration::Quarter)];
        let events = to_playback_events(&score, &opts(None));
        assert_eq!(events.len(), 1);
        assert_eq!(events[0].pitch_midi, 58);
    }

    #[test]
    fn percussion_channel_9_ignores_transpose_semitones() {
        let mut score = Score::new("T", 120, 4, 4, 0, 1);
        score.parts[0].midi_channel = 9;
        score.parts[0].staves[0].transpose_semitones = -2;
        score.parts[0].staves[0].measures[0].voices[0] =
            vec![Note::new(Pitch::new(Step::C, 4), Duration::Quarter)];
        let events = to_playback_events(&score, &opts(None));
        assert_eq!(events.len(), 1);
        assert_eq!(events[0].pitch_midi, 60);
    }

    #[test]
    fn staccato_halves_duration_beats() {
        let mut score = Score::new("T", 120, 4, 4, 0, 1);
        let mut note = Note::new(Pitch::new(Step::C, 4), Duration::Quarter);
        note.articulations
            .push(crate::model::notation::Articulation::Staccato);
        score.parts[0].staves[0].measures[0].voices[0] = vec![note];
        let events = to_playback_events(&score, &opts(None));
        assert_eq!(events.len(), 1);
        assert!((events[0].duration_beats - 0.5).abs() < 1e-9);
    }

    #[test]
    fn staccatissimo_also_halves_duration() {
        let mut score = Score::new("T", 120, 4, 4, 0, 1);
        let mut note = Note::new(Pitch::new(Step::C, 4), Duration::Quarter);
        note.articulations
            .push(crate::model::notation::Articulation::Staccatissimo);
        score.parts[0].staves[0].measures[0].voices[0] = vec![note];
        let events = to_playback_events(&score, &opts(None));
        assert!((events[0].duration_beats - 0.5).abs() < 1e-9);
    }

    #[test]
    fn staccato_does_not_shift_next_note_time() {
        let mut score = Score::new("T", 120, 4, 4, 0, 1);
        let mut n1 = Note::new(Pitch::new(Step::C, 4), Duration::Quarter);
        n1.articulations
            .push(crate::model::notation::Articulation::Staccato);
        let n2 = Note::new(Pitch::new(Step::D, 4), Duration::Quarter);
        score.parts[0].staves[0].measures[0].voices[0] = vec![n1, n2];
        let events = to_playback_events(&score, &opts(None));
        assert_eq!(events.len(), 2);
        assert!((events[1].time_beats - 1.0).abs() < 1e-9);
    }

    #[test]
    fn accent_boosts_velocity_clamped() {
        let mut score = Score::new("T", 120, 4, 4, 0, 1);
        let mut note = Note::new(Pitch::new(Step::C, 4), Duration::Quarter);
        note.articulations
            .push(crate::model::notation::Articulation::Accent);
        score.parts[0].staves[0].measures[0].voices[0] = vec![note];
        let events = to_playback_events(&score, &opts(None));
        assert_eq!(events[0].velocity, 84);
    }

    #[test]
    fn accent_clamped_at_127() {
        let mut score = Score::new("T", 120, 4, 4, 0, 1);
        let mut note = Note::new(Pitch::new(Step::C, 4), Duration::Quarter);
        note.dynamic = Some(crate::model::notation::Dynamic::Ffff);
        note.articulations
            .push(crate::model::notation::Articulation::Accent);
        score.parts[0].staves[0].measures[0].voices[0] = vec![note];
        let events = to_playback_events(&score, &opts(None));
        assert_eq!(events[0].velocity, 127);
    }

    #[test]
    fn tenuto_keeps_full_duration() {
        let mut score = Score::new("T", 120, 4, 4, 0, 1);
        let mut note = Note::new(Pitch::new(Step::C, 4), Duration::Quarter);
        note.articulations
            .push(crate::model::notation::Articulation::Tenuto);
        score.parts[0].staves[0].measures[0].voices[0] = vec![note];
        let events = to_playback_events(&score, &opts(None));
        assert!((events[0].duration_beats - 1.0).abs() < 1e-9);
    }

    #[test]
    fn pedal_start_sets_pedal_field() {
        let mut score = Score::new("T", 120, 4, 4, 0, 1);
        let mut note = Note::new(Pitch::new(Step::C, 4), Duration::Quarter);
        note.pedal_start = true;
        score.parts[0].staves[0].measures[0].voices[0] = vec![note];
        let events = to_playback_events(&score, &opts(None));
        assert!(events[0].pedal);
    }

    #[test]
    fn no_pedal_start_pedal_is_false() {
        let mut score = Score::new("T", 120, 4, 4, 0, 1);
        score.parts[0].staves[0].measures[0].voices[0] =
            vec![Note::new(Pitch::new(Step::C, 4), Duration::Quarter)];
        let events = to_playback_events(&score, &opts(None));
        assert!(!events[0].pedal);
    }

    #[test]
    fn set_tempo_at_measure_changes_time_secs() {
        let mut score = Score::new("T", 120, 4, 4, 0, 2);
        score.parts[0].staves[0].measures[0].voices[0] =
            vec![Note::new(Pitch::new(Step::C, 4), Duration::Whole)];
        score.parts[0].staves[0].measures[1].tempo = Some(60);
        score.parts[0].staves[0].measures[1].voices[0] =
            vec![Note::new(Pitch::new(Step::D, 4), Duration::Whole)];
        let events = to_playback_events(&score, &opts(None));
        assert_eq!(events.len(), 2);
        assert!((events[0].time_secs).abs() < 1e-9);
        assert!((events[0].duration_secs - 2.0).abs() < 1e-9);
        assert!((events[1].time_secs - 2.0).abs() < 1e-9);
        assert!((events[1].duration_secs - 4.0).abs() < 1e-9);
    }

    #[test]
    fn muted_part_produces_no_events() {
        let mut score = Score::new("T", 120, 4, 4, 0, 1);
        score.parts[0].staves[0].measures[0].voices[0] =
            vec![Note::new(Pitch::new(Step::C, 4), Duration::Quarter)];
        let options = PlaybackOptions {
            muted_parts: vec![0],
            ..Default::default()
        };
        assert!(to_playback_events(&score, &options).is_empty());
    }

    #[test]
    fn part_index_field_set_correctly() {
        let mut score = Score::new("T", 120, 4, 4, 0, 1);
        score.parts[0].staves[0].measures[0].voices[0] =
            vec![Note::new(Pitch::new(Step::C, 4), Duration::Quarter)];
        let events = to_playback_events(&score, &opts(None));
        assert_eq!(events[0].part_index, 0);
    }

    // ── loop_region ───────────────────────────────────────────────────────────

    #[test]
    fn loop_region_filters_measures() {
        // 3 measures; notes in measures 0, 1, 2. Loop on [1,2] → only events from 1,2.
        let mut score = Score::new("T", 120, 4, 4, 0, 3);
        for mi in 0..3 {
            score.parts[0].staves[0].measures[mi].voices[0] =
                vec![Note::new(Pitch::new(Step::C, 4), Duration::Whole)];
        }
        let options = PlaybackOptions {
            loop_region: Some((1, 2)),
            ..Default::default()
        };
        let events = to_playback_events(&score, &options);
        assert_eq!(events.len(), 2);
        // First event in the region should start at beat 0 (region-relative)
        assert!((events[0].time_beats).abs() < 1e-9);
    }

    #[test]
    fn loop_region_none_plays_all_measures() {
        let mut score = Score::new("T", 120, 4, 4, 0, 3);
        for mi in 0..3 {
            score.parts[0].staves[0].measures[mi].voices[0] =
                vec![Note::new(Pitch::new(Step::C, 4), Duration::Whole)];
        }
        let events = to_playback_events(&score, &opts(None));
        assert_eq!(events.len(), 3);
    }

    // ── Fermata ───────────────────────────────────────────────────────────────

    #[test]
    fn fermata_multiplier_extends_duration() {
        let mut score = Score::new("T", 120, 4, 4, 0, 1);
        let mut note = Note::new(Pitch::new(Step::C, 4), Duration::Quarter);
        note.articulations
            .push(crate::model::notation::Articulation::Fermata);
        score.parts[0].staves[0].measures[0].voices[0] = vec![note];
        let options = PlaybackOptions {
            fermata_multiplier: 2.0,
            ..Default::default()
        };
        let events = to_playback_events(&score, &options);
        assert_eq!(events.len(), 1);
        assert!((events[0].duration_beats - 2.0).abs() < 1e-9);
    }

    #[test]
    fn fermata_default_multiplier_is_1_5() {
        let mut score = Score::new("T", 120, 4, 4, 0, 1);
        let mut note = Note::new(Pitch::new(Step::C, 4), Duration::Quarter);
        note.articulations
            .push(crate::model::notation::Articulation::Fermata);
        score.parts[0].staves[0].measures[0].voices[0] = vec![note];
        let events = to_playback_events(&score, &PlaybackOptions::default());
        assert_eq!(events.len(), 1);
        assert!((events[0].duration_beats - 1.5).abs() < 1e-9);
    }

    #[test]
    fn non_fermata_note_unaffected_by_fermata_multiplier() {
        let mut score = Score::new("T", 120, 4, 4, 0, 1);
        score.parts[0].staves[0].measures[0].voices[0] =
            vec![Note::new(Pitch::new(Step::C, 4), Duration::Quarter)];
        let options = PlaybackOptions {
            fermata_multiplier: 3.0,
            ..Default::default()
        };
        let events = to_playback_events(&score, &options);
        assert!((events[0].duration_beats - 1.0).abs() < 1e-9);
    }

    // ── swing ─────────────────────────────────────────────────────────────────

    #[test]
    fn swing_triplet_first_eighth_is_long() {
        // Two eighth notes in one measure; swing=0.67 → first=0.67, second=0.33
        let mut score = Score::new("T", 120, 4, 4, 0, 1);
        score.parts[0].staves[0].measures[0].voices[0] = vec![
            Note::new(Pitch::new(Step::C, 4), Duration::Eighth),
            Note::new(Pitch::new(Step::D, 4), Duration::Eighth),
        ];
        let options = PlaybackOptions {
            swing: Some(0.67),
            ..Default::default()
        };
        let events = to_playback_events(&score, &options);
        // events are sorted by time_beats; C comes first (time_beats=0), D second
        let e_c = events.iter().find(|e| e.pitch_midi == 60).unwrap();
        let e_d = events.iter().find(|e| e.pitch_midi == 62).unwrap();
        assert!(
            (e_c.duration_beats - 0.67).abs() < 1e-9,
            "first eighth should be 0.67"
        );
        assert!(
            (e_d.duration_beats - 0.33).abs() < 1e-9,
            "second eighth should be 0.33"
        );
        // D starts at 0.67, not 0.5
        assert!(
            (e_d.time_beats - 0.67).abs() < 1e-9,
            "second note start should be at 0.67"
        );
    }

    #[test]
    fn swing_non_eighth_not_affected() {
        let mut score = Score::new("T", 120, 4, 4, 0, 1);
        score.parts[0].staves[0].measures[0].voices[0] =
            vec![Note::new(Pitch::new(Step::C, 4), Duration::Quarter)];
        let options = PlaybackOptions {
            swing: Some(0.67),
            ..Default::default()
        };
        let events = to_playback_events(&score, &options);
        assert!(
            (events[0].duration_beats - 1.0).abs() < 1e-9,
            "quarter note unaffected"
        );
    }

    #[test]
    fn swing_none_is_straight() {
        let mut score = Score::new("T", 120, 4, 4, 0, 1);
        score.parts[0].staves[0].measures[0].voices[0] =
            vec![Note::new(Pitch::new(Step::C, 4), Duration::Eighth)];
        let options = PlaybackOptions {
            swing: None,
            ..Default::default()
        };
        let events = to_playback_events(&score, &options);
        assert!(
            (events[0].duration_beats - 0.5).abs() < 1e-9,
            "no swing = straight eighth"
        );
    }

    #[test]
    fn channel_matches_part_midi_channel() {
        let mut score = Score::new("T", 120, 4, 4, 0, 1);
        score.parts[0].midi_channel = 3;
        score.parts[0].staves[0].measures[0].voices[0] =
            vec![Note::new(Pitch::new(Step::C, 4), Duration::Quarter)];
        let events = to_playback_events(&score, &opts(None));
        assert_eq!(events[0].channel, 3);
    }

    #[test]
    fn swing_unit_default_is_eighth() {
        assert_eq!(PlaybackOptions::default().swing_unit, Duration::Eighth);
    }

    #[test]
    fn swing_unit_sixteenth() {
        let mut score = Score::new("T", 120, 4, 4, 0, 1);
        score.parts[0].staves[0].measures[0].voices[0] = vec![
            Note::new(Pitch::new(Step::C, 4), Duration::Sixteenth),
            Note::new(Pitch::new(Step::D, 4), Duration::Sixteenth),
        ];
        let options = PlaybackOptions {
            swing: Some(0.67),
            swing_unit: Duration::Sixteenth,
            ..Default::default()
        };
        let events = to_playback_events(&score, &options);
        let e_c = events.iter().find(|e| e.pitch_midi == 60).unwrap();
        let e_d = events.iter().find(|e| e.pitch_midi == 62).unwrap();
        assert!(
            (e_c.duration_beats - 0.335).abs() < 1e-9,
            "first 16th should be 0.335"
        );
        assert!(
            (e_d.duration_beats - 0.165).abs() < 1e-9,
            "second 16th should be 0.165"
        );
    }

    #[test]
    fn swing_resets_per_measure() {
        // 2 measures each with 2 eighth notes; each measure's first eighth should be "long"
        let mut score = Score::new("T", 120, 4, 4, 0, 2);
        let pair = || {
            vec![
                Note::new(Pitch::new(Step::C, 4), Duration::Eighth),
                Note::new(Pitch::new(Step::D, 4), Duration::Eighth),
            ]
        };
        score.parts[0].staves[0].measures[0].voices[0] = pair();
        score.parts[0].staves[0].measures[1].voices[0] = pair();
        let options = PlaybackOptions {
            swing: Some(0.67),
            ..Default::default()
        };
        let events = to_playback_events(&score, &options);
        // Four events sorted by time: m0-C, m0-D, m1-C, m1-D
        let durations: Vec<f64> = events.iter().map(|e| e.duration_beats).collect();
        // m0 first (long)
        assert!((durations[0] - 0.67).abs() < 1e-9, "m0 first note long");
        // m0 second (short)
        assert!((durations[1] - 0.33).abs() < 1e-9, "m0 second note short");
        // m1 first (long again — reset)
        assert!(
            (durations[2] - 0.67).abs() < 1e-9,
            "m1 first note long (reset)"
        );
        // m1 second (short)
        assert!((durations[3] - 0.33).abs() < 1e-9, "m1 second note short");
    }

    // ── compute_playback_position ─────────────────────────────────────────────

    #[test]
    fn playback_position_at_zero_is_measure_0_beat_0() {
        let score = Score::new("T", 120, 4, 4, 0, 4);
        let pos = compute_playback_position(&score, &PlaybackOptions::default(), 0.0).unwrap();
        assert_eq!(pos.measure_index, 0);
        assert!(pos.beat.abs() < 1e-9);
    }

    #[test]
    fn playback_position_at_half_measure_is_beat_2() {
        // 4/4, 120 BPM → 1 measure = 2.0 s; 0.5 s = beat 1.0
        let score = Score::new("T", 120, 4, 4, 0, 4);
        let pos = compute_playback_position(&score, &PlaybackOptions::default(), 0.5).unwrap();
        assert_eq!(pos.measure_index, 0);
        assert!(
            (pos.beat - 1.0).abs() < 1e-9,
            "expected beat 1.0, got {}",
            pos.beat
        );
    }

    #[test]
    fn playback_position_beyond_score_is_none() {
        // 4/4, 120 BPM, 1 measure = 2.0 s; 10.0 s is beyond
        let score = Score::new("T", 120, 4, 4, 0, 1);
        assert!(compute_playback_position(&score, &PlaybackOptions::default(), 10.0).is_none());
    }

    #[test]
    fn playback_position_tempo_change_takes_effect() {
        // measure 0: 120 BPM (2.0 s), measure 1: 60 BPM (4.0 s)
        // At elapsed=2.5 s → inside measure 1, 0.5 s into it → beat 0.5
        let mut score = Score::new("T", 120, 4, 4, 0, 2);
        score.parts[0].staves[0].measures[1].tempo = Some(60);
        let pos = compute_playback_position(&score, &PlaybackOptions::default(), 2.5).unwrap();
        assert_eq!(pos.measure_index, 1);
        assert!(
            (pos.beat - 0.5).abs() < 1e-9,
            "expected beat 0.5, got {}",
            pos.beat
        );
    }

    #[test]
    fn playback_position_loop_region_starts_at_zero() {
        // loop_region=[1,2] → elapsed=0 should map to measure 1, beat 0
        let score = Score::new("T", 120, 4, 4, 0, 4);
        let options = PlaybackOptions {
            loop_region: Some((1, 2)),
            ..Default::default()
        };
        let pos = compute_playback_position(&score, &options, 0.0).unwrap();
        assert_eq!(pos.measure_index, 1);
        assert!(pos.beat.abs() < 1e-9);
    }

    // ── MetronomeConfig ───────────────────────────────────────────────────────

    #[test]
    fn metronome_injects_beat_events() {
        // 4/4, 1 measure → should inject 4 metronome events
        let score = Score::new("T", 120, 4, 4, 0, 1);
        let options = PlaybackOptions {
            metronome: Some(MetronomeConfig::default()),
            ..Default::default()
        };
        let events = to_playback_events(&score, &options);
        let metro_events: Vec<_> = events.iter().filter(|e| e.is_metronome).collect();
        assert_eq!(metro_events.len(), 4, "expected 4 metronome clicks in 4/4");
    }

    #[test]
    fn metronome_accent_is_first_beat() {
        let score = Score::new("T", 120, 4, 4, 0, 1);
        let metro = MetronomeConfig::default();
        let options = PlaybackOptions {
            metronome: Some(metro.clone()),
            ..Default::default()
        };
        let events = to_playback_events(&score, &options);
        let mut metro_events: Vec<_> = events.iter().filter(|e| e.is_metronome).collect();
        metro_events.sort_by(|a, b| a.time_beats.partial_cmp(&b.time_beats).unwrap());
        assert_eq!(metro_events[0].pitch_midi, metro.accent_pitch);
        assert_eq!(metro_events[0].velocity, metro.accent_velocity);
    }

    #[test]
    fn metronome_regular_beat_pitch() {
        let score = Score::new("T", 120, 4, 4, 0, 1);
        let metro = MetronomeConfig::default();
        let options = PlaybackOptions {
            metronome: Some(metro.clone()),
            ..Default::default()
        };
        let events = to_playback_events(&score, &options);
        let mut metro_events: Vec<_> = events.iter().filter(|e| e.is_metronome).collect();
        metro_events.sort_by(|a, b| a.time_beats.partial_cmp(&b.time_beats).unwrap());
        for ev in &metro_events[1..] {
            assert_eq!(ev.pitch_midi, metro.beat_pitch);
            assert_eq!(ev.velocity, metro.beat_velocity);
        }
    }

    #[test]
    fn metronome_events_are_marked() {
        let score = Score::new("T", 120, 4, 4, 0, 1);
        let options = PlaybackOptions {
            metronome: Some(MetronomeConfig::default()),
            ..Default::default()
        };
        let events = to_playback_events(&score, &options);
        assert!(events.iter().any(|e| e.is_metronome));
    }

    #[test]
    fn metronome_none_produces_no_extra_events() {
        let score = Score::new("T", 120, 4, 4, 0, 1);
        let events = to_playback_events(&score, &PlaybackOptions::default());
        assert!(events.iter().all(|e| !e.is_metronome));
    }
}
