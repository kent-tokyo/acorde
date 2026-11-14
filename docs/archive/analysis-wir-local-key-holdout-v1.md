# When in Rome local-key label holdout: fixed result

The [preflight and protocol](analysis-wir-local-key-holdout-preflight-v1.md)
were committed as `906335bb` before the twelve selected `analysis.txt` files
were fetched. The [scorer](../../scripts/score_wir_local_key_holdout.py)
checked the pinned score, protocol, binary, implementation, and annotation
Git blob hashes; the [file-hash record](analysis-wir-local-key-holdout-all-hashes-v1.json)
also records the retrieved annotation SHA-256 values. The
[result](analysis-wir-local-key-holdout-v1.json) and
[position rows](analysis-wir-local-key-holdout-v1-rows.jsonl) retain work-level
and source-position evidence. Both tools parsed 12/12 prepared scores; their
pitched timelines matched in all 1,462 physical measures before annotations
were opened.

| Preregistered measure | Acorde radius 4 | music21 10.5.2 window 2 |
| --- | ---: | ---: |
| Exact local-key labels, all scored positions | 1,924 / 2,921 | **1,934 / 2,921** |
| Exact labels, positions available to both | 1,921 / 2,903 | **1,934 / 2,903** |
| Prediction available | 2,921 / 2,921 | 2,903 / 2,921 |
| Work wins | 2 | **7** |

Three works tie. The 2,946 source labels yield 2,921 scored positions and
25 predeclared exclusions: 22 labels at duplicate source beats and three in
missing or ambiguous source measure numbers. The excluded fraction is 0.85%,
below the protocol's 5% ceiling. No score or prediction failed to parse.
Acorde's coverage meets the gate, but its paired exact count trails by 13 and
its two work wins fall short of the required seven. **The preregistered
local-key gate fails.**

By group, Acorde versus music21 exact labels are 930 versus 916 in the three
Beethoven quartets, 107 versus 125 in Bach WTC I 10–12, 807 versus 813 in
Mozart K309–311, and 80 versus 80 in Louise Reichardt songs. The aggregate
quartet lead comes from one work; music21 wins the other two. These group
counts describe the fixed result and are not a new selection rule.

The previous 24-work development lead cannot support an overall claim that
Acorde surpasses music21 in local-key analysis. This failed holdout remains
archived. The candidate, music21 window, scorer, and thresholds were not
changed after opening its labels. Any future local-key tuning that uses this
feedback requires a new disjoint label holdout under the protocol stop rule.
