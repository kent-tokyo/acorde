# Explicit accordion reduction

`arrange_for_accordion_with_options` reduces existing source parts to a
two-staff accordion score. It is separate from
`arrange_with_explicit_chords`, which generates new accompaniment from supplied
harmony. The existing `arrange_for_accordion` API is unchanged.

`AccordionReductionOptions` selects one right-hand part and an ordered list of
left-hand parts. `None` chooses the highest-ranked pitched part for the right
hand; an empty left list chooses the other pitched parts. Selected source
voices are copied into separate left-hand voices, up to the model's four-voice
capacity. This preserves each copied note's pitch, duration, ties, tuplets and
identity, including in pickups and repeated sections.

With `loss_policy: "require-all"`, a source note that cannot fit, an unselected
source note, or incompatible measure metadata returns `score: null` and
diagnostics. With `"permit-reduction"`, omitted notes are listed individually
with `NoteAddr` source locations and `target: null` in `source_map`; incompatible
metadata is reported. The returned score is never an unreported reduction.
The optional `octave_fit` setting applies a whole-score octave shift and
reports its semitone amount. It is off unless explicitly requested.

The result includes selected part indices in `resolved_options`, source-to-output
note addresses, diagnostics, the optional score, transposition semitones, and
`contract_version: 1`. The input score is not modified. The same JSON contract
is available through the WASM function `arrange_for_accordion_with_options`.

This first contract requires selected parts to share bar numbers, physical
lengths, repeat markers and endings. Extra measure annotations from a second
left-hand part cannot be represented on the combined staff and are diagnosed.
Source spanners and object-specific style overrides currently return an
unsupported-notation diagnostic because their part addresses need remapping.
