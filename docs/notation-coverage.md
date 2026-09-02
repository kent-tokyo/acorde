# Notation coverage matrix

This matrix is versioned with the library. It describes the v1.0.x capability slices and is
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
| Tempo and time signature | yes | partial | yes | yes | no | yes | yes / yes / yes |
| Key signature and clef | yes | no | yes | yes | no | yes | yes / yes / MusicXML |
| Barlines, repeats, navigation | partial | partial | partial | partial | no | yes | partial / partial / partial |
| Dynamics and articulations | yes | no | partial | partial | no | yes | yes / yes / MusicXML |
| Lyrics and expression text | partial | no | no | partial | no | yes | partial / partial / MusicXML |
| Ties, slurs, tuplets, grace/cue notes | yes | partial | partial | partial | no | yes | partial / partial / MusicXML |
| Hairpins, pedal, ottava, trill | yes | no | no | partial | no | yes | partial / yes / MusicXML |
| Glissando spanners and cross-staff placement | yes (standard start/stop and `<staff>`) | no | no | no | no | yes | yes / yes / MusicXML |
| Typed expression, technique, lyric, chord, rehearsal, generic text | partial | no | no | partial | no | yes | partial / partial / MusicXML |
| Volta brackets and part groups | yes | no | partial | partial | no | yes | yes / yes / MusicXML |
| MIDI channel, program, transposition | yes | yes | no | partial | no | yes | yes / no / MIDI |
| Percussion | partial | partial | no | partial | no | yes | partial / partial / MIDI |
| Tablature positions and staff metadata | partial (staff-lines, string/fret) | no | no | no | no | yes | partial / partial / MusicXML |
| Microtonal accidentals | partial (fractional `<alter>`) | no | partial (quarter accidental subset) | partial | partial (`qs`/`qf`) | yes | partial / partial / format-specific |

## Reading the matrix

Tablature currently preserves MusicXML `staff-details/staff-lines` and note `technical/string`
plus `fret`. Microtones use `Pitch::microtone_cents`; ABC supports `^/` and `_/`, and MEI supports
`qs` and `qf`. MIDI pitch-bend and vendor-specific accidental spellings remain partial.

- A `partial` import must not be interpreted as lossless interchange. Until diagnostic reports are
  available, callers should validate the resulting `Score` and retain the source document.
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

The matrix applies to v1.0.x. Each `yes` slice must have a fixture or focused round-trip test in
the repository. Known losses are tracked here until `ImportReport` and `ExportReport` expose source
location, severity, preserved value, and loss reason through the native, CLI, and WASM APIs. The
MEI boundary currently supports one part, one staff/layer per measure, title, notes, rests,
accidentals, dots, and power-of-two durations; known unsupported MEI elements are surfaced as
warning diagnostics, while other MEI data remains intentionally outside the subset.
