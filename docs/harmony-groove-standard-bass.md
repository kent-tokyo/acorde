# Standard-bass screen for harmony and groove proposals

`screen_harmony_groove_standard_bass(source, resolved, profile)` is an opt-in
Rust API, also exposed in full and viewer WASM with three JSON strings. It
checks whether one **exact generated candidate** fits the caller's
`StandardBassProfile`.

## Use the screen

1. Save the original `Score`, the candidate's `resolved` contract, and the
   instrument's button profile.
2. Screen that candidate. The function replays it from the saved source;
   changed source data or an invalid button inventory returns a typed error.
3. Inspect `playable_with_supplied_profile`, `button_states`, and the
   location-aware `diagnostics`. If it fails, a host can present the
   unrestricted candidate as free-bass or generate and screen a simpler
   edition separately.

The left hand includes the generated accompaniment part and any source bass
staff named by the request within the selected measures. The screen evaluates
playback events on every repeat visit and checks each sounding state after an
attack or release. It matches the complete MIDI pitch **multiset** against
one bass button, one chord button, or a reachable bass-plus-chord pair.
Repeated unisons remain distinct. A chord label or pitch-class match alone
does not pass. Button IDs, including counterbass IDs, get their meaning from
the supplied profile; Acorde does not infer an instrument layout.

Each `button_states` entry records time, measure, source note addresses,
sounding pitches, selected button IDs, and the buttons' pitches. A false
verdict rules out this score on this profile under the one-bass/one-chord
model. It says nothing about another profile or a revoiced edition. The
screen does not remove or alter melody notes.

## Limits

The result checks exact pitch and simultaneous button reach. It does not
certify fingering, ergonomics, bellows, hand transitions, or another
instrument's buttons; performer review remains necessary. Microtonal pitch
and pitch bends in the selected left hand are rejected instead of rounded.
The screen allows at most 4,096 left-hand playback events and 8,192 possible
bass/chord pairs. Exceeding either bound returns an error, not an unplayable
verdict.

For score layout and saved preset/layer configs, see the
[three-staff migration](migrations.md#v1315-standard-bass-accompaniment-staves).
