# Independent bass line

`acorde_core::arrange_bass_line` and the full/viewer WASM export
`arrange_bass_line(score_json, options_json)` append a single-note General MIDI
bass part to a validated score. Existing parts remain unchanged. A caller can
first use `arrange_chordal_pad` and then add bass with the same explicit chords;
the new parts receive separate non-percussion MIDI channels.

`BassLineOptions` supplies `chords`, `pattern`, `midi_program`, `low_midi`,
`high_midi`, `center_midi`, and optional `rhythm_sections`. Chords use
`PositionedAccordionChord` and must
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

`rhythm_sections` adds sparse, nonoverlapping written-measure ranges. Each
section repeats its `slots` in every bar of that range; uncovered bars use the
base pattern. A slot's `ticks` are units of 1/32 quarter-note beat. The slots
must sum to each bar's actual length, including pickups and irregular meters.
`tone: null` is a rest. Sounding tones can be `bass`, `third`, `fifth`,
`next-bass`, `approach-below`, or `approach-above`; `accent: true` marks a
sounding attack. This lets callers specify quarter-note walking motion,
offbeat rests and accents, repeated ostinato cells, and a denser fill in a
phrase-ending bar. `next-bass` and approach tones must begin no more than one
quarter-note beat before the next chord change. A slot crossing a chord change
is split and reattacked using the new harmony. Unsupported tones or lengths
return no score. Generated-note addresses exclude rests.

For example, a 4/4 bar before an F chord can walk through C, E, G and an E
approach tone:

```json
{"start_measure":0,"end_measure_exclusive":1,"slots":[
  {"ticks":32,"tone":"bass","accent":true},
  {"ticks":32,"tone":"third"},
  {"ticks":32,"tone":"fifth"},
  {"ticks":32,"tone":"approach-below"}
]}
```

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
512 chord changes, 8192 generated events, 64 rhythm sections, 64 slots per
section, and 32 quarter-note beats per bar.
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
