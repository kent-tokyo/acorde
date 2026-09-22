# Print layout contract

`acorde-layout::compute_print_layout` converts a score and a `PrintConfig` into deterministic,
host-neutral page and system metadata in millimetres.

```text
Score → PrintConfig → PrintLayoutResult → SVG/PDF/print host
```

## What Acorde provides

- `PrintPreset`: deterministic A4/Letter full-score and extracted-part starting configurations.
- `PrintConfig`: paper, margins, bleed/safe area, scale, measures/systems capacity, pickup,
  keep-together, notation-break, final-page, color, crop-mark, glyph-resource, and publication
  policies.
- `PrintLayoutResult`: stable page/system addresses, physical measure spans, break reasons,
  multirest ownership, page-scoped span continuations, and publication metadata.
- `PublicationConfig`: title-page, running title, header/footer, page number, part label,
  section, spacer, frame, and image-resource descriptors. Image bytes and drawing stay outside
  the contract.

The serialized print-layout contract is version **32**. Consumers must validate a received
`PrintLayoutResult` before reuse; incompatible contract versions, invalid page/system addresses,
non-finite geometry, and invalid page metadata are rejected.

## Deterministic layout rules

Explicit system/page breaks are preserved. Keep-together ranges and notation-aware repeat/volta
policies either fit a system/page or return a typed error; they are never silently split.
Multirests retain their underlying physical extent. `FinalPagePolicy::Balance` balances automatic
pagination, but does not override explicit breaks. Extracted-part layout is an explicit view and
does not mutate the score.

`PageLayout::span_segments` and `SystemLayout::span_segments` identify continuation ownership so
a renderer does not reconstruct spans by index arithmetic. `measure_marks` carries repeats,
voltas, navigation, rehearsal marks, and normalized text annotations for the primary score staff.

## Host boundary

acorde owns logical geometry, collision constraints, glyph-resource descriptors, and preflight.
The host owns font lookup and metrics, shaping, embedding, raster output, PDF backend, save/preview
UI, and OS printing. Built-in vector glyph coverage is checked before SVG output; unsupported
critical glyphs are rejected rather than silently blanked.

Therefore a successful layout or SVG preflight is not proof of shaped-font collision quality,
PDF fidelity, font embedding, or printer output.

## Phase 19A corpus gate

[`benchmarks/print/phase19a.json`](../benchmarks/print/phase19a.json) pins self-authored fixtures
for rich notation, multipart scores, tablature, and explicit system/page breaks.

```bash
python3 -B scripts/validate_print_corpus.py --acorde target/debug/acorde
```

The runner validates fixture provenance, repeats `print-report` and non-interactive SVG preflight,
requires deterministic output with finite geometry, and rejects import or renderer issues. CI runs
`--check-only`, which validates the fixture contract without requiring a built renderer.

Use `acorde print-report --fail-on-issues` for a machine-readable local publication preflight.
It deliberately does not create a PDF or contact a printer.
