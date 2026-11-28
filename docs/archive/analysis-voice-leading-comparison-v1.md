# Bach chorale consecutive fifths and octaves: Acorde and music21 (#502)

The gold labels are Luke Dahn's published list of consecutive fifths and
octaves in the Bach chorales
([source](https://www.bach-chorales.com/ConsecutivesInChorales.htm), read
2026-11-28). [`freeze_voice_leading_dahn_labels.py`](../../scripts/freeze_voice_leading_dahn_labels.py)
matched 47 of its 49 rows to music21 10.5.2's bundled chorale files and split
the 41 matched chorales by a seeded hash before either tool ran
([labels](analysis-voice-leading-dahn-labels-v1.json)). The six Dietel
manuscript variants are excluded, and 22.5 and 167.5 have no corpus file.

| Split | Tool | Chorales | Gold | Predicted | Covered | Unlisted | Recall | Precision (lower bound) | F1 |
| --- | --- | ---: | ---: | ---: | ---: | ---: | ---: | ---: | ---: |
| Development | Acorde | 21 | 27 | 26 | 20 | 6 | 0.741 | 0.769 | 0.755 |
| Development | music21 recipe | 21 | 27 | 27 | 21 | 6 | 0.778 | 0.778 | 0.778 |
| Holdout | Acorde | 19 of 20 | 24 | 29 | 9 | 20 | 0.375 | 0.310 | 0.340 |
| Holdout | music21 recipe | 19 of 20 | 24 | 29 | 9 | 20 | 0.375 | 0.310 | 0.340 |

The numbers come from [`evaluate_analysis_labels.py`](../../scripts/evaluate_analysis_labels.py)
([report](analysis-voice-leading-label-evaluation-v1.json)) through
[`adapt_voice_leading_dahn_report.py`](../../scripts/adapt_voice_leading_dahn_report.py).
By interval on the holdout, both tools cover 8 of 18 fifths and 1 of 6
octaves. BWV 251 has six parts in the corpus, so both tools are recorded as
failures there and its label is not counted. The source list does not claim
to be exhaustive, so an unlisted detection is not a confirmed error and
precision is only a lower bound.

## The two detectors

[`compare_voice_leading_dahn.py`](../../scripts/compare_voice_leading_dahn.py)
treats the four corpus parts as S, A, T and B.

* Acorde: every `acorde analyze-voice-leading` quartet flagged
  `parallel_fifth` or `parallel_octave`, with measure numbers from
  `acorde report`.
* music21 (`music21-figuredBass-checker-v1`): music21's own
  `figuredBass.checker.parallelFifths` and `parallelOctaves` on each
  consecutive pair of `extractHarmonies` moments.

Both count antiparallel motion. A detection covers a label when the interval
and voice pair match and either chord's measure number is one the label
lists. The rule was fixed on the development split. The two detectors agree
on 24 of 26–27 development detections and 27 of 30 holdout detections, so
the holdout gives no evidence that either is better.

## Why the holdout recall is low

The matching rule uses the measure numbers of the corpus files. Dahn's
numbers often differ from them. In development, four chorales were off by
four to six measures, which looks like counting repeated measures. In the
holdout, 14 of the 15 missed labels have a detection with the same interval
and voice pair elsewhere in the same chorale, three to six measures (most
often four) before the listed measure. If any such detection counted, recall
would be 23 of 24 for both tools in the holdout and 24 (Acorde) or 25
(music21) of 27 in development. That check was made after scoring and is a
diagnostic, not a result. The frozen numbers above stand.

## Holdout procedure

The [protocol](analysis-voice-leading-comparison-protocol-v1.json) pins the
labels, the [development comparison](analysis-voice-leading-comparison-development-v1.json),
the binary, every script, and music21 10.5.2 with its checker and
voice-leading sources. It was committed before either tool ran on the 20
holdout chorales. The [holdout comparison](analysis-voice-leading-comparison-holdout-v1.json)
was then produced once. BWV 323 was moved from holdout to development before
the freeze, because its notes and both tools' output were looked at while
checking the label parser. The protocol sets no superiority gate. Under its
stop rule the holdout chorales are now development data.

A measure-numbering map between Dahn's edition and the corpus files would
be the next step. It must be built and frozen before any new holdout is
scored.
