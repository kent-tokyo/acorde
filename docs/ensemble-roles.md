# Whole-piece ensemble roles

`acorde_core::arrange_ensemble_roles` and the full/viewer WASM export
`arrange_ensemble_roles(score_json, options_json)` compose an existing score
with independently scheduled chordal-pad, bass and countermelody parts. All
original parts, note IDs and playback events remain unchanged. The returned `role_parts`
identifies the appended parts, and `activity` gives the effective on/off state
in every physical measure.

Supply a `harmony` object using `ChordalPadOptions`, a `bass` object using
`BassLineOptions`, a `countermelody` object using `CountermelodyOptions`, or
any combination. Each supplied role is active by default.
`sections` contains half-open physical-measure ranges with `role`
(`harmony`, `bass` or `countermelody`), `start_measure`,
`end_measure_exclusive`, and `enabled`.
Sections for different roles can overlap; sections for the same role cannot.
An inactive measure is written as rests of exactly its actual duration. The
underlying role generators validate the supplied score, chord positions and
registers. By default the countermelody contour keeps advancing through a
silenced section, preserving playback of saved contracts. Set
`restart_countermelody_at_section_entry: true` to pause that contour while
silent and start it from the motif center on the next entrance. The flag is
saved in `resolved_options`; omitted flags default to `false`.

Optional `harmony_dynamic`, `bass_dynamic` and `countermelody_dynamic` set a
sustained level on each role's first sounding note, including a role that
enters after rests. Use a level from `Pppp` through `Ffff`; accent and compound
marks are rejected. Each level affects that generated staff's playback
velocity and MusicXML notation. It does not change source parts. Omitted
levels keep the earlier velocity behavior, and the resolved request records
the selected levels for replay. This gives the caller a way to balance roles;
the library does not infer an ideal loudness from note counts.

`min_bass_harmony_gap_semitones` checks simultaneous written notes: every
harmony tone must be at least the requested number of semitones above the bass.
Zero still excludes unison and a bass above harmony. Values above 24 are
rejected. The pad's own `min_melody_gap_semitones` can separately keep it below
a selected source melody staff. `CountermelodyOptions` extracts a bounded
interval contour from the selected source melody, places sparse chord tones on
alternating meter groups, and inverts the contour every second traversal. Its
`min_melody_gap_semitones` keeps each generated tone beneath overlapping source
notes. Optional `phrase_end_measures` thins each authored ending to its first
metrical group and restarts the next phrase with an inverted contour. Without
authored endings, the existing traversal rule applies.
`min_harmony_countermelody_gap_semitones` checks that simultaneous
countermelody tones sit above the entire harmony chord; zero still rejects
unison and role crossing. A generated bass must remain below the countermelody.
Set `max_generated_attacks_per_measure` to an integer from 1 to 64 to bound
combined density; a chord counts as one attack,
and tied notation fragments do not add attacks. Omit it for no density limit.

The result contains `resolved_options`, `generated_notes`, structured
diagnostics and a contract version. Reusing `resolved_options` against the
same source reproduces pitches and timing even though new note IDs are made.
An invalid request returns no score, activity or generated-note addresses.
The three roles still have their individual generator limits of 512 measures,
8192 source notes and 8192 generated events. This API preserves and keeps the
source melody active; the performance-copy API below provides melodic handoffs.

For an explicit melodic handoff, call
`arrange_ensemble_with_melody(score, EnsembleMelodyRequest)` or the matching
full/viewer WASM JSON export. `melody_part_index` and `melody_staff_index`
select the original melody; the current API supports its first staff.
`melody_sections` contains non-overlapping half-open measure ranges with
`enabled` flags. The melody is active by default. A disabled measure in the
returned performance score contains duration-matched rests across its active
voices. A tie entering or leaving that silent measure, including a tie on
only some pitches of a chord, is closed on the surviving note. A tie into an
active alternate ending remains when the repeat-expanded measure sequence and
any supplied explicit tie source identify that surviving connection. Other
original parts and all generated-role measures remain as arranged.
The borrowed source score is never modified, so its original notes and IDs
remain available to the caller. Countermelody generation runs before the
performance copy is silenced and can take over a selected melody rest. The
result reports both melody and generated-role activity; invalid sections or
an unsupported source return no performance score. The resolved request
replays the same note pitches and timing.

```js
const arranged = JSON.parse(arrange_ensemble_roles(scoreJson, JSON.stringify({
  harmony: { chords, pattern: "held", midi_program: 48,
             low_midi: 48, center_midi: 64, high_midi: 84 },
  bass: { chords, pattern: "root-fifth", midi_program: 32,
          low_midi: 28, center_midi: 40, high_midi: 60 },
  countermelody: { chords, melody_part_index: 0,
                    motif_start_measure: 0, motif_end_measure_exclusive: 2,
                    midi_program: 73, low_midi: 60, center_midi: 72,
                    high_midi: 80, min_melody_gap_semitones: 4 },
  harmony_dynamic: "Pp", bass_dynamic: "Mp", countermelody_dynamic: "P",
  sections: [
    { role: "bass", start_measure: 0, end_measure_exclusive: 4,
      enabled: false },
    { role: "harmony", start_measure: 8, end_measure_exclusive: 12,
      enabled: false },
    { role: "countermelody", start_measure: 0, end_measure_exclusive: 4,
      enabled: false }
  ],
  restart_countermelody_at_section_entry: true,
  min_bass_harmony_gap_semitones: 3,
  min_harmony_countermelody_gap_semitones: 3,
  max_generated_attacks_per_measure: 6
})));
```
