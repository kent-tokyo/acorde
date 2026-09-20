# Migration notes

This page summarizes compatibility-relevant changes from older releases. Detailed release history
is kept in [`CHANGELOG.md`](../CHANGELOG.md); current APIs and feature boundaries are documented
in the crate READMEs and the [notation coverage matrix](notation-coverage.md).

## v0.3–v0.5 — browser contract and hardening

The `Score → LayoutResult → SVG` pipeline and stable `part:staff:measure:voice:note` addresses
remain the browser integration boundary. Versioned render metadata, reviewed browser baselines,
accessibility text, and WASM size/resource checks were added. Hosts continue to own selection,
hover, playback, DOM insertion, and CSS sizing.

## v0.6–v0.7 — parser and validation correctness

MusicXML first-measure attributes are applied correctly. Structural validation now reports empty
scores, missing staves/measures, mismatched staff measure counts, and invalid time signatures.
Consumers that exhaustively match validation errors should handle the added variants.

## v0.8–v0.9 — portable score patches

WASM score patch/apply APIs were added. Patches can represent measure metadata, barlines, rehearsal
marks, volta brackets, and explicit note insertion positions. Unsafe or structurally broad changes
fall back to `ReplaceScore`, preserving target-score data instead of silently dropping it.

## Current release

### v1.2.0 — SoundFont materialized decoding

`SoundFontPresetZone` is now constructed through `SoundFontPresetZone::new` or
`SoundFontPresetZone::with_channel_layout`; direct Rust struct literals are no longer supported.
Use `with_channel_layout` when a provider has resolved mono, linked-stereo, or interleaved-stereo
source PCM. `ResolvedPresetZoneMetadata` serializes the added `channel_layout` and
`decode_channels` fields. JSON consumers must ignore unknown additive fields until they opt into
the new audio setup path.

For SF3, use `ResolvedPresetZoneMetadata::decode_sample_region` or
`decode_materialized_sample_region` for a materialized zone. These select the Ogg logical stream
by materialized sample ID and use stream-relative frame/loop coordinates. The older
`decode_sample_region` remains available for its legacy first-stream convenience contract.

For v1.1.3, use the typed `Command::SetMeasureText` command for undoable measure-level
`StyledText` editing. Check the render metadata `contract_version` before consuming newer fields;
v1.1.3 exposes position-aware `text_annotations` in SVG metadata. Contract version 3 adds
optional placement and source-coordinate offset fields; older consumers can ignore these fields.

For v1.1.5, `compatibility_report` is exposed through WASM and the browser
adapter. It compares canonical score JSON values and returns explicit score and deterministic
analysis gate booleans; it does not replace format-specific import/export diagnostics.

For v1.1.7, MSCX import reports malformed numeric fallbacks with stable,
source-located diagnostic codes. `Pitch::to_scientific_name()` preserves extended accidental
spellings and scientific-name parsing rejects accidental overflow. The fuzz lockfile is aligned
with the 1.1.7 workspace crates; consumers should use the report APIs and pinned lockfile when
building reproducible interchange checks.

The same candidate adds `Score.spanners`: typed, stable-ID ranges for slur, glissando, trill line,
pedal, and ottava. Missing `spanners` deserializes as an empty list. Existing note-level flags
(`slur_start`, `pedal_end`, and related fields) remain accepted as legacy input and continue to
produce legacy layout spans; new editing and MusicXML work should use typed spanners. A typed
endpoint takes precedence over a matching legacy endpoint during MusicXML serialization so a
consumer does not emit two copies of the same notation.

Structural commands keep typed endpoints attached to their original notes: insertion and
container edits rebase positional `NoteAddr` values, while deleting or wholesale-replacing an
endpoint removes the complete typed span when its original note cannot be resolved. This avoids
silently retargeting a span; legacy note-level flags and their serialized `NoteAddr` shape remain
compatible.
