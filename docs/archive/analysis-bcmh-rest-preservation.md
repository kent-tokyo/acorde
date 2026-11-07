# BCMH Humdrum rest preservation (2026-11-07)

This reruns the [part and measure audit](analysis-bcmh-structure-audit.md)
after changing the Humdrum importer to leave underfull active voices underfull.
The earlier importer added hidden rests at the end of each short voice to fill
the longest part in a bar. The pinned source contains no rest tokens at those
positions. The revised importer still uses hidden rests to align a newly split
voice with its already elapsed onset, and keeps the shared bar length for the
next measure.

The pinned BCMH archive SHA-256 is
`9aa8ac78052156932a04431fd56d7a217fe4cd07d0dc726dac1b9dbf85870b1c`.
The revised Acorde CLI SHA-256 is
`4936fca3ac52df582b74bb1f17f8305a8e5ec3ca01ad4908f74339cc44962de8`.
music21 is 10.5.2. The [per-score result](analysis-bcmh-rest-preservation.json)
preserves input hashes, both parsers' outcomes and the structural flags.

| Paired successful parses | Before | After |
| --- | ---: | ---: |
| Matching pitch and duration bags | 98/98 | 98/98 |
| Matching pitched onsets by part and measure | 97/98 | 97/98 |
| Matching visible-rest onsets by part and measure | 97/98 | 97/98 |
| Matching all-rest onsets, including hidden rests | 81/98 | 97/98 |
| Explicit-label global keys selected by Acorde / music21 | 83/89 each | 83/89 each |

All 16 changed scores previously differed only by Acorde-generated trailing
hidden rests; no music21-only rest was found in those cases. `chor011_rev.krn`
still differs in part/measure structure because the source omits one barline
field. Agreement here does not establish absolute timing across bars, voice
identity, pitch spelling, tie semantics, or general analysis parity.

Reproduce from the pinned archive with an Acorde CLI built from this change:

```sh
uv run --no-project --with music21==10.5.2 python3 -B \
  scripts/compare_bcmh_keys.py /path/to/BCMH_dataset.zip \
  /path/to/acorde /tmp/bcmh-rest-preservation.json --rank-tied-keys
```
