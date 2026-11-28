# Melody composition

`compose_melody(&MelodyCompositionRequest)` writes an eight-bar (or longer) melody from a
key, a chord progression, a phrase form and an optional one-bar motif. It is behind the
`unstable-arrangement` feature of `acorde-core` and carries no SemVer promise
([#497](https://github.com/kent-tokyo/acorde/issues/497)).

The result is a one-part score that the existing arrangers can accompany, and a report
that counts what the composition rules achieved.

## Request

| Field | Meaning |
| --- | --- |
| `key` | `fifths` -7..=7 and `mode` `major` or `minor` |
| `time_numerator`, `time_denominator` | 2/4, 3/4, 4/4 or 6/8 |
| `measures` | A positive multiple of 8, at most 128 |
| `form` | `period` or `sentence` |
| `chords` | `{measure, offset_eighths, chord}` in time order; the first starts at 0/0 |
| `motif` | Optional `{rhythm_eighths, steps}` filling one bar; derived from `seed` when absent |
| `low_midi`, `high_midi` | Range of at least an octave |
| `max_leap_semitones` | 2..=12 |
| `seed` | The same request and seed always give the same melody |

`motif.steps[i]` is the move in scale steps from the previous note to note `i`. Rhythm
values are 1, 2, 3, 4, 6 or 8 eighths.

## Form

Each eight-bar unit uses the motif (M), its inversion (I), a fragment (F: the first half
of the motif twice, the second time a step lower) and a cadence bar (C):

| Form | Bars | Bar 4 ends on | Bar 8 ends on |
| --- | --- | --- | --- |
| `period` | M I M C · M I F C | Degree 2, 5 or 7 | Degree 1 |
| `sentence` | M I M I · F F C C | Degree 2, 5 or 7 | Degree 1 |

## Rules

- Notes on strong beats (beats 1 and 3 of 4/4, the dotted-quarter beats of 6/8, beat 1
  otherwise) move to the nearest tone of the chord sounding there.
- Phrase endings move to the nearest target degree that is also a chord tone.
- Other notes follow the scale. In minor, a scale tone a semitone from a chord tone outside
  the scale is replaced by that tone, so a melody over E7 in A minor uses G♯.
- Every note stays in range and within the leap limit. A tritone leap is replaced by the
  nearest permitted tone, except into a forced phrase ending.

## Report

`report` gives the note count, strong-beat notes and how many are chord tones, the lowest
and highest pitch, the largest leap and each phrase ending with its degree, target and
whether it was met.

## Checks

The unit tests compose 4 meters × 2 forms × 16 seeds and require every strong beat on a
chord tone, every phrase ending met, the range and leap limit kept, and at most one tritone
leap. `crates/core/tests/melody_composition.rs` arranges 64 composed melodies (major and
minor) with a held pad and a root bass and runs [arrangement assessment](arrangement-assessment.md):
no melody masking and no out-of-range notes. Weak-beat passing tones still form semitone
clashes with the held pad (32,400 ticks across the 64 arrangements, about one beat per
arrangement); the composer does not see the accompaniment voicing.
