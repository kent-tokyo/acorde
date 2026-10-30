# Verovio 6.3.0 engraving baseline

This is a reproducible starting point for the goal of exceeding Verovio on
layout, engraving, and print for ordinary piano and chamber scores. Page count
alone is not an engraving-quality score.

## Fixed input and setup

- Date: 2026-10-30.
- Input: `tests/fixtures/openscore_lieder_aloha_oe.mxl`, CC0-1.0, SHA-256
  `b195ae477a10cbaee281d9b728cac001ae1d50fe4527078ccae14092eee4936c`
  (see `tests/fixtures/manifest.json`).
- A4 portrait, 210 × 297 mm; top/bottom margins 16 mm, left/right 14 mm.
- Verovio: Homebrew CLI 6.3.0, default automatic breaks and bundled fonts.
- Acorde: `v1.3.3` and the staff-aware print-layout change on this branch;
  default four measures per system. The Acorde result is logical page geometry,
  not completed page SVG or PDF.

```bash
verovio -a --page-width 2100 --page-height 2970 \
  --page-margin-top 160 --page-margin-bottom 160 \
  --page-margin-left 140 --page-margin-right 140 \
  -o /tmp/verovio-aloha-a4.svg tests/fixtures/openscore_lieder_aloha_oe.mxl

acorde print-report tests/fixtures/openscore_lieder_aloha_oe.mxl \
  --preset a4-score --measures-per-system 4
```

| Result | Verovio 6.3.0 | Acorde v1.3.3 | Acorde with staff-aware height |
| --- | ---: | ---: | ---: |
| A4 pages | 3 rendered SVG pages | 1 logical page | 3 logical pages |
| Systems | not recorded | 7 | 7 |
| Reserved system height | renderer-managed | 24 mm | 88 mm (five staves) |
| MusicXML import losses | not assessed | 73 | 73 |

The Acorde import diagnostics are mostly unsupported MusicXML text styling;
they remain unfixed. The matching page count does not establish equivalent
notation placement, font shaping, collision clearance, PDF output, or print
quality. Verovio chooses its own system breaks and fonts, so this is a
same-input, same-paper comparison rather than an identical-layout comparison.

## Gates before a quality claim

1. Produce complete page SVGs from `PrintLayoutResult`, including publication
   text, cross-page spans, and independent page viewports.
2. Export those pages as vector PDF and visually inspect every rendered page.
3. Pin a larger licensed piano/chamber corpus and compare missing symbols,
   clipped content, collisions, page geometry, and legibility at print size.
4. Run blind human review of the resulting PDFs and record the rubric and
   individual scores. Do not infer superiority from page count or preflight.
