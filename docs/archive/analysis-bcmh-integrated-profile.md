# Integrated BCMH global-key rerun (2026-11-07)

This repeats the [pinned 100-chorale BCMH protocol](analysis-bcmh-key-baseline.md)
after Humdrum field recovery and the opt-in tonal-profile ranking were integrated
into main through [PR #408](https://github.com/kent-tokyo/acorde/pull/408).
It uses the same CC BY 4.0 corpus archive (SHA-256
`9aa8ac78052156932a04431fd56d7a217fe4cd07d0dc726dac1b9dbf85870b1c`),
music21 10.5.2, cleaned `**kern` inputs, annotations and comparator. The Acorde
release binary SHA-256 is
`998eccaa25c5848215cbdf87edc88acd241473e9e599e7b537188dc791eded79`.
The [per-score JSON](analysis-bcmh-integrated-profile.json) preserves source and
cleaned-input hashes, parser failures, note/chord event counts, pitch/duration
bags, tied candidates and first choices.

| Gate | Integrated result |
| --- | ---: |
| Corpus scores | 100 |
| Parsed by Acorde / music21 | 98 / 98 |
| Paired parses with equal note/chord event counts | 98 / 98 |
| Paired parses with equal unordered pitch/duration event bags | 98 / 98 |
| Paired parses with explicit tonic/mode annotation | 89 |
| Acorde ranked first choice / music21 single choice equals that label | 83 / 83 |
| Acorde tied candidates contain that label | 85 / 89 |

Nine additional scores use the first harmony key as a fallback label. Including
those gives 84/98 correct first choices for **each** tool, but the 89 explicit
labels are the primary denominator. This is the development corpus used to
design the ranking rule; matching music21 here is not independent accuracy
evidence. The unordered note bags do not verify order, parts, voices, measure
placement, rests, pitch spelling or ties. The separate
[Mozart sonata holdout](analysis-mozart-key-holdout.md) remains 50/53 for Acorde
versus 51/53 for music21, with one MusicXML export failure. Harmony, cadence
and voice-leading accuracy remain unmeasured on independent labeled corpora.

Reproduce from the pinned archive and a binary built from PR #408:

```sh
uv run --no-project --with music21==10.5.2 python3 -B \
  scripts/compare_bcmh_keys.py /path/to/BCMH_dataset.zip \
  /path/to/acorde /tmp/bcmh-integrated-profile.json --rank-tied-keys
```
