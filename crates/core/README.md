# acorde-core

Score data model and command engine for Rust and WebAssembly.

This crate has no I/O, filesystem, renderer, or async-runtime dependency. It provides Score, Part,
Staff, Measure, Note, notation types, ScoreEngine, serializable Command values, undo/redo,
validation, playback-event generation, score diff/patch, transposition, and music-theory helpers.
Pitches preserve fractional cents, while notes and staves can carry tablature string/fret and
tuning metadata with serde defaults for older score JSON.

~~~rust
use acorde_core::{Command, ScoreEngine, SetTempoCmd};

let mut engine = ScoreEngine::new();
engine.apply(Command::SetTempo(SetTempoCmd { bpm: 120 }))?;
engine.undo()?;
~~~

The current Score JSON schema version is 1. Add acorde-io for format conversion and acorde-layout
for renderer-independent layout.

Security invariants and resource-limit ownership are documented in the [security contract](../../docs/security/threat-model.md).

[API documentation](https://docs.rs/acorde-core) · [Repository](https://github.com/kent-tokyo/acorde)
