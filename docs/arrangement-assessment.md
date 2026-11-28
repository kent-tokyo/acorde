# Automatic arrangement assessment

`assess_arrangement(&Score, &ArrangementAssessmentRequest)` counts audible problems in a
finished arrangement. It is behind the `unstable-arrangement` feature of `acorde-core`
and carries no SemVer promise ([#497](https://github.com/kent-tokyo/acorde/issues/497)).

The counts are deterministic signals for regression tests and for filtering candidates
before listening. They are not a quality score and do not replace listening review.

## What it reads

The score is read as it plays with default playback options: repeats are expanded and
every part is included, source and generated parts alike. Positions are in ticks of
480 per quarter-note beat, plus the physical source measure.

- **Melody line:** the highest sounding pitch of `melody_part`.
- **Bass line:** the lowest sounding pitch of `bass_part`, or of every non-melody part
  when `bass_part` is `None`.
- **Part lines:** the highest sounding pitch of each part.

## Counts

| Field | Meaning |
| --- | --- |
| `melody_masked_ticks` | Another part sounds above the melody. |
| `melody_semitone_clash_ticks` | Another part sounds a minor second or minor ninth below the melody, or a semitone class above it. A major seventh below is not counted. |
| `outer_parallel_fifths`, `outer_parallel_octaves` | Melody and bass both move and keep a perfect fifth or octave (compound intervals included). |
| `part_parallel_perfects` | The same, between the top lines of any two parts. Unison doublings are excluded. |
| `out_of_range_notes` | Notes outside a requested `ranges` entry. |
| `bass_leaps`, `max_bass_leap_semitones` | Bass moves larger than `max_bass_leap_semitones` (default 12), and the largest move. |

Parallel motion needs both lines to sound in adjacent spans; a rest breaks it. Masking
also counts source parts other than the melody, so a second source part written above
the melody (the `multipart-4-4` corpus case) is reported as masking.

`findings` lists the first 256 located findings in time order; `findings_truncated` marks
a longer list. Invalid parts, ranges or leap limits return a typed error and no report.

## Corpus baseline

`tests/fixtures/arrangement/assessment-baseline-v1.json` freezes the counts for the eight
corpus v1 cases: the five explicit pad-and-bass profiles and the three unified proposals
for each case (64 results). The test `crates/core/tests/arrangement_assessment_corpus.rs`
fails when any count changes, and checks that deliberately damaged copies (pad raised two
octaves, bass shifted a semitone, bass replaced by the melody a twelfth lower) raise the
matching count. Regenerate the baseline after a reviewed change with:

```sh
ACORDE_PRINT_ASSESSMENT_BASELINE=1 cargo test -p acorde-core --features unstable-arrangement \
  --test arrangement_assessment_corpus -- --nocapture
```

First baseline observations:

- The explicit profiles have no melody clearance, and 33 of 40 mask the melody for part of
  their length. Four mask it for its whole length (`held-open` in three cases, `held-root`
  in one).
- The unified proposals use melody clearance and mask only where the second source part of
  `multipart-4-4` lies above the melody.
- 15 of 24 proposals have no semitone clash; the others clash for 240–960 ticks.
- No result has outer parallel fifths. Outer parallel octaves occur in 8 of 40 profiles and
  2 of 24 proposals.

Each count is a lead for listening, not a verdict: a held suspension or an intended octave
doubling is counted too.
