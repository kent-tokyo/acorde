# Harmonic-minor leading-tone labels in the BCMH development corpus (2026-11-07)

This follows the [source-beat BCMH comparison](analysis-bcmh-harmony-music21.md)
with music21 10.5.2. The pinned 100-score archive, notes-only input, global-key
estimation, source-derived beat alignment and narrow label normalization are
unchanged. The two per-score reports are the [baseline](analysis-bcmh-harmonic-minor-before.json)
and [new result](analysis-bcmh-harmonic-minor-after.json). The CLI SHA-256 values
are `a1448224bddf79fba69b0e14fbb85de45274a62b4019d239271c53704c22307e`
before and `f5313afd0c92ae552d414001390fdf14aaad69108c23e289f0a6d27ac578f3a5`
after. Both reports retain the archive/input hashes and per-score details.

The earlier comparator counted an absent Roman label without distinguishing a
missing chord from a chord outside its natural-minor scale map. The revised
comparator records both causes. Before the code change, 210 of the 382 absent
labels were missing sonorities; the other 172 were detected chords with no
Roman label. Of those 172, 121 were diminished triads. The new rule recognizes
a diminished chord rooted a semitone below a minor tonic as a leading-tone
chord and preserves figured-bass inversions. Other chromatic roots are still
unlabeled; it does not infer a local key or secondary function. The existing
slash-bass contract of `roman_numeral` remains unchanged.

| Fixed development-corpus measure | Before | After | music21 |
| --- | ---: | ---: | ---: |
| Parsed scores | 98/100 | 98/100 | 98/100 |
| Scores with equal pitched timelines | 97 | 97 | 97 paired |
| Source-beat-aligned measures | 1,305 | 1,305 | same |
| Source Roman-labeled rows in those measures | 4,627 | 4,627 | same |
| Acorde Roman labels produced | 4,245 | 4,291 | 4,627 |
| Acorde missing sonority / unlabeled chord | 210 / 172 | 210 / 126 | n/a |
| Exact source-label matches | 2,107 | 2,141 | 1,968 |
| Matches after narrow figure normalization | 2,155 | 2,189 | 2,073 |

The change adds labels to 46 rows; 34 match the held-aside source annotation.
Across the 97 eligible scores, 23 gain at least one normalized match and none
lose one. The 12 newly labeled mismatches show why coverage alone is not
accuracy. These chorales previously informed Acorde's analysis work, so the
observed increase is **development-corpus evidence only**. It cannot prove a
music21 lead on untouched scores. Acorde still leaves 336 of these 4,627 rows
without a Roman label, while music21 labels all 4,627. The 75 excluded
measures and three scores without equal paired timelines are unchanged.

Reproduce both reports with the comparator on the baseline and changed CLI
binaries respectively:

```sh
uv run --no-project --with 'music21==10.5.2' python3 -B \
  scripts/compare_bcmh_harmony_music21.py \
  /path/to/BCMH_dataset.zip /path/to/acorde /tmp/bcmh-harmony.json
cargo test -p acorde-core -p acorde-analysis --lib
```

The core and analysis library suites passed (626 and 56 tests). The full
optimized local macOS WASM artifact changed from 7,094,092 to 7,094,476 bytes
(+384) using the same toolchain; Linux CI remains the artifact-budget gate.
Next, separate the 210 pitch sets without a chord template from the 126
recognized but unlabeled chords, and evaluate secondary dominants, local keys,
cadences and voice-leading on independently annotated holdouts.
