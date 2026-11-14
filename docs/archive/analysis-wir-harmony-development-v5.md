# Harmony candidate development after cohort 2

The candidate adds opt-in exact tertian labels from bar-opening arpeggios and short forward note windows. Existing vertical labels and the default CLI mode are unchanged. Cohorts 1 and 2 were inspected during development; cohort 2's original [fixed evaluation](analysis-wir-harmony-holdout-2-v1.md) failed before it was reclassified. The current candidate is identified by its SHA-256 in each JSON report.

| Previously inspected corpus | Aligned labels | Acorde correct | music21 10.5.2 correct | Acorde predictions |
|---|---:|---:|---:|---:|
| [When in Rome cohort 1](analysis-wir-harmony-development-v5.json) | 1,159 | 366 | 348 | 734 |
| [When in Rome cohort 2](analysis-wir-harmony-development-2-v2.json) | 2,246 | 399 | 394 | 1,154 |
| [BCMH](analysis-bcmh-harmony-labels-v5.json) | 4,627 | 2,193 | 2,073 | 4,300 |

The [cohort 1](analysis-wir-harmony-development-label-evaluation-v3.json), [cohort 2](analysis-wir-harmony-development-2-label-evaluation-v1.json), and [BCMH](analysis-bcmh-harmony-label-evaluation-v2.json) common-evaluator projections verify their prepared inputs and reproduce these counts. The three corpora and annotation conventions differ, so their counts should not be pooled. All are development evidence; the separate [cohort 3 result](analysis-wir-harmony-holdout-3-v1.md) tests the frozen candidate.
