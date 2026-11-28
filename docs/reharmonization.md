# Automatic reharmonization candidates

`suggest_reharmonizations(&Score, &ReharmonizationRequest)` lists approach-chord changes to an
existing `ChordPlan` inside an editable measure range. `apply_reharmonizations(&ChordPlan,
&[ReharmonizationCandidate])` writes chosen candidates into a new plan for the existing
arrangers. Both are behind the `unstable-arrangement` feature of `acorde-core` and carry no
SemVer promise ([#497](https://github.com/kent-tokyo/acorde/issues/497)).

The existing [phrase-harmony targets](harmony-targets.md) realize a secondary dominant or
modulation at a position the caller names. This API finds the positions, and adds tritone
substitution and two-five insertion.

## Techniques

For each pair of adjacent plan entries, the second entry is the arrival. It must be a major-
or minor-type chord (a third and a perfect fifth). The first entry is replaced:

| Technique | Replacement | Example |
| --- | --- | --- |
| `secondary-dominant` | V7 of the arrival | E7 before Am, C7 before F |
| `tritone-substitution` | Dominant a semitone above the arrival | Db7 before C, Bb7 before Am |
| `two-five` | ii7 then V7, splitting the span at its midpoint; iiø7 before a minor arrival | Dm7–G7 before C, Bø7–E7 before Am |

A two-five needs a span of at least two beats inside one measure (or up to the next bar
line) on the 1/64-beat grid. A secondary dominant that equals the existing chord is not
proposed. The arrival and every other entry stay unchanged.

An entry is editable only when its whole span lies in `start_measure..end_measure_exclusive`,
touches no `locked_measures`, and is not in `locked_entries`.

## Melody fit and ranking

Each candidate classifies the melody notes under its new chords. A note is a chord tone, an
available tension, or an avoid note. Avoid notes are listed with their measure, offset,
duration in the chord, pitch and chord.

| Chord | Available tensions (semitones above the root) |
| --- | --- |
| Dominant | b9, 9, #9, #11, b13, 13 |
| Minor seventh | 9, 11, 13 |
| Half-diminished | 9, 11, b13 |

Candidates are ranked by avoid-note beats, then avoid-note count, technique and position.
Each candidate also records the root motion into the arrival: +5 for a fifth down, −1 for a
semitone down.

`apply_reharmonizations` rejects a candidate that does not match the plan. It also rejects
two candidates that replace the same entry, or where one replaces the other's arrival.
Written entries are marked `generated-reharmonization` and keep the replaced entry's sources.

## Corpus check

`crates/core/tests/reharmonization_corpus.rs` runs every candidate on the eight corpus v1
cases (26 candidates). For each one, it arranges a held smooth pad with melody clearance and
a root bass. It checks the following, and freezes the counts in
`tests/fixtures/arrangement/reharmonization-baseline-v1.json`:

- Only the replaced entry changes.
- The source parts are unchanged.
- The [arrangement assessment](arrangement-assessment.md) shows no added masking and no
  added outer parallel fifths or octaves.

First observations:

- No candidate adds masking, outer parallels, out-of-range notes or bass leaps.
- 10 of 26 candidates increase the semitone-clash ticks against the melody. 7 of those are
  tritone substitutions.
- 8 of the 24 candidates with no avoid notes still increase semitone-clash ticks. The
  assessment counts a b9 or #9 against the melody as a clash, even though the melody-fit
  table allows it on a dominant.

So a zero avoid-note count is not enough to choose a candidate. Arrange the applied plan
and filter with the assessment, for example through `suggest_arrangements` limits. Listening
review is still needed before calling any candidate an improvement.

Regenerate the baseline after a reviewed change with:

```sh
ACORDE_PRINT_REHARMONIZATION_BASELINE=1 cargo test -p acorde-core --features unstable-arrangement \
  --test reharmonization_corpus -- --nocapture
```
