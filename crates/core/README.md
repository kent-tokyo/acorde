# acorde-core

Score data model and command engine for Rust and WebAssembly.

This crate has no I/O, filesystem, renderer, or async-runtime dependency. It provides Score, Part,
Staff, Measure, Note, notation types, ScoreEngine, serializable Command values, undo/redo,
validation, playback-event generation, score diff/patch, transposition, and music-theory helpers.
Pitches preserve fractional cents, and playback events expose exact MIDI-cent values in addition to
the rounded MIDI note, while notes and staves can carry tablature string/fret and
tuning metadata with serde defaults for older score JSON.
Validation rejects deserialized `microtone_cents` values outside the canonical -99..99 range.
`assign_tablature_positions` provides deterministic capo-aware placement, while
`optimize_tablature_positions` considers movement between notes and chord strings. `SetTabPositionCmd`
makes explicit positions editable through the command engine.
Ordered alternate fingering candidates are stored in `Note.fingerings`; `SetFingeringsCmd` keeps
the first candidate synchronized with the legacy `Note.fingering` field.
`Note::select_fingering` provides a non-mutating deterministic source-order, lowest-number, or
highest-number projection.
MusicXML guitar bend amounts are retained as optional cent values on `Note` and can be edited with
`SetGuitarBendAlterCmd`.
Unpitched notes can be resolved with `Part::percussion_instrument_for_note`, which prefers an
explicit note instrument ID, then a declared MIDI display-key mapping, and never guesses an
unmatched sound identity.
MEI staff-group boundaries are represented separately as `Part.staff_groups`; they are not
confused with `Score.part_groups`, which connect distinct parts.
Chord symbols preserve optional vertical placement and structured MusicXML degree
value/alter/type data through `ChordSymbol.degrees`. MEI/MusicXML/MSCX harmonic-function metadata
is preserved through the serde-defaulted `ChordSymbol.harmony_function` field.
`Pitch::try_with_microtone()` rejects cents outside the canonical `-99..=99` range without
clamping; the legacy `with_microtone()` constructor retains its clamping behavior for
compatibility. Measure-level styled text can be edited transactionally with
`Command::SetMeasureText`: provide an existing `text_index` to replace/remove an entry, or the
current length to append one. Invalid indexes return an error before mutation, and the operation
participates in the normal undo/redo and JSON command-history contracts.

~~~rust
use acorde_core::{Command, ScoreEngine, SetTempoCmd};

let mut engine = ScoreEngine::new();
engine.apply(Command::SetTempo(SetTempoCmd { bpm: 120 }))?;
engine.undo()?;
~~~

The current Score JSON schema version is 1. MIDI pitch-bend, Controller Change, Program Change,
and key/channel Aftertouch events are preserved per part with canonical 480-PPQ tick and channel
metadata. Add acorde-io for format conversion and acorde-layout
for renderer-independent layout.

Security invariants and resource-limit ownership are documented in the [security contract](../../docs/security/threat-model.md).

[API documentation](https://docs.rs/acorde-core) · [Repository](https://github.com/kent-tokyo/acorde)
