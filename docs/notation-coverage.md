# Notation coverage matrix

This matrix is versioned with the library. It describes the v1.1.x capability slices and is
intended to make information loss explicit. MEI import reports now identify supported-subset
losses for known unsupported elements; other partial-format losses remain documented here until
their parser-specific diagnostics are implemented.

Legend: **yes** means the value is represented and covered by tests; **partial** means the common
subset is supported; **no** means the format is not supported by that path. “Preserved” refers to
the `Score` model; rendering and export can have narrower format-specific coverage.

| Feature slice | MusicXML import | MIDI import | ABC import | MSCZ/MSCX import | MEI subset | JSON | Preserved / rendered / exported |
|---|---:|---:|---:|---:|---:|---:|---|
| Parts, staves, measures, voices | yes | partial | partial | partial | partial | yes | yes / yes / yes |
| Pitch, rests, chords, duration | yes | partial | partial | partial | partial | yes | yes / yes / yes |
| Tempo and time signature | yes | partial | yes | yes | partial | yes | yes / yes / yes |
| Key signature and clef | yes | no | yes | yes | partial | yes | yes / yes / MusicXML |
| Barlines, repeats, navigation | partial | partial | partial | partial | partial (barlines, common `dir` navigation marks) | yes | partial / partial / partial |
| Dynamics and articulations | yes | no | partial | partial | partial (common articulations/ornaments) | yes | yes / yes / MusicXML |
| Lyrics and expression text | partial | no | no | partial | partial (single-syllable lyrics, `dir`) | yes | partial / partial / MusicXML |
| Ties, slurs, tuplets, grace/cue notes | yes | partial | partial | partial | partial (ties/slurs/tuplets/grace) | yes | partial / partial / MusicXML |
| Hairpins, pedal, ottava, trill | yes | no | no | partial | no | yes | partial / yes / MusicXML |
| Glissando spanners and cross-staff placement | yes (standard start/stop and `<staff>`) | no | no | no | no | yes | yes / yes / MusicXML |
| Typed expression, technique, lyric, chord, rehearsal, figured-bass, generic text | partial | no | no | partial (`Harmony/name`, `Text`) | partial (`harm`, `fb`, `reh`, `dir`) | yes | partial / partial / MusicXML |
| Volta brackets and part groups | yes | no | partial | partial | no | yes | yes / yes / MusicXML |
| MIDI channel, program, transposition | yes | yes | no | partial | no | yes | yes / no / MIDI |
| Percussion | partial | partial | no | partial | no | yes | partial / partial / MIDI |
| Tablature positions and staff metadata | partial (staff-lines, tuning, string/fret, chord positions) | no | no | partial (StaffType, tuning, string/fret) | no | yes | partial / partial / MusicXML, MSCX |
| Microtonal accidentals | partial (fractional `<alter>`) | no | partial (quarter accidental subset) | partial | partial (`qs`/`qf`) | yes | partial / partial / format-specific |

## Reading the matrix

Tablature currently preserves MusicXML `staff-details/staff-lines`, `staff-tuning`, and note
`technical/string` plus `fret`, including per-pitch positions for chords; the SVG renderer displays
explicit positions and guitar technique labels. Core automatic string/fret assignment and
sequence-aware movement optimization are available for configured tablature staves. Alternate
tunings in non-MusicXML formats and instrument-specific engraving remain partial. Microtones use
`Pitch::microtone_cents`; ABC supports double accidentals plus pure `^/` and `_/` quarter-tone
spellings, and MEI supports `qs` and `qf`. These
declared quarter-tone spellings are 50 cents with no additional semitone alter; exact comparisons
can use `Pitch::to_midi_cents()`. MIDI pitch-bend and vendor-specific accidental spellings remain
partial. Playback events retain exact `pitch_midi_cents` alongside rounded `pitch_midi`; host audio
rendering remains backend-dependent. MSCX tab staffs preserve `StaffType group="tab"` line/tuning
data and note-level string/fret when present; `Tuplet`/`endTuplet` ranges preserve their
`actualNotes`/`normalNotes` ratio, `acciaccatura`/`appoggiatura` grace markers map to the
canonical grace-note flags, and Arpeggio direction maps to the canonical arpeggiate flag.
MSCX Tremolo subtypes (`r8`/`c8` through `r64`/`c64` and `buzzroll`) map to the canonical
speed-level articulation; the current model does not distinguish one-note from two-note tremolo.
Simple MSCX `Harmony/name` values use the same typed display-label boundary as MEI, and the
bounded `harmonyInfo/root` subset attaches canonical `ChordSymbol` data; structured harmony
semantics remain partial. MEI attached harm labels in the documented chord-quality subset
map to note-level ChordSymbol; MusicXML degree value/alter/type maps to `ChordDegree`; unresolved
or unsupported attachments remain diagnostics. Attached MEI `harm@deg` is retained as optional
harmonic-analysis metadata, and standard `harm@type` is retained as optional classification
metadata. MEI `harm@func` and MusicXML/MSCX harmony function values share the canonical
`ChordSymbol.harmony_function` field; unattached or timing-only attributes remain source-located
diagnostics.
MEI simple `fb`/`f` values map in order to `Measure.figured_bass` and typed display-level
`TextStyle::FiguredBass`; MEI leading accidental semantics, common `|`/`+` decorations,
balanced parentheses, source text, and `f@extender` are also preserved. MusicXML `figured-bass` figure number, alter, prefix, and suffix values map to structured
`Measure.figured_bass`; richer MEI and vendor-specific figured-bass semantics remain partial and
explicitly diagnosed. MusicXML `<unpitched>` percussion notes retain display-step/display-octave placement
and the canonical `Note.is_unpitched` flag; explicit instrument declarations and retained MIDI
display keys resolve sound identity, while unmatched identities remain source-located by the
import report.
MSCX tablature imports ordered `<Fingering>` values into `Note.fingerings` and mirrors the first
candidate in `Note.fingering`; deterministic source-order/lowest/highest selection is available,
while format-specific glyph fidelity remains
partial.
MSCX `FiguredBassItem/digit` values map in order to `Measure.figured_bass` and typed display text;
`continuationLine` maps to the canonical extender flag, while parentheses and other figured-bass engraving properties remain
partial.
The SVG renderer exposes the ordered candidates through a deterministic `acorde-tab-fingering`
annotation; external font/glyph equivalence remains outside the core renderer contract.
MusicXML note-level `instrument@id` is retained as `Note.instrument_id`; concrete percussion sound
catalog mapping remains partial.
Core validation rejects deserialized `microtone_cents` values outside -99..99.

- A `partial` import must not be interpreted as lossless interchange. Callers should validate the
  resulting `Score`, inspect the format report, and retain the source document when they need
  unsupported fields such as ABC tablature or vendor-specific notation.
- JSON is the complete internal serialization surface for fields currently present in `Score`.
  Adding a model field requires the backwards-compatible serde/default and parser/serializer work
  described in `AGENTS.md`.
- “Exported” names the canonical format path currently implemented, not a promise that every
  imported feature can be reconstructed byte-for-byte.
- Glissando supports MusicXML `<glissando type="start|stop">`; unsupported custom glissando
  variants are not inferred. Cross-staff notes retain their source note address and record the
  target staff; a target outside the part is rejected by the command engine.
- `TextStyle` is the typed JSON model for expression, technique, lyrics, chord symbols, rehearsal
  marks, and generic text. Only measure-level direction text currently has MusicXML emission;
  other style-specific placement remains provider/render-layer work.

## Versioning and evidence

The matrix applies to v1.1.x. Each `yes` slice must have a fixture or focused round-trip test in
the repository. The fixture provenance and evidence mode are pinned in
[`tests/fixtures/manifest.json`](../tests/fixtures/manifest.json); the evaluation rules are in
[`interchange-evidence.md`](interchange-evidence.md). Known losses are tracked here until `ImportReport` and `ExportReport` expose source
location, severity, preserved value, and loss reason through the native, CLI, and WASM APIs. The
WASM bindings expose report variants for MusicXML, MXL, MEI, MIDI, ABC, MSCZ, and MSCX imports,
plus MusicXML, MEI, MIDI, and ABC exports. The
MEI boundary currently supports one part, multiple numbered staves, up to four layers per measure,
title,
score-level and measure-level meter, score-level key signature and clef, notes, rests, accidentals,
dots, power-of-two durations, tuplets (`num`/`numbase`), grace notes (`@grace`), common dynamics/articulations/ornaments, lyrics, ties, slurs, repeat
barlines, multi-rests, measure-level chord labels, rehearsal marks, and directions; known unsupported MEI elements are surfaced as warning diagnostics, while
other MEI data remains intentionally outside the subset. Malformed MEI measure numbers, meter
attributes, tempo values, and multi-rest counts are retained as source-located diagnostics when
the parser must use a canonical fallback.
The machine-readable phase evidence is recorded in
[`interchange-report.json`](interchange-report.json); its external corpus gates are not counted
as local implementation evidence.
