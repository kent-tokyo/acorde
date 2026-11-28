# Mozart cadence labels: Acorde and music21 recipe (#502)

Both tools are scored at the same eligible positions in the 36 Batik Mozart
movements: part-0 note onsets of the projected cadence CSV, minus each
movement's preflight unequal-timeline measures. Gold PAC and IAC count as
Authentic, HC as Half, DC as Deceptive and EC as Evaded. Both tools predict
only Authentic cadences, so recall is bounded by the PAC and IAC share of the
gold: 170 of 333 in development and 250 of 431 in the holdout.

| Split (18 movements each) | Tool | Gold | Predicted | Correct | Precision | Recall | F1 |
| --- | --- | ---: | ---: | ---: | ---: | ---: | ---: |
| Development | Acorde | 333 | 59 | 30 | 0.508 | 0.090 | 0.153 |
| Development | music21 recipe | 333 | 59 | 27 | 0.458 | 0.081 | 0.138 |
| Holdout | Acorde | 431 | 59 | 30 | 0.508 | 0.070 | 0.122 |
| Holdout | music21 recipe | 431 | 70 | 44 | **0.629** | **0.102** | **0.176** |

The numbers come from [`evaluate_analysis_labels.py`](../../scripts/evaluate_analysis_labels.py)
([report](analysis-mozart-cadence-label-evaluation-v1.json)) through
[`adapt_mozart_cadence_report.py`](../../scripts/adapt_mozart_cadence_report.py).
Both tools parsed all 36 movements. On the holdout, the five preflight
unequal-timeline measures in three movements remove two of the 433 projected
cadences, which leaves 431. Every correct prediction of either tool is a
PAC, and no prediction landed outside an eligible position.

## The pinned music21 recipe

music21 10.5.2 has no cadence detector, so
[`compare_mozart_cadence_tools.py`](../../scripts/compare_mozart_cadence_tools.py)
pins a recipe, `music21-dominant-tonic-outer-voices-v1`, that applies the
conditions of Acorde's opt-in vertical rule with music21 calls only:

1. Estimate one key with `score.analyze("key")`.
2. Label each chord from `score.chordify()` with
   `roman.romanNumeralFromChord(chord, key).figure`.
3. Mark a plain I chord (`I`, `I6`, `I64`; nothing applied) as Authentic when
   the nearest of the previous eight chords with another figure is at most one
   measure earlier and is a plain V. The lowest pitch must also rise five
   semitones modulo 12, and the top pitch of the I chord must share the bass
   pitch class.

The recipe and Acorde therefore differ in their chord labelling and key
estimate, not in the cadence rule. The recipe was written to mirror that rule
and was not tuned on the development labels.

## Holdout procedure

The [protocol](analysis-mozart-cadence-protocol-v1.json) pins the selection,
prepared inputs, preflight, development report, binary, every script and
music21 10.5.2. It was committed before
[`project_mozart_cadence_holdout.py`](../../scripts/project_mozart_cadence_holdout.py)
read the 18 reserved CSVs. That script refuses to run if any pin differs. The
[annotation hashes](analysis-mozart-cadence-holdout-hashes-v1.json),
[projected positions](analysis-mozart-cadence-holdout-positions-v1.json) and
[holdout comparison](analysis-mozart-cadence-comparison-holdout-v1.json) were
produced once, after the freeze. The
[development comparison](analysis-mozart-cadence-comparison-development-v1.json)
uses the existing development projection. The protocol sets no superiority
gate. Under its stop rule the 18 reserved movements are now development data.

These are rule-family diagnostics. Neither tool detects half, deceptive or
evaded cadences at these positions, and neither result supports a claim of
general cadence analysis.
