# acorde

Umbrella crate for the acorde score library.

It re-exports acorde-core, acorde-io, and acorde-layout as acorde::core, acorde::io, and
acorde::layout. Its default I/O features are musicxml and midi; enable abc or mscz when needed.
acorde-render-svg is intentionally not re-exported and must be added directly.

~~~toml
[dependencies]
acorde = "0.9"
acorde-render-svg = "0.9"
~~~

[API documentation](https://docs.rs/acorde) · [Repository](https://github.com/kent-tokyo/acorde)
