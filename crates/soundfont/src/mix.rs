//! Deterministic, score-independent post-render mixing for native PCM16 WAV.

use acorde_core::{OfflineRenderManifest, OfflineRenderSemanticEventKind};
use serde::{Deserialize, Serialize};
use sha2::{Digest, Sha256};

use crate::{Error, offline};

/// The electric piano option selects GM Electric Piano 2 (program 3) from the
/// caller's identified SF2. It does not claim a live FM synthesizer.
#[derive(Debug, Clone, PartialEq, Serialize, Deserialize, Default)]
#[serde(deny_unknown_fields)]
pub struct MixRenderConfig {
    #[serde(default)]
    pub electric_piano_parts: Vec<usize>,
    #[serde(default)]
    pub detune: Option<DetuneConfig>,
    #[serde(default)]
    pub reverb: Option<ReverbConfig>,
    #[serde(default)]
    pub sidechain: Option<SidechainConfig>,
}

#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
#[serde(deny_unknown_fields)]
pub struct DetuneConfig {
    pub cents: u8,
    pub wet: f32,
}

#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
#[serde(deny_unknown_fields)]
pub struct ReverbConfig {
    pub room_ms: u16,
    pub feedback: f32,
    pub wet: f32,
}

#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
#[serde(deny_unknown_fields)]
pub struct SidechainConfig {
    pub kick_part_index: usize,
    /// Explicit frame positions from the stage-5 kick part, strictly increasing.
    pub kick_frames: Vec<u64>,
    pub depth: f32,
    pub release_ms: u16,
}

#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
pub struct MixRenderReport {
    pub soundfont_sha256: String,
    pub manifest_sha256: String,
    pub wav_sha256: String,
    pub config: MixRenderConfig,
    /// Peak before PCM16 gain limiting, relative to 32768.
    pub pre_encode_peak: f32,
    pub clipping_detected: bool,
    pub cutoff_detected: bool,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct MixRenderResult {
    pub wav: Vec<u8>,
    pub report: MixRenderReport,
}

fn sha256(bytes: &[u8]) -> String {
    const HEX: &[u8; 16] = b"0123456789abcdef";
    let mut out = String::with_capacity(64);
    for byte in Sha256::digest(bytes) {
        out.push(char::from(HEX[usize::from(byte >> 4)]));
        out.push(char::from(HEX[usize::from(byte & 0x0f)]));
    }
    out
}

fn valid_fraction(value: f32) -> bool {
    value.is_finite() && (0.0..=1.0).contains(&value)
}

fn validate_config(
    manifest: &OfflineRenderManifest,
    config: &MixRenderConfig,
) -> Result<(), Error> {
    if config.electric_piano_parts.len() > 32
        || config.electric_piano_parts.iter().any(|part| {
            !manifest
                .frame_events
                .iter()
                .any(|frame| frame.event.part_index == *part)
                || manifest
                    .frame_events
                    .iter()
                    .any(|frame| frame.event.part_index == *part && frame.event.channel == 9)
        })
        || config
            .electric_piano_parts
            .windows(2)
            .any(|pair| pair[0] >= pair[1])
        || (!config.electric_piano_parts.is_empty()
            && manifest.semantic_events.iter().any(|event| {
                matches!(
                    event.kind,
                    OfflineRenderSemanticEventKind::ProgramChange { .. }
                )
            }))
    {
        return Err(Error::InvalidConfig);
    }
    if let Some(detune) = &config.detune
        && (!(1..=25).contains(&detune.cents) || !valid_fraction(detune.wet) || detune.wet == 0.0)
    {
        return Err(Error::InvalidConfig);
    }
    if let Some(reverb) = &config.reverb
        && (!(20..=100).contains(&reverb.room_ms)
            || !valid_fraction(reverb.feedback)
            || reverb.feedback > 0.85
            || !valid_fraction(reverb.wet)
            || reverb.wet == 0.0
            || manifest.release_tail_frames
                < u64::from(manifest.sample_rate_hz) * u64::from(reverb.room_ms) / 500)
    {
        return Err(Error::InvalidConfig);
    }
    if let Some(sidechain) = &config.sidechain {
        if sidechain.kick_frames.is_empty()
            || sidechain.kick_frames.len() > 4096
            || !valid_fraction(sidechain.depth)
            || sidechain.depth == 0.0
            || !(10..=2000).contains(&sidechain.release_ms)
            || sidechain
                .kick_frames
                .windows(2)
                .any(|pair| pair[0] >= pair[1])
            || sidechain
                .kick_frames
                .last()
                .is_some_and(|last| *last >= manifest.duration_frames)
        {
            return Err(Error::InvalidConfig);
        }
        let actual = kick_frames(manifest, sidechain.kick_part_index);
        if actual != sidechain.kick_frames {
            return Err(Error::InvalidConfig);
        }
    }
    Ok(())
}

/// Frame positions of every kick attack in a caller-selected stage-5 part.
pub fn kick_frames(manifest: &OfflineRenderManifest, kick_part_index: usize) -> Vec<u64> {
    manifest
        .frame_events
        .iter()
        .filter(|frame| frame.event.part_index == kick_part_index && frame.event.pitch_midi == 36)
        .map(|frame| frame.start_frame)
        .collect()
}

fn detune(mix: &mut [f32], channels: usize, sample_rate: u32, config: &DetuneConfig) {
    let input = mix.to_vec();
    let period = (f64::from(sample_rate) * 2.0).round() as usize;
    let ratio = 2.0_f64.powf(f64::from(config.cents) / 1200.0) - 1.0;
    let depth = (ratio * f64::from(sample_rate) / 2.0).ceil();
    let depth = depth as usize;
    for frame in 0..mix.len() / channels {
        let phase = (frame % period) as f64 / period as f64;
        let triangle = if phase < 0.5 {
            phase * 2.0
        } else {
            (1.0 - phase) * 2.0
        };
        let delay = 2.0 + depth as f64 * (2.0 * triangle);
        let from = frame as f64 - delay;
        if from < 0.0 {
            continue;
        }
        let low = from.floor() as usize;
        let fraction = (from - low as f64) as f32;
        for channel in 0..channels {
            let delayed = input[low * channels + channel] * (1.0 - fraction)
                + input[(low + 1) * channels + channel] * fraction;
            let index = frame * channels + channel;
            mix[index] = input[index] * (1.0 - config.wet) + delayed * config.wet;
        }
    }
}

fn reverb(mix: &mut [f32], channels: usize, sample_rate: u32, config: &ReverbConfig) {
    let input = mix.to_vec();
    let frames = mix.len() / channels;
    let base = usize::from(config.room_ms) * sample_rate as usize / 1000;
    let delays = [base, base * 7 / 6, base * 4 / 3, base * 3 / 2];
    let mut feedback = vec![0.0_f32; mix.len()];
    for frame in 0..frames {
        for channel in 0..channels {
            let index = frame * channels + channel;
            let mut wet = 0.0;
            for delay in delays {
                if frame >= delay {
                    wet += feedback[(frame - delay) * channels + channel] * 0.25;
                }
            }
            feedback[index] = input[index] + wet * config.feedback;
            mix[index] = input[index] * (1.0 - config.wet) + wet * config.wet;
        }
    }
}

fn sidechain(mix: &mut [f32], channels: usize, sample_rate: u32, config: &SidechainConfig) {
    let release = (u64::from(sample_rate) * u64::from(config.release_ms) / 1000).max(1);
    let mut kick_index = 0;
    let mut last = None;
    for frame in 0..mix.len() / channels {
        while config
            .kick_frames
            .get(kick_index)
            .is_some_and(|kick| *kick <= frame as u64)
        {
            last = Some(config.kick_frames[kick_index]);
            kick_index += 1;
        }
        let gain = last.map_or(1.0, |kick| {
            let elapsed = frame as u64 - kick;
            let amount = 1.0 - (elapsed.min(release) as f32 / release as f32);
            1.0 - config.depth * amount
        });
        for channel in 0..channels {
            mix[frame * channels + channel] *= gain;
        }
    }
}

fn analyze_mix(mix: &[f32], channels: usize, sample_rate: u32) -> (f32, bool, bool) {
    let peak = mix.iter().map(|value| value.abs()).fold(0.0_f32, f32::max);
    let tail_frames = (sample_rate as usize / 100).min(mix.len() / channels);
    let cutoff_peak = mix[mix.len() - tail_frames * channels..]
        .iter()
        .map(|value| value.abs())
        .fold(0.0_f32, f32::max);
    (peak / 32768.0, peak >= 32767.0, cutoff_peak > 327.68)
}

/// Render with a fixed SF2 asset and separate settings. With every option off,
/// the WAV bytes exactly match `render_offline_wav`.
pub fn render_offline_mix_wav(
    manifest: &OfflineRenderManifest,
    soundfont_bytes: &[u8],
    config: &MixRenderConfig,
) -> Result<MixRenderResult, Error> {
    validate_config(manifest, config)?;
    let manifest_hash = sha256(&serde_json::to_vec(manifest).map_err(|_| Error::InvalidConfig)?);
    let mut routed = manifest.clone();
    for part in &config.electric_piano_parts {
        for event in &mut routed.events {
            if event.part_index == *part {
                event.program = 3;
            }
        }
        for frame in &mut routed.frame_events {
            if frame.event.part_index == *part {
                frame.event.program = 3;
            }
        }
    }
    let split_part = config
        .sidechain
        .as_ref()
        .map(|sidechain| sidechain.kick_part_index);
    let (mut mix, split) = offline::render_offline_pcm(&routed, soundfont_bytes, split_part)?;
    let channels = usize::from(manifest.channels);
    if let Some(detune_config) = &config.detune {
        detune(&mut mix, channels, manifest.sample_rate_hz, detune_config);
    }
    if let Some(reverb_config) = &config.reverb {
        reverb(&mut mix, channels, manifest.sample_rate_hz, reverb_config);
    }
    if let Some(sidechain_config) = &config.sidechain {
        sidechain(
            &mut mix,
            channels,
            manifest.sample_rate_hz,
            sidechain_config,
        );
    }
    if let Some(kick_mix) = split {
        for (value, kick) in mix.iter_mut().zip(kick_mix) {
            *value += kick;
        }
    }
    let (pre_encode_peak, clipping_detected, cutoff_detected) =
        analyze_mix(&mix, channels, manifest.sample_rate_hz);
    let wav = offline::encode_wav(&mix, manifest.sample_rate_hz, manifest.channels)?;
    Ok(MixRenderResult {
        report: MixRenderReport {
            soundfont_sha256: sha256(soundfont_bytes),
            manifest_sha256: manifest_hash,
            wav_sha256: sha256(&wav),
            config: config.clone(),
            pre_encode_peak,
            clipping_detected,
            cutoff_detected,
        },
        wav,
    })
}

#[cfg(feature = "full-sf2")]
fn decode_canonical_wav(wav: &[u8], expected_samples: usize) -> Result<Vec<f32>, Error> {
    if wav.len() != 44 + expected_samples * 2
        || &wav[0..4] != b"RIFF"
        || &wav[8..16] != b"WAVEfmt "
        || &wav[36..40] != b"data"
    {
        return Err(Error::InvalidSample);
    }
    Ok(wav[44..]
        .chunks_exact(2)
        .map(|sample| f32::from(i16::from_le_bytes([sample[0], sample[1]])))
        .collect())
}

#[cfg(feature = "full-sf2")]
fn filtered_manifest(
    manifest: &OfflineRenderManifest,
    part: usize,
    keep: bool,
) -> OfflineRenderManifest {
    let mut selected = manifest.clone();
    selected
        .frame_events
        .retain(|frame| (frame.event.part_index == part) == keep);
    selected.events = selected
        .frame_events
        .iter()
        .map(|frame| frame.event.clone())
        .collect();
    selected.semantic_events.clear();
    let latest = selected
        .frame_events
        .iter()
        .map(|frame| frame.end_frame)
        .max()
        .unwrap_or(0);
    selected.release_tail_frames = selected.duration_frames.saturating_sub(latest);
    selected
}

/// Render through the complete SF2 synthesizer, then apply the selected mix.
/// A sidechain uses separate kick and accompaniment renders, so kick attacks
/// remain outside the ducked bus. No score or MusicXML fields are changed.
#[cfg(feature = "full-sf2")]
pub fn render_offline_mix_wav_full_sf2(
    manifest: &OfflineRenderManifest,
    soundfont_bytes: &[u8],
    config: &MixRenderConfig,
) -> Result<MixRenderResult, Error> {
    validate_config(manifest, config)?;
    if config.sidechain.is_some()
        && manifest.semantic_events.iter().any(|event| {
            matches!(
                event.kind,
                OfflineRenderSemanticEventKind::Controller { .. }
                    | OfflineRenderSemanticEventKind::PitchBend { .. }
                    | OfflineRenderSemanticEventKind::ProgramChange { .. }
                    | OfflineRenderSemanticEventKind::Pedal { .. }
                    | OfflineRenderSemanticEventKind::Aftertouch { .. }
                    | OfflineRenderSemanticEventKind::GuitarBend { .. }
            )
        })
    {
        return Err(Error::UnsupportedOfflineFeature(
            "sidechain with MIDI automation",
        ));
    }
    let manifest_hash = sha256(&serde_json::to_vec(manifest).map_err(|_| Error::InvalidConfig)?);
    let mut routed = manifest.clone();
    for part in &config.electric_piano_parts {
        for event in &mut routed.events {
            if event.part_index == *part {
                event.program = 3;
            }
        }
        for frame in &mut routed.frame_events {
            if frame.event.part_index == *part {
                frame.event.program = 3;
            }
        }
    }
    let channels = usize::from(manifest.channels);
    let samples = usize::try_from(manifest.duration_frames)
        .ok()
        .and_then(|frames| frames.checked_mul(channels))
        .ok_or(Error::InvalidConfig)?;
    let rendered = if let Some(sidechain_config) = &config.sidechain {
        let main = filtered_manifest(&routed, sidechain_config.kick_part_index, false);
        let kick = filtered_manifest(&routed, sidechain_config.kick_part_index, true);
        let main_wav = crate::render_offline_wav_full_sf2(&main, soundfont_bytes)?;
        let kick_wav = crate::render_offline_wav_full_sf2(&kick, soundfont_bytes)?;
        (main_wav, Some(kick_wav))
    } else {
        (
            crate::render_offline_wav_full_sf2(&routed, soundfont_bytes)?,
            None,
        )
    };
    let mut mix = decode_canonical_wav(&rendered.0, samples)?;
    if let Some(detune_config) = &config.detune {
        detune(&mut mix, channels, manifest.sample_rate_hz, detune_config);
    }
    if let Some(reverb_config) = &config.reverb {
        reverb(&mut mix, channels, manifest.sample_rate_hz, reverb_config);
    }
    if let Some(sidechain_config) = &config.sidechain {
        sidechain(
            &mut mix,
            channels,
            manifest.sample_rate_hz,
            sidechain_config,
        );
    }
    if let Some(kick_wav) = rendered.1 {
        let kick = decode_canonical_wav(&kick_wav, samples)?;
        for (value, kick_sample) in mix.iter_mut().zip(kick) {
            *value += kick_sample;
        }
    }
    let (pre_encode_peak, clipping_detected, cutoff_detected) =
        analyze_mix(&mix, channels, manifest.sample_rate_hz);
    let wav = if config.detune.is_none() && config.reverb.is_none() && config.sidechain.is_none() {
        rendered.0
    } else {
        offline::encode_wav(&mix, manifest.sample_rate_hz, manifest.channels)?
    };
    Ok(MixRenderResult {
        report: MixRenderReport {
            soundfont_sha256: sha256(soundfont_bytes),
            manifest_sha256: manifest_hash,
            wav_sha256: sha256(&wav),
            config: config.clone(),
            pre_encode_peak,
            clipping_detected,
            cutoff_detected,
        },
        wav,
    })
}

#[cfg(all(test, feature = "full-sf2"))]
#[allow(dead_code)]
#[path = "../../../tests/fixtures/arrangement/rhythm_section.rs"]
mod fixture;

#[cfg(all(test, feature = "full-sf2"))]
mod tests {
    use acorde_core::{
        OfflineRenderRequest, PlaybackOptions, arrange_rhythm_section,
        build_offline_render_manifest,
    };

    use super::*;

    const SF2: &[u8] = include_bytes!("../../../tests/fixtures/arrangement/mix-audition.sf2");

    fn input() -> (OfflineRenderManifest, usize) {
        let source = fixture::source();
        let arranged = arrange_rhythm_section(&source, &fixture::request(false)).unwrap();
        let manifest = build_offline_render_manifest(
            &arranged.score,
            &PlaybackOptions::default(),
            &OfflineRenderRequest {
                sample_rate_hz: 22_050,
                channels: 2,
                release_tail_millis: 1_000,
                ..Default::default()
            },
        )
        .unwrap();
        (manifest, arranged.kick_part)
    }

    #[test]
    fn disabled_mix_matches_legacy_wav_exactly_and_reports_source() {
        let (manifest, _) = input();
        let legacy = crate::render_offline_wav_full_sf2(&manifest, SF2).unwrap();
        let config = MixRenderConfig::default();
        let first = render_offline_mix_wav_full_sf2(&manifest, SF2, &config).unwrap();
        let second = render_offline_mix_wav_full_sf2(&manifest, SF2, &config).unwrap();
        assert_eq!(first.wav, legacy);
        assert_eq!(first.wav, second.wav);
        assert_eq!(first.report, second.report);
        assert_eq!(first.report.soundfont_sha256.len(), 64);
        assert_eq!(first.report.wav_sha256.len(), 64);
    }

    #[test]
    fn preset_and_each_effect_change_audio_independently() {
        let (manifest, kick_part) = input();
        let base =
            render_offline_mix_wav_full_sf2(&manifest, SF2, &MixRenderConfig::default()).unwrap();
        let kick_frames = kick_frames(&manifest, kick_part);
        assert!(!kick_frames.is_empty());
        let variants = [
            MixRenderConfig {
                electric_piano_parts: vec![0],
                ..Default::default()
            },
            MixRenderConfig {
                detune: Some(DetuneConfig {
                    cents: 12,
                    wet: 0.35,
                }),
                ..Default::default()
            },
            MixRenderConfig {
                reverb: Some(ReverbConfig {
                    room_ms: 45,
                    feedback: 0.55,
                    wet: 0.25,
                }),
                ..Default::default()
            },
            MixRenderConfig {
                sidechain: Some(SidechainConfig {
                    kick_part_index: kick_part,
                    kick_frames: kick_frames.clone(),
                    depth: 0.6,
                    release_ms: 250,
                }),
                ..Default::default()
            },
        ];
        for config in variants {
            let result = render_offline_mix_wav_full_sf2(&manifest, SF2, &config).unwrap();
            assert_eq!(result.wav.len(), base.wav.len());
            assert_ne!(result.wav, base.wav);
            assert_eq!(
                result.wav,
                render_offline_mix_wav_full_sf2(&manifest, SF2, &config)
                    .unwrap()
                    .wav
            );
            assert_eq!(result.report.config, config);
        }
        let invalid = MixRenderConfig {
            sidechain: Some(SidechainConfig {
                kick_part_index: kick_part,
                kick_frames: vec![1],
                depth: 0.6,
                release_ms: 250,
            }),
            ..Default::default()
        };
        assert!(matches!(
            render_offline_mix_wav_full_sf2(&manifest, SF2, &invalid),
            Err(Error::InvalidConfig)
        ));
    }

    #[test]
    fn signal_report_detects_rail_and_non_silent_tail() {
        assert_eq!(analyze_mix(&vec![0.0; 800], 1, 8_000), (0.0, false, false));
        let mut signal = vec![0.0; 800];
        signal[100] = 40_000.0;
        signal[799] = 500.0;
        let (peak, clipped, cutoff) = analyze_mix(&signal, 1, 8_000);
        assert!(peak > 1.0);
        assert!(clipped);
        assert!(cutoff);
    }
}
