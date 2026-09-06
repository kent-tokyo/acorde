# Acorde scorecard

The machine-readable scorecard is [`scorecard.json`](scorecard.json). It is a conservative
inventory of the current contracts, not a claim of parity with MuseScore, music21, OSMD, or
Verovio. Capability labels must remain aligned with the [notation coverage matrix](notation-coverage.md)
and backed by a fixture or focused test.

The current serialized contracts are SVG render metadata version 3 and print layout version 24;
both values are recorded in `scorecard.json` and must be updated together with their contract
documentation and regression tests.

The `evidence.security_checks` commands are local release gates. Browser E2E, cross-browser
raster fidelity, and host-measured latency require their respective environments and are not
claimed by native Rust test results alone.
