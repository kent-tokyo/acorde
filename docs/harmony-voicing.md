# Ordered harmony voices over a transition graph

`plan_harmony_voicings` is the lower-level stage-2 planner, available in Rust
and full/viewer WASM. It accepts harmonic nodes with interval-wide bass and
melody context plus the actual transition graph. The
[fixed request](../tests/fixtures/arrangement/harmony-voicing.json) demonstrates
Dm9 → G13 → Cmaj9 and a repeat return to Dm9.

This component does not yet derive context from a `Score`, schedule anticipations
or write accompaniment notes. Those connections remain required by stages 2–3
of the [implementation plan](arrangement-harmony-groove-plan.md).

The root policies are `include`, `automatic` and `omit`. Automatic omission
requires a supplied root bass below the accompaniment register, sustained for
the entire interval. Missing, wrong or high bass context retains the root.
An authored root omission remains an omission even with the include policy.
Dm9 with a low D therefore uses F–A–C–E; the result records the reason for each
root decision.

Thirds, sevenths, sixths, suspensions and explicitly added/altered tones are
required. Other tones fill the requested voice count, with octave doubling for
sparse chords. A request that cannot fit the required tones fails rather than
dropping a requested alteration. The melody ceiling and accompaniment range are
hard constraints. Each node has three to five strictly ordered voices.

Voice index is persistent from low to high across every edge. The report gives
each voice's starting/ending pitch, signed leap and compound degree. Leap limits
apply to repeat returns as well as forward transitions. Guide-tone motion is
weighted in the cost; the optional parallel-perfect penalty is a style choice,
not a universal prohibition. Multiple identical edges retain their frequency
weight in the cost.

The contract limits input to 64 nodes, 512 edges, a 48-semitone register width,
32 candidates per node and a 64-path beam. At most 131,072 partial search states
are evaluated. `search_limited` records candidate or beam pruning; the result
does not claim a global optimum. `NoVoicing` identifies a node without a local
solution. `NoPath` reports whether pruning occurred and does not imply that
every mathematically possible path was searched.

The native tests cover bass fallback, authored omission, explicit tensions,
suspensions, repeat-edge voice identities, hard register/leap failures, style
costs and the 64-node bound. Focused Chrome tests compare the complete result
with native output in both profiles and reject invalid edges/excessive graphs.
Score-derived context, source-bound replay, exported notes and the 8/16-bar
three-candidate audio comparison are still outstanding.
