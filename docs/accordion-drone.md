# Explicit accordion drone (contract v1)

`acorde_core::arrange_with_drone` and the viewer WASM export
`arrange_with_drone(score_json, options_json)` take a validated, one-part,
one-staff monophonic melody and a MIDI bass pitch chosen by the host. The
minimal options are `{"pitch_midi":36,"bass_mode":"standard-bass","layers":null}`.
No chord progression is required or inferred. A result contains the generated
two-staff score, resolved options, source/generated note addresses, structured
diagnostics, and `contract_version: 1`. Errors return no partial score.

The bass note sustains through each physical measure, including melody rests.
Long durations may be written as tied notes inside the measure. Each new
measure reattacks the note, including on a repeat; no tie crosses a barline.
The melody staff, irregular first or final measures, repeat marks and endings
are copied from the source. The generated staff has the same actual lengths.
Playback uses the ordinary score repeat sequence and note ties. The chosen
pitch does not change when an optional harmony progression changes. The API
does not judge whether that combination sounds consonant.

`standard-bass` accepts MIDI 36–47 as one conservative bass-button octave;
`free-bass` accepts MIDI 36–84. These checks do not prove a particular
instrument's button layout. The melody must fit the right-hand MIDI 48–96
range. Source structure, range, rhythm and output validation errors have
diagnostic codes and measure indices where applicable.

Optional `layers` uses `LayeredAccordionOptions` to add explicit chord-based
melodic voices. It must set `accompaniment: null`, `swing: "straight"`, and
the same `bass_mode` as the drone. This lets a host add a harmony-derived
voice while the drone stays at its explicit pitch. Without `layers`, neither
chords nor a layered configuration are required. Drone options serialize for
deterministic replay against the same parsed score. This API does not add a
built-in preset or change the versioned preset/free-layer configuration
contracts in `accordion-preset-contract.md`.
