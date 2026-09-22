# External interoperability evidence

This is a checksum-pinned Phase 18 observation log, not a compatibility, engraving-quality, or
playback-quality score. SVG element counts are structural diagnostics only; they are not visual
similarity measurements.

## Phase 18 comparison contract

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

## Phase 18C — Verovio / MEI / SVG

On 2026-09-22, Verovio 6.3.0 ran `verovio -o <output.svg>
tests/fixtures/interchange_subset.mei` against the registered self-authored MEI fixture (SHA-256
`992c79629f7f44af59a43f7169c5829b96d0a3f6802a07f81dec371651d1defe`). The result is recorded by
`scripts/record_svg_interoperability_observation.py` and validated with
`scripts/validate_external_evidence.py`.

The 25,231-byte SVG has SHA-256
`35558eff1e2f4ff7bcc5234819a173533400d781027ac89d18991e395659d274`; it contains two `svg`, 36
`g`, 22 `path`, two `text`, and six `use` elements. Verovio warned that `accid="qs"` was
unsupported and its tie could not be matched. The rehearsal mark `A` is present in a `tspan`, but
the fixture strings `Cmaj7` and `D.C. al Fine` are absent from the extracted SVG text.

This is a structural producer observation only. Element counts and missing extracted text do not
measure visual engraving, semantic equivalence, or MEI compatibility.

## Phase 18C — MIDI event projection

Mido 1.3.3 parsed the registered public-domain `just_perfect_fifth_on_c.mid` fixture (SHA-256
`ff60a251a10c7886d4342a6d37fbe0c85ebdd24571d013cbcddfb37ce4399505`). Its selected projection
matched Acorde exactly for pitch bends, control changes, program changes, and aftertouch; both saw
five sounding notes and zero percussion notes. The initial tempo was 90 BPM and the initial meter
was 4/4 on both sides. Mido additionally records later tempo events (60 BPM at tick 1536 and 90
BPM at tick 2047); Phase 18C only asserts the initial-score mapping, not those later changes.

`scripts/compare_midi_events.py` records the two projections and their booleans. It does not
evaluate scheduling, synthesis, audio output, timing rewrites, or complete MIDI interchange.

## Phase 18D — no-score evidence inventory

`scripts/summarize_interoperability_evidence.py` accepts one or more individually validated
Phase 18 reports and writes their report SHA-256 values, tool identity, fixture identity, and
kind-specific outcomes. It deliberately produces no compatibility percentage, no aggregate
equivalence result, and no cross-format parity claim.

## MuseScore / MusicXML

MuseScore Studio 4.7.4 was present locally. Its MSCX-to-MusicXML output (SHA-256
`6b421d5d69ba23d536e844d19098f24f98fa5ed0a622ec875e79ddeecb412043`) declares the standard
MusicXML 4.0 public DTD. Acorde now accepts that one conventional public declaration without
loading it, while still rejecting internal subsets, `ENTITY`, and `SYSTEM` declarations.

The existing `interchange_subset.mscx` fixture has contradictory `<pitch>` and `<tpc>` values;
MuseScore exported it as a measure rest. It is therefore excluded from semantic comparison.
Repeated headless conversion of the valid `external_tab.musicxml` fixture terminated inside the
local MuseScore process with a mutex error before writing an artifact. This is a host-runner
failure, not evidence about Acorde or MuseScore semantic parity. A valid MuseScore round-trip
remains a Phase 17A gate.

### Phase 18B CC0 MSCZ observation

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

The Phase 18A envelope validated this produced artifact and Acorde's compatibility projection
reported 40 semantic changes, analysis disagreement in `Motifs` and `PhraseBoundaries`, and 65
candidate import-loss diagnostics. The changes comprised 24 `NoteModified`, 8
`MeasurePresentationChanged`, 2 `NoteAdded`, and one each of `MetadataChanged`,
`PartNamesChanged`, `ScoreTextChanged`, `StaffConfigurationChanged`, `TimeSigChanged`, and
`UnrepresentedFieldChanged`. The 65 losses were 55
`musicxml.unsupported-placement-attribute` and 10
`musicxml.unsupported-render-attribute` diagnostics.

This completes the bounded 18B execution gate: a successful external process, exact source/output
bytes, options, typed changes, and typed losses are recorded. The semantic projection is not
equivalent and the artifact is not committed as a fixture, so it does **not** establish lossless
interchange, engraving, playback, or complete MuseScore compatibility. It instead names the next
model/I/O losses through a reproducible real-data observation.

## alphaTab / tablature

The new minimal authored MusicXML fixture `tests/fixtures/external_tab.musicxml` has SHA-256
`3cce8d2f100e3b68a3430c4ccaa0ad737318179ba74ddf47bd31bc41b3ff37df`. It defines standard guitar
tuning and a low-E third-fret G2. Acorde parses it as internal low-to-high tuning
`[40, 45, 50, 55, 59, 64]`, internal string 1, fret 3, and G2, with no diagnostics.

alphaTab 1.8.4, loaded through Node 24.5.0 `ScoreLoader.loadScoreFromBytes`, parsed Acorde's
MusicXML export (SHA-256 `2587a92b88188ee7cc84af69a4479960d4d0ad1798fdef97565c2c50dceadea8`)
as one master bar with tuning `[64, 59, 55, 50, 45, 40]`, string 1, fret 3, and
`realValue` 43. Acorde now converts between its low-to-high internal string indexing and the
high-to-low MusicXML technical-string convention at the parser and serializer boundaries.

This only checks a single structural/playback-pitch projection. No browser rendering, scheduling
trace, SoundFont output, or cross-browser visual comparison was performed.

## Related analysis evidence

The separate [music21 observation](external-analysis-evidence.md) records the bounded Phase 17D
MusicXML analysis projection. It does not imply agreement with any engraving or interchange
result above.
