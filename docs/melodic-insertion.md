# Melodic insertions

`acorde_core::insert_melodic_notes` and the viewer/full WASM export of the same
name retain the original conservative API. They take an explicit key, selected
part/staff/voice and end-exclusive measure range, passing or neighbor kind,
`shorten-note` or `fill-rest` source, maximum insertion count, MIDI range and
maximum semitone leap. No harmony is inferred. An undotted half note can become
a dotted quarter plus an eighth; an undotted quarter can become a dotted eighth
plus a sixteenth. The following source note keeps its onset.

`insert_melodic_notes_with_context(score, options, context)` adds an **optional,
explicit** `ChordPlan`, sorted `local_keys` (effective from each measure),
`phrase_end_measures`, host `selections`, and an optional
`max_insertions_per_measure`. The WASM function accepts these as
three JSON arguments. Empty context fields keep the legacy selection. A chord
plan must cover the insertion beat; unsupported chord roots/kinds and uncovered
positions are diagnosed without inventing harmony. Supported chord kinds are
major, minor, diminished, augmented, dominant/dominant-seventh,
major-seventh, minor-seventh, half-diminished and diminished-seventh.

For each eligible anchor the contextual result returns candidates with source
address, insertion beat, pitch, kind, deterministic rank, reason codes,
constraint violations and selected flag. Chord tones rank above resolved
weak-beat nonchord tones that resolve into the chord at the next original note;
a matching chromatic alteration can be chosen when the explicit chord supports
it. Unresolved dissonances and nonchord tones at a marked phrase ending are
rejected.
The caller may preview candidates and reapply an explicit `source_anchor`/`pitch`
selection. A selection outside the safe candidates leaves that anchor unchanged.
This is a bounded suggestion rule, not a general harmony or phrase analyzer.

Both functions return an edited **copy** and provenance. `source_mapping` maps
every selected source note to its output address; `insertions` maps each shortened
anchor and new note, including the original duration. Source note IDs, pitches,
onsets, bar lengths and repeat marks remain. Generated IDs and output are
reproducible for the same input and settings. Keep the input score for undo;
this API does not add a `ScoreEngine` history entry.

Typed spanners, harmony-range endpoints and note style targets are remapped to
the original notes' new addresses. Linked views retain their source parts and
staves. Authored beams, slurs, articulations and chord symbols on *other* notes
in a selected measure remain attached; the generated note has fresh notation.
An anchor with a tie, tuplet, authored beam, fermata, tremolo or cross-staff
placement is not split. Measures with another sounding voice remain unchanged.
Unsupported bars receive an address-bearing diagnostic while safe bars can
still be edited. `score: null` means invalid input; a non-null score with no
insertions means no applicable edit; insertions plus diagnostics mean partial
success. Rust callers can use `MelodicInsertionResult::outcome()`; the contextual
JSON result also includes `outcome` (`invalid`, `no-change`, `partial`, or
`applied`). Only 128 selected bars and 128 insertions are supported per request.

The fixture `crates/io/tests/fixtures/fujisan-1911.mscz` is from the public
fisarmonica.org score [Fujisan 1911](https://github.com/kent-tokyo/fisarmonica.org/blob/main/public/scores/fujisan-1911.mscz)
(SHA-256 `bf925dc1970f05d8f57e450892a96e159f86231473560f00539a5d944d9e64fb`).
Its source key is D major. The pinned 16-bar, passing/shorten-note test adds six
notes while preserving the original onset times. The viewer test compares native
and WASM binding results, renders the edited score to SVG, checks playback events and verifies
the MSCZ roundtrip of selected voice pitches and note durations. This fixture
tests an application path; it does not establish general musical quality.
