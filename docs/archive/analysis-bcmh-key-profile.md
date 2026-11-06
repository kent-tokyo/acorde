# Opt-in tonal-profile ranking on BCMH (2026-11-07)

This follows the [fixed BCMH baseline](analysis-bcmh-key-baseline.md). The
same 100 annotated Bach chorales, cleaned `**kern` inputs, Acorde parser,
music21 10.5.2, and primary denominator are used. The only Acorde change is
`acorde analyze INPUT --rank-tied-keys`: it orders the legacy equal-coverage
key candidates by Pearson correlation between duration-weighted pitch-class
counts and Krumhansl-Kessler major/minor profiles. The default analysis result
and candidate set remain unchanged. The existing `confidence` is still
diatonic coverage, not the correlation or a calibrated probability.

| Explicit-key scores parsed by both with equal note/chord event count | Baseline Acorde first | Profile-ranked Acorde first | music21 single key |
| --- | ---: | ---: | ---: |
| 82 | 46 correct | 76 correct | 76 correct |

The [per-score JSON](analysis-bcmh-key-profile.json) records input and Acorde
binary hashes, parser failures, note/chord event counts, and every ordered candidate.
Acorde parsed 90/100 inputs; music21 parsed 98/100. Only 88 of the 90 paired
parses had equal event counts, and only 82 of those had an explicit global-key
interpretation. Gold was present somewhere in Acorde's tied candidates for
78/82, which is a different measure from first-candidate agreement.
The unordered pitch/duration event bags match on the same 88 paired parses;
order, placement, rests, pitch spelling and ties are not compared.

This is an **in-sample result**: the 82 scores exposed the relative-major/
minor tie and informed this ranking method. An independently fixed holdout is
required before claiming general global-key parity. This experiment does not
compare harmony, cadence, voice leading, or other analysis tasks. Import
failures and note-count differences remain unresolved.

Reproduce with the archive and Acorde binary versions recorded in the
[baseline](analysis-bcmh-key-baseline.md):

```sh
uv run --no-project --with music21==10.5.2 python3 \
  scripts/compare_bcmh_keys.py BCMH_dataset.zip /path/to/acorde out.json \
  --rank-tied-keys
```
