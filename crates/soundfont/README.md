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
It uses linear sample interpolation and writes mono or stereo output. SF2 note releases decay
through the requested tail, and score pedal spans hold notes until pedal-up on the same part.
This basic native renderer does not implement MIDI controller automation (including CC64),
program-change or pitch-bend automation, effects, SF3 input, or live audio
playback. Unsupported expressive events return errors. A supplied SF2 must contain every
requested bank/program/key/velocity zone.
The CLI can opt into the core's deterministic trill, turn and related ornament attacks
with `--realize-ornaments`; the renderer still rejects unhandled expression explicitly.

The optional `full-sf2` feature adds `render_offline_wav_full_sf2`, backed by
[RustySynth](https://github.com/sinshu/rustysynth) (MIT). The CLI enables it with
`acorde audio-render score.musicxml gm.sf2 output.wav --full-sf2`. It interprets SF2 filters,
modulation, and envelopes and keeps Acorde's frame schedule and 64 MiB PCM16 output bound.
This mode requires 16–192 kHz. Reverb and chorus are disabled; its eight-frame synthesis blocks
can delay a score-frame attack or release by up to seven samples. It routes simultaneous score
voices and overlapping same-pitch notes to separate MIDI channels when available. It rejects
unrepresentable channel counts, unsupported score expression, and SF3 input with errors. The
provider-neutral zone materializer is unchanged and still reports unsupported generators.
Each note's effective program is selected at its attack, including measure-local instrument
changes from the score schedule.
Authored MIDI pitch bend and CC 1 (modulation), 7 (volume), 10 (pan), 11 (expression), and 64
(sustain) are applied at their scheduled frames to every voice channel routed from the source
part and MIDI channel. Authored MIDI program changes select the preset for subsequent notes on
the same part and source channel; the change is routed to split voice channels before attacks at
the same frame. Missing presets fail explicitly. An unrouted event, unsupported controller,
aftertouch, or simultaneous score-pedal and CC64 control returns an error rather than disappearing.

`normalize_pcm16_wav_peak` can level either Acorde renderer's completed WAV to a caller-selected
linear peak. The CLI exposes it as `--normalize-peak-dbfs=-1`, with an explicit range of
-48 to -0.1 dBFS. The pass preserves the sample count, channels, silence, and relative
dynamics, and reports any samples already at the PCM rail. It operates after PCM16
quantization, so it cannot recover pre-existing clipping or add bit depth. The default
rendering output is unchanged when the option is omitted.

It does not require an asset or audio device. Applications may provide their own licensed assets
and renderer; the bounded native renderer is optional. The checksum is path-independent FNV-1a
64-bit over the asset bytes.
`PLAYBACK_CONTRACT_VERSION` identifies the action contract consumed by providers.

Security invariants and resource-limit ownership are documented in the
[security contract](../../docs/security/threat-model.md). SoundFont bytes, provider metadata,
and decoded samples are untrusted; this crate remains bounded, synchronous, and filesystem-free.
