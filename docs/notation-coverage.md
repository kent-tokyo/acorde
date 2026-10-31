# Notation coverage matrix

This matrix describes the v1.3.6 public capability boundary. **Yes** means the value is modelled
and covered by a focused test; **partial** means a documented common subset; **no** means that
format path does not carry it. “Model/SVG/export” distinguishes the canonical score from visible
rendering and the broadest implemented export route. It is not a lossless-interchange claim.

| Slice | MusicXML | MIDI | ABC | MSCX/MSCZ | MEI | Model / SVG / export |
|---|---:|---:|---:|---:|---:|---|
| Parts, staves, measures, voices | yes | partial | partial | partial | partial | yes / yes / yes |
| Pitch, rests, chords, standard durations | yes | partial | partial | partial | partial | yes / yes / yes |
| Tempo, meter, key, clef | yes | partial | yes | yes | partial | yes / yes / MusicXML |
| Barline, repeat, navigation, volta | partial | partial | partial | partial | partial | yes / partial / MusicXML |
| Dynamics, articulations, ornaments | yes | no | partial | partial | partial | yes / yes / MusicXML |
| Lyrics and expression text | partial | no | partial | partial | partial | partial / partial / MusicXML, ABC |
| Ties, slurs, tuplets, grace/cue notes | yes | partial | partial | partial | partial | partial / partial / MusicXML |
| Hairpin, pedal, ottava, trill, glissando | yes | no | no | partial | partial | yes / partial / MusicXML |
| Chord symbols, rehearsal, figured bass, text | partial | no | no | partial | partial | yes / partial / MusicXML |
| MIDI channel, program, control data | yes | yes | no | partial | no | yes / no / MIDI |
| Percussion and unpitched placement | partial | partial | no | partial | no | partial / partial / MIDI |
| TAB staff, tuning, string/fret positions | partial | no | no | partial | partial | partial / partial / MusicXML, MSCX |
| Guitar Pro 3–8 | import only | no | no | no | no | partial / partial / MusicXML, MEI, MSCX |
| Courtesy/editorial accidentals | yes | no | no | yes | partial | yes / partial / MusicXML, MEI, MSCX |
| Microtonal cents and quarter-tone subset | partial | no | partial | partial | partial | partial / partial / format-specific |
| Print, PDF, and browser presentation | n/a | n/a | n/a | n/a | n/a | logical print / SVG / host PDF |

## What callers must do

- Keep original source material when format-specific placement, vendor extensions, exact typography,
  complex TAB techniques, or byte identity matter.
- Call `validate` after import and use `parse_*_with_report` / `serialize_*_with_report` for a
  machine-readable loss boundary. A clean report only covers diagnostics implemented by that API.
- JSON is the complete serialization of fields currently in `Score`. New model fields are
  serde-defaulted for old JSON, but consumers with exhaustive Rust matches or struct literals can
  still need source changes; see [migration notes](migrations.md).
- MusicXML/MXL is the broadest interchange path. MIDI is an event projection, not a notation
  round-trip; Guitar Pro is import-only; all other formats are bounded subsets.
- The SVG renderer retains typed source addresses and deterministic geometry. Font shaping,
  publication typography, OS printing, and audio synthesis belong to a host.

## Important known limits

- MEI TAB articulations are not exported where the observed Verovio path rejects them.
- ABC does not yet preserve octave-clef variants or later lyric verses on every path.
- Guitar Pro right-hand fingering, slap/pop, whammy, volume swells, harmonic types, and chord
  diagrams need model work before they can be preserved.
- Irregular/cadenza bars and rests shorter than a 128th note require a single model, validation,
  playback, and serializer contract before support is claimed.
- Vendor-specific MSCX extensions and unmatched spanner endpoints remain diagnostics rather than
  guessed semantics.

## Evidence and maintenance

Every **yes** row must have a committed fixture or focused test. Fixture provenance is recorded in
[`tests/fixtures/manifest.json`](../tests/fixtures/manifest.json); local acceptance rules are in
[interchange evidence](interchange-evidence.md). Version-pinned external observations are kept in
[external interoperability evidence](external-interoperability-evidence.md) and do not promote a
local subset to a general compatibility claim.

When a new field is added, update the parser, serializer, report path, focused tests, and this
matrix together. Do not use a renderer fallback or a format-specific approximation to hide a loss.
