# Musical position comparison

`acorde_core::compare_musical_positions(reference, current, options)` compares
two scores for arrangement display. It returns `changes` with reference and
current `NoteAddr` values, an `added`, `removed`, or `modified` kind, changed
field names, and timing/pitch/notation domains. The same operation is exported
from the viewer WASM package as `compare_musical_positions(reference_json,
current_json, options_json)`. Both inputs remain unchanged. Structural
`diff_scores`, score patches, and compatibility reports keep their existing
index-based semantics.

With the default `strict` structure policy, the scores must have the same part,
staff, and measure shape. For differently structured arrangements, pass
`{"structure_policy":"index_prefix"}` to explicitly match existing voice
indices and report notes in additional voices as additions or removals. For
other correspondences, supply `voice_pairs` containing `reference` and
`current` `{part,staff,voice}` addresses. `note_pairs` can also supply known
source identity as pairs of complete note addresses. Invalid or duplicate
correspondences produce a diagnostic and no inferred match.

Notes are timed separately inside each measure using Acorde's duration model,
including dots and tuplets. Matching first preserves an unchanged note at the
same onset, then tries sound identity at the same onset, sound identity within
the measure, and finally a unique note at the same onset. Ambiguous remaining
notes are reported as additions and removals with a diagnostic. IDs and layout
offsets are ignored. A modified note reports `onset_beats` when its written
position changes; `tuplet_boundary`, ties, articulations, and other notation
fields remain visible even when playback timing is equal.

The returned addresses correspond to SVG `data-note-addr` metadata. The host
chooses highlighting, comparison controls, and scrolling.
