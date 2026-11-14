# When in Rome harmony cohort 2: failed frozen evaluation

The [protocol](analysis-wir-harmony-holdout-2-protocol-v1.json) pinned the candidate binary, source, comparator, input hashes, normalization, and decision rule before the primary `analysis.txt` labels were read. Both programs received the same 12 [answer-free prepared scores](analysis-wir-harmony-holdout-2-inputs-v1.json); three quartet scores had 1,274 embedded `<harmony>` elements removed. The [first report](analysis-wir-harmony-holdout-2-v1.json) and [common-evaluator projection](analysis-wir-harmony-holdout-2-label-evaluation-v1.json) are retained unchanged as the historical holdout result.

| Measure | Acorde | music21 10.5.2 |
|---|---:|---:|
| Parsed works | 12/12 | 12/12 |
| Predictions at 2,246 aligned labels | 1,023 | 2,240 |
| Correct canonical Roman figures | 348 | 394 |
| Correct / aligned labels | 15.5% | 17.5% |

There were 2,256 source labels; ten were excluded because their source positions were duplicated. The frozen superiority gate failed. The result was inspected to guide later changes, so this entire cohort is now **development data**. Its later [development rerun](analysis-wir-harmony-development-2-v2.json) cannot serve as a new holdout result.
