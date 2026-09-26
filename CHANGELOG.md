# Changelog

All notable released changes are summarized here. The repository tags and commit history retain
the full implementation record. Score JSON additions are backward-compatible unless marked
**[breaking]**; consumers should accept unknown additive fields and retain `#[serde(default)]`
compatibility.

The format follows [Keep a Changelog](https://keepachangelog.com/en/1.1.0/).

## [Unreleased]

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
