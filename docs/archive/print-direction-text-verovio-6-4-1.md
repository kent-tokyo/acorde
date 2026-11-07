# MusicXML direction text in the A4 corpus (2026-11-07)

This repeats the [opt-in ten-score A4 comparison](print-density-ensemble-verovio-6-4-1.md)
against pinned Verovio 6.4.1, using the same source files and eight-measure
system cap. The [per-score report](print-direction-text-verovio-6-4-1.json)
records source hashes, import diagnostics, marks, page geometry and collision
screens. Different generated part IDs can change the converted MusicXML hash
for MSCZ inputs between runs; source hashes and musical counts are stable.

Two MusicXML `<other-dynamics>` values contain prose: *Aloha Oe* has `solo`
and Webern has `äusserst zart`. Previously Acorde discarded both. The parser
now keeps nonempty direction-level values as positioned expression text while
reporting that they have no mapped playback dynamic. The strings appear in the
new SVGs and in text extracted from the A4 PDFs. A MusicXML export normalizes
them to `<words>`; it does not preserve the original element name or imply a
known loudness value. Note-level `<other-dynamics>` remains unsupported.

The comparison script now counts standard dynamic glyphs separately from
`<other-dynamics>` prose, and counts a Verovio hairpin only when its SVG group
has drawn geometry. This corrects two misleading baselines:

| Score | Source observation | Acorde / Verovio drawn hairpins |
| --- | --- | ---: |
| Quilter Prelude | Two wedge starts have no stops; Verovio emits two empty groups and Acorde reports an unterminated wedge | 0 / 0 |
| Webern | A duplicate same-number, same-time crescendo start at measure 14 is reported by Acorde | 20 / 21 |

The Webern difference remains open: its duplicate wedge is ambiguous in the
source, and Verovio draws an additional overlapping wedge. This is a measured
interoperability gap, not evidence that either interpretation is musically
correct. Quilter's two absent wedges are explained by its missing stops; the
empty Verovio groups were not visible marks.

The ten-score totals remain **18 pages / 54 systems** for Acorde and **16
pages / 40 systems** for Verovio. The smallest measured Acorde staff space is
1.65 mm, compared with Verovio's 1.8 mm. Page-edge and candidate text-collision
counts do not increase. Aloha and Webern PDFs were rasterized at 110 dpi for
a screen check of the added text. The blind physical-size review and untouched
piano/chamber holdout are still required before a parity claim.

## Reproduction

Build the Rust 1.89 CLI and `print_mxl` example in release mode, then rerun the
ten `NAME:SOURCE:8` arguments in the [baseline command](print-corpus-verovio-6-4-1.md#reproduction)
with `VEROVIO_NPM_PREFIX=/tmp/acorde-verovio-6-4-1`. The CLI binary SHA-256
used here is `63aa69636bb9e53c12b1c611e5e774ef94e18c271770b68b7807c71a0b2d7169`;
the `print_mxl` SHA-256 is
`73a05b50535ec86beb6d07535d6ddf0dbd145b4eb042698b3f330cd35aa2d407`.
The optimized local full WASM was 7,062,665 bytes, 436 bytes above the
preceding local build and below the 7,117,000-byte CI limit. Linux CI remains
the release-relevant size check.
