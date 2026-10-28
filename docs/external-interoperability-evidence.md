# External interoperability evidence

This is a checksum-pinned observation log, not a compatibility, engraving-quality, or
playback-quality score. SVG element counts are structural diagnostics only; they are not visual
similarity measurements.

## Record a comparison

[`benchmarks/interoperability/phase18a.json`](../benchmarks/interoperability/phase18a.json)
defines the checked-in corpus and required comparison fields. Run
`python3 -B scripts/validate_interoperability_manifest.py` to verify its fixture provenance and
SHA-256 values. After an external tool creates an artifact, record its exact output bytes and
Acorde's field-level compatibility result with:

```bash
python3 scripts/record_interoperability_comparison.py \
  tests/fixtures/openscore_omr_score_1003.mscz external.musicxml \
  --acorde acorde --tool-name 'MuseScore Studio' --tool-version 4.7.4 \
  --options-json '{"output_format":"MusicXML"}' \
  --output comparison.json
python3 scripts/validate_external_evidence.py comparison.json
```

The record embeds `acorde compatibility-report`'s typed `changes`, source/candidate diagnostics,
and the separate semantic, analysis, and loss booleans. A process that writes an artifact but exits
nonzero uses `execution_status: "output-produced-with-nonzero-exit"`; it is diagnostic evidence,
not a passing external gate.

## Verovio / MEI / SVG

On 2026-09-22, Verovio 6.3.0 rendered the registered self-authored MEI fixture
`interchange_subset.mei` (SHA-256 `992c79629f7f44af59a43f7169c5829b96d0a3f6802a07f81dec371651d1defe`).
`scripts/record_svg_interoperability_observation.py` records the artifact and
`scripts/validate_external_evidence.py` validates its envelope.

The 25,231-byte SVG has SHA-256
`35558eff1e2f4ff7bcc5234819a173533400d781027ac89d18991e395659d274`. Verovio warned that
`accid="qs"` was unsupported and its tie could not be matched; extracted SVG text omits `Cmaj7`
and `D.C. al Fine`.

This is a structural producer observation only. Element counts and missing extracted text do not
measure visual engraving, semantic equivalence, or MEI compatibility.

## MIDI event projection

Mido 1.3.3 parsed the registered public-domain `just_perfect_fifth_on_c.mid` fixture (SHA-256
`ff60a251a10c7886d4342a6d37fbe0c85ebdd24571d013cbcddfb37ce4399505`). Its selected projection
matched Acorde for pitch bends, controls, programs, aftertouch, five sounding notes, initial
90 BPM, and initial 4/4. Later tempo events and all scheduling or audio behaviour remain outside
this observation.

`scripts/compare_midi_events.py` records the two projections and their booleans. It does not
evaluate scheduling, synthesis, audio output, timing rewrites, or complete MIDI interchange.

## Evidence inventory

`scripts/summarize_interoperability_evidence.py` accepts one or more individually validated
reports and writes their report SHA-256 values, tool identity, fixture identity, and
kind-specific outcomes. It deliberately produces no compatibility percentage, no aggregate
equivalence result, and no cross-format parity claim.

## MuseScore / MusicXML

MuseScore Studio 4.7.4 produced MusicXML with the conventional public DTD (output SHA-256
`6b421d5d69ba23d536e844d19098f24f98fa5ed0a622ec875e79ddeecb412043`), which Acorde accepts
without loading. Internal subsets, `ENTITY`, and `SYSTEM` declarations remain rejected. The
contradictory `interchange_subset.mscx` pitch/TPC fixture is excluded; a headless mutex failure on
`external_tab.musicxml` is host-runner evidence only. A valid MuseScore round-trip remains a
separate gate.

### CC0 MSCZ observation

On 2026-09-22, MuseScore Studio 4.7.4 was invoked with factory settings:

```bash
mscore -F -o <output.musicxml> tests/fixtures/openscore_omr_score_1003.mscz
```

It exited 0 and produced a 21,455-byte MusicXML artifact (SHA-256
`6352065fc821d3dd0587fd743406f536230e1f16da1cb8a74f16fc4ae51b7570`) from the pinned CC0 MSCZ
fixture (SHA-256 `77ec1090af66b21a726e29d9e36e119d4a4ef112d25a4f758b5d00e8bd865711`). The direct command
without `-F` produced the same artifact but then terminated with `libc++abi` `mutex lock failed`;
the alternate native wrapper exited 137 without output. Those remain host-runner diagnostics and
are not used as the successful comparison execution.

The comparison reported 40 semantic changes, analysis disagreement in `Motifs` and
`PhraseBoundaries`, and 65 candidate import-loss diagnostics: 55 unsupported placement
attributes and 10 unsupported render attributes.

This completes the bounded 18B execution gate: a successful external process, exact source/output
bytes, options, typed changes, and typed losses are recorded. The semantic projection is not
equivalent and the artifact is not committed as a fixture, so it does **not** establish lossless
interchange, engraving, playback, or complete MuseScore compatibility. It instead names the next
model/I/O losses through a reproducible real-data observation.

## alphaTab / tablature

The authored `external_tab.musicxml` fixture (SHA-256
`3cce8d2f100e3b68a3430c4ccaa0ad737318179ba74ddf47bd31bc41b3ff37df`) covers a low-E third-fret
G2. alphaTab 1.8.4 / Node 24.5.0 parsed Acorde's MusicXML export (SHA-256
`2587a92b88188ee7cc84af69a4479960d4d0ad1798fdef97565c2c50dceadea8`) with the expected
high-to-low external string order, string 1, fret 3, and `realValue` 43. This is one
structural/playback-pitch projection, not browser, scheduler, SoundFont, or visual evidence.

## Related analysis evidence

The separate [music21 observation](external-analysis-evidence.md) records a bounded MusicXML
analysis projection. It does not imply agreement with any engraving or interchange result above.
