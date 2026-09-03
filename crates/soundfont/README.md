# acorde-soundfont

`acorde-soundfont` defines the optional SoundFont playback boundary for acorde.
It validates bounded SF2/SF3 RIFF metadata, resolves bank/program presets, and
manages provider-neutral note lifecycle actions from `acorde_core::PlaybackEvent`.

The provider contract also exposes `SampleRegion`, deterministic key/velocity zone selection,
`SampleVoiceParameters`, `SampleAction`, bounded `DecodedSample` validation, and a FIFO
`SampleCache`. `SampleDecoder`, `SampleRenderer`, and the versioned `SoundFontProvider` contract
provide the stable integration boundary: a
separately licensed SF2/SF3 decoder supplies sample regions and bounded PCM, and a host-owned
renderer consumes that PCM with `SampleAction`. This crate does not bundle proprietary assets, a
codec, an audio device, or a synthesizer. Unsupported compression or generator features must be
reported by that provider before creating a region.
`SoundFontPresetZone` and `select_preset_zone` provide the shared bank/program/key/velocity
mapping API; providers can translate SF2 generators or SF3 metadata once and Composer does not
need to duplicate that parsing. Each mapped region carries bounded half-open sample frame
offsets, sample rate, loop points, tuning, attenuation, and envelope parameters.
`schedule_preset_note_on` combines that selection with the validated voice plan in one call.

`PROVIDER_CONTRACT_VERSION` identifies the adapter contract. Providers advertise
`ProviderCapabilities` and hosts should call `validate_provider_capabilities` before decoding;
unsupported PCM/Vorbis or synthesis support returns a typed error. The checked-in regression
suite includes a CC0 SF2 fixture and the MIT-licensed FluidR3Mono SF3 fixture, with provenance
and checksums in `tests/fixtures/`; the SF3 test is gated behind `sf3-vorbis`. Production and
provider-owned assets remain separate.

`validate_sample_region` rejects malformed key/velocity ranges, loop points, sample rates, and
envelope values before scheduling. Unsupported generator data should be rejected by the provider
as `Error::UnsupportedGenerator` rather than silently approximated.

The built-in `decode_sf2_pcm16` path decodes bounded PCM16 from an SF2 `smpl` chunk, and
`render_sample_action` provides deterministic nearest-neighbor playback with gain, envelope,
looping, and pitch-rate handling. Enable the optional `sf3-vorbis` feature to use the separately
licensed `lewton` decoder for SF3 Ogg/Vorbis payloads; without it, Vorbis returns an explicit
unsupported-compression error.

It does not decode samples or include a synthesizer. Applications provide a
separately licensed SF2/SF3 renderer, so no external sample assets or vendor
library become a requirement for acorde itself. The checksum is path-independent
FNV-1a 64-bit over the asset bytes. `PLAYBACK_CONTRACT_VERSION` identifies the
action contract consumed by providers.
