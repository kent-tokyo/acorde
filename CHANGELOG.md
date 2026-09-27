# Changelog

All notable released changes are summarized here. The repository tags and commit history retain
the full implementation record. Score JSON additions are backward-compatible unless marked
**[breaking]**; consumers should accept unknown additive fields and retain `#[serde(default)]`
compatibility.

The format follows [Keep a Changelog](https://keepachangelog.com/en/1.1.0/).

## [Unreleased]

- **Guitar Pro 3/4/5 import.** `acorde_io::parse_gp` now also reads `.gp3`, `.gp4` and `.gp5`
  binary files (detected from their bytes; the CLI accepts the extensions). They share the GPIF
  path's mapping and `gp.*` diagnostics: tuning and capo, string/fret, bends, hammer/pull,
  slides, dead notes, harmonics, grace notes, palm mute/let ring, tremolo picking, strokes,
  lyrics, markers, repeats and endings, tempo changes, and directions. All 90 GP3/4/5 files in
  alphaTab's test data import and export to schema-valid MusicXML and to MEI that Verovio loads.
  Accent and ghost velocities that GP5 stores as note dynamics are not read as dynamic marks.
- Guitar Pro coda/segno/fine directions import as navigation marks (GPIF and GP5).
- **Per-pitch chord ties.** `Note::pitch_tie_starts`/`pitch_tie_ends` (additive, omitted from
  JSON when empty) record a tie on only some notes of a chord. MusicXML, MEI and Guitar Pro
  import keep them, MusicXML and MEI export write each note's own tie, playback sustains only
  the tied pitches, and the SVG renderer draws one tie per tied notehead (upper notes bow up,
  lower notes down). The 1.2.8 `*.partial-chord-tie` diagnostics are no longer needed.

## [1.2.8] - 2026-09-27

- **Guitar Pro import.** New `gp` feature (`acorde_io::parse_gp`, `parse_gp_with_report`; enabled
  in the CLI and WASM builds, umbrella feature `acorde/gp`). It reads Guitar Pro 7/8 `.gp` and
  Guitar Pro 6 `.gpx` files: tracks become parts, staves keep tuning and capo, and notes keep
  string/fret positions, rhythm, ties, dynamics, lyrics, repeats, endings, sections, and tempo,
  plus bends (with curves), hammer-ons/pull-offs, slides, dead notes, palm mute/let ring,
  harmonics, accents, left-hand fingering, tremolo picking, pick strokes, and brushes (as
  arpeggios). Other content is reported as `gp.*` diagnostics
  with counts. All 133 GP6/7/8 files in alphaTab's test data import, validate, and export to
  schema-valid MusicXML and to MEI that Verovio loads. GP3/4/5 binary files are not supported.
- SVG tablature staves now draw a TAB clef and no key signature. Fret numbers mask the string
  line behind them, and dead notes are written `X`.
  Tab staves no longer draw beams placed by pitch, and a technique text such as "let ring"
  that continues from note to note is written once per run within a bar.
- SVG whole and half rests now use SMuFL proportions (about 1.1 × 0.5 spaces), the whole rest
  hangs from the fourth line instead of the top line, and a measure rest is centred in its bar.
  Rests on tablature staves sit in the middle space. Guitar Pro bars holding only a placeholder
  rest import as measure rests.
- Rendering no longer fails with "minimum measure widths exceed the available system width"
  because of f32 rounding, or when a very short bar sits next to long ones.
- MusicXML cue notes now keep their written time at the I/O boundary. Import advances the cursor
  past them, and export writes their duration followed by a `<backup>`. All 268 music21 corpus
  exports validate against the MusicXML XSD.
- MusicXML `<slide>` is written in `<notations>` and read from either place.
- A tie on only some notes of a chord is now reported (`musicxml.partial-chord-tie`,
  `gp.partial-chord-tie`). The model ties whole chords, so such a tie is still applied to every
  note of the chord; 26 of 205 music21 corpus files contain one.
- MEI tablature course numbers now follow acorde's string order (string 1 = lowest), so tab
  exported to Verovio is no longer upside down.
- GM instrument ranges widened to professional ranges (string quartet high positions, drop-D and
  7-string guitar, 5-string bass, trombone pedal tones), so standard repertoire no longer fails
  validation with `OutOfRange`.

## [1.2.7] - 2026-09-27

- MEI export now writes every part. It previously wrote only the first part and dropped the
  rest without a diagnostic. Parts become labelled `<staffDef>`s or labelled `<staffGrp>`s
  (the form Verovio and MuseScore use), part groups become enclosing `<staffGrp>`s, and
  `<instrDef>` carries the MIDI channel and program. MEI import splits parts when the first
  `<scoreDef>` names instruments; unlabelled layouts still import as one part.
- MEI chords are exported as `<chord>`; before, only the first pitch was written. `<chord>` is
  imported too; it used to be rejected. Lyrics are written as standard `<verse n><syl wordpos>`
  note content for every verse. Articulations are written as `@artic`. Fermatas, trills,
  mordents, turns, breaths, caesuras, dynamics, hairpins, slurs, ties, pedals, and ottavas are
  measure-level control events with `@startid`/`@endid`, and their spans may cross barlines.
  Hairpins were previously dropped without a diagnostic.
- MEI now covers mid-piece meter, key, and clef changes, system and page breaks, beams, grouped
  tuplets, cross-staff notes (`@staff`), barlines on `measure@left`/`@right`, measure rests in
  layers, and typed measure texts (`<dir type="acorde-…">`). Accidentals are written visibly only
  where the key signature and earlier accidentals in the measure do not already imply them
  (`@accid` vs `@accid.ges`), and the key uses MEI 5 `@keysig`. With no explicit beams, the
  default beat beaming is written.
- MEI import reads what Verovio writes: `@accid.ges`, `<accid>` children, `<clef>`/`<keySig>`/
  `<meterSig>` inside `scoreDef`/`staffDef`, `<tie>`, `<space>`, `<mRest>` (as a lone whole rest),
  `@tstamp`-anchored control events, and tuplets recovered from `@dur.ppq`. Two bugs that dropped
  text are fixed: an empty `<title/>` made the importer swallow every later lyric and dynamic,
  and entity references such as `&amp;` were removed from all text.
- The OpenScore fixtures now load in Verovio 6.3 with an empty toolkit log, and Verovio's own
  re-encoding imports back with the same notes, chords, verses, marks, and spans.
- MEI tablature: `<tuning>`/`<course>` become the staff's `TablatureConfig`, `<tabGrp>` notes
  with only `@tab.course`/`@tab.fret` take their pitch from the tuning, and tablature staves
  export as `notationtype="tab.guitar"` with `<tabGrp>`. Tablature MEI previously failed to
  import. A single named part keeps its name as a `<label>`.
- MusicXML export now validates against the official MusicXML 4.0 XSD; before, every exported
  fixture was invalid, and the Lieder fixture had 713 errors. `<note>` children are written in
  schema order. Chord members repeat the chord's dots, ties, and tuplet ratio, and their tab
  positions go inside `<notations>`. Beams are written, either explicit or the default beat
  grouping, and tuplets get `<tuplet>` bracket notations. Part ids that are not XML names are
  written as `P-<id>`, and text positions move from `<direction>` to `<words>`/`<rehearsal>`.
- MusicXML import reads `<beam>` groups into `BeamState`. Before, explicit beaming was replaced
  by default beat grouping.
- MusicXML import now handles real-world files that previously failed. On a 268-file sample of
  the music21 corpus, imports went from 153 to 268. The fixes: DOCTYPEs with any external
  identifier are accepted (internal subsets and entities are still refused). UTF-16 and
  BOM-prefixed files are decoded through the new `acorde_io::decode_xml_text`, which the CLI
  and the MXL reader use. Orphan spanner stops (`musicxml.orphan-spanner-stop`), backups past
  the measure start (`musicxml.backup-underflow`), overfull bars (imported as an irregular
  `actual_length`), and chord members on another staff (`musicxml.chord-staff-mismatch`) are
  reported instead of aborting the import. A chord member whose `<voice>` differs from its
  chord joins that chord. `<midi-unpitched>` is read from `<midi-instrument>`.
- MusicXML export fixes found with the XSD on that corpus: `<ending>` comes before `<repeat>`,
  `<wavy-line>` is inside `<ornaments>`, breath marks and caesuras are inside `<articulations>`,
  `<degree>` includes all its required children, harmony kinds are mapped onto the schema
  enumeration, and `<score-instrument>`/`<midi-instrument>` are well formed. 266 of the 268
  exports now validate; the remaining two contain cue notes. MEI export gives unique ids when
  measure numbers repeat, adds the layer Verovio needs for cross-staff notes, and writes the
  percussion clef as `perc`. All 268 MEI exports load in Verovio, and no warning traces to the
  exporter apart from ties the source itself leaves unterminated.
- MuseScore import now sets each staff's starting clef from `Part/Staff/defaultClef` and
  `Instrument/clef`. Before, bass-clef instruments and piano left hands imported in treble. A
  clef change in a later measure also no longer replaces the starting clef.

## [1.2.6] - 2026-09-27

- MuseScore (MSCX/MSCZ) import now keeps voice-level hairpins, pedal lines, and 3.x slurs, slur
  ends, `<LayoutBreak>` system/page/section breaks, and `<BarLine>` double/final/dashed/dotted
  subtypes, which were previously dropped without a diagnostic. Hidden (`visible=0`) hairpins,
  pedals, and dynamics import as visible marks and report `mscx.unsupported-visibility`; unknown
  barline subtypes report `mscx.unsupported-barline`. MSCX export now writes layout breaks and
  those barline subtypes.
- A voice holding one plain whole rest is a measure rest that fills its bar in any time signature
  (`voice_duration_beats`, `Note::is_plain_whole_rest`). MusicXML `<rest measure="yes"/>` and
  MuseScore `durationType=measure` rests in 3/4, 6/8, and other meters no longer fail validation,
  and MSCX export writes them as `measure` rests of the bar's length.
- Added `Articulation::{UpBow, DownBow, Harmonic, OpenString, Stopped, SnapPizzicato}` with
  MusicXML `<technical>`, MSCX, MEI, ABC, and SVG support (ABC reports the harmonic as a loss).
- **[schema]** Added optional `Measure.measure_repeat` for one-measure repeats. The measure stores a
  playable copy of the repeated measure, so playback and validation need no special handling; SVG
  draws the repeat sign instead of the copy. MusicXML `<measure-repeat>` start/stop and MuseScore
  `RepeatMeasure`/`MeasureRepeat`/`measureRepeatCount` import and export. Validation reports
  `InvalidMeasureRepeat` and warns with `MeasureRepeatContentDiffers` when the copy no longer
  matches its source. Multi-measure repeats import as written notes and are diagnosed.
- MusicXML beat repeats and slash styles, lyric extend/elision, dashes, brackets, non-arpeggiate,
  and harmony frames are now source-diagnosed instead of silently dropped.

## [1.2.5] - 2026-09-27

- **[schema]** Added optional `Measure.actual_length` (`MeasureLength`, a fraction of a whole note)
  for pickups, incomplete final bars, and other irregular measures. Validation, playback timing,
  metronome clicks, editing capacity, time-signature changes, logical layout, and SVG spacing use
  it instead of the time signature. MusicXML import keeps a measure whose content ends before the
  time signature at its authored length instead of padding it with rests; export writes the
  shorter content and marks a shortened first measure `implicit="yes"`. MSCX/MSCZ import and
  export map it to MuseScore's `<Measure len="n/d">`. Invalid lengths are reported as
  `ValidationError::InvalidMeasureLength`.
- **[schema]** Added optional `Note.additional_lyrics` (`VerseLyric`) for lyric verses 2–32;
  verse 1 remains `Note.lyric`. MusicXML `<lyric number>` and MuseScore `<Lyrics><no>` import and
  export every verse instead of letting a later verse overwrite verse 1; unnumbered or duplicate
  numbers take the lowest free verse. `SetLyric` gains an optional `verse` (legacy JSON still
  targets verse 1), SVG draws each verse on its own line, and ABC, MEI, and MIDI export report
  unwritten verses as `*.export-unsupported-lyric-verse` losses. Invalid or duplicate verse
  numbers are reported as `ValidationError::InvalidLyricVerse`.

## [1.2.4] - 2026-09-27

- MusicXML `<staves>` now materializes every declared staff, including staves without a numbered
  clef or notes, and export writes it back, so staff count and note staff ownership survive a
  round-trip (#81). A note that continues its voice on another declared staff imports as a
  cross-staff placement owned by the voice's staff (voices 1–4 on staff 1, 5–8 on staff 2, …);
  export numbers voices on additional staves the same way when no source number is retained.
  Note staff references beyond a `<staves>` declaration are source-diagnosed as
  `musicxml.undeclared-staff-reference`.
- MusicXML `<staff-details><capo>` now imports and exports for tablature staves and time-local
  tablature changes; the former `musicxml.export-unsupported-capo` loss is removed.
- Added undoable `RespellStaffRegion` (Respell Pitches for a selection) with flat, sharp, and
  local-key policies. Tie chains crossing the range boundary keep one spelling, and unpitched notes
  keep their staff position. WASM `ScoreEngine.respell_staff_region` exposes it.
- Added undoable `CycleEnharmonicSpelling` (MuseScore's "Change enharmonic spelling") for a note
  or one chord member, cycling natural, sharp, flat, double sharp, and double flat spellings of
  the same sounding pitch while keeping microtones.
- Added undoable `ResequenceRehearsalMarks` (letters, numbers, or measure numbers continuing the
  first mark), `SetSystemBreakInterval` (a line break every N measures, or none), and
  `RemoveTrailingEmptyMeasures` (keeps rests with marks and moves a final barline).

## [1.2.3] - 2026-09-23

- Consolidated English/Japanese entry points, print guidance, release notes, and evidence links.
- Added Phase 18 external-observation and Phase 19 logical print-corpus documentation. These are
  evidence and contract improvements, not a new compatibility or publication-quality claim.

## [1.2.2] - 2026-09-22

- Accepted the conventional, non-loading MusicXML public DTD declaration emitted by MuseScore;
  internal subsets, `ENTITY`, and `SYSTEM` remain rejected.
- Corrected MusicXML tablature string-number conversion at the parser/serializer boundary.
- Added bounded checksum-pinned interoperability observations and separated annotation, span,
  tablature, and command-remapping internals without changing the public command API.
- Updated workspace versions, scorecard, examples, browser metadata pin, and the 5 MiB WASM gate.

## [1.2.1] - 2026-09-22

- Unified deterministic collision lanes for legacy spans, tablature connections, and
  lyrics/dynamics/chords/articulations; measure text treats resolved annotations as obstacles.

## [1.2.0] - 2026-09-21

- Added channel-aware SoundFont materialization and SF3 logical-stream decoding.
- **[breaking]** `SoundFontPresetZone` must be built with `new` or `with_channel_layout`.
  Materialized-zone JSON gained additive `channel_layout` and `decode_channels` fields.

## [1.1.7] - 2026-09-20

- Added backward-compatible typed `Score.spanners` for numbered slur, glissando, trill, pedal,
  and ottava ranges, with editing, layout, and SVG identity support.
- Preserved MusicXML source voice numbers and cursor gaps; added safe MSCX numeric-fallback
  diagnostics and bounded SoundFont zone inheritance.

## Earlier releases

Versions 0.1.0–1.1.6 established the core score model, commands, MusicXML/MIDI/ABC/MEI/MSCX
boundaries, deterministic SVG and browser contracts, analysis, playback, security hardening,
print metadata, and release infrastructure. Use the matching tag, [migration notes](docs/migrations.md),
and focused contract documents when upgrading from those versions.
