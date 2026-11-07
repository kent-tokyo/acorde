# Print lyric legibility against Verovio 6.4.1 (2026-11-07)

This follow-up repeats the [fixed ten-score A4 comparison](print-density-ensemble-verovio-6-4-1.md)
with an explicit eight-measure density cap. Both engines receive the same MusicXML
within each run. The [per-score JSON](print-lyric-legibility-verovio-6-4-1.json)
records source and shared-input hashes, diagnostics, print geometry, mark counts,
and browser bounding-box collision candidates. Printed lyrics now use a
1.9-staff-space font and a 1.08-space character width estimate, up from 1.6 and
0.91 respectively. Screen SVGs keep their 1.6/0.91 metrics. A lower staff note
with an altered or explicitly displayed
accidental reserves an extra 0.75 staff spaces above it when lyrics are present.

| Score with lyrics | Acorde lyric size before / after | Verovio | Acorde staff space before / after |
| --- | ---: | ---: | ---: |
| Aloha Oe | 2.63 / 3.08 mm | 4.05 mm | 1.65 / 1.62 mm |
| Just for Today | 2.67 / 3.17 mm | 4.05 mm | 1.67 / 1.67 mm |
| Webern | 2.75 / 3.19 mm | 4.05 mm | 1.72 / 1.68 mm |
| OMR 1003 | 2.88 / 3.42 mm | 4.05 mm | 1.80 / 1.80 mm |

Across all ten scores, Acorde still uses 18 pages and 54 systems, versus
Verovio's 16 pages and 40 systems. The browser screen finds no new page-edge
or text/notation collision candidates in Acorde's final run; the existing one
text/text candidate on Aloha Oe remains. An intermediate 1.9-font run without
the accidental clearance introduced a 0.78 mm² overlap candidate between
Webern's lyric “dei” and a flat on the piano staff. The clearance removes it.
The first Webern PDF page was rasterized at 180 dpi for a visual spot check.
These are screening checks, not a print-size blind review or a guarantee that
the page has no other collision. Two scores lose 0.03–0.04 mm of staff space.
The remaining lyric gap to Verovio and smaller non-lyric text are unresolved.

## Reproduction and limits

The run used Rust 1.97.0, `wasm-pack` 0.15.0, pinned npm `verovio@6.4.1`,
headless Chrome, and A4 with 14 mm horizontal / 16 mm vertical margins.
The Acorde CLI and `print_mxl` example were built in the debug profile because
the release build lacked space in this worktree; their SHA-256 hashes were
`cda30047fc678bc117578001855f856ae7a7ab8fbf4a99e7041f5aecc8bd1e18`
and `855b32100bb63f11d3632f67337c1eab07fb6f7590ff2ba1f69aa16d071b60a0`.
Build profile does not enter the geometry calculations, but this is not a
release-binary comparison. To rerun, use the ten inputs in the
[6.4.1 baseline command](print-corpus-verovio-6-4-1.md#reproduction), append
`:8` to each input, and set `ACORDE_BINARY=target/debug/acorde`,
`PRINT_MXL_BINARY=target/debug/examples/print_mxl`, and
`VEROVIO_NPM_PREFIX=/tmp/acorde-verovio-6-4-1`.

All 32 print-page tests and the renderer test suite pass. The optimized local
full WASM artifact is 7,092,718 bytes, below the 7,153,000-byte CI limit;
Linux CI has not yet measured this change. A 390 px Webern screen SVG was
spot-checked; its dense three-staff opening remains cramped, so a narrow-view
usability review is still needed independently of this print change.
The next print work is to improve notation-aware horizontal packing and the
small chord-symbol, volta and measure-number text without shrinking the
staves or concealing any marks. The independent A/B gate in
[#390](https://github.com/kent-tokyo/acorde/issues/390) remains open.
