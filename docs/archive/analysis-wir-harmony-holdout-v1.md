# When in Rome harmony holdout, first frozen run

Historical first run. The [v2 report](analysis-wir-harmony-holdout-v2.md)
excludes six labels at three duplicate source positions. A later
[timing correction](analysis-wir-harmony-development-v3.md) shows that 113
additional exclusions here came from the comparator's decimal accumulation.
The current comparison script reproduces v3.

The [selection](analysis-wir-harmony-holdout-selection-v1.json) was fixed from
the complete Git tree at `MarkGotham/When-in-Rome@1c61fe41b8c2910296d7d2bcbf6476c7c1f2fe35`
before reading score or gold contents or running either analyzer. The
[file-hash record](analysis-wir-harmony-holdout-hashes-v1.json) verifies each
download against its Git blob and records SHA-256 for the 12 `score.mxl` and 12
`analysis.txt` files. Musical inputs stay outside this repository. None of the
12 MusicXML scores has a `<harmony>` element. This run is the first evaluation
of these works; no engine change was made after seeing its predictions.

The [complete per-work and per-label report](analysis-wir-harmony-holdout-v1.json)
used Acorde `target/debug/acorde` SHA-256 recorded in that JSON and music21
10.5.2. Both tools analyzed the same `score.mxl` bytes. The gold positions and
Roman figures came from human `analysis.txt` files parsed with music21's
RomanText reader. That reader is a convenience and a possible format affinity
for music21; the gold annotations themselves were authored independently of
this Acorde run. Each analyzer estimated its own global key. music21 formed a
Roman numeral from sounding notes at each gold position; Acorde used its
`analyze-vertical --infer-key` sonority at the exact position. The comparison
accepts only whitespace, `%`/`ø`, `°`/`o`, and `42`/`2` inversion spelling as
equivalent. Local gold keys, accidentals, applied chords, and other figures are
not normalized away.

| Measure | Acorde | music21 10.5.2 |
|---|---:|---:|
| Scores parsed | 12/12 | 12/12 |
| Predictions at 1,052 aligned gold positions | 509 | 1,050 |
| Correct canonical Roman figures | 252 | 310 |
| Correct / aligned gold positions | 24.0% | 29.5% |
| Correct / predictions | 49.5% | 29.5% |

There were 1,168 source labels. Another 116 were excluded before scoring: 113
in measures where the two imports had unequal pitched note timelines, and 3
where a source measure could not be mapped uniquely. The 113 include all 106
labels of Schubert's *Wohin* because its imports disagreed in every annotated
measure. Per-work results and exclusion reasons remain in the JSON. Both the
scored count and excluded count are necessary when interpreting this result.

music21 wins this particular held-out exact-position Roman-figure recall by
58 labels. Acorde emits fewer labels and has higher precision conditional on
emitting one. This is a demanding harmonic-span task: a human may mark a chord
at a beat where the notation sounds only the opening note of an arpeggio.
Neither result establishes general harmonic-analysis quality. Acorde does not
meet the planned music21 parity gate on this task. If these works inform a
code fix, they become development data and a new untouched holdout must be
frozen before a new superiority claim.

Reproduce the current v3 run with the pinned files downloaded and verified
by `scripts/fetch_wir_harmony_holdout.py`:

```sh
PYTHONPATH=/path/to/music21-10.5.2 python3 -B \
  scripts/compare_wir_harmony_music21.py \
  docs/archive/analysis-wir-harmony-holdout-selection-v1.json \
  docs/archive/analysis-wir-harmony-holdout-hashes-v1.json \
  /path/to/verified-holdout target/debug/acorde /tmp/wir-report.json
```
