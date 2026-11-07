# Print staff-size follow-up against Verovio 6.4.1 (2026-11-07)

This repeats the [ten-score A4 screening](print-corpus-verovio-6-4-1.md)
after two print changes: the preferred SVG staff space rises from 1.7 to
1.8 mm, and the staff-aware page layout reserves more height only for
three-staff systems. The [per-score JSON](print-staff-size-verovio-6-4-1.json)
keeps source hashes, import diagnostics, physical page geometry, SVG screening
counts and mark shortfalls. Both renderers read the same MusicXML within each
run. This is a machine screening, not a print-size blind review.

The source is based on `9a0f8078`. The Acorde CLI SHA-256 is
`e621c057ae7ca80e3a0690a5d519cd65cfe4bca492a8def514f9239a0d6176ca`;
the changed `print_mxl` example SHA-256 is
`4097580b647a07b60813cd1dbcbc437d346cad2db44fb9a134b8d798e0392b2b`.
Verovio is npm `verovio@6.4.1` with the package integrity recorded in the
baseline. Run the same `compare_print_corpus.py` command shown there with
`CARGO_TARGET_DIR` pointing to the rebuilt Acorde binaries.

| Score | Pages before/after | Systems before/after | Acorde staff space before/after (mm) |
| --- | ---: | ---: | ---: |
| Just for Today | 4/4 | 11/11 | 1.57/1.67 |
| Quilter Prelude | 1/1 | 4/4 | 1.70/1.72 |
| Webern | 2/2 | 5/5 | 1.29/1.72 |
| OMR 1003 | 1/1 | 2/2 | 1.70/1.80 |
| OMR 1016 | 2/2 | 7/7 | 1.70/1.80 |
| OMR 1033 | 1/1 | 5/5 | 1.70/1.80 |
| OMR 1035 | 1/1 | 5/5 | 1.70/1.72 |
| Other three scores | unchanged | unchanged | unchanged |
| **Ten-score total** | **18/18** | **57/57** | — |

The source-file SHA-256 values match the baseline. MuseScore inputs are
converted to MusicXML by the unchanged CLI; generated MusicXML part IDs vary
between runs, so their raw shared-input hashes differ. Two current reruns were
identical after removing those generated `id` attributes. The direct MXL
inputs have identical shared-input hashes.

The minimum rendered text size rises from 1.55 to 2.06 mm on Webern, and from
1.88 to 2.00 mm on *Just for Today*. Across all ten scores, automated
text/notation and text/text overlap counts, page-edge counts and mark
shortfalls do not increase. Webern has one additional element flagged beyond
the nominal content margin (10 to 11); that box needs PDF inspection. The six
previous OMR 1036 page-edge candidates remain. Verovio still has fewer total
pages and systems and larger text on this corpus. The unblinded measurements
do not settle readability or overall engraving quality; [#390](https://github.com/kent-tokyo/acorde/issues/390)
still requires independent blind review.

Both Webern PDF pages were also rasterized at 110 dpi and inspected on screen.
That spot check found no obvious new clipping or mark overlap. It is not a
physical-size print inspection and does not resolve the margin-box candidate.
