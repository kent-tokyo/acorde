# acorde

Umbrella crate for the acorde score library.

It re-exports acorde-core, acorde-io, and acorde-layout as acorde::core, acorde::io, and
acorde::layout. Its default I/O features are musicxml and midi; enable abc, mscz, or mei when needed.
The optional `soundfont` feature re-exports `acorde-soundfont` as `acorde::soundfont`; it provides
bounded SF2/SF3 metadata and provider-neutral playback lifecycle actions without bundling samples.
acorde-render-svg is intentionally not re-exported and must be added directly.

~~~toml
[dependencies]
acorde = "1.0.4"
acorde-render-svg = "1.0.4"
~~~

[API documentation](https://docs.rs/acorde) · [Repository](https://github.com/kent-tokyo/acorde)

Umbrella-crate dependency and feature-boundary rules are documented in the [security contract](../../docs/security/threat-model.md).
