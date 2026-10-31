# Explicit-chord accordion accompaniment

`acorde_core::arrange_with_explicit_chords` takes a one-part, one-staff monophonic
melody and `GuidedAccordionOptions`. The matching WASM function accepts the score
and options as JSON strings. Neither function infers harmony from the melody.

Each `PositionedAccordionChord` gives a zero-based `measure_index`, a quarter-note
`offset_beats` on a 1/64-beat grid, and a `ChordSymbol`. The first chord must start
at measure 0, beat 0. A chord remains in effect until the next supplied chord;
additional chords can begin inside a measure. The first scope accepts major,
minor, diminished and augmented triads, with an optional slash bass. Unsupported
kinds and chord degrees return a diagnostic.

The accompaniment patterns include held bass chord, bass/chord pulse,
alternating bass chord, arpeggio, rising arpeggio, Alberti bass, offbeat
chords, swing accompaniment, and walking bass. Held notes can be tied to fill
a chord region. Alternating bass and chord normally requires whole-beat regions;
arpeggio requires half-beat regions. A fractional first pickup is a narrow
exception for bass/chord pulse, alternating bass, and offbeat chords when its
start and chord changes fall on half-beat boundaries. A region that cannot be
represented exactly is rejected. No rests are silently added to a short bar.

The pattern phase starts at beat 0 of each authored measure. For a fractional
first pickup, bass/chord pulse, alternating bass, and offbeat chords instead
use the suffix of the nominal meter: a 0.5-beat pickup in 6/8 starts at
virtual beat 2.5, while a 1.5-beat pickup starts at virtual beat 1.5.
The next full measure starts at beat 0. Whole-beat pickups retain their
established measure-start phase.
The phase continues across an actual chord change inside a measure. Repeating
an identical chord inside a measure does not restart the phase or change the
generated notes. The supplied chord positions remain in `resolved_options`;
repeating a chord does not request a pattern reset.

The result contains `score` on success, the supplied `resolved_options`,
`source_notes` and `generated_notes` address lists, `diagnostics`, and
`contract_version: 1`. On failure `score` is `null`, so callers cannot use a
partial arrangement. The input score is unchanged. The source melody staff is
copied intact, including pickup length, ties, repeats and alternate endings;
the generated accompaniment is the second staff. The first preset uses MIDI
ranges 48–96 for the melody hand and 36–84 for the accompaniment hand. These
are conservative defaults for this operation, not a model of every accordion.

For the viewer WASM build, use `--no-default-features --features viewer` and call
`arrange_with_explicit_chords(score_json, options_json)`. The returned score can
be passed to `render_score_svg` and `to_playback_events_ex` in the same build.
