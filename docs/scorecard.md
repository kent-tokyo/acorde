# Acorde scorecard

The machine-readable scorecard is [`scorecard.json`](scorecard.json). It is a conservative
inventory of the current contracts, not a claim of parity with MuseScore, music21, OSMD, or
Verovio. Capability labels must remain aligned with the [notation coverage matrix](notation-coverage.md)
and backed by a fixture or focused test.

The current serialized contracts are SVG render metadata version 22, analysis result version 13,
print layout version 32, glyph-resource metadata version 1, and tablature performance version 4;
all values are recorded in
`scorecard.json` and must be updated together with their contract documentation and regression
tests. The physical print layout is also exposed through WASM and the browser adapter; this does
not include PDF, font embedding, or printer APIs.

The SVG tab contract includes deterministic bend curves and edge-owned continuation segments for
slide, hammer-on, and pull-off connections, while typed connection metadata remains available to
browser playback and editing hosts.

The framework-neutral browser adapter now covers transactional loading for MusicXML, MXL, MEI,
ABC, MIDI, MSCX, and MSCZ, plus MusicXML/MEI/ABC/MIDI and bounded MSCX/MSCZ export. This is an adapter/API capability,
not evidence of global format losslessness or external application parity.

The `evidence.security_checks` commands are local release gates and now match the CI candidate
workflow, including formatting, all-feature/all-target validation, and workspace packaging.
Browser E2E, cross-browser raster fidelity, and host-measured latency require their respective
environments and are not claimed by native Rust test results alone.

Release v1.2.2 is published on 2026-09-22; `external_publication_verified` records that release
state only, not ecosystem compatibility or downstream adoption.
