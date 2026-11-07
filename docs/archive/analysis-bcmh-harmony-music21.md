# BCMH harmony comparison with music21 10.5.2 (2026-11-07)

This is a **development-corpus** comparison of vertical Roman-numeral labels.
It is not an untouched holdout, a cadence or voice-leading evaluation, or a
claim of general analysis parity. The BCMH chorales were used during Acorde's
key and harmony development.

The [comparator](../../scripts/compare_bcmh_harmony_music21.py) uses the pinned
100-score BCMH archive (SHA-256
`9aa8ac78052156932a04431fd56d7a217fe4cd07d0dc726dac1b9dbf85870b1c`),
music21 10.5.2, and Acorde CLI SHA-256
`a1448224bddf79fba69b0e14fbb85de45274a62b4019d239271c53704c22307e`.
It removes the answer `**harm` spine and tonic/mode interpretations from each
score before **both** programs read the same `**kern` music. Each program
independently estimates the global key; all 97 structurally equal scores
produce the same key in both programs. Acorde uses `analyze-vertical
--infer-key`; music21 uses `score.analyze("key")`, sounding pitches at each
beat, and `roman.romanNumeralFromChord`.

The comparator derives each source annotation's beat directly from its
`**kern` reciprocal durations. It compares the two imported pitched-event
timelines by part, measure, onset, pitch and duration, then accepts a measure
only when the source data-row beats exactly equal music21's pitched onset
beats. Unsupported or contradictory source rhythm, rest-only annotation rows,
and other onset mismatches remain excluded with a recorded reason. Acorde
missing labels count as misses within accepted measures. The
[per-score JSON](analysis-bcmh-harmony-music21.json) records hashes, parse and
alignment results, and bounded error examples.

| Check | Result |
| --- | ---: |
| Both programs parsed | 98/100 scores |
| Equal pitched-event timelines | 97/98 paired scores |
| Source-beat-aligned measures | 1,305 |
| Excluded measures in those 97 scores | 75 |
| Labeled source rows in accepted measures | 4,627 |
| Acorde labels produced | 4,245/4,627 |
| music21 labels produced | 4,627/4,627 |
| Exact source-label matches, Acorde / music21 | 2,107 / 1,968 |
| Matches after narrow equivalent-figure normalization | 2,155 / 2,073 |

The normalization treats `%` and `ø`, `°` and `o`, and terminal `42` and `2`
as equivalent spellings. It does not collapse different functions such as
`I7` and `V7/IV`. On the accepted rows, 1,959 normalized matches are common
to both tools; 196 are Acorde-only and 114 music21-only. Across scores,
Acorde has more matches on 47, music21 on 35, and 15 are tied. The 82-label
aggregate advantage is small relative to the task and development exposure;
it does not establish an independent accuracy lead. Acorde's 382 missing
labels and errors in secondary dominants/local tonicization remain concrete
work.

Of the 75 excluded measures, 48 have source beats different from the pitched
onset set, 14 have a new source event before a previous duration ends, seven
change spine count, three cross a barline, two have unsupported durations, and
one has a null token without active duration. Some source rows may mark a
rest-only change; this strict vertical-onset contract intentionally excludes
them. `chor011` has a malformed barline and fails the equal-timeline check;
`chor086` and `chor101` fail to parse in both tools. These exclusions and the
unscored 208 source-labeled rows prevent treating 4,627 as all annotated rows.

Reproduce with the pinned archive and the CLI from the opt-in vertical-harmony
change:

```sh
uv run --no-project --with 'music21==10.5.2' python3 -B \
  scripts/compare_bcmh_harmony_music21.py \
  /path/to/BCMH_dataset.zip /path/to/acorde /tmp/bcmh-harmony-music21.json
python3 -B -m unittest discover -s scripts/tests \
  -p 'test_compare_bcmh*.py'
```

Next, expand labels beyond simultaneous-note onsets, reduce Acorde's missing
labels, and assess secondary dominants and local key changes. Freeze new,
independently annotated harmony, cadence and voice-leading holdouts before
using any of these results for the music21 parity gate.
