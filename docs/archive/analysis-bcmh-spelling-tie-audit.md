# BCMH pitch-spelling and tie audit (2026-11-07)

This extends the [rest-preservation audit](analysis-bcmh-rest-preservation.md)
on the same 100 annotated BCMH chorales. The leading harmony spine and
tonic/mode interpretations were stripped before both parsers read the scores.
The archive SHA-256 is
`9aa8ac78052156932a04431fd56d7a217fe4cd07d0dc726dac1b9dbf85870b1c`.
The Acorde CLI was built from main merge commit `5b78724e` with Rust 1.97.0;
its SHA-256 is
`c50fba03b779ba7ea97ea6ad0cf1bf08691fb2f0ca6ddb0f6e5c7577dba64f6e`.
The comparator used music21 10.5.2. The [per-score report](analysis-bcmh-spelling-tie-audit.json)
records input and binary hashes, parse failures and comparison flags.

For each part and measure, the added check compares multisets of pitched events
by onset, duration, written pitch step, alteration and octave. The tie check
also compares start and end state per written pitch. A music21 `continue` tie
has both states. Chord pitches are sorted so notation order within a chord does
not affect equality. Existing MIDI-pitch and rest checks are retained.

| Check on paired successful parses | Agreement |
| --- | ---: |
| Successful parses | 98/100 in each tool |
| Pitch/duration bags | 98/98 |
| Pitched onsets within part and measure | 97/98 |
| Written spelling within part and measure | 97/98 |
| Written spelling and per-pitch ties within part and measure | 97/98 |
| All-rest onsets within part and measure | 97/98 |
| Explicit-label global keys, Acorde / music21 | 83/89 each |

`chor011_rev.krn` is the sole nonagreement. It fails the older part/measure-count
check because one source barline field is missing, so this comparison cannot
isolate a spelling or tie difference there. The other 97 paired scores have
equal written pitches and tie states at the same local positions.
`chor086_rev.krn` and `chor101_rev.krn` are rejected by both parsers.

This is development-corpus import evidence, not accuracy for harmony, cadence
or voice leading. It does not compare absolute onset positions across measure
boundaries, polyphonic voice identity, source-to-score semantics independent
of music21, or an untouched analysis holdout. The BCMH chorales here mostly
have one voice per part, so a polyphonic corpus is needed for voice placement.

Reproduce with the pinned archive, the stated CLI build and music21 10.5.2:

```sh
uv run --no-project --with music21==10.5.2 python3 -B \
  scripts/compare_bcmh_keys.py /path/to/BCMH_dataset.zip \
  /path/to/acorde /tmp/bcmh-spelling-ties.json --rank-tied-keys
```
