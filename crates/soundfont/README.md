# acorde-soundfont

`acorde-soundfont` defines the optional SoundFont playback boundary for acorde.
It validates bounded SF2/SF3 RIFF metadata, resolves bank/program presets, and
manages provider-neutral note lifecycle actions from `acorde_core::PlaybackEvent`.

It does not decode samples or include a synthesizer. Applications provide a
separately licensed SF2/SF3 renderer, so no external sample assets or vendor
library become a requirement for acorde itself. The checksum is path-independent
FNV-1a 64-bit over the asset bytes. `PLAYBACK_CONTRACT_VERSION` identifies the
action contract consumed by providers.
