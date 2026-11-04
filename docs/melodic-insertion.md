# Conservative melodic insertions

`acorde_core::insert_melodic_notes` and the viewer/full WASM `insert_melodic_notes`
export create an edited copy of a score. Supply a zero-based part, staff, voice and
end-exclusive measure range, an explicit `KeySignature`, insertion kind (`passing`,
`upper-neighbor`, or `lower-neighbor`), source (`shorten-note` or `fill-rest`),
maximum insertion count, MIDI range, and maximum semitone leap. No harmony or key
is inferred. The same input score and options yield the same note IDs and musical
content; no seed is needed.

The current conservative subset operates within a measure. It inserts a diatonic
passing tone between notes two letter steps apart, or an upper/lower diatonic
neighbor when the surrounding pitches are the same. It shortens an undotted half
note/rest into a dotted quarter plus an inserted eighth, or an undotted quarter
into a dotted eighth plus an inserted sixteenth. For example, C half + E half
becomes C dotted quarter + D eighth + E half; E retains its original start.
With `fill-rest`, a preceding pitch and following pitch surround the rest, which
is shortened at its end. Source pitches must fit the explicit key; the generated pitch and each adjoining
leap must fit the requested pitch range and leap limit; chromatic, microtonal and other cases are diagnosed.

`score` is a copy, and `source_mapping` maps every addressed source note in the
selected bars to its output address. Each `insertions` record identifies the
source anchor, output anchor and generated note, their original duration, and
the generated pitch. Use these addresses with interactive SVG to distinguish
added notes. Keep the original input score to undo; this API does not add an
entry to `ScoreEngine` history. The returned options can be saved and reapplied
to the same input. Callers must inspect `diagnostics`: a bar may be unchanged
while another is edited, or `score` may be `null` for invalid/unsupported input.

The current version leaves bars with polyphony, ties, tuplets, articulations, authored beams or
legacy span endpoints unchanged and diagnoses them. It rejects scores with
typed spanners, harmony ranges, object overrides or linked views because adding
notes would require remapping their addresses. Key changes that disagree with the
explicit key are diagnosed per bar. Only 128 selected bars and 128 insertions
are supported per request. The source score, later note onsets, bar lengths,
repeats, and ending notes are preserved. These restrictions are deliberate;
no chord-tone, cross-measure insertion or general ornamentation is inferred.
