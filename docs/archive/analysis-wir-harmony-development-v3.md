# When in Rome harmony development baseline after timing correction

The [v1](analysis-wir-harmony-holdout-v1.md) and
[v2](analysis-wir-harmony-holdout-v2.md) reports treated 113 labels as import
timeline mismatches. Inspection showed that the comparator had accumulated
rounded decimal durations of triplet notes. Acorde and music21 imported the
same pitches, durations, and exact onset fractions in those measures. The
comparator now accumulates Acorde durations as rational numbers and rounds
only the final event positions. The comparison's gold-position and canonical
Roman-label rules are unchanged. The previous reports remain archived as
methodology history; their 113 exclusions must not be cited as import losses.

The [complete v3 report](analysis-wir-harmony-development-v3.json) and
[common-evaluator projection](analysis-wir-harmony-development-label-evaluation-v1.json)
use the same 12 verified MusicXML inputs and primary RomanText annotations
from `MarkGotham/When-in-Rome@1c61fe41b8c2910296d7d2bcbf6476c7c1f2fe35`.
The input and analyzer hashes are recorded in the JSON. Both programs parsed
all 12 scores. The 12 works have been inspected and are **development data**.

| Measure | Acorde | music21 10.5.2 |
|---|---:|---:|
| Predictions at 1,159 aligned labels | 563 | 1,157 |
| Correct canonical Roman figures | 285 | 348 |
| Correct / aligned gold positions | 24.6% | 30.0% |
| Correct / predictions | 50.6% | 30.1% |

There are 1,168 source labels. Nine remain unscored: six at three duplicated
source measure/beat positions and three whose source measure cannot be mapped
uniquely. No pitched-timeline mismatch remains in this cohort. Acorde omits
596 aligned labels; music21 omits two. Acorde's conditional precision is
higher, but music21 identifies 63 more correct labels at the required
positions. Both systems use independently estimated global keys, while the
primary RomanText annotations may change local key. music21's RomanText
reader supplies gold positions and figures, which is a format-affinity
limitation of this comparison. The result does not establish general harmony
quality or superiority.

A separate [12-work selection](analysis-wir-harmony-holdout-2-selection-v1.json)
and [file-hash record](analysis-wir-harmony-holdout-2-hashes-v1.json) were frozen
before the ensuing engine work. Its works have no overlap with this cohort;
the annotation bytes were fetched only for Git-blob and SHA-256 verification.
An input-only inspection found 1,274 embedded `<harmony>` elements in its
three Beethoven quartet scores. A deterministic
[input-preparation record](analysis-wir-harmony-holdout-2-inputs-v1.json)
pins the MusicXML inputs after removing those answer elements. The remaining
nine MXL files retain their original bytes. Both parsers accepted the three
cleaned quartet inputs, but no gold labels or analysis predictions have been
read or scored. This remains the next untouched harmony holdout.
