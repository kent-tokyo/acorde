# Acorde scorecard

The machine-readable scorecard is [`scorecard.json`](scorecard.json). Its `release` field names
the current release, and `base_release` names the previous published version. It is a conservative
inventory of the current contracts, not a claim of parity with MuseScore, music21, OSMD, or
Verovio. Capability labels must remain aligned with the [notation coverage matrix](notation-coverage.md)
and backed by a fixture or focused test.

Current contracts are SVG metadata 22, analysis result 14, print layout 34, glyph-resource
metadata 1, and tablature performance 4. `scorecard.json`, its contract documentation, and their
regression tests must change together.

- Print layout is available through WASM and the browser adapter. `acorde-render-svg` renders
  physical page SVGs from validated layout breaks, and `acorde-pdf` joins them into vector PDF.
  Text-font supply, broader publication resources, and printer APIs remain host work.
- The browser adapter transactionally loads MusicXML, MXL, MEI, ABC, MIDI, MSCX, and MSCZ; it
  exports MusicXML, MEI, ABC, MIDI, and bounded MSCX/MSCZ. This is not a losslessness or parity claim.
- `acorde mcp` is a local stdio MCP adapter over the CLI paths: inspection tools, reviewed
  in-memory editing of ten `Command` types with undo/redo, and opt-in (`--allow-write`)
  export to a destination other than the source. It has no network transport and is not a
  hosted service.
- Analysis evidence includes source-positioned BCMH and When in Rome
  development comparisons with verified inputs. The fixed third 12-work
  When in Rome harmony holdout passes its aggregate Roman-at-annotation-position
  gate: 559 versus 508 correct labels for music21 10.5.2. This task-scoped
  result does not establish overall analysis superiority; other analysis tasks
  need independent holdouts. A local-key development comparison scores
  2,325/3,405 for Acorde and 2,157/3,405 for music21 at their respective
  development-selected window sizes, with a 12–10–2 per-work split; no
  paired local-key holdout has been run.
  A separate Mozart cadence corpus has 36 verified
  answer-free inputs, but its 18-movement development baseline exposes a large
  cadence recall gap: the opt-in resolution rule matches 30/333 with 29 false
  positives. The reserved cadence labels remain unread.
- `evidence.security_checks` mirrors local release gates. Browser E2E, raster fidelity, latency,
  publication, and downstream adoption need their own evidence.
