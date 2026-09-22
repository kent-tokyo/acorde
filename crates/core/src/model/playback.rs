use super::duration::Duration;
use super::notation::{Articulation, ChordSymbol, GuitarTechnique, TabPosition};
use super::repeat::measure_sequence;
use super::score::{GuitarBendPoint, NoteAddr, Score};
use serde::{Deserialize, Serialize};
use std::collections::{BTreeMap, BTreeSet};

fn default_fermata_multiplier() -> f64 {
    1.5
}
fn default_fermata_hold_beats() -> f64 {
    0.0
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

/// Explicit opt-in event realization policy.
///
/// [`Authored`](Self::Authored) is the default: ornaments and arpeggiation remain notation
/// semantics on one event. The versioned realization profile is deliberately opt-in so hosts
/// never receive invented attacks merely by upgrading acorde.
#[derive(Debug, Clone, Copy, Default, Serialize, Deserialize, PartialEq, Eq)]
#[serde(rename_all = "kebab-case")]
pub enum PlaybackRealizationProfile {
    /// Preserve authored event semantics without generating auxiliary attacks.
    #[default]
    Authored,
    /// Deterministic chromatic ornament attacks and chord arpeggiation for preview providers.
    ///
    /// Trills and shakes alternate the written pitch with one chromatic semitone above; mordents
    /// and turns use a fixed four-attack figure. Chord tones marked `arpeggiate` are staggered
    /// low-to-high (or high-to-low) by at most 0.18 beats. This is a reproducible preview policy,
    /// not a claim about historical performance practice.
    OrnamentArpeggioV1,
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
    /// Additional beat-domain hold requested after a fermata note. Default 0 keeps legacy
    /// multiplier-only behavior; hosts decide how to realize the resulting silence.
    #[serde(default = "default_fermata_hold_beats")]
    pub fermata_hold_beats: f64,
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
    /// Optional realization policy. The default retains authored ornament and arpeggio marks
    /// without synthesizing extra events.
    #[serde(default)]
    pub realization_profile: PlaybackRealizationProfile,
}

impl Default for PlaybackOptions {
    fn default() -> Self {
        Self {
            bpm_override: None,
            muted_parts: Vec::new(),
            loop_region: None,
            fermata_multiplier: 1.5,
            fermata_hold_beats: 0.0,
            swing: None,
            swing_unit: Duration::Eighth,
            metronome: None,
            realization_profile: PlaybackRealizationProfile::Authored,
        }
    }
}

/// A single sounding event suitable for audio playback engines (e.g. Web Audio, Tone.js).
///
/// Grace notes and rests are excluded. Chords are expanded to one event per pitch.
#[derive(Debug, Clone, Serialize, Deserialize, PartialEq)]
pub struct PlaybackEvent {
    /// Stable source note address (`part:staff:measure:voice:note`), or `None` for metronome events.
    /// Chord pitches share the address of their source note.
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub address: Option<String>,
    /// Typed source address for notation hosts; `None` for metronome events.
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub source: Option<NoteAddr>,
    /// Original MusicXML voice number when the source used a non-default identifier.
    /// `source` remains the stable four-slot canonical address used by editing APIs.
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub source_voice_number: Option<u32>,
    /// Absolute beat position from the start of the score.
    pub time_beats: f64,
    /// Absolute time in seconds from the start of the score.
    pub time_secs: f64,
    /// MIDI pitch number (0–127).
    pub pitch_midi: u8,
    /// Exact sounding pitch in hundredths of a MIDI semitone.
    ///
    /// `pitch_midi` is retained as a compatibility convenience for integer-MIDI hosts;
    /// microtonal-capable hosts should use this field.
    #[serde(default)]
    pub pitch_midi_cents: i32,
    /// Authored normalized pitch-bend curve for a bend-capable host. Each point is relative to
    /// `pitch_midi_cents` and positioned within this event's sounding duration. Empty means the
    /// event has no authored continuous bend.
    #[serde(default)]
    pub pitch_bend_curve: Vec<GuitarBendPoint>,
    /// Authored pause requested after this note by a breath mark or caesura. The host decides
    /// how to realize the silence while preserving this deterministic beat-domain contract.
    #[serde(default)]
    pub post_note_pause_beats: f64,
    /// Authored articulation and ornament marks for a provider to interpret. The core schedule
    /// deliberately does not invent auxiliary pitches for trill, mordent, or turn.
    #[serde(default)]
    pub articulations: Vec<Articulation>,
    /// Authored chord symbol at this note position. It is a semantic cue for accompaniment or
    /// harmonic playback providers; acorde does not invent accompaniment notes.
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub chord_symbol: Option<ChordSymbol>,
    /// Authored guitar playing technique, when present. Providers may map this stable semantic
    /// value to keyswitches or synthesis behavior without score re-parsing.
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub guitar_technique: Option<GuitarTechnique>,
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
    /// Effective General MIDI program after applying any measure-local instrument change.
    #[serde(default)]
    pub program: u8,
    /// Stable effective instrument ID when the part or measure declares one.
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub instrument_id: Option<String>,
    /// `true` for metronome click events injected via [`MetronomeConfig`].
    #[serde(default)]
    pub is_metronome: bool,
}

/// Version of the host-neutral playback timing comparison contract.
pub const PLAYBACK_COMPARISON_CONTRACT_VERSION: u16 = 2;
/// Maximum number of events accepted by one comparison operation.
pub const MAX_PLAYBACK_COMPARISON_EVENTS: usize = 1_000_000;
const MAX_PLAYBACK_MISMATCHES: usize = 256;
/// Maximum number of projected tablature events retained in one report.
pub const MAX_TAB_PERFORMANCE_EVENTS: usize = 1_000_000;
const MAX_TAB_PERFORMANCE_DIAGNOSTICS: usize = 1_024;

/// Version of the host-neutral offline rendering manifest contract.
pub const OFFLINE_RENDER_CONTRACT_VERSION: u16 = 1;

/// Encoded audio format requested from a host-side offline renderer.
///
/// acorde does not encode audio; this keeps the requested output explicit in the deterministic
/// schedule handed to a Composer or other host.
#[derive(Debug, Clone, Copy, Default, Serialize, Deserialize, PartialEq, Eq)]
pub enum OfflineRenderFormat {
    #[default]
    Wav,
    Flac,
    Mp3,
}

/// Host-neutral parameters for an offline render operation.
#[derive(Debug, Clone, Serialize, Deserialize, PartialEq, Eq)]
pub struct OfflineRenderRequest {
    /// Requested encoded format. The host reports unsupported formats explicitly.
    #[serde(default)]
    pub format: OfflineRenderFormat,
    /// PCM sample rate used to convert event seconds to exact sample frames.
    #[serde(default = "default_offline_render_sample_rate")]
    pub sample_rate_hz: u32,
    /// Requested interleaved output channel count.
    #[serde(default = "default_offline_render_channels")]
    pub channels: u8,
    /// Stable name/version of the selected host provider, when already chosen.
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub provider_identity: Option<String>,
}

fn default_offline_render_sample_rate() -> u32 {
    48_000
}

fn default_offline_render_channels() -> u8 {
    2
}

impl Default for OfflineRenderRequest {
    fn default() -> Self {
        Self {
            format: OfflineRenderFormat::Wav,
            sample_rate_hz: default_offline_render_sample_rate(),
            channels: default_offline_render_channels(),
            provider_identity: None,
        }
    }
}

/// Sample-accurate event schedule for a host-side offline render.
///
/// `duration_frames` is derived from the final event's end time and deliberately excludes codec
/// padding. A host owns synthesis, tail policy, file creation, and encoded bytes.
#[derive(Debug, Clone, Serialize, Deserialize, PartialEq)]
pub struct OfflineRenderManifest {
    pub contract_version: u16,
    pub playback_contract_version: u16,
    pub format: OfflineRenderFormat,
    pub sample_rate_hz: u32,
    pub channels: u8,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub provider_identity: Option<String>,
    pub duration_frames: u64,
    pub events: Vec<PlaybackEvent>,
}

/// Host-reported result metadata for an [`OfflineRenderManifest`].
///
/// This is a result contract, not proof of audio quality or device-independent equivalence.
#[derive(Debug, Clone, Serialize, Deserialize, PartialEq, Eq)]
pub struct OfflineRenderResult {
    pub contract_version: u16,
    pub format: OfflineRenderFormat,
    pub sample_rate_hz: u32,
    pub channels: u8,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub provider_identity: Option<String>,
    pub duration_frames: u64,
    pub output_bytes: u64,
}

/// A host-side auxiliary send between two logical playback buses.
#[derive(Debug, Clone, Serialize, Deserialize, PartialEq)]
pub struct PlaybackAuxSend {
    pub source_bus: String,
    pub destination_bus: String,
    /// Gain applied before the destination effect chain, in dB.
    pub gain_db: f64,
}

/// Identifies a host effect attached to a logical bus without carrying provider-specific settings.
#[derive(Debug, Clone, Serialize, Deserialize, PartialEq, Eq)]
pub struct PlaybackEffectRoute {
    pub bus_id: String,
    pub effect_id: String,
    #[serde(default = "default_effect_enabled")]
    pub enabled: bool,
}

fn default_effect_enabled() -> bool {
    true
}

/// Host-neutral routing choices resolved after score playback events are generated.
#[derive(Debug, Clone, Serialize, Deserialize, PartialEq)]
pub struct PlaybackRoutingConfig {
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub provider_identity: Option<String>,
    #[serde(default = "default_master_bus")]
    pub master_bus: String,
    #[serde(default = "default_metronome_bus")]
    pub metronome_bus: String,
    /// Part-index routes, overridden by a matching stable instrument ID.
    #[serde(default)]
    pub part_buses: BTreeMap<usize, String>,
    #[serde(default)]
    pub instrument_buses: BTreeMap<String, String>,
    #[serde(default)]
    pub aux_sends: Vec<PlaybackAuxSend>,
    #[serde(default)]
    pub effect_routes: Vec<PlaybackEffectRoute>,
}

fn default_master_bus() -> String {
    "master".into()
}

fn default_metronome_bus() -> String {
    "metronome".into()
}

impl Default for PlaybackRoutingConfig {
    fn default() -> Self {
        Self {
            provider_identity: None,
            master_bus: default_master_bus(),
            metronome_bus: default_metronome_bus(),
            part_buses: BTreeMap::new(),
            instrument_buses: BTreeMap::new(),
            aux_sends: Vec::new(),
            effect_routes: Vec::new(),
        }
    }
}

/// One unique event source route in a [`PlaybackRoutingManifest`].
#[derive(Debug, Clone, Serialize, Deserialize, PartialEq, Eq, PartialOrd, Ord)]
pub struct PlaybackBusRoute {
    pub part_index: usize,
    pub channel: u8,
    pub program: u8,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub instrument_id: Option<String>,
    pub is_metronome: bool,
    pub bus_id: String,
}

/// Deterministic logical routing graph for score playback events.
#[derive(Debug, Clone, Serialize, Deserialize, PartialEq)]
pub struct PlaybackRoutingManifest {
    pub contract_version: u16,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub provider_identity: Option<String>,
    pub master_bus: String,
    pub metronome_bus: String,
    pub routes: Vec<PlaybackBusRoute>,
    pub aux_sends: Vec<PlaybackAuxSend>,
    pub effect_routes: Vec<PlaybackEffectRoute>,
}

/// Version of the host-neutral routing manifest contract.
pub const PLAYBACK_ROUTING_CONTRACT_VERSION: u16 = 1;

fn valid_routing_id(value: &str) -> bool {
    !value.is_empty()
        && value.len() <= 64
        && value
            .bytes()
            .all(|byte| byte.is_ascii_alphanumeric() || matches!(byte, b'.' | b'_' | b'-'))
}

fn validate_playback_routing(config: &PlaybackRoutingConfig) -> Result<(), crate::Error> {
    if !valid_routing_id(&config.master_bus)
        || !valid_routing_id(&config.metronome_bus)
        || config.part_buses.values().any(|bus| !valid_routing_id(bus))
        || config
            .instrument_buses
            .values()
            .any(|bus| !valid_routing_id(bus))
        || config.aux_sends.len() > 64
        || config.effect_routes.len() > 128
    {
        return Err(crate::Error::InvalidPlaybackRouting);
    }
    for send in &config.aux_sends {
        if !valid_routing_id(&send.source_bus)
            || !valid_routing_id(&send.destination_bus)
            || !send.gain_db.is_finite()
            || !(-120.0..=24.0).contains(&send.gain_db)
        {
            return Err(crate::Error::InvalidPlaybackRouting);
        }
    }
    if config
        .effect_routes
        .iter()
        .any(|effect| !valid_routing_id(&effect.bus_id) || !valid_routing_id(&effect.effect_id))
    {
        return Err(crate::Error::InvalidPlaybackRouting);
    }
    Ok(())
}

/// Resolve a bounded, deterministic logical routing graph for a score schedule.
pub fn build_playback_routing_manifest(
    score: &Score,
    playback_options: &PlaybackOptions,
    config: &PlaybackRoutingConfig,
) -> Result<PlaybackRoutingManifest, crate::Error> {
    validate_playback_routing(config)?;
    let events = to_playback_events_bounded(score, playback_options)?;
    let mut routes = BTreeMap::new();
    for event in events {
        let bus_id = if event.is_metronome {
            config.metronome_bus.clone()
        } else if let Some(bus) = event
            .instrument_id
            .as_ref()
            .and_then(|instrument_id| config.instrument_buses.get(instrument_id))
        {
            bus.clone()
        } else if let Some(bus) = config.part_buses.get(&event.part_index) {
            bus.clone()
        } else {
            config.master_bus.clone()
        };
        let route = PlaybackBusRoute {
            part_index: event.part_index,
            channel: event.channel,
            program: event.program,
            instrument_id: event.instrument_id,
            is_metronome: event.is_metronome,
            bus_id,
        };
        routes.insert(route.clone(), route);
    }
    Ok(PlaybackRoutingManifest {
        contract_version: PLAYBACK_ROUTING_CONTRACT_VERSION,
        provider_identity: config.provider_identity.clone(),
        master_bus: config.master_bus.clone(),
        metronome_bus: config.metronome_bus.clone(),
        routes: routes.into_values().collect(),
        aux_sends: config.aux_sends.clone(),
        effect_routes: config.effect_routes.clone(),
    })
}

/// Build a bounded sample-accurate schedule for a host-side offline render.
pub fn build_offline_render_manifest(
    score: &Score,
    playback_options: &PlaybackOptions,
    request: &OfflineRenderRequest,
) -> Result<OfflineRenderManifest, crate::Error> {
    if !(8_000..=384_000).contains(&request.sample_rate_hz) || !(1..=8).contains(&request.channels)
    {
        return Err(crate::Error::InvalidOfflineRenderRequest);
    }
    let events = to_playback_events_bounded(score, playback_options)?;
    let duration_secs = events
        .iter()
        .map(|event| event.time_secs + event.duration_secs)
        .fold(0.0_f64, f64::max);
    if !duration_secs.is_finite() || duration_secs < 0.0 {
        return Err(crate::Error::InvalidOfflineRenderRequest);
    }
    let duration_frames = (duration_secs * f64::from(request.sample_rate_hz)).ceil();
    if !duration_frames.is_finite() || duration_frames > u64::MAX as f64 {
        return Err(crate::Error::InvalidOfflineRenderRequest);
    }
    Ok(OfflineRenderManifest {
        contract_version: OFFLINE_RENDER_CONTRACT_VERSION,
        playback_contract_version: PLAYBACK_COMPARISON_CONTRACT_VERSION,
        format: request.format,
        sample_rate_hz: request.sample_rate_hz,
        channels: request.channels,
        provider_identity: request.provider_identity.clone(),
        duration_frames: duration_frames as u64,
        events,
    })
}

/// Timing tolerances for comparing a host/backend event trace with acorde's schedule.
#[derive(Debug, Clone, Serialize, Deserialize, PartialEq)]
pub struct PlaybackTimingTolerance {
    pub start_secs: f64,
    pub duration_secs: f64,
}

impl Default for PlaybackTimingTolerance {
    fn default() -> Self {
        Self {
            start_secs: 0.005,
            duration_secs: 0.005,
        }
    }
}

/// A typed difference in a host playback trace. Audio rendering is deliberately not compared.
#[derive(Debug, Clone, Serialize, Deserialize, PartialEq)]
pub enum PlaybackTimingMismatch {
    EventCount { expected: usize, actual: usize },
    EventIdentity { index: usize },
    StartTime { index: usize, error_secs: f64 },
    Duration { index: usize, error_secs: f64 },
}

/// Deterministic report for a bounded playback timing comparison.
#[derive(Debug, Clone, Serialize, Deserialize, PartialEq)]
pub struct PlaybackTimingReport {
    pub contract_version: u16,
    pub expected_events: usize,
    pub actual_events: usize,
    pub matched_events: usize,
    pub max_start_error_secs: f64,
    pub max_duration_error_secs: f64,
    pub within_tolerance: bool,
    pub mismatches: Vec<PlaybackTimingMismatch>,
}

/// Compare a host-provided event trace with the deterministic score schedule.
///
/// This contract covers event identity and timing only. Web Audio scheduling,
/// SoundFont decoding, device latency, and rendered PCM remain host/provider concerns.
pub fn compare_playback_timing(
    expected: &[PlaybackEvent],
    actual: &[PlaybackEvent],
    tolerance: &PlaybackTimingTolerance,
) -> Result<PlaybackTimingReport, crate::Error> {
    if !tolerance.start_secs.is_finite()
        || tolerance.start_secs < 0.0
        || !tolerance.duration_secs.is_finite()
        || tolerance.duration_secs < 0.0
    {
        return Err(crate::Error::InvalidPlaybackComparison);
    }
    if expected.len() > MAX_PLAYBACK_COMPARISON_EVENTS {
        return Err(crate::Error::PlaybackComparisonTooLarge(expected.len()));
    }
    if actual.len() > MAX_PLAYBACK_COMPARISON_EVENTS {
        return Err(crate::Error::PlaybackComparisonTooLarge(actual.len()));
    }
    let mut mismatches = Vec::new();
    if expected.len() != actual.len() {
        mismatches.push(PlaybackTimingMismatch::EventCount {
            expected: expected.len(),
            actual: actual.len(),
        });
    }
    let mut matched_events = 0;
    let mut max_start_error_secs: f64 = 0.0;
    let mut max_duration_error_secs: f64 = 0.0;
    for (index, (expected_event, actual_event)) in expected.iter().zip(actual).enumerate() {
        let source_identity_matches = match (&expected_event.source, &actual_event.source) {
            (Some(expected), Some(actual)) => expected == actual,
            _ => expected_event.address == actual_event.address,
        };
        let identity_matches = source_identity_matches
            && expected_event.pitch_midi_cents == actual_event.pitch_midi_cents
            && expected_event.velocity == actual_event.velocity
            && expected_event.part_index == actual_event.part_index
            && expected_event.channel == actual_event.channel
            && expected_event.program == actual_event.program
            && expected_event.instrument_id == actual_event.instrument_id
            && expected_event.pitch_bend_curve == actual_event.pitch_bend_curve
            && expected_event.post_note_pause_beats == actual_event.post_note_pause_beats
            && expected_event.articulations == actual_event.articulations
            && expected_event.chord_symbol == actual_event.chord_symbol
            && expected_event.guitar_technique == actual_event.guitar_technique
            && expected_event.is_metronome == actual_event.is_metronome;
        let start_error_secs = (expected_event.time_secs - actual_event.time_secs).abs();
        let duration_error_secs = (expected_event.duration_secs - actual_event.duration_secs).abs();
        if start_error_secs.is_finite() {
            max_start_error_secs = max_start_error_secs.max(start_error_secs);
        }
        if duration_error_secs.is_finite() {
            max_duration_error_secs = max_duration_error_secs.max(duration_error_secs);
        }
        let start_matches =
            start_error_secs.is_finite() && start_error_secs <= tolerance.start_secs;
        let duration_matches =
            duration_error_secs.is_finite() && duration_error_secs <= tolerance.duration_secs;
        if identity_matches && start_matches && duration_matches {
            matched_events += 1;
            continue;
        }
        if mismatches.len() < MAX_PLAYBACK_MISMATCHES {
            if !identity_matches {
                mismatches.push(PlaybackTimingMismatch::EventIdentity { index });
            }
            if !start_matches && mismatches.len() < MAX_PLAYBACK_MISMATCHES {
                mismatches.push(PlaybackTimingMismatch::StartTime {
                    index,
                    error_secs: start_error_secs,
                });
            }
            if !duration_matches && mismatches.len() < MAX_PLAYBACK_MISMATCHES {
                mismatches.push(PlaybackTimingMismatch::Duration {
                    index,
                    error_secs: duration_error_secs,
                });
            }
        }
    }
    Ok(PlaybackTimingReport {
        contract_version: PLAYBACK_COMPARISON_CONTRACT_VERSION,
        expected_events: expected.len(),
        actual_events: actual.len(),
        matched_events,
        max_start_error_secs,
        max_duration_error_secs,
        within_tolerance: mismatches.is_empty(),
        mismatches,
    })
}

/// Version of the score-schedule timing corpus contract.
pub const PLAYBACK_TIMING_CORPUS_CONTRACT_VERSION: u16 = 1;
const MAX_PLAYBACK_TIMING_CORPUS_CASES: usize = 256;

/// One score-backed timing case, independent from an audio synthesis backend.
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct PlaybackTimingCase {
    pub id: String,
    pub score: Score,
    #[serde(default)]
    pub options: PlaybackOptions,
    pub expected_events: Vec<PlaybackEvent>,
    #[serde(default)]
    pub tolerance: PlaybackTimingTolerance,
}

/// Result for one [`PlaybackTimingCase`].
#[derive(Debug, Clone, Serialize, Deserialize, PartialEq)]
pub struct PlaybackTimingCaseReport {
    pub id: String,
    pub report: PlaybackTimingReport,
}

/// Deterministic aggregate result for a score-schedule timing corpus.
#[derive(Debug, Clone, Serialize, Deserialize, PartialEq)]
pub struct PlaybackTimingCorpusReport {
    pub contract_version: u16,
    pub total_cases: usize,
    pub passed_cases: usize,
    pub cases: Vec<PlaybackTimingCaseReport>,
}

/// Compare score-generated schedules against a bounded corpus of expected event traces.
///
/// The corpus verifies notation-to-event semantics only. It makes no assertion about synthesized
/// PCM, codec output, browser scheduling, or real-time device latency.
pub fn evaluate_playback_timing_corpus(
    cases: &[PlaybackTimingCase],
) -> Result<PlaybackTimingCorpusReport, crate::Error> {
    if cases.is_empty() || cases.len() > MAX_PLAYBACK_TIMING_CORPUS_CASES {
        return Err(crate::Error::InvalidPlaybackTimingCorpus);
    }
    let mut ids = BTreeSet::new();
    let mut reports = Vec::with_capacity(cases.len());
    let mut passed_cases = 0;
    for case in cases {
        if !valid_routing_id(&case.id) || !ids.insert(case.id.clone()) {
            return Err(crate::Error::InvalidPlaybackTimingCorpus);
        }
        let actual = to_playback_events_bounded(&case.score, &case.options)?;
        let report = compare_playback_timing(&case.expected_events, &actual, &case.tolerance)?;
        if report.within_tolerance {
            passed_cases += 1;
        }
        reports.push(PlaybackTimingCaseReport {
            id: case.id.clone(),
            report,
        });
    }
    Ok(PlaybackTimingCorpusReport {
        contract_version: PLAYBACK_TIMING_CORPUS_CONTRACT_VERSION,
        total_cases: cases.len(),
        passed_cases,
        cases: reports,
    })
}

/// Version of the host-neutral tablature performance projection contract.
pub const TAB_PERFORMANCE_CONTRACT_VERSION: u16 = 4;

#[derive(Debug, Clone, Serialize, Deserialize, PartialEq)]
pub struct TablaturePerformanceEvent {
    pub playback: PlaybackEvent,
    pub string: u8,
    pub fret: u8,
    /// Authored guitar technique for the host playback adapter, when present.
    #[serde(default)]
    pub technique: Option<GuitarTechnique>,
    /// Authored bend alteration in cents, when the technique is `Bend`.
    #[serde(default)]
    pub bend_alter_cents: Option<i16>,
    /// Normalized bend/hold/release curve for a bend-capable host. Positions are per-mille of
    /// this event's sounding duration and values are cents relative to the written pitch.
    #[serde(default)]
    pub bend_curve: Vec<GuitarBendPoint>,
    pub expected_pitch_midi_cents: i32,
    pub pitch_error_cents: i32,
}

#[derive(Debug, Clone, Serialize, Deserialize, PartialEq)]
pub enum TablaturePerformanceDiagnostic {
    NoTablatureStaff {
        address: String,
    },
    MissingPosition {
        address: String,
        pitch_index: usize,
    },
    StringOutOfRange {
        address: String,
        string: u8,
        lines: u8,
    },
    TuningUnavailable {
        address: String,
        string: u8,
    },
    PitchMismatch {
        address: String,
        pitch_index: usize,
        error_cents: i32,
    },
}

#[derive(Debug, Clone, Serialize, Deserialize, PartialEq)]
pub struct TablaturePerformanceReport {
    pub contract_version: u16,
    pub events: Vec<TablaturePerformanceEvent>,
    pub diagnostics: Vec<TablaturePerformanceDiagnostic>,
}

/// Version of the score-model tablature round-trip diagnostic contract.
pub const TAB_ROUND_TRIP_CONTRACT_VERSION: u16 = 1;

#[derive(Debug, Clone, Serialize, Deserialize, PartialEq)]
pub struct TablatureRoundTripReport {
    pub contract_version: u16,
    pub checked_notes: usize,
    pub positioned_notes: usize,
    pub equivalent: bool,
    pub diagnostics: Vec<TablatureRoundTripDiagnostic>,
}

#[derive(Debug, Clone, Serialize, Deserialize, PartialEq)]
pub enum TablatureRoundTripDiagnostic {
    StructureMismatch {
        address: String,
    },
    TablatureConfigMismatch {
        address: String,
    },
    PositionMismatch {
        address: String,
        pitch_index: usize,
        expected: Option<TabPosition>,
        actual: Option<TabPosition>,
    },
}

/// Verify that authored tablature survives the canonical score JSON round-trip.
///
/// This checks score-model persistence only. It does not claim MusicXML/MSCX or external
/// application equivalence; those format and host comparisons remain separate gates.
pub fn tablature_round_trip_report(
    score: &Score,
) -> Result<TablatureRoundTripReport, crate::Error> {
    let encoded = serde_json::to_string(score)
        .map_err(|error| crate::Error::TabRoundTripSerialization(error.to_string()))?;
    let restored: Score = serde_json::from_str(&encoded)
        .map_err(|error| crate::Error::TabRoundTripSerialization(error.to_string()))?;
    let mut checked_notes = 0;
    let mut positioned_notes = 0;
    let mut diagnostics = Vec::new();
    for (part_index, part) in score.parts.iter().enumerate() {
        let Some(restored_part) = restored.parts.get(part_index) else {
            diagnostics.push(TablatureRoundTripDiagnostic::StructureMismatch {
                address: format!("{part_index}"),
            });
            continue;
        };
        for (staff_index, staff) in part.staves.iter().enumerate() {
            let address = format!("{part_index}:{staff_index}");
            let Some(restored_staff) = restored_part.staves.get(staff_index) else {
                diagnostics.push(TablatureRoundTripDiagnostic::StructureMismatch { address });
                continue;
            };
            if staff.tablature != restored_staff.tablature {
                diagnostics.push(TablatureRoundTripDiagnostic::TablatureConfigMismatch {
                    address: address.clone(),
                });
            }
            for (measure_index, measure) in staff.measures.iter().enumerate() {
                let Some(restored_measure) = restored_staff.measures.get(measure_index) else {
                    diagnostics.push(TablatureRoundTripDiagnostic::StructureMismatch {
                        address: format!("{address}:{measure_index}"),
                    });
                    continue;
                };
                for (voice_index, voice) in measure.voices.iter().enumerate() {
                    let Some(restored_voice) = restored_measure.voices.get(voice_index) else {
                        diagnostics.push(TablatureRoundTripDiagnostic::StructureMismatch {
                            address: format!("{address}:{measure_index}:{voice_index}"),
                        });
                        continue;
                    };
                    for (note_index, note) in voice.iter().enumerate() {
                        checked_notes += 1;
                        if !note.tab_positions.is_empty() || note.tab_position.is_some() {
                            positioned_notes += 1;
                        }
                        let note_address =
                            format!("{address}:{measure_index}:{voice_index}:{note_index}");
                        let Some(restored_note) = restored_voice.get(note_index) else {
                            diagnostics.push(TablatureRoundTripDiagnostic::StructureMismatch {
                                address: note_address,
                            });
                            continue;
                        };
                        let pitch_count = note.pitches.len().max(note.tab_positions.len()).max(1);
                        for pitch_index in 0..pitch_count {
                            let expected = note
                                .tab_positions
                                .get(pitch_index)
                                .cloned()
                                .or_else(|| note.tab_position.clone());
                            let actual = restored_note
                                .tab_positions
                                .get(pitch_index)
                                .cloned()
                                .or_else(|| restored_note.tab_position.clone());
                            if expected != actual {
                                diagnostics.push(TablatureRoundTripDiagnostic::PositionMismatch {
                                    address: note_address.clone(),
                                    pitch_index,
                                    expected,
                                    actual,
                                });
                            }
                        }
                    }
                }
            }
        }
    }
    Ok(TablatureRoundTripReport {
        contract_version: TAB_ROUND_TRIP_CONTRACT_VERSION,
        checked_notes,
        positioned_notes,
        equivalent: diagnostics.is_empty(),
        diagnostics,
    })
}

/// Project score playback events onto authored tablature positions.
///
/// This operation never invents a string or fret. Callers may run
/// [`assign_tablature_positions`](crate::assign_tablature_positions) before this
/// projection when automatic positions are desired.
pub fn project_tablature_performance(
    score: &Score,
    options: &PlaybackOptions,
) -> Result<TablaturePerformanceReport, crate::Error> {
    let playback = to_playback_events(score, options);
    if playback.len() > MAX_TAB_PERFORMANCE_EVENTS {
        return Err(crate::Error::TabPerformanceTooLarge(playback.len()));
    }
    let mut events = Vec::new();
    let mut diagnostics = Vec::new();
    for event in playback {
        let Some(address) = event.address.as_deref() else {
            continue;
        };
        let Some((part_index, staff_index, measure_index, voice_index, note_index)) =
            parse_playback_address(address)
        else {
            continue;
        };
        let Some(staff) = score
            .parts
            .get(part_index)
            .and_then(|part| part.staves.get(staff_index))
        else {
            continue;
        };
        let Some(note) = staff
            .measures
            .get(measure_index)
            .and_then(|measure| measure.voices.get(voice_index))
            .and_then(|voice| voice.get(note_index))
        else {
            continue;
        };
        let Some(tab) = staff.tablature_at(measure_index) else {
            push_tab_diagnostic(
                &mut diagnostics,
                TablaturePerformanceDiagnostic::NoTablatureStaff {
                    address: address.into(),
                },
            );
            continue;
        };
        let transpose_cents = if event.channel == 9 {
            0
        } else {
            i32::from(staff.transpose_semitones) * 100
        };
        let written_pitch_cents = event.pitch_midi_cents - transpose_cents;
        let pitch_index = note
            .pitches
            .iter()
            .position(|pitch| pitch.to_midi_cents() == written_pitch_cents)
            .unwrap_or(0);
        let position = note
            .tab_positions
            .get(pitch_index)
            .or(note.tab_position.as_ref());
        let Some(position) = position else {
            push_tab_diagnostic(
                &mut diagnostics,
                TablaturePerformanceDiagnostic::MissingPosition {
                    address: address.into(),
                    pitch_index,
                },
            );
            continue;
        };
        if position.string == 0 || position.string > tab.lines {
            push_tab_diagnostic(
                &mut diagnostics,
                TablaturePerformanceDiagnostic::StringOutOfRange {
                    address: address.into(),
                    string: position.string,
                    lines: tab.lines,
                },
            );
            continue;
        }
        let Some(tuning) = tab.tuning_midi.get(usize::from(position.string - 1)) else {
            push_tab_diagnostic(
                &mut diagnostics,
                TablaturePerformanceDiagnostic::TuningUnavailable {
                    address: address.into(),
                    string: position.string,
                },
            );
            continue;
        };
        let expected_pitch_midi_cents =
            tuning.saturating_add(i16::from(position.fret) + i16::from(tab.capo)) as i32 * 100;
        let pitch_error_cents = event.pitch_midi_cents - expected_pitch_midi_cents;
        if pitch_error_cents != 0 {
            push_tab_diagnostic(
                &mut diagnostics,
                TablaturePerformanceDiagnostic::PitchMismatch {
                    address: address.into(),
                    pitch_index,
                    error_cents: pitch_error_cents,
                },
            );
        }
        events.push(TablaturePerformanceEvent {
            playback: event,
            string: position.string,
            fret: position.fret,
            technique: note.guitar_technique.clone(),
            bend_alter_cents: note.guitar_bend_alter_cents,
            bend_curve: note.guitar_bend_curve.clone(),
            expected_pitch_midi_cents,
            pitch_error_cents,
        });
    }
    Ok(TablaturePerformanceReport {
        contract_version: TAB_PERFORMANCE_CONTRACT_VERSION,
        events,
        diagnostics,
    })
}

fn parse_playback_address(address: &str) -> Option<(usize, usize, usize, usize, usize)> {
    let mut fields = address.split(':').map(|field| field.parse::<usize>().ok());
    Some((
        fields.next()??,
        fields.next()??,
        fields.next()??,
        fields.next()??,
        fields.next()??,
    ))
}

fn push_tab_diagnostic(
    diagnostics: &mut Vec<TablaturePerformanceDiagnostic>,
    diagnostic: TablaturePerformanceDiagnostic,
) {
    if diagnostics.len() < MAX_TAB_PERFORMANCE_DIAGNOSTICS {
        diagnostics.push(diagnostic);
    }
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
        for (staff_index, staff) in part.staves.iter().enumerate() {
            for voice_idx in 0..4usize {
                let mut time_beats = 0.0f64;
                let mut time_secs_cursor = 0.0f64;
                let mut current_bpm = bpm;
                for &idx in &seq {
                    let measure = match staff.measures.get(idx) {
                        Some(m) => m,
                        None => continue,
                    };
                    let instrument = measure
                        .instrument_change
                        .as_ref()
                        .or(part.instrument.as_ref());
                    let channel = instrument.map_or(part.midi_channel, |value| value.midi_channel);
                    let program = instrument.map_or(part.midi_program, |value| value.midi_program);
                    let instrument_id = instrument.map(|value| value.id.clone());
                    if let Some(b) = measure.tempo {
                        current_bpm = b.max(1) as f64;
                    }
                    let measure_beats = measure
                        .time_sig
                        .as_ref()
                        .unwrap_or(&score.settings.time_signature)
                        .total_beats();
                    let ramp_end_bpm = measure
                        .tempo_ramp_to
                        .map(f64::from)
                        .filter(|bpm| *bpm > 0.0);
                    let measure_start_beats = time_beats;
                    let measure_start_secs = time_secs_cursor;
                    let mut local_beats = 0.0f64;
                    let mut swing_first = true;
                    for (note_index, note) in measure.voices[voice_idx].iter().enumerate() {
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
                            let fermata_hold_beats = if options.fermata_hold_beats.is_finite() {
                                options.fermata_hold_beats.max(0.0)
                            } else {
                                0.0
                            };
                            let post_note_pause_beats =
                                note.articulations
                                    .iter()
                                    .fold(0.0f64, |pause, articulation| {
                                        pause.max(match articulation {
                                            Articulation::BreathMark => 0.25,
                                            Articulation::Caesura => 0.5,
                                            Articulation::Fermata => fermata_hold_beats,
                                            _ => 0.0,
                                        })
                                    });
                            let transpose = if channel == 9 {
                                0i8
                            } else {
                                staff.transpose_semitones
                            };
                            for pitch in &note.pitches {
                                let midi = (pitch.to_midi() + transpose as i16).clamp(0, 127) as u8;
                                events.push(PlaybackEvent {
                                    address: Some(format!(
                                        "{part_index}:{staff_index}:{idx}:{voice_idx}:{note_index}"
                                    )),
                                    source: Some(NoteAddr {
                                        part: part_index,
                                        staff: staff_index,
                                        measure: idx,
                                        voice: voice_idx,
                                        note: note_index,
                                    }),
                                    source_voice_number: measure.source_voice_numbers[voice_idx],
                                    time_beats: measure_start_beats + local_beats,
                                    time_secs: measure_start_secs
                                        + tempo_ramp_seconds(
                                            current_bpm,
                                            ramp_end_bpm,
                                            measure_beats,
                                            local_beats,
                                        ),
                                    pitch_midi: midi,
                                    pitch_midi_cents: pitch.to_midi_cents()
                                        + transpose as i32 * 100,
                                    pitch_bend_curve: note.guitar_bend_curve.clone(),
                                    post_note_pause_beats,
                                    articulations: note.articulations.clone(),
                                    chord_symbol: note.chord_symbol.clone(),
                                    guitar_technique: note.guitar_technique.clone(),
                                    velocity,
                                    duration_beats: sounding_dur,
                                    duration_secs: tempo_ramp_seconds(
                                        current_bpm,
                                        ramp_end_bpm,
                                        measure_beats,
                                        local_beats + sounding_dur,
                                    ) - tempo_ramp_seconds(
                                        current_bpm,
                                        ramp_end_bpm,
                                        measure_beats,
                                        local_beats,
                                    ),
                                    pedal,
                                    part_index,
                                    channel,
                                    program,
                                    instrument_id: instrument_id.clone(),
                                    is_metronome: false,
                                });
                            }
                        }
                        local_beats += dur;
                    }
                    // A voice may omit rests, but the next measure still starts
                    // at the notated bar boundary. This also keeps sparse voices
                    // aligned with compute_playback_position and other voices.
                    time_beats = measure_start_beats + measure_beats;
                    time_secs_cursor = measure_start_secs
                        + tempo_ramp_seconds(
                            current_bpm,
                            ramp_end_bpm,
                            measure_beats,
                            measure_beats,
                        );
                    if let Some(end_bpm) = ramp_end_bpm {
                        current_bpm = end_bpm;
                    }
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
            let measure = first_staff.and_then(|staff| staff.measures.get(idx));
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
            let measure_beats = ts.total_beats();
            let ramp_end_bpm = measure
                .and_then(|measure| measure.tempo_ramp_to)
                .map(f64::from)
                .filter(|bpm| *bpm > 0.0);
            let beat_unit = ts.beat_unit_beats();
            let num_beats = (ts.total_beats() / beat_unit).round() as u32;
            for b in 0..num_beats {
                let is_accent = b == 0;
                let beat_offset_beats = b as f64 * beat_unit;
                let beat_offset_secs =
                    tempo_ramp_seconds(metro_bpm, ramp_end_bpm, measure_beats, beat_offset_beats);
                events.push(PlaybackEvent {
                    address: None,
                    source: None,
                    source_voice_number: None,
                    time_beats: cursor_beats + b as f64 * beat_unit,
                    time_secs: cursor_secs + beat_offset_secs,
                    pitch_midi: if is_accent {
                        metro.accent_pitch
                    } else {
                        metro.beat_pitch
                    },
                    pitch_midi_cents: i32::from(if is_accent {
                        metro.accent_pitch
                    } else {
                        metro.beat_pitch
                    }) * 100,
                    pitch_bend_curve: Vec::new(),
                    post_note_pause_beats: 0.0,
                    articulations: Vec::new(),
                    chord_symbol: None,
                    guitar_technique: None,
                    velocity: if is_accent {
                        metro.accent_velocity
                    } else {
                        metro.beat_velocity
                    },
                    duration_beats: beat_unit * 0.1,
                    duration_secs: tempo_ramp_seconds(
                        metro_bpm,
                        ramp_end_bpm,
                        measure_beats,
                        beat_offset_beats + beat_unit * 0.1,
                    ) - beat_offset_secs,
                    pedal: false,
                    part_index: usize::MAX,
                    channel: metro.channel,
                    program: 0,
                    instrument_id: None,
                    is_metronome: true,
                });
            }
            cursor_secs +=
                tempo_ramp_seconds(metro_bpm, ramp_end_bpm, measure_beats, measure_beats);
            cursor_beats += measure_beats;
            if let Some(end_bpm) = ramp_end_bpm {
                metro_bpm = end_bpm;
            }
        }
    }

    events = merge_tied_events(score, events);
    events = apply_realization_profile(score, events, options.realization_profile);
    events.sort_by(|a, b| {
        a.time_beats
            .partial_cmp(&b.time_beats)
            .unwrap_or(std::cmp::Ordering::Equal)
    });
    events
}

const ORNAMENT_ATTACK_COUNT: usize = 4;
const ARPEGGIO_MAX_SPREAD_BEATS: f64 = 0.18;

fn apply_realization_profile(
    score: &Score,
    mut events: Vec<PlaybackEvent>,
    profile: PlaybackRealizationProfile,
) -> Vec<PlaybackEvent> {
    if profile == PlaybackRealizationProfile::Authored {
        return events;
    }

    let mut chord_groups: BTreeMap<String, Vec<usize>> = BTreeMap::new();
    for (index, event) in events.iter().enumerate() {
        if !event.is_metronome {
            if let Some(address) = &event.address {
                chord_groups.entry(address.clone()).or_default().push(index);
            }
        }
    }
    for indices in chord_groups.into_values() {
        let Some(source) = events[indices[0]].source.as_ref() else {
            continue;
        };
        let Some(direction) = score
            .parts
            .get(source.part)
            .and_then(|part| part.staves.get(source.staff))
            .and_then(|staff| staff.measures.get(source.measure))
            .and_then(|measure| measure.voices.get(source.voice))
            .and_then(|voice| voice.get(source.note))
            .and_then(|note| note.arpeggiate)
        else {
            continue;
        };
        if indices.len() < 2 {
            continue;
        }
        let mut ordered = indices;
        ordered.sort_by_key(|&index| events[index].pitch_midi_cents);
        if !direction {
            ordered.reverse();
        }
        let max_spread = events[ordered[0]]
            .duration_beats
            .max(0.0)
            .mul_add(0.5, 0.0)
            .min(ARPEGGIO_MAX_SPREAD_BEATS);
        let step = max_spread / (ordered.len() - 1) as f64;
        for (ordinal, index) in ordered.into_iter().enumerate() {
            shift_event_time(&mut events[index], step * ordinal as f64);
        }
    }

    let mut realized = Vec::with_capacity(events.len());
    for event in events {
        let offsets = ornament_offsets(&event.articulations);
        if offsets.is_empty() || event.is_metronome || event.duration_beats <= 0.0 {
            realized.push(event);
            continue;
        }
        let attack_beats = event.duration_beats / ORNAMENT_ATTACK_COUNT as f64;
        let attack_secs = event.duration_secs / ORNAMENT_ATTACK_COUNT as f64;
        for (ordinal, semitones) in offsets.into_iter().enumerate() {
            let mut attack = event.clone();
            let displacement = i32::from(semitones) * 100;
            attack.pitch_midi_cents = (attack.pitch_midi_cents + displacement).clamp(0, 12_700);
            attack.pitch_midi =
                ((i32::from(attack.pitch_midi) + i32::from(semitones)).clamp(0, 127)) as u8;
            attack.time_beats += attack_beats * ordinal as f64;
            attack.time_secs += attack_secs * ordinal as f64;
            attack.duration_beats = attack_beats;
            attack.duration_secs = attack_secs;
            if ordinal > 0 {
                attack.chord_symbol = None;
            }
            realized.push(attack);
        }
    }
    realized
}

fn shift_event_time(event: &mut PlaybackEvent, offset_beats: f64) {
    if offset_beats <= 0.0 || event.duration_beats <= 0.0 {
        return;
    }
    event.time_beats += offset_beats;
    event.time_secs += event.duration_secs * (offset_beats / event.duration_beats);
}

fn ornament_offsets(articulations: &[Articulation]) -> Vec<i8> {
    if articulations
        .iter()
        .any(|articulation| matches!(articulation, Articulation::Trill | Articulation::Shake))
    {
        return vec![0, 1, 0, 1];
    }
    if articulations
        .iter()
        .any(|articulation| matches!(articulation, Articulation::Mordent))
    {
        return vec![0, 1, 0, 0];
    }
    if articulations
        .iter()
        .any(|articulation| matches!(articulation, Articulation::InvertedMordent))
    {
        return vec![0, -1, 0, 0];
    }
    if articulations
        .iter()
        .any(|articulation| matches!(articulation, Articulation::Turn))
    {
        return vec![1, 0, -1, 0];
    }
    if articulations
        .iter()
        .any(|articulation| matches!(articulation, Articulation::InvertedTurn))
    {
        return vec![-1, 0, 1, 0];
    }
    Vec::new()
}

/// Integrate seconds over `beats` while BPM changes linearly across a measure.
fn tempo_ramp_seconds(start_bpm: f64, end_bpm: Option<f64>, measure_beats: f64, beats: f64) -> f64 {
    let beats = beats.max(0.0);
    let Some(end_bpm) = end_bpm else {
        return beats / start_bpm * 60.0;
    };
    if measure_beats <= 0.0 || (end_bpm - start_bpm).abs() < f64::EPSILON {
        return beats / start_bpm * 60.0;
    }
    let delta = end_bpm - start_bpm;
    let bpm_at_beats = start_bpm + delta * beats / measure_beats;
    if bpm_at_beats <= 0.0 {
        return beats / start_bpm * 60.0;
    }
    60.0 * measure_beats / delta * (bpm_at_beats / start_bpm).ln()
}

/// Coalesce adjacent playback events that are connected by authored ties.
///
/// Ties are a notational continuation, not repeated attacks. The event keeps the first
/// source address and accumulates the sounding duration of each contiguous segment. Malformed
/// or non-contiguous tie endings remain as independent events so playback never silently drops
/// a note.
fn merge_tied_events(score: &Score, events: Vec<PlaybackEvent>) -> Vec<PlaybackEvent> {
    use std::collections::HashMap;

    let mut merged = Vec::with_capacity(events.len());
    let mut pending: HashMap<(usize, usize, usize, i32), usize> = HashMap::new();
    for event in events {
        let Some(source) = event.source.as_ref() else {
            merged.push(event);
            continue;
        };
        let tied_note = score
            .parts
            .get(source.part)
            .and_then(|part| part.staves.get(source.staff))
            .and_then(|staff| staff.measures.get(source.measure))
            .and_then(|measure| measure.voices.get(source.voice))
            .and_then(|voice| voice.get(source.note));
        let Some(note) = tied_note else {
            merged.push(event);
            continue;
        };
        let key = (
            source.part,
            source.staff,
            source.voice,
            event.pitch_midi_cents,
        );
        if note.tie_end
            && pending.get(&key).is_some_and(|&index| {
                merged.get(index).is_some_and(|previous| {
                    (previous.time_beats + previous.duration_beats - event.time_beats).abs() < 1e-9
                })
            })
        {
            let index = pending[&key];
            let previous = &mut merged[index];
            previous.duration_beats += event.duration_beats;
            previous.duration_secs += event.duration_secs;
            if !note.tie_start {
                pending.remove(&key);
            }
            continue;
        }

        let index = merged.len();
        merged.push(event);
        if note.tie_start {
            pending.insert(key, index);
        } else {
            pending.remove(&key);
        }
    }
    merged
}

/// Convert a score into playback events while enforcing the host-comparison event bound.
pub fn to_playback_events_bounded(
    score: &Score,
    options: &PlaybackOptions,
) -> Result<Vec<PlaybackEvent>, crate::Error> {
    let events = to_playback_events(score, options);
    if events.len() > MAX_PLAYBACK_COMPARISON_EVENTS {
        return Err(crate::Error::PlaybackComparisonTooLarge(events.len()));
    }
    Ok(events)
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
    fn bounded_playback_events_match_unbounded_schedule_within_limit() {
        let score = Score::new("bounded", 120, 4, 4, 0, 1);
        let options = opts(Some(120));
        let unbounded = to_playback_events(&score, &options);
        let bounded = to_playback_events_bounded(&score, &options).expect("bounded schedule");
        assert_eq!(bounded, unbounded);
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
        assert_eq!(events[0].address.as_deref(), Some("0:0:0:0:0"));
        assert_eq!(
            events[0].source,
            Some(NoteAddr {
                part: 0,
                staff: 0,
                measure: 0,
                voice: 0,
                note: 0,
            })
        );
        assert!((events[0].time_beats).abs() < 1e-9);
        assert_eq!(events[0].pitch_midi, 60);
        assert_eq!(events[0].velocity, 64);
        assert!((events[0].duration_beats - 1.0).abs() < 1e-9);
        assert_eq!(events[0].part_index, 0);
    }

    #[test]
    fn tied_notes_are_one_continuous_playback_event() {
        let mut score = Score::new("T", 120, 4, 4, 0, 1);
        let mut first = Note::new(Pitch::new(Step::C, 4), Duration::Quarter);
        first.tie_start = true;
        let mut second = Note::new(Pitch::new(Step::C, 4), Duration::Quarter);
        second.tie_end = true;
        score.parts[0].staves[0].measures[0].voices[0] = vec![first, second];

        let events = to_playback_events(&score, &opts(None));
        assert_eq!(events.len(), 1);
        assert_eq!(
            events[0].source.as_ref().map(|address| address.note),
            Some(0)
        );
        assert!((events[0].time_beats).abs() < 1e-9);
        assert!((events[0].duration_beats - 2.0).abs() < 1e-9);
        assert!((events[0].duration_secs - 1.0).abs() < 1e-9);
    }

    #[test]
    fn ties_cross_measure_boundaries_and_tempo_changes_without_retriggering() {
        let mut score = Score::new("T", 120, 4, 4, 0, 2);
        let first_measure = &mut score.parts[0].staves[0].measures[0];
        let mut first = Note::new(Pitch::new(Step::C, 4), Duration::Whole);
        first.tie_start = true;
        first_measure.voices[0] = vec![first];

        let second_measure = &mut score.parts[0].staves[0].measures[1];
        second_measure.tempo = Some(60);
        let mut second = Note::new(Pitch::new(Step::C, 4), Duration::Whole);
        second.tie_end = true;
        second_measure.voices[0] = vec![second];

        let events = to_playback_events(&score, &opts(None));
        assert_eq!(events.len(), 1);
        assert_eq!(
            events[0].source.as_ref().map(|address| address.measure),
            Some(0)
        );
        assert!((events[0].time_beats).abs() < 1e-9);
        assert!((events[0].duration_beats - 8.0).abs() < 1e-9);
        assert!((events[0].duration_secs - 6.0).abs() < 1e-9);
    }

    #[test]
    fn malformed_tie_end_does_not_drop_playback_event() {
        let mut score = Score::new("T", 120, 4, 4, 0, 1);
        let mut note = Note::new(Pitch::new(Step::C, 4), Duration::Quarter);
        note.tie_end = true;
        score.parts[0].staves[0].measures[0].voices[0] = vec![note];
        let events = to_playback_events(&score, &opts(None));
        assert_eq!(events.len(), 1);
        assert!((events[0].duration_beats - 1.0).abs() < 1e-9);
    }

    #[test]
    fn microtonal_playback_event_keeps_exact_midi_cents() {
        let mut score = Score::new("T", 120, 4, 4, 0, 1);
        score.parts[0].staves[0].measures[0].voices[0] = vec![Note::new(
            Pitch::with_microtone(Step::C, 4, 0, 50),
            Duration::Quarter,
        )];
        let events = to_playback_events(&score, &opts(None));
        assert_eq!(events[0].pitch_midi, 61);
        assert_eq!(events[0].pitch_midi_cents, 6050);
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
        assert!(
            events
                .iter()
                .all(|event| event.address.as_deref() == Some("0:0:0:0:0"))
        );
        assert!(events.iter().all(|e| e.time_beats.abs() < 1e-9));
    }

    #[test]
    fn playback_events_preserve_authored_chord_symbol_without_inventing_notes() {
        let mut score = Score::new("Harmony", 120, 4, 4, 0, 1);
        let mut note = Note::new(Pitch::new(Step::C, 4), Duration::Quarter);
        note.chord_symbol = Some(ChordSymbol {
            root: "C".into(),
            kind: "major-seventh".into(),
            bass: Some("E".into()),
            placement: None,
            extender: false,
            harmonic_degree: None,
            harmony_function: None,
            harmony_type: None,
            chord_ref: None,
            range_end: None,
            degrees: Vec::new(),
        });
        score.parts[0].staves[0].measures[0].voices[0] = vec![note];

        let events = to_playback_events(&score, &PlaybackOptions::default());
        assert_eq!(events.len(), 1);
        assert_eq!(events[0].pitch_midi, 60);
        assert_eq!(
            events[0]
                .chord_symbol
                .as_ref()
                .map(ChordSymbol::display_text),
            Some("Cmaj7/E".into())
        );
    }

    #[test]
    fn authored_realization_default_keeps_ornament_as_one_semantic_event() {
        let mut score = Score::new("authored ornament", 120, 4, 4, 0, 1);
        let mut note = Note::new(Pitch::new(Step::C, 4), Duration::Quarter);
        note.articulations.push(Articulation::Trill);
        score.parts[0].staves[0].measures[0].voices[0] = vec![note];

        let events = to_playback_events(&score, &PlaybackOptions::default());
        assert_eq!(events.len(), 1);
        assert_eq!(events[0].pitch_midi, 60);
        assert_eq!(events[0].articulations, vec![Articulation::Trill]);
    }

    #[test]
    fn ornament_arpeggio_profile_realizes_trill_with_pinned_timing() {
        let mut score = Score::new("realized ornament", 120, 4, 4, 0, 1);
        let mut note = Note::new(Pitch::new(Step::C, 4), Duration::Quarter);
        note.articulations.push(Articulation::Trill);
        score.parts[0].staves[0].measures[0].voices[0] = vec![note];

        let events = to_playback_events(
            &score,
            &PlaybackOptions {
                realization_profile: PlaybackRealizationProfile::OrnamentArpeggioV1,
                ..Default::default()
            },
        );
        assert_eq!(events.len(), 4);
        assert_eq!(
            events
                .iter()
                .map(|event| event.pitch_midi)
                .collect::<Vec<_>>(),
            vec![60, 61, 60, 61]
        );
        for (index, event) in events.iter().enumerate() {
            assert!((event.time_beats - index as f64 * 0.25).abs() < 1e-9);
            assert!((event.duration_beats - 0.25).abs() < 1e-9);
            assert!((event.time_secs - index as f64 * 0.125).abs() < 1e-9);
            assert!((event.duration_secs - 0.125).abs() < 1e-9);
        }
    }

    #[test]
    fn ornament_arpeggio_profile_staggers_chord_in_authored_direction() {
        let mut score = Score::new("realized arpeggio", 120, 4, 4, 0, 1);
        let mut chord = Note::new(Pitch::new(Step::C, 4), Duration::Quarter);
        chord.pitches.push(Pitch::new(Step::E, 4));
        chord.pitches.push(Pitch::new(Step::G, 4));
        chord.arpeggiate = Some(false);
        score.parts[0].staves[0].measures[0].voices[0] = vec![chord];

        let events = to_playback_events(
            &score,
            &PlaybackOptions {
                realization_profile: PlaybackRealizationProfile::OrnamentArpeggioV1,
                ..Default::default()
            },
        );
        assert_eq!(events.len(), 3);
        let by_pitch: BTreeMap<_, _> = events
            .iter()
            .map(|event| (event.pitch_midi, event.time_beats))
            .collect();
        assert!((by_pitch[&67] - 0.0).abs() < 1e-9);
        assert!((by_pitch[&64] - 0.09).abs() < 1e-9);
        assert!((by_pitch[&60] - 0.18).abs() < 1e-9);
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
    fn metronome_events_have_no_source_address() {
        let score = Score::new("T", 120, 4, 4, 0, 1);
        let options = PlaybackOptions {
            metronome: Some(MetronomeConfig::default()),
            ..Default::default()
        };
        let events = to_playback_events(&score, &options);
        assert!(!events.is_empty());
        assert!(events.iter().all(|event| event.address.is_none()));
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
    fn fermata_hold_is_an_explicit_post_note_request_separate_from_extension() {
        let mut score = Score::new("T", 120, 4, 4, 0, 1);
        let mut note = Note::new(Pitch::new(Step::C, 4), Duration::Quarter);
        note.articulations
            .push(crate::model::notation::Articulation::Fermata);
        score.parts[0].staves[0].measures[0].voices[0] = vec![note];
        let options = PlaybackOptions {
            fermata_multiplier: 2.0,
            fermata_hold_beats: 0.75,
            ..Default::default()
        };
        let events = to_playback_events(&score, &options);
        assert!((events[0].duration_beats - 2.0).abs() < 1e-9);
        assert!((events[0].post_note_pause_beats - 0.75).abs() < 1e-9);
    }

    #[test]
    fn invalid_fermata_hold_is_safely_ignored() {
        let mut score = Score::new("T", 120, 4, 4, 0, 1);
        let mut note = Note::new(Pitch::new(Step::C, 4), Duration::Quarter);
        note.articulations
            .push(crate::model::notation::Articulation::Fermata);
        score.parts[0].staves[0].measures[0].voices[0] = vec![note];
        let options = PlaybackOptions {
            fermata_hold_beats: f64::NAN,
            ..Default::default()
        };
        assert_eq!(
            to_playback_events(&score, &options)[0].post_note_pause_beats,
            0.0
        );
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

    #[test]
    fn multi_voice_events_preserve_source_voice_addresses() {
        let mut score = Score::new("T", 120, 4, 4, 0, 1);
        score.parts[0].staves[0].measures[0].voices[0] =
            vec![Note::new(Pitch::new(Step::C, 4), Duration::Quarter)];
        score.parts[0].staves[0].measures[0].voices[1] =
            vec![Note::new(Pitch::new(Step::E, 4), Duration::Quarter)];

        let events = to_playback_events(&score, &PlaybackOptions::default());
        let addresses: Vec<&str> = events
            .iter()
            .filter_map(|event| event.address.as_deref())
            .collect();
        assert!(addresses.contains(&"0:0:0:0:0"));
        assert!(addresses.contains(&"0:0:0:1:0"));
    }

    #[test]
    fn playback_exposes_original_musicxml_voice_number_alongside_slot_address() {
        let mut score = Score::new("T", 120, 4, 4, 0, 1);
        let measure = &mut score.parts[0].staves[0].measures[0];
        measure.voices[1] = vec![Note::new(Pitch::new(Step::E, 4), Duration::Quarter)];
        measure.source_voice_numbers[1] = Some(5);

        let events = to_playback_events(&score, &PlaybackOptions::default());
        let event = events
            .iter()
            .find(|event| event.address.as_deref() == Some("0:0:0:1:0"))
            .expect("event from slot 1");
        assert_eq!(event.source.as_ref().map(|source| source.voice), Some(1));
        assert_eq!(event.source_voice_number, Some(5));
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
    fn metronome_integrates_measure_tempo_ramp_and_carries_ending_tempo() {
        let mut score = Score::new("T", 120, 4, 4, 0, 2);
        score.parts[0].staves[0].measures[0].tempo_ramp_to = Some(60);
        let options = PlaybackOptions {
            metronome: Some(MetronomeConfig::default()),
            ..Default::default()
        };

        let mut clicks: Vec<_> = to_playback_events(&score, &options)
            .into_iter()
            .filter(|event| event.is_metronome)
            .collect();
        clicks.sort_by(|left, right| left.time_beats.total_cmp(&right.time_beats));

        let expected_second_beat = tempo_ramp_seconds(120.0, Some(60.0), 4.0, 1.0);
        let expected_second_measure = tempo_ramp_seconds(120.0, Some(60.0), 4.0, 4.0);
        assert!((clicks[1].time_secs - expected_second_beat).abs() < 1e-9);
        assert!((clicks[4].time_secs - expected_second_measure).abs() < 1e-9);
        assert!((clicks[5].time_secs - (expected_second_measure + 1.0)).abs() < 1e-9);
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

    #[test]
    fn offline_render_manifest_uses_event_end_as_exact_frame_length() {
        let mut score = Score::new("T", 120, 4, 4, 0, 1);
        score.parts[0].staves[0].measures[0].voices[0][0] =
            Note::new(Pitch::new(Step::C, 4), Duration::Quarter);
        let request = OfflineRenderRequest {
            format: OfflineRenderFormat::Flac,
            sample_rate_hz: 48_000,
            channels: 2,
            provider_identity: Some("test-provider@1".into()),
        };

        let manifest = build_offline_render_manifest(&score, &PlaybackOptions::default(), &request)
            .expect("valid render manifest");
        assert_eq!(manifest.contract_version, OFFLINE_RENDER_CONTRACT_VERSION);
        assert_eq!(manifest.format, OfflineRenderFormat::Flac);
        assert_eq!(manifest.duration_frames, 24_000);
        assert_eq!(manifest.events.len(), 1);
        assert_eq!(
            manifest.provider_identity.as_deref(),
            Some("test-provider@1")
        );
    }

    #[test]
    fn offline_render_manifest_rejects_out_of_range_audio_shape() {
        let score = Score::new("T", 120, 4, 4, 0, 1);
        let request = OfflineRenderRequest {
            sample_rate_hz: 0,
            ..Default::default()
        };
        assert!(matches!(
            build_offline_render_manifest(&score, &PlaybackOptions::default(), &request),
            Err(crate::Error::InvalidOfflineRenderRequest)
        ));
    }

    #[test]
    fn routing_manifest_prefers_instrument_bus_and_isolates_metronome() {
        let mut score = Score::new("routing", 120, 4, 4, 0, 1);
        let mut instrument = crate::InstrumentDefinition::new("flute", "Flute");
        instrument.midi_channel = 2;
        instrument.midi_program = 73;
        score.parts[0].instrument = Some(instrument);
        score.parts[0].staves[0].measures[0].voices[0] =
            vec![Note::new(Pitch::new(Step::C, 4), Duration::Quarter)];
        let options = PlaybackOptions {
            metronome: Some(MetronomeConfig::default()),
            ..Default::default()
        };
        let mut config = PlaybackRoutingConfig {
            provider_identity: Some("sf2-host@1".into()),
            ..Default::default()
        };
        config.part_buses.insert(0, "parts".into());
        config
            .instrument_buses
            .insert("flute".into(), "winds".into());
        config.aux_sends.push(PlaybackAuxSend {
            source_bus: "winds".into(),
            destination_bus: "reverb".into(),
            gain_db: -12.0,
        });
        config.effect_routes.push(PlaybackEffectRoute {
            bus_id: "reverb".into(),
            effect_id: "convolution".into(),
            enabled: true,
        });

        let manifest = build_playback_routing_manifest(&score, &options, &config)
            .expect("valid routing manifest");
        assert_eq!(manifest.contract_version, PLAYBACK_ROUTING_CONTRACT_VERSION);
        assert_eq!(manifest.provider_identity.as_deref(), Some("sf2-host@1"));
        assert!(manifest.routes.iter().any(|route| {
            route.instrument_id.as_deref() == Some("flute") && route.bus_id == "winds"
        }));
        assert!(
            manifest
                .routes
                .iter()
                .any(|route| route.is_metronome && route.bus_id == "metronome")
        );
        assert_eq!(manifest.aux_sends.len(), 1);
        assert_eq!(manifest.effect_routes.len(), 1);
    }

    #[test]
    fn routing_manifest_rejects_unsafe_bus_and_non_finite_send_gain() {
        let score = Score::new("routing", 120, 4, 4, 0, 1);
        let config = PlaybackRoutingConfig {
            master_bus: "not a bus".into(),
            ..Default::default()
        };
        assert!(matches!(
            build_playback_routing_manifest(&score, &PlaybackOptions::default(), &config),
            Err(crate::Error::InvalidPlaybackRouting)
        ));

        let mut config = PlaybackRoutingConfig::default();
        config.aux_sends.push(PlaybackAuxSend {
            source_bus: "part".into(),
            destination_bus: "master".into(),
            gain_db: f64::NAN,
        });
        assert!(matches!(
            build_playback_routing_manifest(&score, &PlaybackOptions::default(), &config),
            Err(crate::Error::InvalidPlaybackRouting)
        ));
    }

    #[test]
    fn timing_corpus_reports_score_schedule_results_without_audio_claims() {
        let mut score = Score::new("corpus", 120, 4, 4, 0, 1);
        score.parts[0].staves[0].measures[0].voices[0] =
            vec![Note::new(Pitch::new(Step::C, 4), Duration::Quarter)];
        let expected_events = to_playback_events(&score, &PlaybackOptions::default());
        let report = evaluate_playback_timing_corpus(&[PlaybackTimingCase {
            id: "quarter-note".into(),
            score,
            options: PlaybackOptions::default(),
            expected_events,
            tolerance: PlaybackTimingTolerance::default(),
        }])
        .expect("valid timing corpus");
        assert_eq!(
            report.contract_version,
            PLAYBACK_TIMING_CORPUS_CONTRACT_VERSION
        );
        assert_eq!(report.total_cases, 1);
        assert_eq!(report.passed_cases, 1);
        assert!(report.cases[0].report.within_tolerance);
    }

    #[test]
    fn timing_corpus_rejects_empty_or_duplicate_case_ids() {
        assert!(matches!(
            evaluate_playback_timing_corpus(&[]),
            Err(crate::Error::InvalidPlaybackTimingCorpus)
        ));
        let score = Score::new("corpus", 120, 4, 4, 0, 1);
        let case = PlaybackTimingCase {
            id: "duplicate".into(),
            expected_events: to_playback_events(&score, &PlaybackOptions::default()),
            score,
            options: PlaybackOptions::default(),
            tolerance: PlaybackTimingTolerance::default(),
        };
        assert!(matches!(
            evaluate_playback_timing_corpus(&[case.clone(), case]),
            Err(crate::Error::InvalidPlaybackTimingCorpus)
        ));
    }

    fn comparison_event(time_secs: f64, duration_secs: f64) -> PlaybackEvent {
        PlaybackEvent {
            address: Some("0:0:0:0:0".into()),
            source: Some(NoteAddr {
                part: 0,
                staff: 0,
                measure: 0,
                voice: 0,
                note: 0,
            }),
            source_voice_number: None,
            time_beats: 0.0,
            time_secs,
            pitch_midi: 60,
            pitch_midi_cents: 6000,
            pitch_bend_curve: Vec::new(),
            post_note_pause_beats: 0.0,
            articulations: Vec::new(),
            chord_symbol: None,
            guitar_technique: None,
            velocity: 64,
            duration_beats: 1.0,
            duration_secs,
            pedal: false,
            part_index: 0,
            channel: 0,
            program: 0,
            instrument_id: None,
            is_metronome: false,
        }
    }

    #[test]
    fn legacy_playback_event_json_defaults_bend_curve() {
        let event: PlaybackEvent = serde_json::from_str(
            r#"{"address":null,"source":null,"source_voice_number":null,"time_beats":0.0,"time_secs":0.0,"pitch_midi":60,"pitch_midi_cents":6000,"velocity":64,"duration_beats":1.0,"duration_secs":0.5,"pedal":false,"part_index":0,"channel":0,"is_metronome":false}"#,
        )
        .expect("legacy playback event remains readable");
        assert!(event.pitch_bend_curve.is_empty());
        assert_eq!(event.guitar_technique, None);
    }

    #[test]
    fn tempo_ramp_uses_integrated_event_timing_and_carries_target_forward() {
        let mut score = Score::new("ramp", 120, 4, 4, 0, 2);
        for measure in &mut score.parts[0].staves[0].measures {
            measure.voices[0] = vec![
                crate::Note::new(crate::Pitch::new(crate::Step::C, 4), crate::Duration::Half),
                crate::Note::new(crate::Pitch::new(crate::Step::D, 4), crate::Duration::Half),
            ];
        }
        score.parts[0].staves[0].measures[0].tempo_ramp_to = Some(60);
        let events = to_playback_events(&score, &PlaybackOptions::default());
        assert_eq!(events.len(), 4);
        let expected_measure_secs = 4.0 * 60.0 / (60.0 - 120.0) * (60.0f64 / 120.0).ln();
        assert!((events[2].time_secs - expected_measure_secs).abs() < 1e-9);
        assert!((events[0].duration_secs - 1.1507282898).abs() < 1e-6);
        assert!((events[2].duration_secs - 2.0).abs() < 1e-9);
    }

    #[test]
    fn breath_and_caesura_expose_deterministic_post_note_pause() {
        let mut score = Score::new("pause", 120, 4, 4, 0, 1);
        let mut breath = crate::Note::new(
            crate::Pitch::new(crate::Step::C, 4),
            crate::Duration::Quarter,
        );
        breath.articulations.push(Articulation::BreathMark);
        let mut caesura = crate::Note::new(
            crate::Pitch::new(crate::Step::D, 4),
            crate::Duration::Quarter,
        );
        caesura
            .articulations
            .extend([Articulation::BreathMark, Articulation::Caesura]);
        score.parts[0].staves[0].measures[0].voices[0] = vec![breath, caesura];

        let events = to_playback_events(&score, &PlaybackOptions::default());
        assert_eq!(events.len(), 2);
        assert_eq!(events[0].post_note_pause_beats, 0.25);
        assert_eq!(events[1].post_note_pause_beats, 0.5);
        assert_eq!(
            events[1].articulations,
            vec![Articulation::BreathMark, Articulation::Caesura]
        );
    }

    #[test]
    fn measure_instrument_change_resolves_on_playback_events() {
        let mut score = Score::new("instrument", 120, 4, 4, 0, 2);
        let mut base = crate::InstrumentDefinition::new("piano", "Piano");
        base.midi_channel = 1;
        base.midi_program = 0;
        score.parts[0].instrument = Some(base);
        let mut change = crate::InstrumentDefinition::new("flute", "Flute");
        change.midi_channel = 2;
        change.midi_program = 73;
        score.parts[0].staves[0].measures[1].instrument_change = Some(change);
        for measure in &mut score.parts[0].staves[0].measures {
            measure.voices[0] = vec![crate::Note::new(
                crate::Pitch::new(crate::Step::C, 4),
                crate::Duration::Whole,
            )];
        }

        let events = to_playback_events(&score, &PlaybackOptions::default());
        assert_eq!(events.len(), 2);
        assert_eq!(
            (
                events[0].channel,
                events[0].program,
                events[0].instrument_id.as_deref()
            ),
            (1, 0, Some("piano"))
        );
        assert_eq!(
            (
                events[1].channel,
                events[1].program,
                events[1].instrument_id.as_deref()
            ),
            (2, 73, Some("flute"))
        );
    }

    #[test]
    fn generic_playback_event_preserves_authored_guitar_technique() {
        let mut score = Score::new("technique", 120, 4, 4, 0, 1);
        let mut note = Note::new(Pitch::new(Step::E, 4), Duration::Quarter);
        note.guitar_technique = Some(GuitarTechnique::HammerOn);
        score.parts[0].staves[0].measures[0].voices[0] = vec![note];

        let event = to_playback_events(&score, &PlaybackOptions::default())
            .into_iter()
            .next()
            .expect("one note event");
        assert_eq!(event.guitar_technique, Some(GuitarTechnique::HammerOn));
    }

    #[test]
    fn playback_timing_comparison_reports_tolerance_and_identity() {
        let expected = [comparison_event(1.0, 0.5)];
        let actual = [comparison_event(1.004, 0.503)];
        let report = compare_playback_timing(
            &expected,
            &actual,
            &PlaybackTimingTolerance {
                start_secs: 0.005,
                duration_secs: 0.005,
            },
        )
        .expect("comparison");
        assert!(report.within_tolerance);
        assert_eq!(report.matched_events, 1);
        assert_eq!(
            report.contract_version,
            PLAYBACK_COMPARISON_CONTRACT_VERSION
        );

        let mut typed_only = comparison_event(1.004, 0.503);
        typed_only.address = None;
        let report = compare_playback_timing(
            &expected,
            &[typed_only],
            &PlaybackTimingTolerance::default(),
        )
        .expect("typed source comparison");
        assert_eq!(report.matched_events, 1);

        let actual = [comparison_event(1.02, 0.6)];
        let report =
            compare_playback_timing(&expected, &actual, &PlaybackTimingTolerance::default())
                .expect("comparison");
        assert!(!report.within_tolerance);
        assert!(
            report
                .mismatches
                .iter()
                .any(|m| matches!(m, PlaybackTimingMismatch::StartTime { index: 0, .. }))
        );
        assert!(
            report
                .mismatches
                .iter()
                .any(|m| matches!(m, PlaybackTimingMismatch::Duration { index: 0, .. }))
        );
    }

    #[test]
    fn playback_timing_comparison_rejects_instrument_route_mismatch() {
        let expected = comparison_event(0.0, 0.5);
        let mut actual = expected.clone();
        actual.program = 73;
        assert!(
            compare_playback_timing(&[expected], &[actual], &PlaybackTimingTolerance::default(),)
                .expect("comparison")
                .mismatches
                .iter()
                .any(|mismatch| matches!(
                    mismatch,
                    PlaybackTimingMismatch::EventIdentity { index: 0 }
                ))
        );
    }

    #[test]
    fn playback_timing_comparison_rejects_invalid_tolerance() {
        let error = compare_playback_timing(
            &[],
            &[],
            &PlaybackTimingTolerance {
                start_secs: -0.1,
                duration_secs: 0.0,
            },
        );
        assert!(matches!(
            error,
            Err(crate::Error::InvalidPlaybackComparison)
        ));
    }

    #[test]
    fn tablature_performance_projection_keeps_position_and_reports_pitch_error() {
        let mut score = Score::new("Tab", 120, 4, 4, 0, 1);
        score.parts[0].staves[0].tablature = Some(super::super::notation::TablatureConfig {
            lines: 6,
            tuning_midi: vec![40, 45, 50, 55, 59, 64],
            capo: 0,
        });
        let notes = &mut score.parts[0].staves[0].measures[0].voices[0];
        *notes = vec![crate::Note::new(
            crate::Pitch::new(crate::Step::C, 4),
            crate::Duration::Quarter,
        )];
        notes[0].tab_position = Some(super::super::notation::TabPosition {
            string: 1,
            fret: 20,
        });
        notes[0].guitar_technique = Some(super::super::notation::GuitarTechnique::Bend);
        notes[0].guitar_bend_alter_cents = Some(150);
        notes[0].guitar_bend_curve = vec![
            super::super::score::GuitarBendPoint {
                position_per_mille: 0,
                alter_cents: 0,
            },
            super::super::score::GuitarBendPoint {
                position_per_mille: 500,
                alter_cents: 150,
            },
            super::super::score::GuitarBendPoint {
                position_per_mille: 1_000,
                alter_cents: 0,
            },
        ];
        let report = project_tablature_performance(&score, &PlaybackOptions::default())
            .expect("tablature projection");
        assert_eq!(report.events.len(), 1);
        assert_eq!(report.events[0].string, 1);
        assert_eq!(report.events[0].fret, 20);
        assert_eq!(
            report.events[0].technique,
            Some(super::super::notation::GuitarTechnique::Bend)
        );
        assert_eq!(report.events[0].bend_alter_cents, Some(150));
        assert_eq!(report.events[0].bend_curve.len(), 3);
        assert_eq!(report.events[0].bend_curve[1].position_per_mille, 500);
        assert_eq!(report.events[0].bend_curve[1].alter_cents, 150);
        assert_eq!(
            report.events[0].playback.pitch_bend_curve,
            report.events[0].bend_curve
        );
        assert!(report.diagnostics.is_empty());

        score.parts[0].staves[0].measures[0].voices[0][0].tab_position =
            Some(super::super::notation::TabPosition {
                string: 1,
                fret: 19,
            });
        let report = project_tablature_performance(&score, &PlaybackOptions::default())
            .expect("tablature projection");
        assert_eq!(report.events[0].pitch_error_cents, 100);
        assert!(matches!(
            report.diagnostics[0],
            TablaturePerformanceDiagnostic::PitchMismatch {
                error_cents: 100,
                ..
            }
        ));
    }

    #[test]
    fn tablature_performance_projection_does_not_invent_missing_positions() {
        let mut score = Score::new("Tab", 120, 4, 4, 0, 1);
        score.parts[0].staves[0].tablature = Some(super::super::notation::TablatureConfig {
            lines: 6,
            tuning_midi: vec![40, 45, 50, 55, 59, 64],
            capo: 0,
        });
        score.parts[0].staves[0].measures[0].voices[0].push(crate::Note::new(
            crate::Pitch::new(crate::Step::C, 4),
            crate::Duration::Quarter,
        ));
        let report = project_tablature_performance(&score, &PlaybackOptions::default())
            .expect("tablature projection");
        assert!(report.events.is_empty());
        assert!(matches!(
            report.diagnostics[0],
            TablaturePerformanceDiagnostic::MissingPosition { .. }
        ));
    }

    #[test]
    fn tablature_performance_event_accepts_legacy_json_without_technique() {
        let mut score = Score::new("Tab", 120, 4, 4, 0, 1);
        score.parts[0].staves[0].tablature = Some(super::super::notation::TablatureConfig {
            lines: 6,
            tuning_midi: vec![40, 45, 50, 55, 59, 64],
            capo: 0,
        });
        let mut note = crate::Note::new(
            crate::Pitch::new(crate::Step::C, 4),
            crate::Duration::Quarter,
        );
        note.tab_position = Some(super::super::notation::TabPosition {
            string: 1,
            fret: 20,
        });
        score.parts[0].staves[0].measures[0].voices[0] = vec![note];
        let report = project_tablature_performance(&score, &PlaybackOptions::default())
            .expect("tablature projection");
        let mut legacy = serde_json::to_value(&report.events[0]).expect("event JSON");
        legacy
            .as_object_mut()
            .expect("event object")
            .remove("technique");
        let restored: TablaturePerformanceEvent =
            serde_json::from_value(legacy).expect("legacy event JSON");
        assert_eq!(restored.technique, None);
    }

    #[test]
    fn tablature_round_trip_report_preserves_authored_positions() {
        let mut score = Score::new("Tab", 120, 4, 4, 0, 1);
        score.parts[0].staves[0].tablature = Some(super::super::notation::TablatureConfig {
            lines: 6,
            tuning_midi: vec![40, 45, 50, 55, 59, 64],
            capo: 2,
        });
        let mut note = crate::Note::new(
            crate::Pitch::new(crate::Step::C, 4),
            crate::Duration::Quarter,
        );
        note.tab_positions = vec![crate::TabPosition { string: 5, fret: 1 }];
        score.parts[0].staves[0].measures[0].voices[0] = vec![note];
        let report = tablature_round_trip_report(&score).expect("tab round-trip report");
        assert!(report.equivalent);
        assert_eq!(report.checked_notes, 1);
        assert_eq!(report.positioned_notes, 1);
        assert!(report.diagnostics.is_empty());
    }

    #[test]
    fn tablature_performance_uses_measure_local_tuning_change() {
        let mut score = Score::new("Local tuning", 120, 4, 4, 0, 2);
        let staff = &mut score.parts[0].staves[0];
        staff.tablature = Some(super::super::notation::TablatureConfig {
            lines: 6,
            tuning_midi: vec![40, 45, 50, 55, 59, 64],
            capo: 0,
        });
        staff.measures[1].tablature_change = Some(super::super::notation::TablatureConfig {
            lines: 6,
            tuning_midi: vec![38, 45, 50, 55, 59, 64],
            capo: 0,
        });
        let mut note = Note::new(Pitch::new(Step::C, 3), Duration::Quarter);
        note.tab_position = Some(TabPosition {
            string: 1,
            fret: 10,
        });
        staff.measures[1].voices[0] = vec![note];

        let report = project_tablature_performance(&score, &PlaybackOptions::default())
            .expect("tablature projection");
        assert_eq!(report.events.len(), 1);
        assert_eq!(report.events[0].expected_pitch_midi_cents, 4_800);
        assert!(!report.diagnostics.iter().any(|diagnostic| matches!(
            diagnostic,
            TablaturePerformanceDiagnostic::PitchMismatch { .. }
        )));
    }
}
