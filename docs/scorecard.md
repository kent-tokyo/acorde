# Acorde scorecard

The machine-readable scorecard is [`scorecard.json`](scorecard.json). It is a conservative
inventory of the current contracts, not a claim of parity with MuseScore, music21, OSMD, or
Verovio. Capability labels must remain aligned with the [notation coverage matrix](notation-coverage.md)
and backed by a fixture or focused test.

Current contracts are SVG metadata 22, analysis result 13, print layout 32, glyph-resource
metadata 1, and tablature performance 4. `scorecard.json`, its contract documentation, and their
regression tests must change together.

- Print layout is available through WASM and the browser adapter. `acorde-pdf` converts one Acorde
  SVG viewport to vector PDF; page composition, font assurance, and printer APIs remain host work.
- The browser adapter transactionally loads MusicXML, MXL, MEI, ABC, MIDI, MSCX, and MSCZ; it
  exports MusicXML, MEI, ABC, MIDI, and bounded MSCX/MSCZ. This is not a losslessness or parity claim.
- `evidence.security_checks` mirrors local release gates. Browser E2E, raster fidelity, latency,
  publication, and downstream adoption need their own evidence.
