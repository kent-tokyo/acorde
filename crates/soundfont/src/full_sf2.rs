//! Full SF2 synthesis on the core's frame schedule, using RustySynth.

use std::{
    collections::{BTreeMap, BTreeSet},
    io::Cursor,
    sync::Arc,
};

use acorde_core::{
    Articulation, OfflineRenderFormat, OfflineRenderManifest, OfflineRenderSemanticEventKind,
};
use rustysynth::{SoundFont, Synthesizer, SynthesizerSettings};

use crate::{
    Error, MAX_ASSET_BYTES, MAX_OFFLINE_WAV_BYTES, MAX_POLYPHONY, SoundFontFormat, load,
    offline::pedal_intervals,
};

#[derive(Clone, Copy)]
enum Action {
    NoteOff {
        channel: u8,
        key: u8,
    },
    Pedal {
        channel: u8,
        down: bool,
    },
    NoteOn {
        channel: u8,
        program: u8,
        key: u8,
        velocity: u8,
    },
}

impl Action {
    fn priority(self) -> u8 {
        match self {
            Self::Pedal { down: true, .. } => 0,
            Self::NoteOff { .. } => 1,
            Self::Pedal { down: false, .. } => 2,
            Self::NoteOn { .. } => 3,
        }
    }
}

/// Render a score with the complete SF2 generator and filter path provided by RustySynth.
///
/// Acorde owns the exact frame schedule, validation, output bound, and PCM16 encoding. The
/// synthesizer runs in eight-frame blocks (its minimum), so an attack or release at an arbitrary
/// score frame can affect the output up to seven frames later. Reverb and chorus are disabled to
/// make note timing and the release tail deterministic. This path requires 16–192 kHz.
pub fn render_offline_wav_full_sf2(
    manifest: &OfflineRenderManifest,
    soundfont_bytes: &[u8],
) -> Result<Vec<u8>, Error> {
    manifest.validate().map_err(|_| Error::InvalidEvent)?;
    if manifest.format != OfflineRenderFormat::Wav {
        return Err(Error::UnsupportedOfflineFeature("audio format"));
    }
    if !(1..=2).contains(&manifest.channels) {
        return Err(Error::UnsupportedOfflineFeature("output channel count"));
    }
    if !(16_000..=192_000).contains(&manifest.sample_rate_hz) {
        return Err(Error::UnsupportedOfflineFeature("full SF2 sample rate"));
    }
    if manifest.frame_events.len() != manifest.events.len() {
        return Err(Error::UnsupportedOfflineFeature(
            "legacy schedule without frame events",
        ));
    }
    if soundfont_bytes.len() > MAX_ASSET_BYTES {
        return Err(Error::TooLarge(soundfont_bytes.len()));
    }
    if load(soundfont_bytes, "acorde-full-sf2-v1")?.format != SoundFontFormat::Sf2 {
        return Err(Error::UnsupportedOfflineFeature("SF3 sample decoding"));
    }
    for action in &manifest.semantic_events {
        if matches!(
            action.kind,
            OfflineRenderSemanticEventKind::Controller { .. }
                | OfflineRenderSemanticEventKind::PitchBend { .. }
                | OfflineRenderSemanticEventKind::Aftertouch { .. }
                | OfflineRenderSemanticEventKind::GuitarBend { .. }
        ) {
            return Err(Error::UnsupportedOfflineFeature("MIDI automation"));
        }
    }

    let frames = usize::try_from(manifest.duration_frames)
        .map_err(|_| Error::OfflineOutputTooLarge(MAX_OFFLINE_WAV_BYTES))?;
    let channels = usize::from(manifest.channels);
    let pcm_bytes = frames
        .checked_mul(channels)
        .and_then(|samples| samples.checked_mul(2))
        .ok_or(Error::OfflineOutputTooLarge(MAX_OFFLINE_WAV_BYTES))?;
    if pcm_bytes > MAX_OFFLINE_WAV_BYTES - 44 {
        return Err(Error::OfflineOutputTooLarge(MAX_OFFLINE_WAV_BYTES));
    }

    let sound_font = Arc::new(
        SoundFont::new(&mut Cursor::new(soundfont_bytes))
            .map_err(|error| Error::Decode(error.to_string()))?,
    );
    let mut settings = SynthesizerSettings::new(manifest.sample_rate_hz as i32);
    settings.block_size = 8;
    settings.maximum_polyphony = MAX_POLYPHONY;
    settings.enable_reverb_and_chorus = false;
    let mut synth = Synthesizer::new(&sound_font, &settings)
        .map_err(|error| Error::Decode(error.to_string()))?;
    let pedal_spans = pedal_intervals(manifest)?;
    // Isolate voices so an overlapping unison's note-off cannot release another voice.
    // Reserve every authored channel before assigning spare channels to duplicate lanes.
    let mut lanes = BTreeSet::<(usize, usize, usize, u8)>::new();
    for frame_event in &manifest.frame_events {
        let event = &frame_event.event;
        if frame_event.duration_frames == 0 {
            continue;
        }
        if event.channel >= 16 {
            return Err(Error::UnsupportedOfflineFeature("MIDI channel"));
        }
        let lane = (
            event.part_index,
            event.source.as_ref().map_or(0, |source| source.staff),
            event.source.as_ref().map_or(0, |source| source.voice),
            event.channel,
        );
        lanes.insert(lane);
    }
    let reserved = lanes.iter().map(|lane| lane.3).collect::<BTreeSet<_>>();
    let mut used = BTreeSet::new();
    let mut lane_channels = BTreeMap::new();
    let mut part_channels = BTreeMap::<usize, BTreeSet<u8>>::new();
    for lane in lanes {
        let requested = lane.3;
        let channel = if used.insert(requested) {
            requested
        } else {
            if requested == 9 {
                return Err(Error::UnsupportedOfflineFeature(
                    "multiple percussion voices",
                ));
            }
            let spare = (0..16)
                .find(|channel| {
                    *channel != 9 && !reserved.contains(channel) && !used.contains(channel)
                })
                .ok_or(Error::UnsupportedOfflineFeature(
                    "too many MIDI voice lanes",
                ))?;
            used.insert(spare);
            spare
        };
        lane_channels.insert(lane, channel);
        part_channels.entry(lane.0).or_default().insert(channel);
    }

    let mut actions = BTreeMap::<usize, Vec<Action>>::new();
    let mut active_keys = BTreeMap::<(u8, u8), usize>::new();
    let mut collision_channels = BTreeMap::<usize, BTreeSet<u8>>::new();
    let mut frame_events = manifest.frame_events.iter().collect::<Vec<_>>();
    frame_events.sort_by_key(|event| (event.start_frame, event.end_frame));
    for frame_event in frame_events {
        if frame_event.duration_frames == 0 {
            continue;
        }
        let event = &frame_event.event;
        let lane = (
            event.part_index,
            event.source.as_ref().map_or(0, |source| source.staff),
            event.source.as_ref().map_or(0, |source| source.voice),
            event.channel,
        );
        let &base_channel = lane_channels.get(&lane).ok_or(Error::InvalidEvent)?;
        if event.pitch_midi_cents != i32::from(event.pitch_midi) * 100
            || !event.pitch_bend_curve.is_empty()
            || event.guitar_technique.is_some()
            || event.articulations.iter().any(|mark| {
                !matches!(
                    mark,
                    Articulation::Staccato
                        | Articulation::Staccatissimo
                        | Articulation::Accent
                        | Articulation::Marcato
                        | Articulation::Fermata
                )
            })
        {
            return Err(Error::UnsupportedOfflineFeature(
                "unrealized note expression",
            ));
        }
        let start = usize::try_from(frame_event.start_frame).map_err(|_| Error::InvalidEvent)?;
        let end = usize::try_from(frame_event.end_frame).map_err(|_| Error::InvalidEvent)?;
        if end > frames || start >= end {
            return Err(Error::InvalidEvent);
        }
        if event.pedal
            && !pedal_spans
                .get(&event.part_index)
                .is_some_and(|spans| spans.iter().any(|(down, up)| *down <= start && start < *up))
        {
            return Err(Error::UnsupportedOfflineFeature(
                "pedal without routed span",
            ));
        }
        let collides = |channel: u8| {
            active_keys
                .get(&(channel, event.pitch_midi))
                .is_some_and(|previous_end| start < *previous_end)
        };
        let channel = if collides(base_channel) {
            if base_channel == 9 {
                return Err(Error::UnsupportedOfflineFeature(
                    "overlapping percussion pitch",
                ));
            }
            if let Some(&spare) = collision_channels
                .get(&event.part_index)
                .and_then(|channels| channels.iter().find(|&&channel| !collides(channel)))
            {
                spare
            } else {
                let spare = (0..16)
                    .find(|channel| *channel != 9 && !used.contains(channel))
                    .ok_or(Error::UnsupportedOfflineFeature(
                        "too many overlapping score voices",
                    ))?;
                used.insert(spare);
                collision_channels
                    .entry(event.part_index)
                    .or_default()
                    .insert(spare);
                part_channels
                    .entry(event.part_index)
                    .or_default()
                    .insert(spare);
                spare
            }
        } else {
            base_channel
        };
        active_keys.insert((channel, event.pitch_midi), end);
        let bank = if channel == 9 { 128 } else { 0 };
        if !sound_font.get_presets().iter().any(|preset| {
            preset.get_bank_number() == bank
                && preset.get_patch_number() == i32::from(event.program)
        }) {
            return Err(Error::PresetNotFound {
                bank: bank as u16,
                program: u16::from(event.program),
            });
        }
        actions.entry(start).or_default().push(Action::NoteOn {
            channel,
            program: event.program,
            key: event.pitch_midi,
            velocity: event.velocity,
        });
        actions.entry(end).or_default().push(Action::NoteOff {
            channel,
            key: event.pitch_midi,
        });
    }
    for (part, spans) in pedal_spans {
        let Some(channels) = part_channels.get(&part) else {
            return Err(Error::UnsupportedOfflineFeature("pedal part without notes"));
        };
        for (down, up) in spans {
            for (frame, pressed) in [(down, true), (up, false)] {
                if frame < frames {
                    for &channel in channels {
                        actions.entry(frame).or_default().push(Action::Pedal {
                            channel,
                            down: pressed,
                        });
                    }
                }
            }
        }
    }

    let mut wav = Vec::with_capacity(44 + pcm_bytes);
    wav.extend_from_slice(b"RIFF");
    wav.extend_from_slice(&(36 + pcm_bytes as u32).to_le_bytes());
    wav.extend_from_slice(b"WAVEfmt ");
    wav.extend_from_slice(&16_u32.to_le_bytes());
    wav.extend_from_slice(&1_u16.to_le_bytes());
    wav.extend_from_slice(&u16::from(manifest.channels).to_le_bytes());
    wav.extend_from_slice(&manifest.sample_rate_hz.to_le_bytes());
    wav.extend_from_slice(
        &(manifest.sample_rate_hz * u32::from(manifest.channels) * 2).to_le_bytes(),
    );
    wav.extend_from_slice(&(u16::from(manifest.channels) * 2).to_le_bytes());
    wav.extend_from_slice(&16_u16.to_le_bytes());
    wav.extend_from_slice(b"data");
    wav.extend_from_slice(&(pcm_bytes as u32).to_le_bytes());

    let mut cursor = 0;
    for (frame, mut frame_actions) in actions {
        if frame > cursor {
            render_segment(&mut synth, &mut wav, frame - cursor, channels)?;
            cursor = frame;
        }
        frame_actions.sort_by_key(|action| action.priority());
        for action in frame_actions {
            match action {
                Action::NoteOff { channel, key } => {
                    synth.note_off(i32::from(channel), i32::from(key))
                }
                Action::Pedal { channel, down } => synth.process_midi_message(
                    i32::from(channel),
                    0xB0,
                    64,
                    if down { 127 } else { 0 },
                ),
                Action::NoteOn {
                    channel,
                    program,
                    key,
                    velocity,
                } => {
                    synth.process_midi_message(i32::from(channel), 0xC0, i32::from(program), 0);
                    synth.note_on(i32::from(channel), i32::from(key), i32::from(velocity));
                }
            }
        }
    }
    if cursor < frames {
        render_segment(&mut synth, &mut wav, frames - cursor, channels)?;
    }
    Ok(wav)
}

fn render_segment(
    synth: &mut Synthesizer,
    wav: &mut Vec<u8>,
    frames: usize,
    channels: usize,
) -> Result<(), Error> {
    let mut left = [0.0_f32; 4096];
    let mut right = [0.0_f32; 4096];
    let mut remaining = frames;
    while remaining > 0 {
        let count = remaining.min(left.len());
        synth.render(&mut left[..count], &mut right[..count]);
        for index in 0..count {
            if !left[index].is_finite() || !right[index].is_finite() {
                return Err(Error::Decode("non-finite SF2 synthesis output".into()));
            }
            let to_pcm = |sample: f32| (sample * 32767.0).round().clamp(-32768.0, 32767.0) as i16;
            let mono = (left[index] + right[index]) * 0.5;
            wav.extend_from_slice(
                &to_pcm(if channels == 1 { mono } else { left[index] }).to_le_bytes(),
            );
            if channels == 2 {
                wav.extend_from_slice(&to_pcm(right[index]).to_le_bytes());
            }
        }
        remaining -= count;
    }
    Ok(())
}

#[cfg(test)]
mod tests {
    use acorde_core::{
        Duration, Note, OfflineRenderRequest, Pitch, PlaybackOptions, Score, Step,
        build_offline_render_manifest,
    };

    use super::*;

    const PIANO: &[u8] =
        include_bytes!("../../../tests/fixtures/UprightPianoKW-small-20190703.sf2");

    fn manifest(score: &Score) -> OfflineRenderManifest {
        build_offline_render_manifest(
            score,
            &PlaybackOptions::default(),
            &OfflineRenderRequest {
                sample_rate_hz: 16_000,
                channels: 1,
                ..OfflineRenderRequest::default()
            },
        )
        .expect("frame schedule")
    }

    #[test]
    fn full_sf2_renders_silence_before_the_first_attack() {
        let mut score = Score::new("Full SF2", 120, 4, 4, 0, 1);
        score.parts[0].staves[0].measures[0].voices[0] = vec![
            Note::rest(Duration::Quarter),
            Note::new(Pitch::new(Step::C, 4), Duration::Quarter),
        ];
        let schedule = manifest(&score);
        assert_eq!(schedule.frame_events[0].start_frame, 8_000);
        let wav = render_offline_wav_full_sf2(&schedule, PIANO).expect("SF2 WAV");
        assert_eq!(wav.len(), 44 + schedule.duration_frames as usize * 2);
        assert!(wav[44..44 + 8_000 * 2].iter().all(|byte| *byte == 0));
        assert!(wav[44 + 8_000 * 2..].iter().any(|byte| *byte != 0));
    }

    #[test]
    fn full_sf2_isolates_overlapping_same_pitch_after_fermata() {
        let mut score = Score::new("Overlapping unison", 120, 4, 4, 0, 1);
        let mut first = Note::new(Pitch::new(Step::A, 4), Duration::Quarter);
        first.articulations.push(Articulation::Fermata);
        score.parts[0].staves[0].measures[0].voices[0] =
            vec![first, Note::new(Pitch::new(Step::A, 4), Duration::Quarter)];
        let schedule = manifest(&score);
        assert!(schedule.frame_events[0].end_frame > schedule.frame_events[1].start_frame);
        let wav = render_offline_wav_full_sf2(&schedule, PIANO).expect("overlapping WAV");
        assert_eq!(wav.len(), 44 + schedule.duration_frames as usize * 2);
        assert!(wav[44..].iter().any(|byte| *byte != 0));
    }

    #[test]
    fn full_sf2_rejects_out_of_range_sample_rate() {
        let mut score = Score::new("Rate", 120, 4, 4, 0, 1);
        score.parts[0].staves[0].measures[0].voices[0] =
            vec![Note::new(Pitch::new(Step::C, 4), Duration::Quarter)];
        let mut schedule = manifest(&score);
        schedule.sample_rate_hz = 8_000;
        assert_eq!(
            render_offline_wav_full_sf2(&schedule, PIANO),
            Err(Error::UnsupportedOfflineFeature("full SF2 sample rate"))
        );
    }

    #[test]
    fn full_sf2_pedal_holds_sound_across_a_rest() {
        let mut score = Score::new("Pedal", 120, 4, 4, 0, 1);
        let mut note = Note::new(Pitch::new(Step::C, 4), Duration::Quarter);
        note.pedal_start = true;
        let mut release_note = Note::new(Pitch::new(Step::D, 4), Duration::Quarter);
        release_note.pedal_end = true;
        score.parts[0].staves[0].measures[0].voices[0] =
            vec![note, Note::rest(Duration::Quarter), release_note];
        let held_manifest = manifest(&score);
        let held = render_offline_wav_full_sf2(&held_manifest, PIANO).unwrap();
        let mut missing_pedal = held_manifest.clone();
        missing_pedal
            .semantic_events
            .retain(|event| !matches!(event.kind, OfflineRenderSemanticEventKind::Pedal { .. }));
        assert_eq!(
            render_offline_wav_full_sf2(&missing_pedal, PIANO),
            Err(Error::UnsupportedOfflineFeature(
                "pedal without routed span"
            ))
        );
        score.parts[0].staves[0].measures[0].voices[0][0].pedal_start = false;
        score.parts[0].staves[0].measures[0].voices[0][2].pedal_end = false;
        let dry = render_offline_wav_full_sf2(&manifest(&score), PIANO).unwrap();
        let gap_energy = |wav: &[u8]| -> i64 {
            wav[44 + 12_000 * 2..44 + 14_000 * 2]
                .as_chunks::<2>()
                .0
                .iter()
                .map(|bytes| i64::from(i16::from_le_bytes(*bytes)).abs())
                .sum()
        };
        assert!(gap_energy(&held) > gap_energy(&dry) * 2);
    }
}
