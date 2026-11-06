# Acorde roadmap

This roadmap tracks work still needing evidence or implementation. The
[scorecard](docs/scorecard.md) inventories current contracts, the
[notation coverage matrix](docs/notation-coverage.md) describes tested format support, and the
[changelog](CHANGELOG.md) records releases. Issue acceptance criteria remain authoritative.

## Current baseline

Version 1.3.10 includes a Rust score model with undoable commands, bounded notation I/O,
SVG and print-page rendering, playback scheduling, analysis primitives, WASM bindings,
and the file-based CLI. MusicXML/MXL is the broadest interchange path; Humdrum `**kern` and
Plaine & Easie import are opt-in subsets. Import and export reports identify known losses,
but broad real-score round-trip compatibility remains an evidence question.

The CLI also provides `acorde mcp --root <dir>` over local stdio. Its inspection and
validation tools are read-only by default. Reviewed editing uses sessions, a validated
preview, undo/redo, and an explicit destination; file writing requires `--allow-write`.
The score-model and WASM library crates remain independent of the MCP transport and host
file access. A hosted HTTP MCP service is not part of this release.

## Next evidence gates

1. **Printed scores:** Complete blinded human review of the fixed PDF corpus against
   Verovio 6.3.0 ([#390](https://github.com/kent-tokyo/acorde/issues/390)). Record readability,
   missing marks, collisions, and page use separately. The automated comparison and
   same-input corpus are in [print evidence](docs/archive/print-corpus-verovio.md).
2. **Audio:** Complete blinded listening on the fixed Acorde/alphaTab corpus
   ([#388](https://github.com/kent-tokyo/acorde/issues/388)). Keep audible quality,
   scheduled and measured timing, input losses, and sample-library effects separate.
   See [audio evidence](docs/archive/playback-audio-corpus.md).
3. **Interchange and arrangement:** Expand licensed real-score fixtures and external
   interoperability checks before claiming broad format equivalence or practical
   standard-bass playability. Preserve source-pitch omissions, harmony provenance,
   physical button actions, and performer-review status in each result.

## MCP follow-up

Keep the local tool schemas and diagnostics aligned with the underlying Acorde APIs.
Test tool discovery and calls against a pinned score whenever the CLI or model changes.
Add remote transport only with an explicit host use case, authentication, per-request
authorization, and resource limits.

## Maintenance

Update this page when an issue's acceptance criteria are verified. Record implemented
behavior in the scorecard and coverage matrix, release changes in the changelog,
and fixed-corpus measurements in their evidence documents. A result on one corpus does
not establish general compatibility, engraving parity, or audio superiority.
