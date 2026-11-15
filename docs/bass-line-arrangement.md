# Independent bass line

`acorde_core::arrange_bass_line` and the full/viewer WASM export
`arrange_bass_line(score_json, options_json)` append a single-note General MIDI
bass part to a validated score. Existing parts remain unchanged. A caller can
first use `arrange_chordal_pad` and then add bass with the same explicit chords;
the new parts receive separate non-percussion MIDI channels.

`BassLineOptions` supplies `chords`, `pattern`, `midi_program`, `low_midi`,
`high_midi`, and `center_midi`. Chords use `PositionedAccordionChord` and must
start at physical measure 0, offset 0. Positions lie inside the current bar on
a 1/32-quarter-beat grid. The bass supports the chordal pad's twelve chord
qualities, including a valid slash bass. It does not infer chords.

Patterns:

- `root-pulse` plays the bass root at each quarter-note pulse.
- `root-fifth` alternates the bass root and the chord's fifth. For diminished
  and augmented chords, it uses their altered fifth.
- `approach` uses the same alternation, then plays a chromatic tone a semitone
  below the next chord's bass in the final beat of a region at least two beats
  long. It may be outside the current chord; this is an intentional approach
  tone. A shorter region and the final chord use ordinary alternation.

Pulses continue across barlines until the next chord change. A chord change
starts a new root pulse. Pickups and irregular bars end at their actual shared
playback length. The caller chooses a GM program and written register within
that program's range; a required pitch class with no octave inside the range
returns a diagnostic and no score. The result provides `added_part` and note
addresses. `motion_report` counts bass attacks and absolute semitone movement
between successive attacks in written and repeat-expanded playback order. It
also isolates movement at a jump back to an earlier measure. Tied notation
fragments within one pulse count as one attack. These numbers describe movement,
not musical quality. This is a starting bass texture; it does not prove physical
playability or optimize the complete repeated playback path.

Requests are limited to 32 source parts, 512 measures, 8192 source notes,
512 chord changes, 8192 generated events, and 32 quarter-note beats per bar.
The original score and every previously appended part remain unchanged on
success or failure.

```js
const result = JSON.parse(arrange_bass_line(scoreJson, JSON.stringify({
  chords,
  pattern: "approach",
  midi_program: 32, // GM Acoustic Bass
  low_midi: 28,
  high_midi: 60,
  center_midi: 40
})));
if (result.score) render_score_svg(JSON.stringify(result.score), "{}");
```
