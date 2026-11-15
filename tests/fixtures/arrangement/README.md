# Arrangement corpus v1

`corpus-v1.json` contains eight short, original melodies. Four cases form the
development split and four form the holdout split. Meter, authored note pitches,
explicit chords, repeat, pickup and multi-part flags are fixed. The matching core test
constructs scores, adds a chord part and an independent bass part, and checks
validation plus unchanged source parts and playback.

`baseline-v1.json` freezes the results of five explicit pad-and-bass profiles
for every case: held root-position chords with root bass, offbeat chords with
root/fifth bass, arpeggios with approaching bass, and held open chords with root
bass, plus a grouped pulse with an authored final-measure ending. For each
result it records the source
fingerprint, chord and pattern choices, note-onset and duration distributions,
pitch classes, range, a deterministic fingerprint of playback events, and pad
and bass motion in written and expanded playback order. The
core test compares every result with this file. To inspect a candidate baseline
without changing the fixture, run:

```sh
ACORDE_PRINT_ARRANGEMENT_BASELINE=1 cargo test -p acorde-core --test arrangement_corpus --locked -- --nocapture
```

The holdout split is reserved for final comparison and must not be adjusted to
make an algorithm look better. Its structural invariant and snapshot tests run
in CI; these are not listening review or musical-quality scores. The structural
comparison is recorded in `docs/arrangement-corpus-v1-review.md`.
