# When in Rome local-key holdout v2: fixed result (#501)

**The preregistered gate fails.** On 16 untouched works, the new Acorde
key-path method matches 2,548 exact local keys against music21's 2,576 on
the 3,495 positions where both tools give an estimate.

## What was frozen, and when

The candidate, `analyze-local-keys --method path`
([`estimate_local_key_path_with_aarden_essen_profile`](../../crates/analysis/src/lib.rs)),
was built and tuned only on 48 inspected development works. Those are the
three harmony cohorts and the failed [first local-key holdout](analysis-wir-local-key-holdout-v1.md).
The [development sweep](analysis-wir-local-key-path-development-v1.json) is
reproduced by [`diagnose_wir_local_key_path.py`](../../scripts/diagnose_wir_local_key_path.py),
which also checks that the Rust output matches its Python version at every
measure.

The [selection](analysis-wir-local-key-holdout-2-selection-v1.json) comes
from [`freeze_wir_local_key_holdout_v2.py`](../../scripts/freeze_wir_local_key_holdout_v2.py).
It reads only the pinned Git tree listing, ranks candidates by a seeded
SHA-256, and excludes every earlier work. For quartets, sonatas and songs it
also excludes every earlier parent quartet, sonata or song collection. The
score-only [hashes](analysis-wir-local-key-holdout-2-score-hashes-v1.json),
harmony-stripped [inputs](analysis-wir-local-key-holdout-2-inputs-v1.json),
input-only [preflight](analysis-wir-local-key-holdout-2-preflight-v1.json)
and [protocol](analysis-wir-local-key-holdout-2-protocol-v1.json) were
committed before any `analysis.txt` was fetched. The protocol pins the
binary, the analysis source, the scorer, its support scripts and music21
10.5.2. Both tools parsed all 16 scores, and their pitched timelines matched
in all 1,755 physical measures. The [scorer](../../scripts/score_wir_local_key_holdout_v2.py)
ran once. The [result](analysis-wir-local-key-holdout-2-v1.json) and
[position rows](analysis-wir-local-key-holdout-2-v1-rows.jsonl) keep every
work, and the [all-file hashes](analysis-wir-local-key-holdout-2-all-hashes-v1.json)
record the annotations.

One overlap was missed. The exclusion covered only earlier When in Rome
works. K281/1 and K283/2 are also in the Batik Mozart cadence development set
([#502](https://github.com/kent-tokyo/acorde/issues/502)), so their notes and
Acorde's cadence output had been looked at before this freeze. Their local-key
labels had not, and the key-path settings were not tuned on them. Without
these two movements Acorde has 2,268 and music21 2,256 of 3,119 paired
positions. That subset was chosen after scoring and breaks the protocol's
16-work requirement, so it is not a pass. The frozen result below stands.

## Result

| Preregistered measure | Acorde path | music21 10.5.2 window 2 |
| --- | ---: | ---: |
| Exact labels, positions available to both (gated) | 2,548 / 3,495 | **2,576 / 3,495** |
| Exact labels, all scored positions | 2,553 / 3,500 | **2,576 / 3,500** |
| Tonic pitch class and mode, all positions | 2,553 / 3,500 | **2,611 / 3,500** |
| Prediction available | **3,500 / 3,500** | 3,495 / 3,500 |
| Work wins | 6 | **8** |

Two works tie. The 3,570 source labels yield 3,500 scored positions. The 70
exclusions are labels at duplicate source beats or in missing or ambiguous
source measures, 1.96% against the 5% ceiling. Coverage meets the gate, but
the paired exact count trails by 28.

| Group (4 works each) | Positions | Acorde path | music21 |
| --- | ---: | ---: | ---: |
| Beethoven quartet movements | 1,589 | **1,169** | 1,119 |
| Bach, *The Well-Tempered Clavier* I | 345 | 171 | **210** |
| Mozart piano sonata movements | 1,232 | 1,016 | **1,037** |
| OpenScore Lieder songs | 334 | 197 | **210** |

## What the development numbers did not predict

On the 48 development works the path method matched 6,468 of 8,668 positions,
against 5,847 for the radius-4 window and 5,734 for music21. Choosing the
parameters on three cohorts and scoring the fourth also beat both in every
cohort. That lead did not carry over to these 16 works.

As a post hoc reference only, the existing radius-4 window method scores
2,532 of the same 3,500 positions, so the path method gains 21 positions over
it. This comparison was not preregistered and does not change the result.

## Status

The path method stays opt-in. The window method remains the CLI default, and
no claim that Acorde matches or exceeds music21 on local keys is made. Under
the stop rule these 16 works are now development data. Any further attempt
needs another disjoint holdout. Unused When in Rome pairs with a local
`score.mxl` are now scarce in the Bach keyboard and Lieder groups.

## Common evaluator

[`adapt_wir_local_key_report.py`](../../scripts/adapt_wir_local_key_report.py)
projects the development sweep and this holdout into the
[label evaluator](../analysis-evaluation.md) with separate splits. The
[evaluator report](analysis-wir-local-key-path-label-evaluation-v1.json) gives
6,468/8,668 (Acorde) and 5,734/8,668 (music21) on development, and
2,553/3,500 and 2,576/3,500 on the holdout.

```sh
python3 -B scripts/adapt_wir_local_key_report.py \
  docs/archive/analysis-wir-local-key-path-development-v1.json \
  docs/archive/analysis-wir-local-key-holdout-2-v1.json /tmp/local-key-labels.json
python3 -B scripts/evaluate_analysis_labels.py /tmp/local-key-labels.json \
  --output /tmp/local-key-label-report.json
```

The rerun commands for the sweep and scorer need the pinned When in Rome
files, music21 10.5.2 and the pinned binary:

```sh
python3 -B scripts/diagnose_wir_local_key_path.py WIR_DIR CLEAN_DIR \
  target/release/acorde docs/archive/analysis-wir-local-key-path-development-v1.json --check-rust
python3 -B scripts/score_wir_local_key_holdout_v2.py \
  docs/archive/analysis-wir-local-key-holdout-2-protocol-v1.json \
  docs/archive/analysis-wir-local-key-holdout-2-selection-v1.json \
  docs/archive/analysis-wir-local-key-holdout-2-inputs-v1.json \
  docs/archive/analysis-wir-local-key-holdout-2-preflight-v1.json \
  CLEAN_DIR WIR_DIR target/release/acorde docs/archive/analysis-wir-local-key-holdout-2-v1.json
```
