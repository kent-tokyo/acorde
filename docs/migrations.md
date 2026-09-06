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

For v1.1.3, use the typed `Command::SetMeasureText` command for undoable measure-level
`StyledText` editing. Check the render metadata `contract_version` before consuming newer fields;
v1.1.3 exposes position-aware `text_annotations` in SVG metadata. Contract version 3 adds
optional placement and source-coordinate offset fields; older consumers can ignore these fields.
