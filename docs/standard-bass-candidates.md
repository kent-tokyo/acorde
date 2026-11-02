# Standard-bass button candidates

`model::standard_bass` searches a caller-supplied button inventory for a simultaneous single-bass plus chord-button combination. The inventory must specify the actual MIDI pitches and a position for each button on the instrument, as well as the permitted maximum distance in the same caller-defined units. The API does not infer a layout, register or chord voicing from the number of buttons.

`standard_bass_request_for_chord` converts supported structured chord symbols into a pitch-class request, including an explicit slash bass. It rejects unsupported spellings, kinds and altered degrees. `standard_bass_candidates` returns deterministically ordered candidates, with the actual button IDs and pitches, distance, and retained, omitted and added pitch classes. Omissions and additions require explicit permission. An empty list means this bounded search did not find a candidate, not that the chord cannot be played.

The full and viewer WASM builds expose `standard_bass_chord_candidates(profileJson, chordJson, allowOmissions, allowAdditions)` and return the same candidates as JSON. For example, a C bass sounding MIDI 36 and an E minor button sounding 64, 67 and 71 form an exact Cmaj7 candidate if the profile permits the reach. A profile whose seventh button omits a chord tone must report that omission; a caller must explicitly allow it.

A candidate describes **simultaneous** sound only and always requires performer review. The distance is a screening value, not a fingering certificate. This API does not modify scores, generate playback, plan transitions, or yet integrate with the arrangement and preset APIs. In particular, an alternating bass/chord rhythm must not be represented as if both buttons sound together.
