//! Bounded native WAV rendering from the core's sample-frame schedule.

use std::collections::BTreeMap;

use acorde_core::{
    Articulation, OfflineRenderFormat, OfflineRenderManifest, OfflineRenderSemanticEventKind,
};

use crate::{
    DecodedSampleRegion, Error, MAX_POLYPHONY, load_materialized, render_sample_region_action,
    schedule_sample_note_on, select_preset_zones,
};

/// Maximum complete PCM16 WAV file size produced by one render.
pub const MAX_OFFLINE_WAV_BYTES: usize = 64 * 1024 * 1024;

/// Render a validated score schedule with a caller-supplied SF2 SoundFont into PCM16 WAV.
///
/// The frame schedule owns note onset and release positions. This deliberately rejects
/// unsupported performance controls instead of silently claiming audio equivalence.
/// Sample loops, tuning, gain, and attack/decay/sustain use the existing bounded
/// SoundFont voice renderer. Note-off release and score pedal spans are mixed on the
/// integer frame clock. MIDI automation and SF3 are not yet implemented here.
pub fn render_offline_wav(
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
    if manifest.frame_events.len() != manifest.events.len() {
        return Err(Error::UnsupportedOfflineFeature(
            "legacy schedule without frame events",
        ));
    }
    for action in &manifest.semantic_events {
        if matches!(
            action.kind,
            OfflineRenderSemanticEventKind::Controller { .. }
                | OfflineRenderSemanticEventKind::PitchBend { .. }
                | OfflineRenderSemanticEventKind::ProgramChange { .. }
                | OfflineRenderSemanticEventKind::Aftertouch { .. }
                | OfflineRenderSemanticEventKind::GuitarBend { .. }
        ) {
            return Err(Error::UnsupportedOfflineFeature("MIDI automation"));
        }
    }

    let channels = usize::from(manifest.channels);
    let frame_count = usize::try_from(manifest.duration_frames)
        .map_err(|_| Error::OfflineOutputTooLarge(MAX_OFFLINE_WAV_BYTES))?;
    let pcm_bytes = frame_count
        .checked_mul(channels)
        .and_then(|samples| samples.checked_mul(2))
        .ok_or(Error::OfflineOutputTooLarge(MAX_OFFLINE_WAV_BYTES))?;
    if pcm_bytes > MAX_OFFLINE_WAV_BYTES - 44 {
        return Err(Error::OfflineOutputTooLarge(MAX_OFFLINE_WAV_BYTES));
    }
    let pedal_intervals = pedal_intervals(manifest)?;
    let asset = load_materialized(soundfont_bytes, "acorde-native-wav-v1")?;
    if asset.asset.format != crate::SoundFontFormat::Sf2 {
        return Err(Error::UnsupportedOfflineFeature("SF3 sample decoding"));
    }
    let mut mix = vec![0.0_f32; frame_count * channels];
    let mut decoded = BTreeMap::<(u64, u32, u32, u8), DecodedSampleRegion>::new();
    let mut voice_id = 1_u64;
    for frame_event in &manifest.frame_events {
        if frame_event.duration_frames == 0 {
            continue;
        }
        let event = &frame_event.event;
        if event.pitch_midi_cents != i32::from(event.pitch_midi) * 100
            || !event.pitch_bend_curve.is_empty()
            || event.guitar_technique.is_some()
            || event.articulations.iter().any(|mark| {
                !matches!(
                    mark,
                    Articulation::Staccato
                        | Articulation::Staccatissimo
                        | Articulation::Accent
                        | Articulation::Tenuto
                        | Articulation::Marcato
                        | Articulation::Fermata
                )
            })
        {
            return Err(Error::UnsupportedOfflineFeature(
                "unrealized note expression",
            ));
        }
        let bank = if event.channel == 9 { 128 } else { 0 };
        let zones = select_preset_zones(
            asset.zones(),
            bank,
            u16::from(event.program),
            event.pitch_midi,
            event.velocity,
        );
        if zones.is_empty() {
            return Err(Error::PresetNotFound {
                bank,
                program: u16::from(event.program),
            });
        }
        if zones.len() > MAX_POLYPHONY {
            return Err(Error::InvalidEvent);
        }
        let start = usize::try_from(frame_event.start_frame).map_err(|_| Error::InvalidEvent)?;
        if event.pedal
            && !pedal_intervals
                .get(&event.part_index)
                .is_some_and(|intervals| {
                    intervals
                        .iter()
                        .any(|(down, up)| *down <= start && start < *up)
                })
        {
            return Err(Error::UnsupportedOfflineFeature(
                "pedal without routed span",
            ));
        }
        let key_off = usize::try_from(frame_event.end_frame).map_err(|_| Error::InvalidEvent)?;
        let release_start = pedal_intervals
            .get(&event.part_index)
            .and_then(|intervals| {
                intervals
                    .iter()
                    .find(|(down, up)| *down <= key_off && key_off < *up)
                    .map(|(_, up)| *up)
            })
            .unwrap_or(key_off);
        for zone in zones {
            let metadata = zone.resolved_metadata();
            let key = (
                metadata.sample_id,
                metadata.start_frame,
                metadata.end_frame,
                metadata.decode_channels,
            );
            if let std::collections::btree_map::Entry::Vacant(entry) = decoded.entry(key) {
                entry.insert(metadata.decode_sample_region(soundfont_bytes)?);
            }
            let sample = decoded.get(&key).ok_or(Error::InvalidSample)?;
            let release_frames = (f64::from(zone.region.release_secs)
                * f64::from(manifest.sample_rate_hz))
            .ceil() as usize;
            let release_frames = release_frames.min(frame_count.saturating_sub(release_start));
            let render_frames = release_start - start + release_frames;
            let mut timed_event = event.clone();
            timed_event.duration_secs = render_frames as f64 / f64::from(manifest.sample_rate_hz);
            let action = schedule_sample_note_on(voice_id, timed_event, &zone.region, 1.0)?;
            voice_id = voice_id.checked_add(1).ok_or(Error::InvalidEvent)?;
            let rendered = render_sample_region_action(sample, &action, manifest.sample_rate_hz)?;
            let source_channels = usize::from(sample.sample.channels);
            let available_frames = rendered.len() / source_channels;
            if available_frames < render_frames {
                return Err(Error::InvalidEvent);
            }
            for local in 0..render_frames {
                let output = (start + local) * channels;
                let source = local * source_channels;
                let envelope = release_gain(start + local, release_start, release_frames);
                if channels == 1 {
                    let value = if source_channels == 1 {
                        f32::from(rendered[source])
                    } else {
                        (f32::from(rendered[source]) + f32::from(rendered[source + 1])) * 0.5
                    };
                    mix[output] += value * envelope;
                } else if source_channels == 1 {
                    let value = f32::from(rendered[source]);
                    mix[output] += value * envelope;
                    mix[output + 1] += value * envelope;
                } else {
                    mix[output] += f32::from(rendered[source]) * envelope;
                    mix[output + 1] += f32::from(rendered[source + 1]) * envelope;
                }
            }
        }
    }
    encode_wav(&mix, manifest.sample_rate_hz, manifest.channels)
}

fn release_gain(frame: usize, release_start: usize, release_frames: usize) -> f32 {
    if frame < release_start {
        1.0
    } else if release_frames == 0 || frame - release_start >= release_frames {
        0.0
    } else {
        let progress = (frame - release_start) as f32 / release_frames as f32;
        2.0_f32.powf(-16.0 * progress)
    }
}

pub(super) fn pedal_intervals(
    manifest: &OfflineRenderManifest,
) -> Result<BTreeMap<usize, Vec<(usize, usize)>>, Error> {
    let mut changes = BTreeMap::<usize, Vec<(usize, bool)>>::new();
    for action in &manifest.semantic_events {
        let OfflineRenderSemanticEventKind::Pedal { down } = action.kind else {
            continue;
        };
        let part = action
            .source
            .as_ref()
            .map(|source| source.part)
            .or_else(|| action.measure.as_ref().map(|measure| measure.part))
            .ok_or(Error::UnsupportedOfflineFeature("unrouted pedal"))?;
        let frame = usize::try_from(action.frame).map_err(|_| Error::InvalidEvent)?;
        changes.entry(part).or_default().push((frame, down));
    }
    let content_end = usize::try_from(manifest.duration_frames - manifest.release_tail_frames)
        .map_err(|_| Error::InvalidEvent)?;
    let mut intervals = BTreeMap::new();
    for (part, mut events) in changes {
        events.sort_unstable_by_key(|(frame, down)| (*frame, *down));
        let mut active = None;
        let mut part_intervals = Vec::new();
        for (frame, down) in events {
            if down {
                active.get_or_insert(frame);
            } else if let Some(start) = active.take() {
                part_intervals.push((start, frame));
            }
        }
        if let Some(start) = active {
            part_intervals.push((start, content_end));
        }
        intervals.insert(part, part_intervals);
    }
    Ok(intervals)
}

fn encode_wav(mix: &[f32], sample_rate_hz: u32, channels: u8) -> Result<Vec<u8>, Error> {
    let data_bytes = mix
        .len()
        .checked_mul(2)
        .ok_or(Error::OfflineOutputTooLarge(MAX_OFFLINE_WAV_BYTES))?;
    let data_len = u32::try_from(data_bytes)
        .map_err(|_| Error::OfflineOutputTooLarge(MAX_OFFLINE_WAV_BYTES))?;
    let byte_rate = sample_rate_hz
        .checked_mul(u32::from(channels) * 2)
        .ok_or(Error::InvalidConfig)?;
    let peak = mix.iter().map(|value| value.abs()).fold(0.0_f32, f32::max);
    let gain = if peak > f32::from(i16::MAX) {
        f32::from(i16::MAX) / peak
    } else {
        1.0
    };
    let mut wav = Vec::with_capacity(44 + data_bytes);
    wav.extend_from_slice(b"RIFF");
    wav.extend_from_slice(&(36 + data_len).to_le_bytes());
    wav.extend_from_slice(b"WAVEfmt ");
    wav.extend_from_slice(&16_u32.to_le_bytes());
    wav.extend_from_slice(&1_u16.to_le_bytes());
    wav.extend_from_slice(&u16::from(channels).to_le_bytes());
    wav.extend_from_slice(&sample_rate_hz.to_le_bytes());
    wav.extend_from_slice(&byte_rate.to_le_bytes());
    wav.extend_from_slice(&(u16::from(channels) * 2).to_le_bytes());
    wav.extend_from_slice(&16_u16.to_le_bytes());
    wav.extend_from_slice(b"data");
    wav.extend_from_slice(&data_len.to_le_bytes());
    for value in mix {
        let pcm = (*value * gain).round().clamp(-32768.0, 32767.0) as i16;
        wav.extend_from_slice(&pcm.to_le_bytes());
    }
    Ok(wav)
}

#[cfg(test)]
mod tests {
    use acorde_core::{
        Articulation, Duration, Note, OfflineRenderRequest, Pitch, PlaybackOptions, Score, Step,
        build_offline_render_manifest,
    };

    use super::*;

    #[test]
    fn native_sf2_tenuto_uses_the_full_scheduled_note_duration() {
        let mut score = Score::new("Tenuto", 120, 4, 4, 0, 1);
        score.parts[0].staves[0].measures[0].voices[0] =
            vec![Note::new(Pitch::new(Step::C, 4), Duration::Quarter)];
        let request = OfflineRenderRequest {
            sample_rate_hz: 8_000,
            channels: 1,
            ..Default::default()
        };
        let sf2 = include_bytes!("../../../tests/fixtures/UprightPianoKW-small-20190703.sf2");
        let plain = build_offline_render_manifest(&score, &PlaybackOptions::default(), &request)
            .expect("plain schedule");
        let plain_wav = render_offline_wav(&plain, sf2).expect("plain WAV");

        score.parts[0].staves[0].measures[0].voices[0][0]
            .articulations
            .push(Articulation::Tenuto);
        let tenuto = build_offline_render_manifest(&score, &PlaybackOptions::default(), &request)
            .expect("tenuto schedule");
        assert_eq!(
            tenuto.frame_events[0].duration_frames,
            plain.frame_events[0].duration_frames
        );
        assert_eq!(
            render_offline_wav(&tenuto, sf2).expect("tenuto WAV"),
            plain_wav
        );
    }

    #[test]
    fn renders_real_sf2_after_exact_silent_frame_prefix() {
        let mut score = Score::new("SF2 WAV", 120, 4, 4, 0, 1);
        score.parts[0].staves[0].measures[0].voices[0] = vec![
            Note::rest(Duration::Quarter),
            Note::new(Pitch::new(Step::C, 4), Duration::Quarter),
        ];
        let manifest = build_offline_render_manifest(
            &score,
            &PlaybackOptions::default(),
            &OfflineRenderRequest {
                sample_rate_hz: 8_000,
                channels: 1,
                ..OfflineRenderRequest::default()
            },
        )
        .expect("frame schedule");
        assert_eq!(manifest.frame_events[0].start_frame, 4_000);
        let wav = render_offline_wav(
            &manifest,
            include_bytes!("../../../tests/fixtures/UprightPianoKW-small-20190703.sf2"),
        )
        .expect("audible SF2 WAV");
        assert_eq!(&wav[..4], b"RIFF");
        assert_eq!(&wav[8..16], b"WAVEfmt ");
        assert_eq!(&wav[36..40], b"data");
        assert_eq!(wav.len(), 44 + manifest.duration_frames as usize * 2);
        let pcm = wav[44..]
            .as_chunks::<2>()
            .0
            .iter()
            .map(|bytes| i16::from_le_bytes([bytes[0], bytes[1]]))
            .collect::<Vec<_>>();
        assert!(pcm[..4_000].iter().all(|sample| *sample == 0));
        assert!(pcm[4_000..].iter().any(|sample| *sample != 0));
    }

    #[test]
    fn wav_mix_normalizes_overlapping_samples_without_clipping() {
        let wav = encode_wav(&[0.0, 40_000.0, -40_000.0], 8_000, 1).unwrap();
        let pcm = wav[44..]
            .as_chunks::<2>()
            .0
            .iter()
            .map(|bytes| i16::from_le_bytes([bytes[0], bytes[1]]))
            .collect::<Vec<_>>();
        assert_eq!(pcm, [0, i16::MAX, -i16::MAX]);
    }

    #[test]
    fn release_tail_contains_decay_then_silence() {
        let mut score = Score::new("Release", 120, 4, 4, 0, 1);
        score.parts[0].staves[0].measures[0].voices[0] =
            vec![Note::new(Pitch::new(Step::C, 4), Duration::Quarter)];
        let manifest = build_offline_render_manifest(
            &score,
            &PlaybackOptions::default(),
            &OfflineRenderRequest {
                sample_rate_hz: 8_000,
                channels: 1,
                release_tail_millis: 1_000,
                ..OfflineRenderRequest::default()
            },
        )
        .unwrap();
        let wav = render_offline_wav(
            &manifest,
            include_bytes!("../../../tests/fixtures/UprightPianoKW-small-20190703.sf2"),
        )
        .unwrap();
        let pcm = wav[44..]
            .as_chunks::<2>()
            .0
            .iter()
            .map(|bytes| i16::from_le_bytes(*bytes))
            .collect::<Vec<_>>();
        assert_eq!(pcm.len(), 12_000);
        assert!(pcm[4_000..5_000].iter().any(|sample| *sample != 0));
        assert!(pcm[10_000..].iter().all(|sample| *sample == 0));
    }

    #[test]
    fn pedal_holds_note_until_the_marked_up_frame() {
        let mut score = Score::new("Pedal", 120, 4, 4, 0, 1);
        let mut note = Note::new(Pitch::new(Step::C, 4), Duration::Quarter);
        note.pedal_start = true;
        let mut release_note = Note::new(Pitch::new(Step::D, 4), Duration::Quarter);
        release_note.pedal_end = true;
        score.parts[0].staves[0].measures[0].voices[0] =
            vec![note, Note::rest(Duration::Quarter), release_note];
        let request = OfflineRenderRequest {
            sample_rate_hz: 8_000,
            channels: 1,
            release_tail_millis: 1_000,
            ..OfflineRenderRequest::default()
        };
        let manifest =
            build_offline_render_manifest(&score, &PlaybackOptions::default(), &request).unwrap();
        assert_eq!(pedal_intervals(&manifest).unwrap()[&0], [(0, 8_000)]);
        let sf2 = include_bytes!("../../../tests/fixtures/UprightPianoKW-small-20190703.sf2");
        let held = render_offline_wav(&manifest, sf2).unwrap();
        let mut missing_pedal = manifest.clone();
        missing_pedal
            .semantic_events
            .retain(|action| !matches!(action.kind, OfflineRenderSemanticEventKind::Pedal { .. }));
        assert_eq!(
            render_offline_wav(&missing_pedal, sf2),
            Err(Error::UnsupportedOfflineFeature(
                "pedal without routed span"
            ))
        );
        score.parts[0].staves[0].measures[0].voices[0][0].pedal_start = false;
        score.parts[0].staves[0].measures[0].voices[0][2].pedal_end = false;
        let dry_manifest =
            build_offline_render_manifest(&score, &PlaybackOptions::default(), &request).unwrap();
        let dry = render_offline_wav(&dry_manifest, sf2).unwrap();
        let gap_energy = |wav: &[u8]| -> i64 {
            wav[44 + 6_000 * 2..44 + 7_000 * 2]
                .as_chunks::<2>()
                .0
                .iter()
                .map(|bytes| i64::from(i16::from_le_bytes(*bytes)).abs())
                .sum()
        };
        assert!(gap_energy(&held) > gap_energy(&dry) * 2);
    }
}
