# When in Rome local keys: paired development diagnostic

The [JSON report](analysis-wir-local-key-paired-development-v1.json) and
[position rows](analysis-wir-local-key-paired-development-v1-rows.jsonl) compare
the opt-in Acorde physical-measure local-key window with [music21 10.5.2's
`floatingKey.KeyAnalyzer`](https://music21.org/music21docs/_modules/music21/analysis/floatingKey.html)
on the same 24 **inspected development works**. It reuses the 3,405 aligned
RomanText positions and answer-free prepared MXL input hashes from the two
existing harmony-development reports. No third-cohort local-key labels were
used. This is a development comparison, not a frozen holdout.

The Acorde radius is four measures on each side, selected by the previous
development sweep. music21 uses its native measure-local analysis and
`smoothInterpretationByMeasure`; the comparator checks the same radius grid
`0, 1, 2, 3, 4, 6, 8, 12` with a fresh analyzer cache for each radius.
music21 10.5.2's smoothing
cache changes on use, so reusing one analyzer across radii changes the answer.
The comparator records both the 10.5.2 implementation hash and its own source
hash. Each score is SHA-256 checked and scanned for embedded harmony answers
before either program analyzes it.

| Development metric | Acorde radius 4 | music21 default radius 4 | music21 best grid radius 2 |
| --- | ---: | ---: | ---: |
| Exact local-key label | 2,325 / 3,405 | 1,590 / 3,405 | 2,157 / 3,405 |
| Pitch class and mode | 2,336 / 3,405 | 1,691 / 3,405 | 2,286 / 3,405 |
| Available at annotated positions | 3,405 / 3,405 | 3,401 / 3,405 | 3,401 / 3,405 |

On the 3,401 positions available to both tools, Acorde has 2,323 exact labels
and music21's grid-best radius has 2,157. Four music21 positions have no
estimate because the score's corresponding measures contain no notes; its
native smoother omits these measures from its return list. The comparator
restores the original measure indices using the raw-key list instead of
shifting later answers. No work failed to parse. Acorde wins 12 works,
music21 at radius two wins 10, and two tie. The aggregate lead is therefore
not a per-work dominance claim.

Both selected radii were chosen on these same development annotations. The
scores were previously inspected for harmony, and the annotation positions
are not untouched. Both methods return one key per physical measure, while
79 of the 1,917 annotated measures contain more than one human local key
(251 positions). Acorde currently uses written pitch classes and the first
staff's written key signature to break enharmonic ties. Thirteen gold labels
use spellings outside its `-7..=7` key-signature candidate set. A final
comparison needs a new fixed local-key holdout, source-beat alignment for
within-measure changes, and explicit transposition and alternate-bar checks.

Reproduce this development comparison with the prepared, hash-verified input
copies and music21 10.5.2 on Python's import path:

```sh
python3 -B scripts/compare_wir_local_keys_music21.py \
  --cohort docs/archive/analysis-wir-harmony-development-v5.json \
  docs/archive/analysis-wir-harmony-holdout-selection-v1.json \
  /private/tmp/acorde-wir-holdout \
  --cohort docs/archive/analysis-wir-harmony-development-2-v2.json \
  docs/archive/analysis-wir-harmony-holdout-2-selection-v1.json \
  /private/tmp/acorde-wir-holdout-2-clean \
  --acorde target/debug/acorde \
  --output docs/archive/analysis-wir-local-key-paired-development-v1.json
```

The temporary input paths may be replaced by copies with the same verified
bytes. The script refuses any music21 version other than 10.5.2.
