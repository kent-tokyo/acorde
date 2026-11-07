# Ten-score print screening against Verovio 6.4.1 (2026-11-07)

This reruns the [licensed piano/chamber print corpus](print-corpus-verovio.md)
against stable Verovio 6.4.1. The [machine-readable result](print-corpus-verovio-6-4-1.json)
records each source and shared MusicXML SHA-256, import diagnostics, page
geometry, candidate collisions, and mark counts. Both engines read the same
MusicXML; MuseScore sources are first converted by Acorde. Import losses remain
separate from engraving findings. This is a screening pass, not a blind review.

## Reproduction

- Acorde: integration commit `fbc3185c`, `acorde` release binary SHA-256
  `998eccaa25c5848215cbdf87edc88acd241473e9e599e7b537188dc791eded79`,
  `print_mxl` release example SHA-256
  `accd693f692e5a2d0709c1298222e9fb23a35d00f40f797528c186ee8f73bc05`.
- Verovio: npm `verovio@6.4.1`, package integrity
  `sha512-+5exuA63AC0FHFo0L/4BlcF6EpEOWIp81pwz/afMupxss/tsrdVLxX8Pqh9QFzUOXcB4mP9Box4xc+hrkKN+LA==`.
  [`export_verovio_pages.mjs`](../../scripts/export_verovio_pages.mjs) loads
  MusicXML or MXL with the official toolkit and writes every page as SVG.
- Both: A4, 14 mm left/right and 16 mm top/bottom margins. Acorde uses the
  `print_mxl` density breaks, at most four measures per system, and Tuffy.ttf
  for PDF text. Verovio uses its default engraving font. The page measurements
  use [`compare_print_corpus.py`](../../scripts/compare_print_corpus.py) and
  headless Chrome's element boxes at an A4 viewport. This repeats the earlier
  protocol with `VEROVIO_NPM_PREFIX` selecting 6.4.1.

```sh
npm install --prefix /tmp/acorde-verovio-6-4-1 verovio@6.4.1 \
  --ignore-scripts --no-audit --no-fund
CARGO_TARGET_DIR=/tmp/acorde-print-target cargo build --release \
  -p acorde-cli --bin acorde --locked
CARGO_TARGET_DIR=/tmp/acorde-print-target cargo build --release \
  -p acorde-pdf --example print_mxl --locked
CARGO_TARGET_DIR=/tmp/acorde-print-target \
VEROVIO_NPM_PREFIX=/tmp/acorde-verovio-6-4-1 \
python3 scripts/compare_print_corpus.py /tmp/print-v641.json /tmp/print-v641 \
  aloha:tests/fixtures/openscore_lieder_aloha_oe.mxl \
  just-for-today:tests/fixtures/openscore_lieder_just_for_today.mxl \
  quilter:tests/fixtures/openscore_lieder_quilter_prelude.mxl \
  webern:tests/fixtures/openscore_lieder_webern_so_ich_traurig_bin.mxl \
  omr-1003:tests/fixtures/openscore_omr_score_1003.mscz \
  omr-1016:tests/fixtures/openscore_omr_score_1016.mscz \
  omr-1033:tests/fixtures/openscore_omr_score_1033.mscz \
  omr-1035:tests/fixtures/openscore_omr_score_1035.mscz \
  omr-1036:tests/fixtures/openscore_omr_score_1036.mscz \
  freilachs:tests/fixtures/freilachs_no_17.mscz
```

## Results

`A/V` means Acorde / Verovio 6.4.1. Staff and font sizes are millimetres.
The Verovio page and system counts are unchanged from the pinned 6.3.0 run.

| Score | Pages A/V | Systems A/V | Staff space A/V | Minimum text A/V |
| --- | ---: | ---: | ---: | ---: |
| Aloha Oe | 3/3 | 6/6 | 1.65/1.80 | 1.98/3.24 |
| Just for Today | 4/3 | 11/7 | 1.57/1.80 | 1.88/3.24 |
| Quilter Prelude | 1/1 | 4/3 | 1.70/1.80 | 2.04/3.24 |
| Webern | 2/2 | 5/4 | 1.29/1.80 | 1.55/3.24 |
| OMR 1003 | 1/1 | 2/1 | 1.70/1.80 | 2.04/4.05 |
| OMR 1016 | 2/1 | 7/4 | 1.70/1.80 | 2.04/3.24 |
| OMR 1033 | 1/1 | 5/3 | 1.70/1.80 | 1.70/3.24 |
| OMR 1035 | 1/1 | 5/4 | 1.70/1.80 | 1.44/3.24 |
| OMR 1036 | 2/2 | 5/4 | 1.50/1.80 | 1.80/3.24 |
| Freilachs No. 17 | 1/1 | 7/4 | 1.67/1.80 | 1.68/3.03 |
| **Total** | **18/16** | **57/40** | — | — |

The script reports six Acorde elements beyond the page edge, all on OMR 1036
under Chrome's Greek-font fallback. The previous Tuffy PDF raster check did not
show page-edge clipping; this run did not recheck that crop. It also reports
zero Acorde and 11 Verovio text/notation overlap candidates, and one Acorde
and 138 Verovio text/text candidates. These bounding-box counts include text
ascent/descent and are not confirmed collisions. Import diagnostics and each
renderer’s mark shortfalls are retained in the JSON.

A visual spot check of *Freilachs* page 1 found Acorde's seven systems spread
over the page with smaller staves, while Verovio's four systems occupy its upper
portion. This is one unblinded screen review, not a print-size preference score.

## Next gate

Raise the smallest staff and text sizes without introducing confirmed
collisions or missing marks, then inspect page use and the suspicious OMR 1036
edge boxes in the rendered PDFs. [Issue #390](https://github.com/kent-tokyo/acorde/issues/390)
still requires randomized, print-size A/B ratings from at least two independent
reviewers, with each score and setup recorded. An untouched piano/chamber
holdout is needed before a broader parity claim.
