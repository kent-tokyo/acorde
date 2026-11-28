# Spelled harmony expansion

`expand_harmony(&ChordSymbol)` returns checked, spelled chord tones before an
instrument or register is chosen. The full and viewer WASM profiles expose the
same operation as `expand_harmony(chord_json)`, returning JSON.

```json
{"root":"D","kind":"minor-ninth","bass":null}
```

This expands to D–F–A–C–E with degrees 1, 3, 5, 7 and 9. Degree 9 retains its
compound interval; it is not replaced by degree 2. Each tone includes its
spelling, role, source directive and octave displacement. `pitch_at_root_octave`
returns an error outside MIDI range rather than clamping the pitch. A slash bass
is a separate field, not an extra upper-voice tone.

Kinds include triads, sixths, sevenths, ninths, elevenths, thirteenths, suspended
chords, power chords and pedal roots. Thirteenth kinds include the eleventh;
subsequent voicing decides which optional tones to omit. Explicit `subtract`
directives remove the exact degree, including an authored root omission.

The [MusicXML degree semantics](https://www.w3.org/2021/06/musicxml40/musicxml-reference/elements/degree-alter/)
are preserved: `alter` is relative to the tone in the base kind; `add` is
relative to dominant-scale intervals, including a minor seventh. For example,
lowering degree 7 in Cmaj7 produces B-flat. G7 with an added flat ninth uses
`{"value":9,"alter":-1,"kind":"add"}`. Altering a degree absent from the base
kind is rejected. Compact imported G7b9 labels are converted to that addition.

The initial contract accepts at most 32 degree directives, degrees 1–13 and
alterations from -2 to +2. Duplicate identical additions collapse, while flat
and sharp ninth additions may coexist. Contradictory edits, unknown kinds,
invalid roots/basses and empty harmonies return typed errors atomically.
`other` can specify a chord entirely through added degrees. Functional sixth
chords, `none` and microtonal degree alterations are outside this contract.

MusicXML preserves the structured kind and directives. MSCZ uses compact names;
an equivalent known alias can canonicalize the representation (G7 plus alter5
flat becomes the `dominant-flat-five` kind), while spelled degrees and sounding
tones remain equal. The fixed interchange corpus checks Cmaj9, Dm9, G13,
altered tensions, B-sharp spelling, diminished sevenths and root omission.

This API is opt-in. Existing standard-bass capabilities and arrangement presets
are unchanged. Bass-aware omission and voice assignment belong to stage 2;
onset scheduling and complete three-way arrangements belong to stage 3 of the
[implementation plan](development/arrangement-harmony-groove-plan.md).
