# acorde-core

Score data model and command engine for Rust and WebAssembly.

This crate has no I/O, filesystem, renderer, or async-runtime dependency. It provides Score, Part,
Staff, Measure, Note, notation types, ScoreEngine, serializable Command values, undo/redo,
validation, playback-event generation, score diff/patch, transposition, and music-theory helpers.
Pitches preserve fractional cents, while notes and staves can carry tablature string/fret and
tuning metadata with serde defaults for older score JSON.
`assign_tablature_positions` provides deterministic capo-aware placement, while
`optimize_tablature_positions` considers movement between notes and chord strings. `SetTabPositionCmd`
makes explicit positions editable through the command engine.

~~~rust
use acorde_core::{Command, ScoreEngine, SetTempoCmd};

let mut engine = ScoreEngine::new();
engine.apply(Command::SetTempo(SetTempoCmd { bpm: 120 }))?;
engine.undo()?;
~~~

The current Score JSON schema version is 1. MIDI pitch-bend events are preserved per part as
signed 14-bit values with PPQ tick and channel metadata. Add acorde-io for format conversion and acorde-layout
for renderer-independent layout.

Security invariants and resource-limit ownership are documented in the [security contract](../../docs/security/threat-model.md).

[API documentation](https://docs.rs/acorde-core) · [Repository](https://github.com/kent-tokyo/acorde)
