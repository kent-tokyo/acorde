# Chord symbols from written accompaniment

`acorde_core::annotate_from_authored_chords(target, source, options)` adds display
symbols to a **new** `Score`. Its source must contain written, simultaneous
three-note (or larger) lower-part chords. A melody alone never supplies inferred
harmony. The function returns the new score, the number of additions, source and
target `NoteAddr` values for each symbol, and diagnostics for missing or
unplaceable material. The input scores are unchanged.

The viewer WASM export has the same semantics:

```js
const result = JSON.parse(acorde.annotate_authored_chords(
  targetScoreJson, accompanimentScoreJson, "{}",
));
const scoreForDisplay = JSON.stringify(result.score);
```

`AuthoredChordOptions` selects source and target part, staff, and voice using
zero-based indices. By default, source is part 2 / staff 1 / voice 1 and target
is part 1 / staff 1 / voice 1. The same score can be passed twice for a combined
arrangement; a companion arrangement can annotate a separate melody score when
their measure counts and numbers agree.

## Placement policy

- A source rest, grace or cue note, hidden note, or note with fewer than three
  simultaneous pitches provides no chord symbol. Only recognized major, minor,
  dominant, diminished, and augmented chords are used.
- Each voice is timed within its measure using written note durations, dots, and
  tuplets. The first recognized harmony in each measure labels the first
  available melody note, including pickup patterns where a bass note precedes
  the written block chord. Later harmony changes go to the first available
  melody note at or after the chord onset. Melody rests, grace and cue notes,
  and hidden notes are not targets.
- Repeated identical root and kind within a measure receive one label until a
  different harmony is placed. An inversion alone does not establish an
  independent slash bass, so an added symbol has no slash bass.
- Existing target chord symbols always remain. If no eligible target note
  remains, the result records `no-target-at-or-after-onset`. Invalid staff,
  voice, measure count, or measure number yields a diagnostic without guessing
  an alignment.

Only `chord_symbol` on target melody notes changes. The written pitches,
durations, ties, repeats, source accompaniment, and playback timing remain as
they were. The returned score can be rendered to SVG or exported to MSCZ.
