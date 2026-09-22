# Interchange evidence contract

Format support is a semantic contract, not a byte-identity promise. The checked-in
[fixture manifest](../tests/fixtures/manifest.json) records format, license/provenance, SHA-256,
evidence mode, and declared losses. Integration tests verify those hashes from embedded fixture
bytes; tests neither fetch a corpus nor rely on filesystem input.

## Evidence modes

| Mode | Required evidence | Does not establish |
|---|---|---|
| `semantic` | parse → canonical score → serialize → parse with field-level comparison | source-byte identity or every format feature |
| `import-only` | parser result and typed diagnostics | export support or round-trip fidelity |
| `decode-render` | bounded provider/codec data and deterministic output | score interchange or audio quality |

Normalization is acceptable only when it is explicit. Unsupported input must be rejected or
reported with a stable code, source location, preserved value where available, and loss reason.
A successful parse alone never changes a capability matrix cell to `yes`.

## Current local boundaries

- **MusicXML/MXL:** canonical score fields, note offsets, typed text, percussion display data,
  structured figured bass, tablature positions, and declared instruments are covered where the
  model has an unambiguous mapping. Vendor attributes, unsupported render data, and unrepresentable
  MIDI pitch bends remain typed losses.
- **MIDI:** notes, channels, controllers, program changes, aftertouch, and pitch bends use bounded
  canonical event data. Off-boundary tempo/meter changes and non-playback notation are diagnosed;
  scheduling and synthesis are not evaluated.
- **ABC:** the documented common subset covers notes/rests/chords, selected decorations, tuplets,
  ties/slurs, grace notes, lyrics, volta, and quarter-tone spellings. Multiple voices/staves,
  tablature, and unsupported rhythm syntax are explicit limits.
- **MEI and MSCX/MSCZ:** documented structural, text, tablature, and selected spanner/harmony
  subsets round-trip or produce source-located diagnostics. Vendor-specific or timing-only
  semantics are not guessed into the canonical model.

The exact feature-level status is the [notation coverage matrix](notation-coverage.md). The
machine-readable local phase record is [`interchange-report.json`](interchange-report.json).
External tool observations, including their fixture hashes and tool versions, are separate in
[external-interoperability-evidence.md](external-interoperability-evidence.md).

## Release-gate interpretation

Local fixtures, deterministic reports, malformed-input checks, and source-located diagnostics
close only the declared local slice. They do not demonstrate broad held-out corpus performance,
lossless interchange, engraving parity, browser fidelity, or host audio equivalence. Those claims
require a permission-cleared corpus or relevant external execution under pinned conditions.

The current public MIDI fixtures and pinned MSCZ files are smoke cases, not population benchmarks.
The Phase 18 evidence inventory retains per-tool outcomes without calculating a compatibility rate
or cross-format parity score.

## Deferred beat-aware MIDI transformation

Beat-aware MIDI quantization remains intentionally deferred. The surrounding patent landscape has
not received a jurisdiction- and claim-specific review by qualified counsel. Raw timing
preservation and diagnostics are safe current behaviour; no beat-aware transformation is exposed
or advertised. This is a risk boundary, not a legal opinion.
