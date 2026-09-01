# acorde

Umbrella crate for the acorde score library.

It re-exports acorde-core, acorde-io, and acorde-layout as acorde::core, acorde::io, and
acorde::layout. Its default I/O features are musicxml and midi; enable abc, mscz, or mei when needed.
acorde-render-svg is intentionally not re-exported and must be added directly.

~~~toml
[dependencies]
acorde = "0.82"
acorde-render-svg = "0.82"
~~~

[API documentation](https://docs.rs/acorde) · [Repository](https://github.com/kent-tokyo/acorde)

Umbrella-crate dependency and feature-boundary rules are documented in the [security contract](../../docs/security/threat-model.md).
