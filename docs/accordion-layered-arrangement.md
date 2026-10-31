# Layered accordion arrangement (contract v1)

`acorde_core::arrange_with_layers` and the viewer WASM export
`arrange_with_layers(score_json, options_json)` take a validated, one-part,
one-staff monophonic melody. Chords must be positioned explicitly; the API does
not infer harmony. It returns a canonical `Score` or structured diagnostics,
resolved options and chords, and source/generated `NoteAddr` lists. Failure
returns no partial score. The source melody staff is cloned, preserving its
note identity, notation, pickup, repeats and endings.

`LayeredAccordionOptions` selects independent layers:

- `accompaniment`: `held-bass-chord`, `bass-chord-pulse`,
  `alternating-bass-chord`, `arpeggio`, `rising-arpeggio`, `alberti-bass`,
  `offbeat-chords`, `swing-accompaniment`, `walking-bass`,
  `waltz-bass-chord` or `waltz-alternating-bass-chord`; `null` leaves the
  left-hand voice empty.
- `alternative_chords`: optional explicit replacement progression. Inversion
  may be `root`, `first`, `second`, `third` or `nearby`; the latter chooses the
  nearest available bass pitch class to the prior resolved bass.
- `melodic_voice`: `unchanged`, `third-below`, `sixth-below` or
  `octave-below`. An added voice uses the resolved chord tones and remains
  separate from the melody. Incompatible chord tones produce diagnostics.
- `swing`: `straight` or `notated-accompaniment-two-to-one`. The latter writes
  quarter/eighth triplet pairs in the accompaniment, and sets `notated_swing`
  in the result. Hosts should leave `PlaybackOptions.swing` unset for this
  output to avoid applying a second timing shift. This fixed 2:1 teaching
  pattern does not describe every swing performance.
- `bass_mode`: `standard-bass` or `free-bass`. The former rejects seventh
  chords and melodic bass patterns requiring free bass. This conservative
  check is not a complete button-map or fingering proof.

Supported chord kinds are `major`, `minor`, `diminished`, `augmented`,
`major-seventh`, `minor-seventh` and `dominant`. Slash bass and chord changes
use the same positioned chord contract as `arrange_with_explicit_chords`.
The waltz patterns use 3/4: bass on beat 1, chord on beats 2 and 3. A first
pickup starts at the matching final beat of the nominal bar and does not count
toward root/fifth alternation; the first complete bar starts with root bass.
Other patterns use their documented measure phase. A chord change keeps the
phase; repeating the same chord within a measure leaves sound and timing
unchanged.
Generated notes are limited to the documented conservative hand ranges in
`arrange.rs`; unsupported rhythms, source structures, harmony and ranges
produce diagnostic codes with a measure index where available.

The output score feeds the ordinary SVG, playback, MusicXML and other export
paths. The API does not migrate any host application by itself. A host can
replace its local pattern and pitch algorithms after it adopts this contract.

For a single sustained bass pitch without a chord progression, use
[`arrange_with_drone`](accordion-drone.md). Its optional `layers` setting can
add a chord-derived melodic voice while the bass pitch stays fixed.
