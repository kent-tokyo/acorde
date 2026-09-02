# acorde-soundfont

`acorde-soundfont` defines the optional SoundFont playback boundary for acorde.
It validates bounded SF2/SF3 RIFF metadata, resolves bank/program presets, and
manages provider-neutral note lifecycle actions from `acorde_core::PlaybackEvent`.

The provider contract also exposes `SampleRegion`, deterministic key/velocity zone selection,
`SampleVoiceParameters`, `SampleAction`, bounded `DecodedSample` validation, and a FIFO
`SampleCache`. A separately licensed SF2/SF3 decoder supplies sample regions and PCM; this crate
does not bundle proprietary assets or a codec. Unsupported compression or generator features must
be reported by that provider before creating a region.

It does not decode samples or include a synthesizer. Applications provide a
separately licensed SF2/SF3 renderer, so no external sample assets or vendor
library become a requirement for acorde itself. The checksum is path-independent
FNV-1a 64-bit over the asset bytes. `PLAYBACK_CONTRACT_VERSION` identifies the
action contract consumed by providers.
