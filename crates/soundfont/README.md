# acorde-soundfont

`acorde-soundfont` defines the optional SoundFont playback boundary for acorde.
It validates bounded SF2/SF3 RIFF metadata, resolves bank/program presets, and
manages provider-neutral note lifecycle actions from `acorde_core::PlaybackEvent`.

`SampleDecoder`, `SampleRenderer`, and the versioned `SoundFontProvider` contract are the stable
integration point. They expose bounded regions, deterministic bank/program/key/velocity selection,
voice actions, decoded PCM validation, and a FIFO cache. Hosts can provide a decoder and
renderer; this crate bundles neither production assets nor an audio device.

`SoundFontPresetZone`, `resolve_preset_zone_metadata`, and `schedule_preset_note_on` keep region,
loop, tuning, envelope, channel-layout, and diagnostic data in one validated representation.
`materialize_zones` creates a bounded, deterministic snapshot for UI, cache keys, and worker
handoff. Unsupported generators, compression, malformed table ranges, and invalid stereo links
remain typed diagnostics rather than approximations.

`PROVIDER_CONTRACT_VERSION` and `validate_provider_capabilities` let a host reject unsupported
PCM/Vorbis or synthesis before decoding. The built-in SF2 PCM16 path and optional
`sf3-vorbis` decoder are bounded helpers; production assets remain separate. Fixture
provenance and checksums are recorded in `tests/fixtures/`.

`render_offline_wav` mixes a validated frame schedule with a caller-supplied SF2 into a bounded
PCM16 WAV. The CLI exposes this as `acorde audio-render score.musicxml piano.sf2 output.wav`.
It uses linear sample interpolation and writes mono or stereo output. This is a basic native
offline renderer: it does not implement release envelopes, pedal, pitch-bend automation,
ornament realization, effects, SF3 input, or live audio playback. Unsupported expressive events
return errors. A supplied SF2 must contain every requested bank/program/key/velocity zone.

It does not require an asset or audio device. Applications may provide their own licensed assets
and renderer; the bounded native renderer is optional. The checksum is path-independent FNV-1a
64-bit over the asset bytes.
`PLAYBACK_CONTRACT_VERSION` identifies the action contract consumed by providers.

Security invariants and resource-limit ownership are documented in the
[security contract](../../docs/security/threat-model.md). SoundFont bytes, provider metadata,
and decoded samples are untrusted; this crate remains bounded, synchronous, and filesystem-free.
