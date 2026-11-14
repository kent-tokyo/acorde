# BCMH source-positioned harmony labels (development rerun, 2026-11-14)

The [complete per-row report](analysis-bcmh-harmony-labels-v4.json) retains all
4,627 accepted source annotation positions, raw gold labels, and both systems'
predictions. The [adapter](../../scripts/adapt_bcmh_harmony_report.py) converts
these to the [common label evaluation contract](../analysis-evaluation.md);
its [saved evaluation](analysis-bcmh-harmony-label-evaluation-v1.json) records
per-work results and the verified-input flags.
The earlier [contract 3 report](analysis-bcmh-harmony-music21.json) retained only
bounded error examples and cannot reconstruct this manifest.

The run used the 100-score [BCMH archive](https://github.com/MusicCogCompLab/BCMH/tree/97dfc249b90a8739457d83a2702644173c7eb4a8) at
`MusicCogCompLab/BCMH@97dfc249b90a8739457d83a2702644173c7eb4a8`, with
inner `BCMH_dataset.zip` SHA-256
`9aa8ac78052156932a04431fd56d7a217fe4cd07d0dc726dac1b9dbf85870b1c`.
The Acorde CLI built from the current analysis worktree has SHA-256
`1608d86ee68e693f07dafd58177ef86ba33fd02cf1235aff7ad427bce1d660d9`;
music21 is 10.5.2. The comparator gives both systems the same stripped
`**kern` input, independently estimates their global keys, and checks imported
pitched positions before scoring Roman numerals at source annotation onsets.

| Check | Acorde | music21 |
| --- | ---: | ---: |
| Parsed scores | 98/100 | 98/100 |
| Canonical Roman matches on 4,627 aligned gold rows | 2,189 | 2,073 |
| Predictions at aligned rows | 4,291 | 4,627 |
| Precision / recall / F1 in the common evaluator | 0.510 / 0.473 / 0.491 | 0.448 / 0.448 / 0.448 |

Of the 100 scores, 97 have equal pitched timelines and are eligible. The other
three are two shared parse failures and one unequal imported timeline. Within
the 97, 75 measures containing 208 source-labeled rows are excluded by the
strict source-versus-imported-onset rule. The common evaluator verified the
SHA-256 of all 100 stripped musical inputs when run with `--input-dir`; its
three failed cases include the timeline exclusion and must not be described
as three parse failures. Across the 97 scored works, Acorde has more matches on
48, music21 on 35, and 14 are tied.

The adapter applies only the comparator's narrow `%`/`ø`, `°`/`o`, and terminal
`42`/`2` normalization. The task is *Roman label at a source-annotated onset*;
it does not measure unsolicited harmony labels elsewhere. The source work list
has been used during development. These counts show a development-corpus lead
of 116 matches on this task, not an untouched holdout result or overall
music21 analysis superiority. Cadence, local-key, and voice-leading labels
still need independent gold and separate adapters.

To reproduce the projection after obtaining the pinned archive and a CLI with
the recorded SHA-256:

```sh
uv run --no-project --with 'music21==10.5.2' python3 -B \
  scripts/compare_bcmh_harmony_music21.py \
  /path/to/BCMH_dataset.zip /path/to/acorde /tmp/bcmh-harmony-v4.json
python3 -B scripts/adapt_bcmh_harmony_report.py \
  /tmp/bcmh-harmony-v4.json /path/to/BCMH_dataset.zip \
  /tmp/bcmh-harmony-manifest.json --input-dir /tmp/bcmh-cleaned-inputs
python3 -B scripts/evaluate_analysis_labels.py \
  /tmp/bcmh-harmony-manifest.json --output /tmp/bcmh-harmony-evaluation.json
```

The adapter rejects contract 3, checks every stripped input hash against the
pinned archive, and verifies that all reported per-score counts reproduce from
the retained positions. The evaluator then reads back every cleaned input
when `--input-dir` is supplied.
