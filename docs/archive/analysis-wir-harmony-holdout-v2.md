# When in Rome harmony holdout, corrected positional scoring

Historical report. A later [timing correction](analysis-wir-harmony-development-v3.md)
found that 113 of these exclusions came from decimal accumulation in the
comparator, rather than different score imports. This inspected cohort is now
development data; its v3 report is the current baseline.

The first [frozen run](analysis-wir-harmony-holdout-v1.md) revealed three
positions in Schubert's *Das Wandern* where the RomanText source repeats a
measure and supplies two labels at the same measure and beat. A single score
position cannot distinguish those repeats. The v2 positional rule excludes
both source labels at each ambiguous position, regardless of either system's
prediction. There was no engine change or label-based tuning between runs.
The [v1 full report](analysis-wir-harmony-holdout-v1.json) remains archived;
the [v2 full report](analysis-wir-harmony-holdout-v2.json) preserves this
intermediate scoring rule. Source selection, input hashes, analyzer versions, and normalization
are unchanged and documented in [v1](analysis-wir-harmony-holdout-v1.md).

| Measure | Acorde | music21 10.5.2 |
|---|---:|---:|
| Scores parsed | 12/12 | 12/12 |
| Predictions at 1,046 aligned gold positions | 509 | 1,044 |
| Correct canonical Roman figures | 252 | 306 |
| Correct / aligned gold positions | 24.1% | 29.3% |
| Correct / predictions | 49.5% | 29.3% |

Of 1,168 source labels, 122 were classified as excluded: 113 for apparent
unequal pitched timelines,
3 for missing or ambiguous source measures, and 6 at duplicated gold positions.
The six represent three repeated positions. The 113 include all 106 labels of
Schubert's *Wohin*; v3 shows that this apparent import disagreement was caused
by decimal timing accumulation in this comparator.
The full JSON retains each scored row and each excluded measure with its
reason. The rerun reproduced the v2 JSON byte for byte.

music21 wins this frozen exact-position Roman-figure recall gate by 54 labels.
Acorde emits fewer labels and has higher precision when it emits one. The
comparison has a known format-affinity limit: music21 parses the primary
RomanText gold annotations. It also tests harmony at the original annotated
position, which may precede the complete notes of an arpeggio. This result
does not establish general harmonic-analysis quality. If these works inform
an engine fix, they become development data and a new untouched holdout is
required for a new superiority claim.
