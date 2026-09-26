# Changelog

All notable released changes are summarized here. The repository tags and commit history retain
the full implementation record. Score JSON additions are backward-compatible unless marked
**[breaking]**; consumers should accept unknown additive fields and retain `#[serde(default)]`
compatibility.

The format follows [Keep a Changelog](https://keepachangelog.com/en/1.1.0/).

## [Unreleased]

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
