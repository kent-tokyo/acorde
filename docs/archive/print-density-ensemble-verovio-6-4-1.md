# Opt-in A4 density for ensemble scores against Verovio 6.4.1 (2026-11-07)

This reruns the [ten-score A4 corpus](print-corpus-verovio-6-4-1.md) with
`PrintSystemBreakStrategy::NotationDensity` and an explicit eight-measure
system cap. The preceding [staff-size run](print-staff-size-verovio-6-4-1.md)
used the same sources with a four-measure cap. The
[per-score results](print-density-ensemble-verovio-6-4-1.json) record input
hashes, import diagnostics, rendered marks, page geometry and screening counts.
Both renderers read the same MusicXML within each run. MuseScore conversion
generates different part IDs between runs, so compare source-file hashes and
musical content rather than raw converted-XML hashes across runs.

In the Acorde layout, a lyric-free score of at least five staves may use a
six-bar-equivalent density budget when the caller raises the cap above four.
The opening row keeps the four-bar budget for clefs and part labels. These
systems reserve 21 mm per extra staff; scores with lyrics, fixed system height
or an explicit capacity above two systems per page keep the prior reservation.
The public default four-measure cap is unchanged.

| Score | Acorde pages | Acorde systems before/after | Staff space before/after |
| --- | ---: | ---: | ---: |
| Aloha Oe | 3 | 6/5 | 1.65/1.65 mm |
| OMR 1036 | 2 | 5/4 | 1.50/1.80 mm |
| Freilachs No. 17 | 1 | 7/6 | 1.67/1.80 mm |
| Other seven scores | unchanged | unchanged | unchanged |
| **Ten-score total** | **18** | **57/54** | **minimum 1.50/1.65 mm** |

Verovio uses 16 pages and 40 systems on this corpus. In OMR 1036 both tools
now use two pages, four systems and 1.8 mm staff space. The score has zero
import diagnostics, 119 noteheads and 39 visible rests in both rendered
outputs. Across the corpus, source hashes and import diagnostics match the
prior run; candidate text/notation collisions, text/text collisions and mark
shortfalls do not increase. Chrome inspection found that the six OMR 1036
page-edge candidates were Greek part labels clipped at the left paper edge.
Constraining those label runs to the available width removes all six in the
same-browser rerun. Its margin-screening count rises from 5 to 11 because the
labels remain inside the 14 mm notation margin, where part labels belong.
*Aloha Oe* still has a sparsely used final page; fewer
systems there are not by themselves a readability improvement.

Both OMR 1036 pages and all three *Aloha Oe* pages were rasterized at 110 dpi
for a screen spot check. No obvious new note or lyric collision was seen. This
was supplemented on OMR 1036 by a Chrome bounding-box check at a 0.02 mm²
intersection threshold: both the preceding and changed SVGs had zero
notehead/notehead and zero notehead/accidental overlaps across 119 noteheads.
This narrow check does not cover stems, beams or articulation geometry. It
does not replace a physical-size print inspection or the independent blind
review in [#390](https://github.com/kent-tokyo/acorde/issues/390). The
untouched piano/chamber holdout remains to be run before a broader parity
claim. Text remains substantially smaller than Verovio's on several scores.

## Reproduction

Use the CLI and `print_mxl` example from this change, with Rust 1.89 or newer,
the pinned `verovio@6.4.1` package and headless Chrome. Run the same ten
specifications in the [baseline command](print-corpus-verovio-6-4-1.md#reproduction),
appending `:8` to each `NAME:SOURCE` argument. The Acorde CLI binary used for
MuseScore conversion has SHA-256
`e621c057ae7ca80e3a0690a5d519cd65cfe4bca492a8def514f9239a0d6176ca`.
The final `print_mxl` example SHA-256 is
`bf3fb26d78a7c50586c9116cf9d309002c32c88e89e574bb3bbca6fbb28c1552`.

The optimized local full WASM artifact was 7,062,235 bytes, up 2,140 bytes
from the preceding local build and below the 7,117,000-byte CI limit. Linux
CI remains the release-relevant size check.
