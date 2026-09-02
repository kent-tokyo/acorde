# Acorde scorecard

The machine-readable scorecard is [`scorecard.json`](scorecard.json). It is a conservative
inventory of the current contracts, not a claim of parity with MuseScore, music21, OSMD, or
Verovio. Capability labels must remain aligned with the [notation coverage matrix](notation-coverage.md)
and backed by a fixture or focused test.

The `evidence.security_checks` commands are local release gates. Browser E2E, cross-browser
raster fidelity, and host-measured latency require their respective environments and are not
claimed by native Rust test results alone.
