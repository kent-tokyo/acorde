//! A bounded, provider-neutral boundary between `acorde` playback events and a
//! SoundFont renderer.
//!
//! This crate validates SF2/SF3 metadata and produces lifecycle actions. It
//! intentionally does not decode samples or ship a synthesizer; applications
//! choose a licensed renderer on the other side of this stable boundary.

use acorde_core::PlaybackEvent;

pub const PLAYBACK_CONTRACT_VERSION: u16 = 1;
pub const MAX_ASSET_BYTES: usize = 64 * 1024 * 1024;
pub const MAX_PRESETS: usize = 16_384;
pub const MAX_POLYPHONY: usize = 256;

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum SoundFontFormat {
    Sf2,
    Sf3,
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
