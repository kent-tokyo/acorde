# Notation coverage matrix

This matrix is versioned with the library. It describes the v0.48.x capability slices and is
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
| Volta brackets and part groups | yes | no | partial | partial | no | yes | yes / yes / MusicXML |
| MIDI channel, program, transposition | yes | yes | no | partial | no | yes | yes / no / MIDI |
| Percussion | partial | partial | no | partial | no | yes | partial / partial / MIDI |
| Tablature and microtonal accidentals | no | no | no | no | no | no | unsupported / no / no |

## Reading the matrix

- A `partial` import must not be interpreted as lossless interchange. Until diagnostic reports are
  available, callers should validate the resulting `Score` and retain the source document.
- JSON is the complete internal serialization surface for fields currently present in `Score`.
  Adding a model field requires the backwards-compatible serde/default and parser/serializer work
  described in `AGENTS.md`.
- “Exported” names the canonical format path currently implemented, not a promise that every
  imported feature can be reconstructed byte-for-byte.

## Versioning and evidence

The matrix applies to v0.48.x. Each `yes` slice must have a fixture or focused round-trip test in
the repository. Known losses are tracked here until `ImportReport` and `ExportReport` expose source
location, severity, preserved value, and loss reason through the native, CLI, and WASM APIs. The
MEI boundary currently supports one part, one staff/layer per measure, title, notes, rests,
accidentals, dots, and power-of-two durations; known unsupported MEI elements are surfaced as
warning diagnostics, while other MEI data remains intentionally outside the subset.
