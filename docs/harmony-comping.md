# Anticipated attacks with unchanged harmonic references

The score-level harmony request accepts optional `comping`. Its default is
the existing held pattern with no anticipation. `offbeat-pulse` places half-beat
attacks on quarter-note offbeats, leaving rests between them. Both patterns use
the score's exact written bar lengths and preserve the original chord-plan
positions.

```json
{
  "pattern": "offbeat-pulse",
  "accent": "anticipations",
  "anticipations": [
    {"chord_entry": 1, "lead": "eighth", "join": "tie"}
  ],
  "max_attacks_per_measure": 4
}
```

An anticipation refers to an existing chord entry and moves its attack back by
an eighth or sixteenth note. For a chord at the next 4/4 bar's start, an eighth
anticipation begins at beat offset 3.5 of the preceding bar. `tie` keeps this
attack sounding through the original boundary; `reattack` articulates that
boundary again. Under the offbeat pattern it sustains through the boundary until
the next scheduled offbeat; under held comping it sustains to the target's next
held boundary. Overlapping earlier attacks are truncated, with no duplicate
attacks left underneath the anticipation.

Each output span records its actual written start/end, original `chord_entry`,
attack identity, and cross-bar tie flags. Context and voicing are evaluated
after scheduling, across all actual intervals. A G13 anticipated over the old
D bass cannot claim automatic G-root support; a low melody in that earlier
interval constrains the G13 voicing too. Source notes and chord positions remain
unchanged, and the resolved request replays both attacks and note provenance.

The request rejects anticipation before the score, outside the selected range,
inside a locked measure, across a preceding harmonic change, or to a nonexistent
entry. Duplicate anticipation entries and excessive attack density also fail.
A pickup may be filled from its exact start when the anticipation fits; it is
never padded or rounded to a full bar.

For a cross-bar anticipation, every performed departure from the earlier bar
must reach the intended target. A tied target must also have the same preceding
bar on every visit. Ambiguous repeats or alternate endings return
`ambiguous-repeat`; this version does not silently unfold or rewrite the score.
An explicitly requested reattack can be valid where an incoming tie is not.

Scheduling is capped at 64 anticipation entries, 64 attacks per written bar and
2,048 attacks total. Generated-note and voicing-graph limits still apply. All
times use checked fractions, and output durations must fit the notation grid.
These explicit controls are the comping component of stage 3. Automatic selection
of the three styles is described in [harmony groove proposals](harmony-groove-proposals.md).
The fixed 8/16-bar comparison still requires listening review and completion gates.

`accent` defaults to `none`. `anticipations` marks only attacks before their
nominal harmony; `all-attacks` marks every attack. The first note of an attack
receives the existing Accent articulation (+20 playback velocity, clamped at
127). Neither notation fragments nor tied continuations acquire another accent.
