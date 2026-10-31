# Print layout contract

`acorde-layout::compute_print_layout` converts a score and a `PrintConfig` into deterministic,
host-neutral page and system metadata in millimetres.

```text
Score → PrintConfig → PrintLayoutResult → page SVG → vector PDF
```

## What Acorde provides

- `PrintPreset`: deterministic A4/Letter full-score and extracted-part starting configurations.
- `PrintConfig`: paper, margins, bleed/safe area, scale, measures/systems capacity, pickup,
  keep-together, notation-break, final-page, system-height, color, crop-mark, glyph-resource,
  and publication policies.
- `PrintLayoutResult`: stable page/system addresses, physical measure spans, break reasons,
  multirest ownership, page-scoped span continuations, and publication metadata.
- `PublicationConfig`: title-page, running title, header/footer, page number, part label,
  section, spacer, frame, and image-resource descriptors. Image bytes and drawing stay outside
  the contract.

The serialized print-layout contract is version **34**; the preset schema is version **2**.
Each page records `content_left_mm`, so asymmetric margins retain their physical notation origin.
Consumers must validate a received
`PrintLayoutResult` before reuse; incompatible contract versions, invalid page/system addresses,
non-finite geometry, and invalid page metadata are rejected.

## Deterministic layout rules

Explicit system/page breaks are preserved. Keep-together ranges and notation-aware repeat/volta
policies either fit a system/page or return a typed error; they are never silently split.
Multirests retain their underlying physical extent. `FinalPagePolicy::Balance` balances automatic
pagination, but does not override explicit breaks. Extracted-part layout is an explicit view and
does not mutate the score.
The configured system height and any explicit systems-per-page request must fit the usable page
height. Persisted layouts reject systems that overlap or extend beyond the physical page instead
of passing clipped geometry to a renderer.
For multi-staff scores, the default `StaffAware` height policy reserves at least 24 mm for the
first staff plus 16 mm for each additional staff, before applying `scale`. This changes page
capacity for piano and chamber scores; extracted single-staff parts use their own staff count.
A two-staff piano score reserves at least 50 mm per system; when its upper staff has a
below-staff direction, the reservation rises to 60 mm and the page SVG uses a wider inter-staff
text lane. The density break budget remains bounded while that vertical lane grows.
Callers with measured system geometry can select `SystemHeightPolicy::Fixed` to use
`system_height_mm` exactly. This is a pagination clearance floor, not a guarantee that lyrics,
annotations, or shaped glyphs fit without collisions.
The default `PickupPolicy::Auto` keeps a short pickup with following music. Select
`DetectFirstMeasure` when the pickup must occupy a separate first system.

`PageLayout::span_segments` and `SystemLayout::span_segments` identify continuation ownership so
a renderer does not reconstruct spans by index arithmetic. `measure_marks` carries repeats,
voltas, navigation, rehearsal marks, and normalized text annotations for the primary score staff.

## Host boundary

acorde owns logical geometry, collision constraints, glyph-resource descriptors, preflight, and
physical page SVG generation through `acorde-render-svg::render_print_page_svgs`. The generated
pages can be joined into vector PDF with `acorde-pdf::export_score_print_pdf`. The host supplies
text-font bytes for PDF export and owns raster output, preview UI, and OS printing. Host glyphs,
images, publication frames, color preservation, and crop marks return explicit errors in the
current page SVG path.
Built-in vector glyph coverage is checked before SVG output; unsupported critical glyphs are
rejected rather than silently blanked.

Therefore a successful layout or page SVG is not proof of shaped-font collision quality,
font embedding, printer output, or superiority to another engraver.

## Corpus gate

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
