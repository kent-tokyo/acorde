//! A bounded, provider-neutral boundary between `acorde` playback events and a
//! SoundFont renderer.
//!
//! This crate validates SF2/SF3 metadata and produces lifecycle actions. It
//! intentionally does not decode samples or ship a synthesizer; applications
//! choose a licensed renderer on the other side of this stable boundary.

use acorde_core::PlaybackEvent;

pub const PLAYBACK_CONTRACT_VERSION: u16 = 1;
/// Version of the provider capability/decoder/renderer adapter contract.
pub const PROVIDER_CONTRACT_VERSION: u16 = 1;
pub const MAX_ASSET_BYTES: usize = 64 * 1024 * 1024;
pub const MAX_PRESETS: usize = 16_384;
pub const MAX_POLYPHONY: usize = 256;
pub const MAX_DECODED_FRAMES: usize = 16_000_000;

/// Generator names understood by the provider-neutral zone contract.
pub const SUPPORTED_GENERATORS: &[&str] = &[
    "keyRange",
    "velRange",
    "overridingRootKey",
    "fineTune",
    "initialAttenuation",
    "sampleModes",
    "startAddrsOffset",
    "endAddrsOffset",
    "startloopAddrsOffset",
    "endloopAddrsOffset",
];

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum SoundFontFormat {
    Sf2,
    Sf3,
}

/// Compression handled by a provider. Decoding remains outside this crate.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum SampleCompression {
    Pcm16,
    Vorbis,
}

/// Capabilities advertised by a separately licensed SoundFont provider.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub struct ProviderCapabilities {
    pub sf2_pcm16: bool,
    pub sf3_vorbis: bool,
    pub synthesis: bool,
}

impl ProviderCapabilities {
    pub const fn metadata_only() -> Self {
        Self {
            sf2_pcm16: false,
            sf3_vorbis: false,
            synthesis: false,
        }
    }
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub struct SampleLoop {
    pub start_frame: u32,
    pub end_frame: u32,
}

/// A provider-neutral SF2/SF3 sample region selected by key and velocity.
#[derive(Debug, Clone, PartialEq)]
pub struct SampleRegion {
    pub sample_id: u64,
    /// Half-open source-frame range in the provider-owned sample payload.
    pub start_frame: u32,
    pub end_frame: u32,
    pub key_min: u8,
    pub key_max: u8,
    pub velocity_min: u8,
    pub velocity_max: u8,
    pub root_key: u8,
    pub fine_tune_cents: i16,
    pub attenuation_db: f32,
    pub sample_rate: u32,
    pub compression: SampleCompression,
    pub loop_points: Option<SampleLoop>,
    pub attack_secs: f32,
    pub decay_secs: f32,
    pub sustain_level: f32,
    pub release_secs: f32,
}

impl SampleRegion {
    pub fn contains(&self, key: u8, velocity: u8) -> bool {
        key >= self.key_min
            && key <= self.key_max
            && velocity >= self.velocity_min
            && velocity <= self.velocity_max
    }
}

/// Validate the bounded generator values needed to turn one zone into a playable voice.
pub fn validate_sample_region(region: &SampleRegion) -> Result<(), Error> {
    if region.key_min > region.key_max {
        return Err(Error::InvalidZone("key range"));
    }
    if region.velocity_min == 0 || region.velocity_min > region.velocity_max {
        return Err(Error::InvalidZone("velocity range"));
    }
    if region.start_frame >= region.end_frame
        || u64::from(region.end_frame) > MAX_DECODED_FRAMES as u64
    {
        return Err(Error::InvalidZone("sample frame range"));
    }
    if region.sample_rate == 0
        || !region.attenuation_db.is_finite()
        || !region.attack_secs.is_finite()
        || !region.decay_secs.is_finite()
        || !region.sustain_level.is_finite()
        || !region.release_secs.is_finite()
        || region.attack_secs < 0.0
        || region.decay_secs < 0.0
        || region.release_secs < 0.0
        || !(0.0..=1.0).contains(&region.sustain_level)
    {
        return Err(Error::InvalidZone("sample/envelope parameters"));
    }
    if let Some(loop_points) = region.loop_points
        && (loop_points.start_frame >= loop_points.end_frame
            || loop_points.end_frame as u64 > MAX_DECODED_FRAMES as u64)
    {
        return Err(Error::InvalidZone("loop points"));
    }
    Ok(())
}

/// Reject a parsed SoundFont generator that this boundary cannot represent safely.
pub fn validate_generator_name(name: &str) -> Result<(), Error> {
    if SUPPORTED_GENERATORS.contains(&name) {
        Ok(())
    } else {
        Err(Error::UnsupportedGenerator(name.to_string()))
    }
}

/// Selects a region deterministically: narrower key/velocity zones win, then sample ID.
pub fn select_sample_region(
    regions: &[SampleRegion],
    key: u8,
    velocity: u8,
) -> Option<&SampleRegion> {
    regions
        .iter()
        .filter(|region| region.contains(key, velocity))
        .min_by_key(|region| {
            (
                u16::from(region.key_max.saturating_sub(region.key_min)),
                u16::from(region.velocity_max.saturating_sub(region.velocity_min)),
                region.sample_id,
            )
        })
}

/// A provider-neutral mapping from one bank/program preset to a playable sample zone.
///
/// SF2 generators or SF3 metadata are interpreted by the provider and materialized here;
/// consumers can then select zones without reimplementing SoundFont parsing.
#[derive(Debug, Clone, PartialEq)]
pub struct SoundFontPresetZone {
    pub bank: u16,
    pub program: u16,
    pub region: SampleRegion,
}

impl SoundFontPresetZone {
    pub fn new(bank: u16, program: u16, region: SampleRegion) -> Result<Self, Error> {
        validate_sample_region(&region)?;
        Ok(Self {
            bank,
            program,
            region,
        })
    }

    pub fn contains(&self, bank: u16, program: u16, key: u8, velocity: u8) -> bool {
        self.bank == bank && self.program == program && self.region.contains(key, velocity)
    }
}

/// Select a preset zone deterministically for a bank/program/key/velocity tuple.
pub fn select_preset_zone(
    zones: &[SoundFontPresetZone],
    bank: u16,
    program: u16,
    key: u8,
    velocity: u8,
) -> Option<&SoundFontPresetZone> {
    zones
        .iter()
        .filter(|zone| zone.contains(bank, program, key, velocity))
        .min_by_key(|zone| {
            (
                u16::from(zone.region.key_max.saturating_sub(zone.region.key_min)),
                u16::from(
                    zone.region
                        .velocity_max
                        .saturating_sub(zone.region.velocity_min),
                ),
                zone.region.sample_id,
            )
        })
}

/// Select a bank/program/key/velocity zone and build its provider-neutral voice plan.
///
/// This is the single mapping entry point intended for Composer and other hosts:
/// SF2/SF3 providers materialize [`SoundFontPresetZone`] values once, then callers
/// do not need to duplicate preset or generator selection logic.
pub fn schedule_preset_note_on(
    voice_id: u64,
    event: PlaybackEvent,
    zones: &[SoundFontPresetZone],
    bank: u16,
    program: u16,
    velocity_exponent: f32,
) -> Result<SampleAction, Error> {
    let zone = select_preset_zone(zones, bank, program, event.pitch_midi, event.velocity)
        .ok_or(Error::InvalidSample)?;
    schedule_sample_note_on(voice_id, event, &zone.region, velocity_exponent)
}

/// Converts MIDI velocity to a deterministic linear gain with a configurable exponent.
pub fn velocity_gain(velocity: u8, exponent: f32) -> Result<f32, Error> {
    if velocity == 0 || !exponent.is_finite() || exponent <= 0.0 {
        return Err(Error::InvalidConfig);
    }
    Ok((f32::from(velocity) / 127.0).powf(exponent))
}

/// Validated PCM returned by a separately licensed decoder.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct DecodedSample {
    pub sample_rate: u32,
    pub channels: u8,
    pub pcm_i16: Vec<i16>,
}

/// A separately licensed provider that decodes a selected region into PCM.
///
/// Implementations own codec dependencies, asset access, and licensing. This
/// crate only receives the bounded, validated PCM result.
pub trait SampleDecoder {
    type Error: std::error::Error + Send + Sync + 'static;

    fn decode(&self, region: &SampleRegion) -> Result<DecodedSample, Self::Error>;
}

/// A host/provider-owned renderer for validated PCM and scheduled voice actions.
///
/// Audio output, device access, and backend-specific state remain outside the
/// reusable score library while hosts retain a stable integration point.
pub trait SampleRenderer {
    type Error: std::error::Error + Send + Sync + 'static;

    fn render(&mut self, sample: &DecodedSample, action: &SampleAction) -> Result<(), Self::Error>;
}

/// Complete adapter contract for an optional, separately licensed provider.
///
/// The provider owns SF2/SF3 parsing, codec dependencies, licensed assets, and audio output.
/// A host can inspect [`Self::capabilities`] before selecting a region and receives the
/// provider's explicit error for unsupported codec, generator, or license conditions.
pub trait SoundFontProvider {
    type Error: std::error::Error + Send + Sync + 'static;

    fn capabilities(&self) -> ProviderCapabilities;
    fn decode(&self, region: &SampleRegion) -> Result<DecodedSample, Self::Error>;
    fn render(&mut self, sample: &DecodedSample, action: &SampleAction) -> Result<(), Self::Error>;
}

/// Validate that a region can be handed to the advertised provider capabilities.
pub fn validate_provider_capabilities(
    region: &SampleRegion,
    capabilities: ProviderCapabilities,
) -> Result<(), Error> {
    let supported = match region.compression {
        SampleCompression::Pcm16 => capabilities.sf2_pcm16,
        SampleCompression::Vorbis => capabilities.sf3_vorbis,
    };
    if !supported {
        return Err(Error::UnsupportedCompression(region.compression));
    }
    if !capabilities.synthesis {
        return Err(Error::UnsupportedProviderCapability("synthesis"));
    }
    Ok(())
}

impl DecodedSample {
    pub fn new(sample_rate: u32, channels: u8, pcm_i16: Vec<i16>) -> Result<Self, Error> {
        if sample_rate == 0 || channels == 0 || channels > 2 || pcm_i16.is_empty() {
            return Err(Error::InvalidSample);
        }
        let frames = pcm_i16.len() / usize::from(channels);
        if frames == 0
            || frames > MAX_DECODED_FRAMES
            || !pcm_i16.len().is_multiple_of(usize::from(channels))
        {
            return Err(Error::SampleTooLarge(frames));
        }
        Ok(Self {
            sample_rate,
            channels,
            pcm_i16,
        })
    }
}

/// Decode an interleaved little-endian PCM16 sample from an SF2 `smpl` chunk.
///
/// `start_frame..end_frame` is half-open. The SF2 container and sample data are supplied by
/// the host, so this dependency-free path does not read files or bundle assets.
pub fn decode_sf2_pcm16(
    data: &[u8],
    start_frame: usize,
    end_frame: usize,
    sample_rate: u32,
    channels: u8,
) -> Result<DecodedSample, Error> {
    if sample_rate == 0 || !(1..=2).contains(&channels) || start_frame >= end_frame {
        return Err(Error::InvalidSample);
    }
    let bytes = find_riff_chunk(data, b"smpl").ok_or(Error::InvalidHeader)?;
    let frame_bytes = usize::from(channels) * 2;
    let start = start_frame
        .checked_mul(frame_bytes)
        .ok_or(Error::SampleTooLarge(start_frame))?;
    let end = end_frame
        .checked_mul(frame_bytes)
        .ok_or(Error::SampleTooLarge(end_frame))?;
    if end > bytes.len() || start >= end {
        return Err(Error::Truncated);
    }
    let pcm = bytes[start..end]
        .chunks_exact(2)
        .map(|pair| i16::from_le_bytes([pair[0], pair[1]]))
        .collect();
    DecodedSample::new(sample_rate, channels, pcm)
}

/// Decode an SF3 Vorbis payload when the separately licensed `sf3-vorbis` feature is enabled.
#[cfg(feature = "sf3-vorbis")]
pub fn decode_sf3_vorbis(
    data: &[u8],
    sample_rate: u32,
    channels: u8,
) -> Result<DecodedSample, Error> {
    use std::io::Cursor;
    if sample_rate == 0 || !(1..=2).contains(&channels) {
        return Err(Error::InvalidSample);
    }
    let payload = find_ogg_payload(data).ok_or(Error::InvalidHeader)?;
    let mut reader = lewton::inside_ogg::OggStreamReader::new(Cursor::new(payload))
        .map_err(|_| Error::Decode("invalid SF3 Vorbis stream".to_string()))?;
    if reader.ident_hdr.audio_channels != channels
        || reader.ident_hdr.audio_sample_rate != sample_rate
    {
        return Err(Error::InvalidSample);
    }
    let mut pcm = Vec::new();
    while let Some(packet) = reader
        .read_dec_packet_itl()
        .map_err(|_| Error::Decode("SF3 Vorbis decode failed".to_string()))?
    {
        pcm.extend(packet);
        if pcm.len() / usize::from(channels) > MAX_DECODED_FRAMES {
            return Err(Error::SampleTooLarge(MAX_DECODED_FRAMES + 1));
        }
    }
    DecodedSample::new(sample_rate, channels, pcm)
}

/// SF3 decoding is intentionally unavailable unless a licensed Vorbis provider is selected.
#[cfg(not(feature = "sf3-vorbis"))]
pub fn decode_sf3_vorbis(
    _data: &[u8],
    _sample_rate: u32,
    _channels: u8,
) -> Result<DecodedSample, Error> {
    Err(Error::UnsupportedCompression(SampleCompression::Vorbis))
}

fn find_riff_chunk<'a>(data: &'a [u8], wanted: &[u8; 4]) -> Option<&'a [u8]> {
    if data.len() < 12 || &data[..4] != b"RIFF" {
        return None;
    }
    find_chunk_in_range(&data[12..], wanted)
}

fn find_chunk_in_range<'a>(mut data: &'a [u8], wanted: &[u8; 4]) -> Option<&'a [u8]> {
    while data.len() >= 8 {
        let id = &data[..4];
        let len = u32::from_le_bytes(data[4..8].try_into().ok()?) as usize;
        let end = 8usize.checked_add(len)?;
        if end > data.len() {
            return None;
        }
        let payload = &data[8..end];
        if id == wanted {
            return Some(payload);
        }
        if id == b"LIST"
            && payload.len() >= 4
            && let Some(found) = find_chunk_in_range(&payload[4..], wanted)
        {
            return Some(found);
        }
        data = &data[end + (len & 1).min(data.len().saturating_sub(end))..];
    }
    None
}

#[cfg(feature = "sf3-vorbis")]
fn find_ogg_payload(data: &[u8]) -> Option<&[u8]> {
    let start = data.windows(4).position(|window| window == b"OggS")?;
    let mut offset = start;
    let mut serial = None;
    loop {
        let header_end = offset.checked_add(27)?;
        if header_end > data.len() || &data[offset..offset + 4] != b"OggS" {
            return None;
        }
        if data[offset + 4] != 0 {
            return None;
        }
        let page_serial = u32::from_le_bytes(data[offset + 14..offset + 18].try_into().ok()?);
        let segment_count = usize::from(data[offset + 26]);
        let lacing_end = header_end.checked_add(segment_count)?;
        if lacing_end > data.len() {
            return None;
        }
        let payload_len = data[header_end..lacing_end]
            .iter()
            .try_fold(0usize, |sum, segment| {
                sum.checked_add(usize::from(*segment))
            })?;
        let page_end = lacing_end.checked_add(payload_len)?;
        if page_end > data.len() {
            return None;
        }
        if serial.is_none() {
            serial = Some(page_serial);
        }
        if Some(page_serial) != serial {
            return None;
        }
        if data[offset + 5] & 0x04 != 0 {
            return Some(&data[start..page_end]);
        }
        offset = page_end;
        if &data[offset..].get(..4)? != b"OggS" {
            return None;
        }
    }
}

/// Render one scheduled sample action into deterministic interleaved PCM16 frames.
pub fn render_sample_action(
    sample: &DecodedSample,
    action: &SampleAction,
    output_rate: u32,
) -> Result<Vec<i16>, Error> {
    if output_rate == 0 {
        return Err(Error::InvalidConfig);
    }
    let SampleAction::Start {
        sample_id,
        parameters,
        event,
        ..
    } = action
    else {
        return Ok(Vec::new());
    };
    if *sample_id == 0 || event.duration_secs < 0.0 || !event.duration_secs.is_finite() {
        return Err(Error::InvalidEvent);
    }
    let channels = usize::from(sample.channels);
    let frames = (event.duration_secs * f64::from(output_rate)).ceil() as usize;
    let frames = frames.min(MAX_DECODED_FRAMES);
    let source_frames = sample.pcm_i16.len() / channels;
    let ratio = f64::from(sample.sample_rate) * f64::from(parameters.playback_rate_ratio)
        / f64::from(output_rate);
    if source_frames == 0 || !ratio.is_finite() || ratio <= 0.0 || !parameters.gain.is_finite() {
        return Err(Error::InvalidSample);
    }
    let mut output = Vec::with_capacity(frames * channels);
    for frame in 0..frames {
        let mut source = frame as f64 * ratio;
        if let Some(loop_points) = parameters.loop_points {
            let loop_start = usize::try_from(loop_points.start_frame).unwrap_or(usize::MAX);
            let loop_end = usize::try_from(loop_points.end_frame).unwrap_or(usize::MAX);
            if loop_start < loop_end && loop_end <= source_frames && source >= loop_end as f64 {
                source = loop_start as f64
                    + (source - loop_start as f64).rem_euclid((loop_end - loop_start) as f64);
            }
        }
        let source_frame = (source as usize).min(source_frames - 1);
        let t = frame as f32 / output_rate as f32;
        let envelope = if parameters.attack_secs > 0.0 && t < parameters.attack_secs {
            t / parameters.attack_secs
        } else if parameters.decay_secs > 0.0 && t < parameters.attack_secs + parameters.decay_secs
        {
            1.0 - (1.0 - parameters.sustain_level)
                * ((t - parameters.attack_secs) / parameters.decay_secs)
        } else {
            parameters.sustain_level
        };
        for channel in 0..channels {
            let value = f32::from(sample.pcm_i16[source_frame * channels + channel])
                * parameters.gain
                * envelope;
            output.push(value.clamp(-32768.0, 32767.0).round() as i16);
        }
    }
    Ok(output)
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct SoundFontPreset {
    pub bank: u16,
    pub program: u16,
    pub name: String,
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct SoundFontAsset {
    pub format: SoundFontFormat,
    /// FNV-1a 64-bit checksum of the bytes; independent of the source path.
    pub checksum: u64,
    pub provider_version: String,
    pub presets: Vec<SoundFontPreset>,
}

impl SoundFontAsset {
    pub fn preset(&self, bank: u16, program: u16) -> Result<&SoundFontPreset, Error> {
        self.presets
            .iter()
            .find(|p| p.bank == bank && p.program == program)
            .ok_or(Error::PresetNotFound { bank, program })
    }
}

#[derive(Debug, thiserror::Error, Clone, PartialEq, Eq)]
pub enum Error {
    #[error("SoundFont is too large ({0} bytes)")]
    TooLarge(usize),
    #[error("invalid SoundFont RIFF header")]
    InvalidHeader,
    #[error("truncated SoundFont chunk")]
    Truncated,
    #[error("SoundFont preset table is missing or empty")]
    NoPresets,
    #[error("SoundFont contains too many presets ({0})")]
    TooManyPresets(usize),
    #[error("SoundFont preset ({bank}, {program}) was not found")]
    PresetNotFound { bank: u16, program: u16 },
    #[error("invalid playback configuration")]
    InvalidConfig,
    #[error("playback event has invalid timing or velocity")]
    InvalidEvent,
    #[error("sample parameters are invalid")]
    InvalidSample,
    #[error("SoundFont zone is invalid: {0}")]
    InvalidZone(&'static str),
    #[error("SoundFont generator is unsupported: {0}")]
    UnsupportedGenerator(String),
    #[error("decoded sample is too large ({0} frames)")]
    SampleTooLarge(usize),
    #[error("provider does not support sample compression {0:?}")]
    UnsupportedCompression(SampleCompression),
    #[error("provider does not support capability: {0}")]
    UnsupportedProviderCapability(&'static str),
    #[error("SoundFont decode failed: {0}")]
    Decode(String),
}

/// Load only the bounded, path-independent metadata needed by a renderer.
pub fn load(data: &[u8], provider_version: impl Into<String>) -> Result<SoundFontAsset, Error> {
    if data.len() > MAX_ASSET_BYTES {
        return Err(Error::TooLarge(data.len()));
    }
    if data.len() < 12 || &data[0..4] != b"RIFF" || &data[8..12] != b"sfbk" {
        return Err(Error::InvalidHeader);
    }
    let format = if data.windows(4).any(|w| w == b"OggS") {
        SoundFontFormat::Sf3
    } else {
        SoundFontFormat::Sf2
    };
    let mut presets = Vec::new();
    let mut pos = 12;
    while pos < data.len() {
        if data.len() - pos < 8 {
            return Err(Error::Truncated);
        }
        let id = &data[pos..pos + 4];
        let len = u32::from_le_bytes([data[pos + 4], data[pos + 5], data[pos + 6], data[pos + 7]])
            as usize;
        let start = pos + 8;
        let end = start.checked_add(len).ok_or(Error::Truncated)?;
        if end > data.len() {
            return Err(Error::Truncated);
        }
        if id == b"phdr" {
            parse_presets(&data[start..end], &mut presets)?;
        }
        if id == b"LIST" && len >= 4 && &data[start..start + 4] == b"pdta" {
            parse_pdta(&data[start + 4..end], &mut presets)?;
        }
        pos = end + (len & 1);
        if pos > data.len() {
            return Err(Error::Truncated);
        }
    }
    presets.sort_by_key(|p| (p.bank, p.program));
    presets.dedup_by_key(|p| (p.bank, p.program));
    if presets.is_empty() {
        return Err(Error::NoPresets);
    }
    Ok(SoundFontAsset {
        format,
        checksum: fnv1a(data),
        provider_version: provider_version.into(),
        presets,
    })
}

fn parse_pdta(data: &[u8], presets: &mut Vec<SoundFontPreset>) -> Result<(), Error> {
    let mut pos = 0;
    while pos < data.len() {
        if data.len() - pos < 8 {
            return Err(Error::Truncated);
        }
        let len = u32::from_le_bytes([data[pos + 4], data[pos + 5], data[pos + 6], data[pos + 7]])
            as usize;
        let start = pos + 8;
        let end = start.checked_add(len).ok_or(Error::Truncated)?;
        if end > data.len() {
            return Err(Error::Truncated);
        }
        if &data[pos..pos + 4] == b"phdr" {
            parse_presets(&data[start..end], presets)?;
        }
        pos = end + (len & 1);
    }
    Ok(())
}

fn parse_presets(data: &[u8], presets: &mut Vec<SoundFontPreset>) -> Result<(), Error> {
    const RECORD: usize = 38;
    if data.len() < RECORD || !data.len().is_multiple_of(RECORD) {
        return Err(Error::Truncated);
    }
    let count = data.len() / RECORD;
    if count > MAX_PRESETS + 1 {
        return Err(Error::TooManyPresets(count));
    }
    for record in data.chunks_exact(RECORD).take(count.saturating_sub(1)) {
        let name_end = record[..20].iter().position(|b| *b == 0).unwrap_or(20);
        let name = String::from_utf8_lossy(&record[..name_end])
            .trim()
            .to_owned();
        let program = u16::from_le_bytes([record[20], record[21]]);
        let bank = u16::from_le_bytes([record[22], record[23]]);
        presets.push(SoundFontPreset {
            bank,
            program,
            name,
        });
        if presets.len() > MAX_PRESETS {
            return Err(Error::TooManyPresets(presets.len()));
        }
    }
    Ok(())
}

fn fnv1a(data: &[u8]) -> u64 {
    data.iter().fold(0xcbf29ce484222325, |hash, byte| {
        (hash ^ u64::from(*byte)).wrapping_mul(0x100000001b3)
    })
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum VoiceStealing {
    Oldest,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub struct PlaybackConfig {
    pub max_polyphony: usize,
    pub voice_stealing: VoiceStealing,
    pub max_cached_samples: usize,
}

#[derive(Debug, Clone, PartialEq)]
pub struct SampleVoiceParameters {
    pub playback_rate_ratio: f32,
    pub gain: f32,
    pub loop_points: Option<SampleLoop>,
    pub attack_secs: f32,
    pub decay_secs: f32,
    pub sustain_level: f32,
    pub release_secs: f32,
}

#[derive(Debug, Clone)]
pub enum SampleAction {
    Start {
        voice_id: u64,
        sample_id: u64,
        event: PlaybackEvent,
        parameters: SampleVoiceParameters,
    },
    Stop {
        voice_id: u64,
        release_secs: f32,
    },
}

/// Build a deterministic provider action for one `PlaybackEvent` and selected region.
pub fn schedule_sample_note_on(
    voice_id: u64,
    event: PlaybackEvent,
    region: &SampleRegion,
    velocity_exponent: f32,
) -> Result<SampleAction, Error> {
    validate_sample_region(region)?;
    if !region.contains(event.pitch_midi, event.velocity) {
        return Err(Error::InvalidSample);
    }
    let gain = velocity_gain(event.velocity, velocity_exponent)?
        * 10.0_f32.powf(-region.attenuation_db / 20.0);
    let semitones = f32::from(event.pitch_midi) - f32::from(region.root_key)
        + f32::from(region.fine_tune_cents) / 100.0;
    let playback_rate_ratio = 2.0_f32.powf(semitones / 12.0);
    Ok(SampleAction::Start {
        voice_id,
        sample_id: region.sample_id,
        event,
        parameters: SampleVoiceParameters {
            playback_rate_ratio,
            gain,
            loop_points: region.loop_points,
            attack_secs: region.attack_secs,
            decay_secs: region.decay_secs,
            sustain_level: region.sustain_level,
            release_secs: region.release_secs,
        },
    })
}

/// Small deterministic FIFO sample cache. The provider owns decoded sample memory.
#[derive(Debug, Clone)]
pub struct SampleCache {
    capacity: usize,
    ids: Vec<u64>,
}

impl SampleCache {
    pub fn new(capacity: usize) -> Result<Self, Error> {
        if capacity == 0 {
            return Err(Error::InvalidConfig);
        }
        Ok(Self {
            capacity,
            ids: Vec::new(),
        })
    }

    pub fn touch(&mut self, sample_id: u64) {
        self.ids.retain(|id| *id != sample_id);
        if self.ids.len() >= self.capacity {
            self.ids.remove(0);
        }
        self.ids.push(sample_id);
    }

    pub fn contains(&self, sample_id: u64) -> bool {
        self.ids.contains(&sample_id)
    }

    pub fn len(&self) -> usize {
        self.ids.len()
    }

    pub fn is_empty(&self) -> bool {
        self.ids.is_empty()
    }

    pub fn clear(&mut self) {
        self.ids.clear();
    }
}

impl Default for PlaybackConfig {
    fn default() -> Self {
        Self {
            max_polyphony: 64,
            voice_stealing: VoiceStealing::Oldest,
            max_cached_samples: 128,
        }
    }
}

#[derive(Debug, Clone)]
pub enum Action {
    NoteOn { voice_id: u64, event: PlaybackEvent },
    NoteOff { voice_id: u64, release_secs: f64 },
    VoiceStolen { voice_id: u64 },
}

#[derive(Debug, Clone)]
struct Voice {
    id: u64,
    address: Option<String>,
    released: bool,
}

/// State machine for a renderer. The renderer consumes [`Action`] values.
/// `PlaybackEvent` remains unchanged and is forwarded intact on note-on.
#[derive(Debug, Clone)]
pub struct PlaybackBoundary {
    config: PlaybackConfig,
    voices: Vec<Voice>,
    next_id: u64,
    sustain: bool,
    cached_samples: Vec<u64>,
}

impl PlaybackBoundary {
    pub fn new(config: PlaybackConfig) -> Result<Self, Error> {
        if config.max_polyphony == 0
            || config.max_polyphony > MAX_POLYPHONY
            || config.max_cached_samples == 0
        {
            return Err(Error::InvalidConfig);
        }
        Ok(Self {
            config,
            voices: Vec::new(),
            next_id: 1,
            sustain: false,
            cached_samples: Vec::new(),
        })
    }

    pub fn note_on(&mut self, event: PlaybackEvent) -> Result<Vec<Action>, Error> {
        if !event.time_secs.is_finite()
            || !event.duration_secs.is_finite()
            || event.duration_secs < 0.0
            || event.velocity == 0
            || event.velocity > 127
        {
            return Err(Error::InvalidEvent);
        }
        let mut actions = Vec::new();
        if self.voices.len() >= self.config.max_polyphony {
            let stolen = self.voices.remove(0);
            actions.push(Action::VoiceStolen {
                voice_id: stolen.id,
            });
        }
        let id = self.next_id;
        self.next_id = self.next_id.wrapping_add(1).max(1);
        self.voices.push(Voice {
            id,
            address: event.address.clone(),
            released: false,
        });
        actions.push(Action::NoteOn {
            voice_id: id,
            event,
        });
        Ok(actions)
    }

    pub fn note_off(
        &mut self,
        address: Option<&str>,
        release_secs: f64,
    ) -> Result<Vec<Action>, Error> {
        if !release_secs.is_finite() || release_secs < 0.0 {
            return Err(Error::InvalidEvent);
        }
        let index = self
            .voices
            .iter()
            .position(|v| !v.released && v.address.as_deref() == address);
        if let Some(index) = index {
            if self.sustain {
                self.voices[index].released = true;
                return Ok(Vec::new());
            }
            let voice = self.voices.remove(index);
            return Ok(vec![Action::NoteOff {
                voice_id: voice.id,
                release_secs,
            }]);
        }
        Ok(Vec::new())
    }

    pub fn set_sustain(&mut self, down: bool, release_secs: f64) -> Result<Vec<Action>, Error> {
        if !release_secs.is_finite() || release_secs < 0.0 {
            return Err(Error::InvalidEvent);
        }
        let was_down = self.sustain;
        self.sustain = down;
        if was_down && !down {
            let released = self
                .voices
                .iter()
                .filter(|v| v.released)
                .map(|v| v.id)
                .collect::<Vec<_>>();
            self.voices.retain(|v| !v.released);
            return Ok(released
                .into_iter()
                .map(|voice_id| Action::NoteOff {
                    voice_id,
                    release_secs,
                })
                .collect());
        }
        Ok(Vec::new())
    }

    pub fn cleanup(&mut self) {
        self.voices.clear();
        self.cached_samples.clear();
    }
    pub fn cache_sample(&mut self, sample_id: u64) {
        if !self.cached_samples.contains(&sample_id) {
            if self.cached_samples.len() >= self.config.max_cached_samples {
                self.cached_samples.remove(0);
            }
            self.cached_samples.push(sample_id);
        }
    }
    pub fn cached_sample_count(&self) -> usize {
        self.cached_samples.len()
    }
    pub fn active_voice_count(&self) -> usize {
        self.voices.len()
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    fn event(address: &str, velocity: u8) -> PlaybackEvent {
        PlaybackEvent {
            address: Some(address.into()),
            time_beats: 0.0,
            time_secs: 0.0,
            pitch_midi: 60,
            velocity,
            duration_beats: 1.0,
            duration_secs: 1.0,
            pedal: false,
            part_index: 0,
            channel: 0,
            is_metronome: false,
        }
    }

    fn region(sample_id: u64, key_min: u8, key_max: u8) -> SampleRegion {
        SampleRegion {
            sample_id,
            start_frame: 0,
            end_frame: 1_000,
            key_min,
            key_max,
            velocity_min: 1,
            velocity_max: 127,
            root_key: 60,
            fine_tune_cents: 0,
            attenuation_db: 0.0,
            sample_rate: 44_100,
            compression: SampleCompression::Pcm16,
            loop_points: Some(SampleLoop {
                start_frame: 10,
                end_frame: 100,
            }),
            attack_secs: 0.01,
            decay_secs: 0.2,
            sustain_level: 0.8,
            release_secs: 0.3,
        }
    }

    #[test]
    fn region_selection_prefers_narrowest_matching_zone() {
        let regions = [region(20, 0, 127), region(10, 60, 60)];
        assert_eq!(
            select_sample_region(&regions, 60, 100).map(|r| r.sample_id),
            Some(10)
        );
        assert_eq!(
            select_sample_region(&regions, 61, 100).map(|r| r.sample_id),
            Some(20)
        );
    }

    #[test]
    fn preset_zone_selection_preserves_bank_program_and_bounds() {
        let zones = vec![
            SoundFontPresetZone::new(0, 0, region(20, 0, 127)).expect("wide zone"),
            SoundFontPresetZone::new(0, 0, region(10, 60, 60)).expect("narrow zone"),
            SoundFontPresetZone::new(1, 0, region(30, 60, 60)).expect("other bank"),
        ];
        let selected = select_preset_zone(&zones, 0, 0, 60, 100).expect("matching zone");
        assert_eq!(selected.region.sample_id, 10);
        assert_eq!(selected.region.start_frame, 0);
        assert!(select_preset_zone(&zones, 1, 0, 60, 100).is_some());
        assert!(select_preset_zone(&zones, 2, 0, 60, 100).is_none());
    }

    #[test]
    fn sample_schedule_contains_tuning_gain_and_envelope() {
        let action = schedule_sample_note_on(7, event("n", 127), &region(3, 0, 127), 1.0)
            .expect("sample action");
        assert!(matches!(action, SampleAction::Start {
            voice_id: 7,
            sample_id: 3,
            parameters: SampleVoiceParameters { gain, playback_rate_ratio, .. },
            ..
        } if (gain - 1.0).abs() < f32::EPSILON && (playback_rate_ratio - 1.0).abs() < f32::EPSILON));
    }

    #[test]
    fn preset_note_plan_selects_bank_program_and_key_velocity_zone() {
        let zones = vec![
            SoundFontPresetZone::new(2, 10, region(1, 0, 127)).expect("zone"),
            SoundFontPresetZone::new(2, 10, region(2, 60, 60)).expect("narrow zone"),
        ];
        let action = schedule_preset_note_on(9, event("preset", 100), &zones, 2, 10, 1.0)
            .expect("preset plan");
        assert!(matches!(action, SampleAction::Start { sample_id: 2, .. }));
    }

    #[test]
    fn decoded_sample_and_cache_are_bounded() {
        assert_eq!(
            DecodedSample::new(44_100, 2, vec![]),
            Err(Error::InvalidSample)
        );
        let mut cache = SampleCache::new(2).expect("cache");
        cache.touch(1);
        cache.touch(2);
        cache.touch(3);
        assert!(!cache.contains(1));
        assert!(cache.contains(2));
        assert_eq!(cache.len(), 2);
    }

    #[test]
    fn provider_capabilities_reject_unsupported_paths_explicitly() {
        let pcm = region(1, 0, 127);
        assert_eq!(
            validate_provider_capabilities(&pcm, ProviderCapabilities::metadata_only()),
            Err(Error::UnsupportedCompression(SampleCompression::Pcm16))
        );

        let capabilities = ProviderCapabilities {
            sf2_pcm16: true,
            sf3_vorbis: false,
            synthesis: false,
        };
        assert_eq!(
            validate_provider_capabilities(&pcm, capabilities),
            Err(Error::UnsupportedProviderCapability("synthesis"))
        );

        let mut vorbis = pcm;
        vorbis.compression = SampleCompression::Vorbis;
        assert_eq!(
            validate_provider_capabilities(
                &vorbis,
                ProviderCapabilities {
                    sf2_pcm16: true,
                    sf3_vorbis: false,
                    synthesis: true,
                }
            ),
            Err(Error::UnsupportedCompression(SampleCompression::Vorbis))
        );
    }

    #[test]
    fn malformed_zone_data_is_rejected_with_a_typed_diagnostic() {
        let mut invalid = region(1, 60, 59);
        assert_eq!(
            validate_sample_region(&invalid),
            Err(Error::InvalidZone("key range"))
        );
        invalid = region(1, 0, 127);
        invalid.loop_points = Some(SampleLoop {
            start_frame: 20,
            end_frame: 10,
        });
        assert_eq!(
            validate_sample_region(&invalid),
            Err(Error::InvalidZone("loop points"))
        );
        assert!(validate_generator_name("keyRange").is_ok());
        assert_eq!(
            validate_generator_name("unsupportedFilterCutoff"),
            Err(Error::UnsupportedGenerator(
                "unsupportedFilterCutoff".to_string()
            ))
        );
    }

    #[test]
    fn decodes_sf2_pcm_and_renders_a_deterministic_note() {
        let mut sf2 = b"RIFFxxxxsfbk".to_vec();
        sf2.extend(b"LIST".as_slice());
        sf2.extend(20u32.to_le_bytes());
        sf2.extend(b"sdta".as_slice());
        sf2.extend(b"smpl".as_slice());
        sf2.extend(8u32.to_le_bytes());
        for value in [1000i16, -1000, 2000, -2000] {
            sf2.extend(value.to_le_bytes());
        }
        let sample = decode_sf2_pcm16(&sf2, 1, 3, 2, 1).expect("PCM16 sample");
        assert_eq!(sample.pcm_i16, vec![-1000, 2000]);
        let mut playback_region = region(1, 0, 127);
        playback_region.attack_secs = 0.0;
        playback_region.sustain_level = 1.0;
        let action = schedule_sample_note_on(1, event("render", 127), &playback_region, 1.0)
            .expect("sample action");
        let rendered = render_sample_action(&sample, &action, 2).expect("rendered sample");
        assert_eq!(rendered, vec![-1000, 2000]);
    }

    #[test]
    fn decodes_real_cc0_sf2_and_renders_non_silent_audio() {
        let data = include_bytes!("../../../tests/fixtures/UprightPianoKW-small-20190703.sf2");
        let sample = decode_sf2_pcm16(data, 0, 1024, 44_100, 1).expect("CC0 SF2 fixture");
        assert_eq!(sample.sample_rate, 44_100);
        assert_eq!(sample.channels, 1);
        assert!(sample.pcm_i16.iter().any(|value| *value != 0));

        let mut playback_region = region(42, 0, 127);
        playback_region.attack_secs = 0.0;
        playback_region.sustain_level = 1.0;
        let action = schedule_sample_note_on(42, event("cc0-sf2", 127), &playback_region, 1.0)
            .expect("SF2 sample action");
        let rendered = render_sample_action(&sample, &action, 44_100).expect("SF2 render");
        assert!(!rendered.is_empty());
        assert!(rendered.iter().any(|value| *value != 0));
    }

    #[cfg(feature = "sf3-vorbis")]
    #[test]
    fn decodes_permitted_synthetic_sf3_vorbis_fixture() {
        let sample = decode_sf3_vorbis(
            include_bytes!("../../../tests/fixtures/synthetic.sf3"),
            8000,
            2,
        )
        .expect("SF3 Vorbis fixture");
        assert_eq!(sample.sample_rate, 8000);
        assert_eq!(sample.channels, 2);
        assert!(!sample.pcm_i16.is_empty());
        assert!(sample.pcm_i16.iter().any(|value| *value != 0));
    }

    #[cfg(feature = "sf3-vorbis")]
    #[test]
    fn decodes_real_mit_sf3_vorbis_and_renders_non_silent_audio() {
        let data = include_bytes!("../../../tests/fixtures/FluidR3Mono_GM.sf3");
        let sample = decode_sf3_vorbis(data, 11_025, 1).expect("MIT SF3 fixture");
        assert_eq!(sample.sample_rate, 11_025);
        assert_eq!(sample.channels, 1);
        assert!(sample.pcm_i16.iter().any(|value| *value != 0));

        let mut playback_region = region(43, 0, 127);
        playback_region.attack_secs = 0.0;
        playback_region.sustain_level = 1.0;
        let action = schedule_sample_note_on(43, event("mit-sf3", 127), &playback_region, 1.0)
            .expect("SF3 sample action");
        let rendered = render_sample_action(&sample, &action, 11_025).expect("SF3 render");
        assert!(!rendered.is_empty());
        assert!(rendered.iter().any(|value| *value != 0));
    }
    fn sf(ogg: bool) -> Vec<u8> {
        let mut p = vec![0u8; 38 * 2];
        p[..6].copy_from_slice(b"Piano\0");
        p[20..22].copy_from_slice(&0u16.to_le_bytes());
        p[22..24].copy_from_slice(&0u16.to_le_bytes());
        p[38..42].copy_from_slice(b"EOP\0");
        let mut ph = b"phdr".to_vec();
        ph.extend((p.len() as u32).to_le_bytes());
        ph.extend(p);
        let mut list = b"LIST".to_vec();
        list.extend(((ph.len() + 4) as u32).to_le_bytes());
        list.extend(b"pdta");
        list.extend(ph);
        let mut out = b"RIFFxxxxsfbk".to_vec();
        out.extend(list);
        if ogg {
            out.extend(b"data");
            out.extend(4u32.to_le_bytes());
            out.extend(b"OggS");
        }
        out
    }
    #[test]
    fn loads_sf2_and_preset() {
        let asset = load(&sf(false), "test").expect("fixture");
        assert_eq!(asset.format, SoundFontFormat::Sf2);
        assert_eq!(asset.preset(0, 0).expect("preset").name, "Piano");
    }
    #[test]
    fn loads_sf3_marker() {
        assert_eq!(
            load(&sf(true), "test").expect("fixture").format,
            SoundFontFormat::Sf3
        );
    }
    #[test]
    fn lifecycle_preserves_velocity_and_sustain() {
        let mut b = PlaybackBoundary::new(PlaybackConfig {
            max_polyphony: 1,
            ..Default::default()
        })
        .expect("config");
        let actions = b.note_on(event("n", 100)).expect("note");
        assert!(
            actions.iter().any(
                |action| matches!(action, Action::NoteOn { event, .. } if event.velocity == 100)
            )
        );
        assert!(b.set_sustain(true, 0.1).expect("sustain").is_empty());
        assert!(b.note_off(Some("n"), 0.2).expect("off").is_empty());
        assert_eq!(b.set_sustain(false, 0.3).expect("release").len(), 1);
        b.cleanup();
        assert_eq!(b.active_voice_count(), 0);
    }
}
