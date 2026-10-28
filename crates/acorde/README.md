# acorde

Umbrella crate for the acorde score library.

It re-exports `acorde-core`, `acorde-io`, and `acorde-layout` as `acorde::core`, `acorde::io`, and
`acorde::layout`. Default I/O is `musicxml` and `midi`; enable `abc`, `mei`, `mscz`, or `gp` as
needed. Optional `omr` and `soundfont` re-export `acorde-omr` and `acorde-soundfont`; use
`soundfont-sf3-vorbis` for the separately licensed Vorbis decoder.

`acorde-render-svg` is intentionally a direct dependency. Format reports retain source-located
import/export diagnostics instead of silently normalizing malformed source values.

~~~toml
[dependencies]
acorde = "=1.2.18"
acorde-render-svg = "=1.2.18"
~~~

[API documentation](https://docs.rs/acorde) · [Repository](https://github.com/kent-tokyo/acorde)

Umbrella-crate dependency and feature-boundary rules are documented in the [security contract](../../docs/security/threat-model.md).
