# Migration notes

This page lists changes that can require consumer action. Release history is in
[`CHANGELOG.md`](../CHANGELOG.md); current format scope is in the
[coverage matrix](notation-coverage.md). Additive JSON fields are serde-defaulted unless stated
otherwise, so stored score JSON remains readable. Rust consumers that use exhaustive `match` or
struct literals must still update for added variants and fields.

## v1.2 series

| Release | Consumer action |
|---|---|
| Unreleased | `Clef::AbcMiddle` represents ABC 2.1 `middle=` visual placement. Handle the added enum variant in exhaustive matches; it changes staff display only and leaves written/playback pitch intact. |
| 1.2.19 | `SvgRenderOptions` gains `staff_gap`; direct struct literals must set a finite positive value or use `..Default::default()`. It is measured in staff spaces and defaults to 10. |
| 1.2.18 | `acorde-omr` is an optional review-first adapter; use its report/command-plan boundary rather than treating image analysis as automatic conversion. `OmrRhythmicEvent` gains serde-defaulted `is_unpitched`, `instrument_id`, and `tab_positions`; `RecognitionConfig.time_signature` defaults to 4/4. Browser transport and `acorde-pdf` remain host-facing additive crates. |
| 1.2.17 | Add `VoltaBracket.numbers` to literals; handle `Duration::{Breve,HundredTwentyEighth}` and new `Articulation` values. `Note.pitch_fingerings` is parallel to chord pitches. |
| 1.2.16 | Handle octave and C-family `Clef` values. `Note.pitch_accidentals` and `pitch_parentheses` are parallel per-pitch fields. |
| 1.2.15 | Handle `Command::{UnrollRepeats,RealizeChordSymbols}`. MIDI import now rebuilds meter-based measures, voices, and barline ties; out-of-range pitches are warnings. |
| 1.2.14 | `Note.hidden` retains timing/playback but hides SVG. Use `Staff::meter_at` or `measure_beats` for in-score meter changes. |
| 1.2.13 | MSCX output always encloses voices in `<voice>`; raw-XML consumers must accept canonical output. MEI/MSCX support broadened but remains subset based. |
| 1.2.12 | Handle added `Dynamic` and `NotationSpannerKind` variants. `Lyric.extend` retains melisma extenders; 8va/8vb direction follows MusicXML semantics. |
| 1.2.11 | `Measure.mid_clefs` preserves in-measure clef changes. Struct literals, split/join code, patches, and serializers must retain it. |
| 1.2.10–1.2.8 | Per-pitch ties, Guitar Pro import, and revised SVG geometry can change serialized output or golden SVGs; update snapshots rather than string-matching old geometry. |
| 1.2.7–1.2.5 | `Measure.actual_length`, `measure_repeat`, and `Note.additional_lyrics` model pickups, repeats, and lyric verses. Use constructors or defaults instead of incomplete literals. |

## Earlier compatibility boundaries

- **v1.1.7 and earlier:** Use `Command::SetMeasureText` for undoable measure text. SVG metadata
  consumers must check its contract version before reading newer fields.
- **Typed spanners:** `Score.spanners` is additive. Typed endpoints take precedence over matching
  legacy note flags during MusicXML serialization; structural edits remove a span if an endpoint
  disappears instead of retargeting it.
- **v1.2.0 SoundFont:** construct `SoundFontPresetZone` through its constructors. SF3 consumers
  should use materialized-zone decode methods; the legacy first-stream helper remains only for
  compatibility.

## Upgrade checklist

1. Update all Acorde crates together to the same version.
2. Rebuild code with exhaustive enum matches and model struct literals.
3. Rebaseline raw XML, MEI, MSCX, SVG, or browser metadata snapshots only after inspecting the
   semantic change.
4. Re-import original source files when a corrected parser interpretation changes their canonical
   score (for example hidden notation, clefs, or 8va/8vb direction).
5. Run the focused format/browser/print checks for the surfaces your host uses.
