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
- Acorde: `v1.3.3` plus the staff-aware height and physical page SVG changes on
  this development branch; default four measures per system, built-in vector notation,
  and the public-domain Tuffy test font for PDF text. Acorde's default now keeps the
  partial pickup with following measures.

```bash
verovio -a --page-width 2100 --page-height 2970 \
  --page-margin-top 160 --page-margin-bottom 160 \
  --page-margin-left 140 --page-margin-right 140 \
  -o /tmp/verovio-aloha-a4.svg tests/fixtures/openscore_lieder_aloha_oe.mxl

acorde print-report tests/fixtures/openscore_lieder_aloha_oe.mxl \
  --preset a4-score --measures-per-system 4

cargo run -p acorde-pdf --example print_mxl -- \
  tests/fixtures/openscore_lieder_aloha_oe.mxl \
  crates/pdf/tests/fixtures/Tuffy.ttf /tmp/acorde-aloha-print.pdf
pdftoppm -scale-to 1400 -png /tmp/acorde-aloha-print.pdf /tmp/acorde-aloha-page
```

| Result | Verovio 6.3.0 | Acorde v1.3.3 | Acorde with staff-aware height | Acorde with page SVG and pickup fix |
| --- | ---: | ---: | ---: | ---: |
| A4 pages | 3 rendered SVG pages | 1 logical page | 3 logical pages | 2 SVG/PDF pages |
| Systems | 6 | 7 | 7 | 6 |
| Reserved system height | renderer-managed | 24 mm | 88 mm (five staves) | 88 mm (five staves) |
| MusicXML import losses | not assessed | 73 | 73 | 73 |

The Acorde import diagnostics are mostly unsupported MusicXML text styling;
they remain unfixed. The PDF has a valid A4 media box on both pages. In the rendered
images, the page title and four part labels are visible, and the pickup is no longer
stretched into a standalone full-width system. Verovio uses three pages with visibly
larger lyrics. The follow-up staff-group change on this branch adds the missing
brace across the piano's two staves; the updated A4 PDF was rendered at 1400 pixels
and inspected. Acorde's lyrics are still very small at that preview size, and the
final system is stretched to the full page width.
The two-page Acorde result is therefore not evidence of superior print quality.
Verovio chooses its own system breaks and fonts; this is a same-input, same-paper
comparison rather than an identical-layout comparison.

## Gates before a quality claim

1. Keep page SVG/PDF generation and cross-page span continuation covered by tests.
2. Improve lyric size, ragged final systems, and publication
   typography against the visual baseline.
3. Pin a larger licensed piano/chamber corpus and compare missing symbols,
   clipped content, collisions, page geometry, and legibility at print size.
4. Run blind human review of the resulting PDFs and record the rubric and
   individual scores. Do not infer superiority from page count or preflight.
