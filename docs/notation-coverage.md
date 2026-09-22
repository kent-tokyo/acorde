# Notation coverage matrix

This matrix is versioned with the library. It describes the v1.2.3 capability slices and is
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
| Barlines, repeats, navigation | partial | partial | partial (common barlines/repeats) | partial | partial (barlines, common `dir` navigation marks) | yes | partial / partial / partial |
| Dynamics and articulations | yes | no | partial (common decorations) | partial | partial (common articulations/ornaments) | yes | yes / yes / MusicXML |
| Lyrics and expression text | partial | no | partial (`w:` lyrics) | partial | partial (single-syllable lyrics, `dir`) | yes | partial / partial / MusicXML, ABC |
| Ties, slurs, tuplets, grace/cue notes | yes (numbered typed slurs) | partial | partial (common tuplets) | partial | partial (ties/slurs/tuplets/grace) | yes | partial / legacy geometry / MusicXML |
| Hairpins, pedal, ottava, trill | yes (typed pedal/ottava/trill endpoints) | no | no | partial | no | yes | partial / legacy geometry / MusicXML |
| Glissando spanners and cross-staff placement | yes (typed numbered start/stop and `<staff>`) | no | no | no | no | yes | yes / legacy geometry / MusicXML |
| Typed expression, technique, lyric, chord, rehearsal, figured-bass, generic text | partial | no | no | partial (`Harmony/name`, `Text`) | partial (`harm`, `fb`, `reh`, `dir`) | yes | partial / partial / MusicXML |
| Volta brackets and part groups | yes | no | partial | partial | no | yes | yes / yes / MusicXML |
| MIDI channel, program, transposition | yes | yes | no | partial | no | yes | yes / no / MIDI |
| Percussion | partial | partial | no | partial | no | yes | partial / partial / MIDI |
| Tablature positions and staff metadata | partial (staff-lines, tuning, string/fret, chord positions) | no | no | partial (StaffType, tuning, string/fret) | no | yes | partial / partial / MusicXML, MSCX |
| Microtonal accidentals | partial (fractional `<alter>`) | no | partial (quarter accidental subset) | partial | partial (`qs`/`qf`) | yes; non-zero cents emit explicit `acorde-microtone` SVG markers | partial / partial / format-specific |

## Reading the matrix

Typed numbered spanners pass through layout and interactive SVG as stable-ID metadata. Current
visible SVG geometry is supplied by the matching legacy note flags produced by MusicXML import;
direct geometry for a spanner authored only through `Score.spanners` is a later engraving phase.

Tablature currently preserves MusicXML `staff-details/staff-lines`, `staff-tuning`, and note
`technical/string` plus `fret`, including per-pitch positions for chords; the SVG renderer displays
explicit positions and guitar technique labels. Core automatic string/fret assignment and
sequence-aware movement optimization are available for configured tablature staves. SVG metadata
also exposes each tablature staff's line count, tuning MIDI values, and capo as typed fields, so
browser hosts do not need to reconstruct tuning from SVG geometry. Alternate
tunings in non-MusicXML formats and instrument-specific engraving remain partial. SVG also exposes
typed slide, hammer-on, pull-off, and cross-measure connection metadata; system-break continuation
segments follow the same edge-owned policy as other spans. Microtones use
`Pitch::microtone_cents`; MSCX/MSCZ preserves explicit stem direction and the canonical
notehead-shape subset (diamond, x, slash, cross, triangle) in its bounded export contract. ABC
supports common normal/double/repeat barlines and maps common
decorations to canonical articulations and deterministically serializes those supported
articulations back to `!name!` markers, while unsupported decorations remain diagnosed. ABC supports
inverted mordent and shake aliases in that same round-trip subset; `uppermordent` and
`lowermordent` map to the corresponding mordent directions.
The same decoration subset is retained on rests, including fermata markers, rather than being
dropped during import. ABC supports common `(p` tuplets and preserves their actual/normal timing
ratios; complete groups serialize as explicit `(p:q:r` markers, while incomplete groups are
reported as source-located export diagnostics instead of being silently flattened. ABC note ties
and slurs are preserved across notes and measure boundaries; slur parentheses are distinguished
from tuplet markers. It also preserves common `{...}` grace groups as `Note.is_grace`. It supports
common `w:` lyric lines with note alignment, syllable boundaries, `~`-joined spaces, and `*`
placeholders for unlyricized notes; only the first voice is serialized. ABC supports
source-located diagnostics for unsupported broken-rhythm markers (`<` and `>`), whose duration
transformations are outside the canonical duration subset. ABC supports
common single-number volta starts (`[1`, `[2`, ...) as `Measure.volta` with deterministic
round-trip serialization; non-`begin` volta kinds and richer multi-number ending syntax remain
outside the declared subset and are source-located on import/export.
ABC also supports double accidentals plus pure `^/` and `_/` quarter-tone spellings in notes and
chord members, with matching semantics for standalone and chord pitches; MEI supports `qs` and
`qf`. ABC barline kinds outside that subset are reported with source-located export diagnostics.
These
declared quarter-tone spellings are 50 cents with no additional semitone alter; exact comparisons
can use `Pitch::to_midi_cents()`. MIDI pitch-bend and vendor-specific accidental spellings remain
partial. Playback events retain exact `pitch_midi_cents` alongside rounded `pitch_midi`; host audio
rendering remains backend-dependent. MusicXML `default-x/default-y`, `relative-x/relative-y`,
Scientific-name formatting preserves authored extended accidental runs; parser overflow is rejected
instead of wrapping into an unrelated pitch. Authored ties are also prevented from connecting to
rests in SVG, while their start/end state remains available in typed metadata.
rendering attributes, and namespaced vendor attributes are now reported with stable,
source-located diagnostics and preserved values. Note-level `default-x/default-y` and
`relative-x/relative-y` numeric offsets are canonical, round-trip through MusicXML, and are
applied by SVG; direction `placement` (`above`/`below`) and MusicXML direction offsets remain
separate `StyledText` fields because their coordinate meanings are not collapsed. Vendor
semantics are not guessed into the canonical model. Other direction placement values are
preserved but reported as a typed diagnostic.
MSCX tab staffs preserve `StaffType group="tab"` line/tuning
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
diagnostics. `harm@tstamp` resolves to a note at the corresponding meter beat, and
`harm@tstamp2`/`endid` preserve a typed harmony end-note address with deterministic MEI output.
Malformed range timestamps and range targets that cannot be resolved to an existing note remain
source-located diagnostics while the import itself stays usable.
When rendered, ranged harmonies are also exposed through versioned SVG metadata with typed start
and end addresses, and through `LayoutResult.spans` as system-aware semantic extender lines, so
browser editors can select or update the range without parsing SVG.
Endpoint-only edits are available through the undoable JSON/WASM command contract, while the
existing full chord-symbol command remains backward compatible.
Core validation rejects range endpoints that do not resolve to an existing note, so invalid
references cannot silently reach layout or SVG publication.
WASM hosts can use the typed `ScoreEngine.set_harmony_range` method with a `NoteAddr` JSON value
and `null` to clear the endpoint, without constructing a full command object.
MusicXML export reports ranged-harmony loss explicitly because the standard harmony element has
no equivalent endpoint contract; MEI remains the loss-preserving export for this field.
MEI editorial and facsimile attributes such as `facs`, `resp`, `cert`, and `evidence` are
source-located with preserved values in import diagnostics; `facsimile`, `surface`, `zone`, and
`graphic` structure elements are diagnosed individually. They remain outside the canonical score
until a reference-preserving model is defined. MEI simple `fb`/`f` values map in order to `Measure.figured_bass` and typed display-level
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
Renderer-side tab validation rejects string numbers outside the owning staff line range and
metrics overflow before emission; it does not silently clamp malformed positions.
MusicXML note-level `instrument@id` is retained as `Note.instrument_id`; concrete percussion sound
catalog mapping remains partial. Core validation rejects deserialized `microtone_cents` values
outside -99..99. The SVG renderer preserves non-zero cents visibly as deterministic
`acorde-microtone` text markers (for example `+25c`); richer quarter-tone glyph equivalence
remain a later glyph-resource phase. Each marker also exposes exact cents and pitch index data
attributes for browser selection without geometry inference.
Unpitched notes preserve their display placement and expose an `acorde-unpitched` SVG hook plus
the preserved `data-acorde-instrument-id` when available, without inventing a percussion sound
identity; percussion clef mappings remain bounded and explicit.
Structured figured bass is projected into the deterministic measure-text SVG path, including a
bounded continuation-line hook for `extender`; duplicate importer display text is suppressed.
Supported mordent, inverted mordent, turn, inverted turn, shake, and tremolo articulations expose
semantic SVG classes with conservative width reservation. Font-specific ornament glyphs and final
publication typography remain host-resource responsibilities. Sloped beam, secondary-beam, and
clearance-shift extents are included in the renderer's vertical margin calculation. Multiple
note-attached articulations are stacked in deterministic source order with matching margin
reservation. Syllabic lyric hyphens are drawn between the actual adjacent note coordinates,
including across measure boundaries on one system, and carry stable source-address attributes for
interactive hosts; font shaping remains outside the core renderer.

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
  marks, figured bass, and generic text. Measure-level styled text is emitted through the
  deterministic SVG path; provider-specific font shaping and final publication placement remain
  render-host work.

## Versioning and evidence

The matrix applies to v1.2.3. Each `yes` slice must have a fixture or focused round-trip test in
the repository. The fixture provenance and evidence mode are pinned in
[`tests/fixtures/manifest.json`](../tests/fixtures/manifest.json); the evaluation rules are in
[`interchange-evidence.md`](interchange-evidence.md). Known losses are tracked here until `ImportReport` and `ExportReport` expose source
location, severity, preserved value, and loss reason through the native, CLI, and WASM APIs. The
WASM bindings expose report variants for MusicXML, MXL, MEI, MIDI, ABC, MSCZ, and MSCX imports,
plus MusicXML, MEI, MIDI, ABC, MSCX, and MSCZ exports. The
MXL reports extract the bounded inner MusicXML entry and reuse its source-located diagnostics,
so compression does not weaken the MusicXML loss boundary.
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
