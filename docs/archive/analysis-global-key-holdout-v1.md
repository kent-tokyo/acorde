# Three-style global-key holdout: fixed result

The [source selection](analysis-global-key-holdout-selection-v1.md) was
committed as `57ac5bb7`, the [score-only preflight](analysis-global-key-holdout-preflight-v1.md)
as `a0f1e643`, and the [scorer and protocol](analysis-global-key-holdout-protocol-v1.md)
as `2acaefe4`, all before inspecting the selected key-label values. The
[result](analysis-global-key-holdout-v1.json) pins the protocol, candidate
binary, scorer, common inputs, and per-score gold and predictions. Both tools
parsed all twelve answer-free inputs; their pitched timelines agreed in all
496 physical measures before the labels were opened.

| Preregistered measure | Acorde | music21 10.5.2 |
| --- | ---: | ---: |
| Predictions available on eligible scores | 11 / 11 | 11 / 11 |
| Exact tonic spelling and mode | 10 / 11 | 10 / 11 |
| Chorales | 2 / 3 | 2 / 3 |
| Keyboard pieces | 4 / 4 | 4 / 4 |
| Chamber movements | 4 / 4 | 4 / 4 |

The key index has two rows for Riemenschneider chorale 201. The frozen rule
requires a unique row, so this score is excluded even though both rows show
the same key. One exclusion is within the protocol's limit, and each style
retains at least three scores. In chorale 200, the independent key index and
human RomanText opening key agree on C major, while both analyzers select
G minor. The remaining ten eligible scores are correct for both. Prediction
coverage is equal. **The preregistered gate fails** because Acorde does not
exceed music21's exact count.

The result is a tie on this small, selected key task; it does not establish
superiority in global-key analysis or music analysis overall. The eight DCML exports are
complete MusicXML despite MuseScore 4.7.5 returning status `-6` after writing
them. The input preflight checks pitched note timing and MIDI pitches, but not
voice placement, spelling, ties, or the fidelity of each export to its MSCX
source. The candidate, comparator, label projection, and gate were not changed
after reading the selected labels. Any improvement informed by this result
requires a new disjoint holdout.
