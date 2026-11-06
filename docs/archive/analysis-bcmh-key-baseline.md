# Bach chorale global-key baseline against music21 10.5.2 (2026-11-07)

This is a bounded comparison on the [Bach Chorales Melody-Harmony Corpus
(BCMH)](https://github.com/MusicCogCompLab/BCMH/tree/97dfc249b90a8739457d83a2702644173c7eb4a8).
Its 100 annotated chorales are published under [CC BY 4.0](https://github.com/MusicCogCompLab/BCMH/blob/97dfc249b90a8739457d83a2702644173c7eb4a8/README.md).
Cite Sears, Verbeten and Percival (2023) as requested by the corpus authors.
The archive SHA-256 is
`9aa8ac78052156932a04431fd56d7a217fe4cd07d0dc726dac1b9dbf85870b1c`.
The scores themselves are not committed here.

The comparator is `scripts/compare_bcmh_keys.py`. For each score it reads the
corpus's key annotation, removes the leading annotation spine and all
tonic/mode interpretations from the four `**kern` music spines, then sends
the same cleaned note file to Acorde v1.3.10 (`d5b47b4`) and music21 10.5.2.
The key-signature interpretation stays because it affects the sounding pitch
of notes without written accidentals. Both estimators operate on the notes;
neither receives the corpus's tonic/mode label. The script records source and
cleaned-input hashes, both tools' note counts, all Acorde tied candidates,
and each tool's parse error separately. The [full result JSON](analysis-bcmh-key-baseline.json)
also records the exact Acorde binary hash.

## Result

| Gate | Count |
| --- | ---: |
| Corpus scores | 100 |
| Parsed by music21 / Acorde | 98 / 90 |
| Parsed by both | 90 |
| Same imported note count | 86 of 90 |
| Same note count and explicit global-key interpretation | 80 |
| music21 single key equals annotation on those 80 | 74 |
| Acorde first key candidate equals annotation on those 80 | 44 |
| Acorde candidate list contains annotation on those 80 | 76 |

The last two Acorde rows measure different things. All 90 mutually parsed
scores produced tied Acorde candidates (88 had two; two had four), so
candidate inclusion is not a single-key accuracy result. The existing
duration-weighted diatonic coverage rule cannot distinguish relative major
and minor keys that share the same notes. Eight scores parsed only in music21;
two parsed in neither tool after the minimal annotation removal. Four of the
90 mutually parsed scores had different imported note counts and are excluded
from the primary 80-score comparison. Nine corpus scores lack a plain
tonic/mode interpretation; their first harmony key was recorded separately
and excluded from the primary comparison.

This corpus measures global-key estimation on Bach chorales only. It does not
measure harmony, cadence, voice leading, calibrated confidence, or general
music-analysis quality. The 80-score set was used to expose the current gap;
future algorithm changes need a separately fixed holdout corpus.

## Reproduction

Build `acorde-cli` from `d5b47b4`, obtain the archive from the pinned BCMH
commit above, and run:

```sh
uv run --no-project --with music21==10.5.2 python3 \
  scripts/compare_bcmh_keys.py BCMH_dataset.zip /path/to/acorde out.json
```

The script rejects an archive with a different SHA-256 or score count.
