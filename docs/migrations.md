# Migration notes

This page lists changes that can require consumer action. Release history is in
[`CHANGELOG.md`](../CHANGELOG.md); current format scope is in the
[coverage matrix](notation-coverage.md). Additive JSON fields are serde-defaulted unless stated
otherwise, so stored score JSON remains readable. Rust consumers that use exhaustive `match` or
struct literals must still update for added variants and fields.

> **Published source-compatibility exception:** 1.2.19 added the public
> `SvgRenderOptions::staff_gap` field. Complete external struct literals written for 1.2.18 no
> longer compile in 1.2.19 or 1.2.20, despite the patch version. Pin `=1.2.18` or migrate the
> literal before updating; do not assume a later 1.2 patch restores the old literal form.

## Unreleased: selectable shared dynamics

`PlaybackOptions.shared_dynamics` and `shared_hairpins` select individual marks to share across
a part's staves. Existing JSON defaults both lists to empty; complete Rust `PlaybackOptions`
struct literals must add them or use `..Default::default()`. Selection is a playback option,
not a persisted score or an automatic interpretation of imported placement.

## Unreleased: explicitly written accidentals

`AccidentalDisplay::Explicit` distinguishes an ordinary written accidental from an automatic or
cautionary one. Rust code that exhaustively matches this enum must handle the new variant. Older
score JSON remains readable; older consumers may not recognize newly serialized `Explicit` values.
ABC, MIDI, and MuseScore export reports identify the cases where this distinction cannot round-trip.

## Unreleased: hidden time signatures

`Measure.time_sig_hidden` preserves hidden MusicXML and MuseScore meter changes. Set it in
complete Rust `Measure` literals; existing score JSON defaults to `false`. Exhaustive matches on
`ScoreChange` and `ScorePatch` must also handle the new time-visibility variants. ABC, MEI and
MIDI export reports identify the unsupported visibility separately from the musical meter.

## v1.3.7

Re-import MusicXML to retain common/cut time symbols, written metronome marks,
positioned direction words, and part volume/pan. Regenerate print SVG/PDF,
MIDI, or WAV output to apply the placement and playback corrections.

The new `Measure.time_symbol` and `Measure.metronome_mark` fields and
`StyledText.position_beats` require values in complete Rust struct literals;
the new score change/patch variants require updates to exhaustive matches.
Older score JSON remains readable through the fields' defaults.
Preset and free-layer engine markers include the workspace
version; reselect saved inputs to create a v1.3.7 configuration before replay.
The new WAV peak target and ornament realization are opt-in.

## v1.3.6

The new `AccordionPattern` waltz variants require updates to exhaustive Rust matches.
Existing preset and free-layer configuration structs keep their field shapes. Their engine
markers include the workspace version, so select the saved chords and technique again to
create a v1.3.6 configuration before replaying under v1.3.6. The new drone API is separate:
pass an explicit MIDI bass pitch and omit `layers` when no chord-derived voice is needed.

Re-import ABC with closing bars or MSCZ with initial staff clefs to apply those parser and
export fixes. Regenerate SVG where inline meter changes or print directions affected layout.
The same-score alphaTab timing observation is fixture-specific, not a compatibility claim.

## v1.3.5

No Rust struct-literal change is required for the preset configuration types. New
`AccordionPresetConfig.input_fingerprint` values start with `v2:`; the existing string field
still carries the full replay identity. Re-parse the same source score, then pass the saved
configuration to `arrange_from_resolved_preset`. The new score's source note IDs are used in
the output. An older 16-digit fingerprint from v1.3.4 remains valid only with the exact
original Score and `1.3.4+arrangement-presets.1` engine marker. To migrate it, load that
original Score and select the saved preset and chords again to obtain a `v2:` configuration.

Re-import ABC sources that rely on implicit alternate-ending closure; previously parsed score
JSON does not gain that correction automatically. Regenerate accompaniment if an in-measure
chord change or redundant chord label affected the old pattern phase. Chord extraction and
melody-aware proposals are opt-in and bounded; inspect their diagnostics before arranging.

## v1.3.4

No intentional Rust source migration is required for the added print, comparison, and audio APIs.
Rebuild exhaustive matches as usual when adopting a newer version. The full-SF2 renderer is
opt-in and requires a caller-supplied licensed SF2 asset; unsupported or unrouted MIDI automation
now returns an error rather than being silently omitted. Regenerate page SVG/PDF and audio output
if your host relies on the corrected print placement or playback control schedule.

## v1.3.3

Rust code using a complete `Note` struct literal must add `tuplet_boundary: None` or an authored
`TupletBoundary` value. `Note::new` and `Note::rest` set the default automatically, and older score
JSON remains readable. Re-import ABC or MusicXML that contains adjacent same-ratio tuplet groups
to retain their written boundaries; a previously normalized score cannot recover them on its own.
Regenerate playback or PDF output if shortened bars, tied staccato, or page bounds affected it.

The new accordion reduction, arrangement, and seeded-preset APIs are opt-in. Hosts should inspect
their structured diagnostics and source mappings before using a generated score.

## v1.3.2

No Rust source migration is required. Re-import original MusicXML or MSCZ files to pick up the
newly retained import semantics; existing canonical score JSON is not rewritten automatically.
Playback schedules and exported MIDI should be regenerated from the corrected score and timing
logic. Browser hosts that need only MSCZ playback events can opt into the `playback-only` WASM
profile with `--no-default-features`; the default full profile is unchanged.

## v1.3.1

No Rust source migration is required for the new WASM viewer profile or MSCZ fixes. Re-import an
original MSCZ file if a previously imported canonical score has incorrect alternate endings,
voice slots, or initial clefs. The corrected parser does not rewrite stored scores automatically.
WASM hosts can opt into the `viewer` feature set; the existing full build remains the default.

## v1.3.0

`SvgRenderOptions` again has its original four public fields: `width`, `staff_size`,
`measures_per_system`, and `interactive`. If you need a non-default inter-staff gap, use:

```rust
let config = SvgRenderConfig::new(options).with_staff_gap(8.0);
let svg = render_svg_with_config(&score, &config)?;
```

Versions 1.2.19 and 1.2.20 added `staff_gap` directly to `SvgRenderOptions`; that was a
source-breaking change in patch releases and those versions are withdrawn from use.

## v1.2 series

| Release | Consumer action |
|---|---|
| 1.2.20 | No additional Rust-source migration. ABC key signatures and the MSCZ Open Container index extend format behaviour; retain the 1.2.19 SVG-options exception above. |
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

### Measured tremolo playback

`Note` now includes the serde-defaulted `tremolo_pair` field, so Rust callers constructing
`Note` with a complete struct literal must set it (usually `None`). MusicXML two-note tremolo
start/stop endpoints are retained here. In default playback, a measured single-note tremolo is
now expanded into attacks at its written stroke rate, including all chord pitches; hosts should
not expand its `Tremolo` articulation again. Two-note alternating playback is not implemented.
The MIDI serializer has its own timing path and does not yet expand these attacks.
