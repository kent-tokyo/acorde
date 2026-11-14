# When in Rome harmony cohort 3: fixed-candidate evaluation

The [protocol](analysis-wir-harmony-holdout-3-protocol-v1.json) pinned the opt-in Acorde binary, evaluator sources, 12-work [selection](analysis-wir-harmony-holdout-3-selection-v1.json), [source hashes](analysis-wir-harmony-holdout-3-hashes-v1.json), [prepared-input hashes](analysis-wir-harmony-holdout-3-inputs-v1.json), and decision rule before either system was run against the gold labels. The 12 works are disjoint from cohorts 1 and 2. Both programs received byte-identical prepared MXL files; three quartet scores had 1,358 embedded `<harmony>` answer elements removed. Both programs estimated a global key independently. The candidate used `analyze-vertical --infer-key --arpeggiated-bars`; the default Acorde mode was not evaluated here.

The [full paired report](analysis-wir-harmony-holdout-3-v1.json) retains every scored prediction and exclusion. The [common-evaluator projection](analysis-wir-harmony-holdout-3-label-evaluation-v1.json) independently verifies all 12 prepared input hashes and reproduces the totals.

| Measure | Acorde | music21 10.5.2 |
|---|---:|---:|
| Parsed works | 12/12 | 12/12 |
| Predictions at 2,341 aligned labels | 1,414 | 2,335 |
| Correct canonical Roman figures | 559 | 508 |
| Correct / aligned labels | 23.9% | 21.7% |
| Correct / predictions | 39.5% | 21.8% |

Of 2,357 source labels, 16 were excluded: 14 at duplicate source positions, one without a unique source-measure mapping, and one with a different pitched event timeline. Scored-label coverage is 99.3%. The preregistered gate passes: both systems parse all works, Acorde has 51 more correct labels and higher exact-position precision. Acorde wins six individual works; the aggregate result does not imply per-work dominance.

This measures Roman figures **only at primary annotation positions** under the stated narrow normalization. It does not score extra predicted positions, local-key analysis, cadence, voice leading, key recognition, or broad music-analysis quality. The gold is read through music21 RomanText, and independent global keys can disagree with annotated local keys. The 12-work result supports only this task-scoped superiority claim for the pinned binary and corpus.
