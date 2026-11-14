# Mozart cadence development: sonority coverage and transition hypotheses

This diagnostic uses only the already inspected 18-movement development split
of the [Batik Mozart cadence corpus](analysis-mozart-cadence-preflight-v1.md).
The 18 reserved cadence CSV files remain unread. It verifies each prepared
MusicXML hash against the existing projection and preflight, then runs the
current Acorde CLI with `analyze-vertical --infer-key --arpeggiated-bars`.
The [per-position result](analysis-mozart-cadence-sonority-development-v1.json)
records the nearest distinct Roman label within the previous measure.

| Human cadence class | Annotated onsets | Vertical sonority at that onset | Roman label there |
| --- | ---: | ---: | ---: |
| PAC | 157 | 64 | 64 |
| IAC | 13 | 4 | 4 |
| HC | 118 | 66 | 66 |
| DC | 17 | 12 | 12 |
| EC | 28 | 14 | 13 |
| **Total** | **333** | **160** | **159** |

The [separate transition diagnostic](analysis-mozart-cadence-transition-development-v1.json)
tests two explicit hypotheses on all 18,467 eligible development note onsets:
an inferred `I64 → V` predicts a half cadence, and an inferred plain `V → vi/VI`
predicts a deceptive cadence. It maps candidate addresses to the same source
note onsets used by the current cadence scorer.

| Hypothesis | Correct class and position | False positives |
| --- | ---: | ---: |
| `I64 → V` half | 32 / 118 HC | 87 |
| `V → vi/VI` deceptive | 1 / 17 DC | 9 |
| **Combined** | **33 / 333 total cadences** | **96** |

These hypotheses alone have worse precision than the opt-in authentic-cadence
rule's 30 correct and 29 false positives. They are **not adopted** as a
production candidate. The same-position vertical coverage also shows why a
rule that merely classifies the current sonority list cannot recover most
annotated cadence onsets. A later rule can use nearby harmonic context, but
must still place its result at the annotated source note and distinguish a
phrase ending from an ordinary progression. This is development evidence,
not a music21 comparison or a held-out result.
