# BCMH vertical-harmony development screening (2026-11-07)

The legacy chord pass groups every pitched note in one voice across a measure.
A new opt-in pass instead gathers notes sounding at each written onset across
parts, staves and voices. It keeps authored chord symbols and exposes the
measure, beat, chord and source-note addresses through the Rust API and
`acorde analyze-vertical`. The existing `analyze` result contract is unchanged.

This screening uses the pinned BCMH archive (SHA-256
`9aa8ac78052156932a04431fd56d7a217fe4cd07d0dc726dac1b9dbf85870b1c`)
and the branch CLI (SHA-256
`a1448224bddf79fba69b0e14fbb85de45274a62b4019d239271c53704c22307e`).
The comparator removes the annotated `**harm` spine and tonic/mode
interpretations before Acorde reads the music. It then compares predictions
with the held-aside source Roman labels. The two [per-score reports](analysis-bcmh-vertical-written.json)
and [inferred-key report](analysis-bcmh-vertical-inferred.json) retain every
parse failure, per-score alignment counts and bounded examples of row-count
and label errors.

| Development-corpus check | Written key | Independently inferred key |
| --- | ---: | ---: |
| Successfully parsed scores | 98/100 | 98/100 |
| Source Roman-labeled rows in parsed scores | 4,937 | 4,937 |
| Measures with equal source-row and predicted-sonority counts | 1,150 | 1,150 |
| Measures excluded for unequal counts | 266 | 266 |
| Labeled rows within equal-count measures | 3,943 | 3,943 |
| Matching Roman labels in those rows | 1,498 (38.0%) | 1,916 (48.6%) |

The inferred-key run uses the first Aarden-Essen global-key candidate. It does
not read the answer labels, but this same BCMH corpus previously informed key
estimation development. Terminal source punctuation such as a fermata semicolon
is removed before string comparison. Other notation variants, such as `%` and
`ø` for a half-diminished chord, remain unequal. A measure is scored only if
the number of source data rows equals the number of Acorde sonorities; the
comparison then pairs them in order. Equal counts alone do **not** prove equal
onset times. The 266 excluded measures and the two parse failures are reported
separately; they are not in the 3,943-label exact-match denominator. Neither
result is a music21 comparison or a frozen
holdout accuracy claim.

On `chor001_rev.krn`, the inferred-key run matches 54 of 55 labeled rows in
equal-count measures. The remaining label is `V7/IV`, predicted as `I7`;
secondary dominants are not modeled in this pass. The score also has one
unequal-count measure. Across the corpus, missed chord templates, local key
changes, tonicization and Roman-figure conventions remain visible errors.
Treating the bass of a minor-key dominant inversion separately from the
natural-minor scale raised the inferred-key count from 1,860 to 1,916 on this
same development subset.

Reproduce with the pinned archive and a CLI built from this change:

```sh
python3 -B scripts/compare_bcmh_vertical_harmony.py \
  /path/to/BCMH_dataset.zip /path/to/acorde /tmp/bcmh-written.json
python3 -B scripts/compare_bcmh_vertical_harmony.py \
  /path/to/BCMH_dataset.zip /path/to/acorde /tmp/bcmh-inferred.json --infer-key
```

Next, verify actual source-row onset alignment, compare music21 on the same
written sonorities, and evaluate secondary dominants and local key changes on
separate annotated holdouts. The print and playback parity gates remain
independent of this analysis screening.
