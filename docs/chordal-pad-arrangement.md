# Chordal accompaniment for an existing score

`acorde_core::arrange_chordal_pad` and the full/viewer WASM export
`arrange_chordal_pad(score_json, options_json)` append one chordal accompaniment
part to a validated score. Existing parts, their note IDs, and playback are
copied unchanged. This API works with a multi-part source; it does not require
an accordion melody reduction.

The caller supplies explicit `PositionedAccordionChord` values. The first
chord must be at physical measure 0, offset 0; later chords may change within
a bar and carry through following bars. A selected proposal's
`preset_selection.chords` or a complete chord plan converted with
`chord_plan_to_preset_selection` can provide them. Positions must be distinct,
inside their measures, and on a 1/32-quarter-beat grid. The generator does
not infer harmony. It supports major, minor, diminished, augmented,
major-seventh, minor-seventh, dominant, suspended-second, suspended-fourth,
major-add9, minor-add9, and half-diminished qualities without degree
alterations. Added ninths use the same four pitch classes in a close pad
voicing; they are not guaranteed to remain a literal ninth above the root.
Slash-bass chords return a diagnostic so an extra or
omitted bass tone is never silently invented.

`ChordalPadOptions` also supplies:

- `pattern`: `held` sustains one chord through each harmony region;
  `quarter-pulse` reattacks it each quarter-note beat; `offbeat-pulse`
  rests for the first half of each beat and strikes the full chord on the
  second half; `arpeggio-up-down` plays one chord tone at a time, rising
  and falling in eighth notes. Every pattern starts at the chord change
  and restarts at a bar boundary. An `offbeat-pulse` region of at most
  half a beat strikes immediately so a very short chord is still heard.
  `meter-pulse` attacks at each metrical group start, accents compound or
  irregular groups, and rests through the second half of each group. A
  chord change inside a group forces an immediate attack; the next group
  start restores the metrical pulse.
- `pattern_sections`: optional half-open physical-measure ranges, each with
  `start_measure`, `end_measure_exclusive`, and `pattern`. They may be supplied
  in any order, but must not overlap or extend beyond the score. Uncovered
  measures use the base `pattern`. Sections change only the texture: chords
  still change at their supplied positions and carry through later bars.
- `meter_group_sections`: optional half-open measure ranges with `groups` in
  denominator units. Each grouping must sum to that measure's numerator;
  sections cannot overlap. Default grouping is 3+3 in 6/8, 3+2 in 5/8,
  2+2+3 in 7/8, three-unit groups in longer compound eighth/sixteenth
  meters, and one denominator unit per group otherwise. A caller can choose
  2+3 for 5/8 or a different authored grouping.
- `phrase_end_measures`: physical measures whose ending barline closes an
  authored phrase. In the last two metrical groups of each marked measure,
  the pad holds its chord instead of repeating the base pattern. A chord
  change within this span still attacks at the supplied position. The
  cadence hold receives an accent. Pickups use the groups that fit within
  their actual length.
- `voicing`: `root-position` (default) keeps the original close root-position
  chords. `smooth` considers close inversions within the selected range and
  chooses the one with the smallest local motion from the preceding chord.
  It preserves every chord tone and the original melody. The first chord stays
  in root position. The choice follows written chord order; repeat-back edges
  and a full-score optimum are not yet part of this option. `open` spreads an
  inner chord tone up one octave and chooses an in-range inversion near the
  target center or preceding chord. It returns a diagnostic if no such spread
  fits the requested range. `global-smooth` starts with close inversions, then
  runs at most eight deterministic coordinate-search passes over the expanded
  playback graph. Repeat-back and navigation edges count; selected melody
  clearance and instrument range remain hard constraints. The report flags a
  pass-limit exit. This bounded search does not promise a global optimum.
- `midi_program`: a General MIDI program from 0 to 127. Choose a polyphonic
  sound for a chord part; the program is a playback timbre, not a claim about
  physical playability or instrument-specific transposition.
- `low_midi`, `high_midi`, and `center_midi`: the allowed written range and
  target center. The range must fit the program's existing GM range table.
  Each chord tone is placed once within it.
  A chord that cannot fit returns no score.
- `min_melody_gap_semitones`: optional 0–24-semitone clearance below the
  selected source melody staff. Every chord tone must remain at or below the
  lowest overlapping melody pitch minus this gap throughout the entire chord
  region, including a later note or measure before the next chord change.
  Without this option, voicing behavior and serialized options stay as before.
- `melody_part_index`: source part containing the melody in its first staff;
  defaults to part 0 when clearance is requested. All voices of that staff
  contribute. It cannot be set without a gap. Grace notes, unpitched notes,
  bends and percussion in the selected staff are not covered by this written
  duration rule and return a diagnostic. An impossible clearance returns no
  score. This constraint does not inspect other source parts or generated
  parts.

The new part receives an unused non-percussion MIDI channel. It copies the
first source staff's measure numbers, meter/key changes, repeat barlines, and
endings. Shared playback bar lengths cover pickups and irregular measures.
If a sustained chord needs multiple notated durations, those fragments are
tied; a pulse starts a new attack. `generated_notes` reports sounding notes,
excluding the written rests in `offbeat-pulse`. The output also reports
`added_part`. It leaves score settings, source parts, views and groups
intact. The caller must review the chosen instrument and voicing: this is a
bounded starting texture, without global voice-leading optimization or a listening
quality score.

`voicings` reports the selected MIDI pitches and, when requested, the melody
ceiling at each explicit chord change. `motion_report` gives symmetric
nearest-tone movement for adjacent written chord changes and for transitions
in expanded playback order. The latter includes repeat-back, volta and
navigation jumps; `backward_jump_motion_cost` isolates transitions at a jump
to an earlier physical measure. An unchanged chord carried into the next bar
does not count as a new change. These are semitone movement measures, not
physical hand distances or musical-quality scores. `smooth` and `open` choose
each voicing from the preceding written chord. `global-smooth` also considers
the expanded playback graph when refining its choices.
For metrical patterns, `resolved_meter_groups` records the grouping selected
in every physical measure. `phrase_start_ticks` records the local 1/32-beat
start of each authored ending. These resolved values make the chosen pulse
and density change inspectable alongside the score.

Requests are limited to 32 source parts, 512 measures, 8192 source notes,
512 chord changes, 8192 generated notated events (including rests), and 32
quarter-note beats per bar.
Invalid source structure, rhythm, channel availability, chord spelling or
register returns structured diagnostics and no partial score.

```js
const result = JSON.parse(arrange_chordal_pad(scoreJson, JSON.stringify({
  chords,
  pattern: "held",
  voicing: "smooth",
  pattern_sections: [
    {start_measure: 4, end_measure_exclusive: 12, pattern: "offbeat-pulse"},
    {start_measure: 12, end_measure_exclusive: 20, pattern: "arpeggio-up-down"}
  ],
  midi_program: 48, // GM String Ensemble 1
  low_midi: 48,
  high_midi: 84,
  center_midi: 64
})));
if (result.score) {
  render_score_svg(JSON.stringify(result.score), "{}");
}
```
