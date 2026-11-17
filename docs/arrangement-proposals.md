# Complete arrangement proposals

`suggest_arrangement_proposals(score, request)` returns exactly three to five
complete, deterministic scores. The full/viewer WASM export accepts the same
score and request as JSON. Each candidate contains the selected `ChordPlan`
with its origin and source attribution, a whole score with separate harmony
and bass parts, optional countermelody, resolved options, measurable features,
reason codes and differences from the first candidate.

The request combines a `ChordProposalRequest`, optional locked source-plan
entry indices, an `EnsembleRoleOptions` instrument and section template, and
`count` from 3 to 5. Harmony and bass options are required in the template;
their `chords` are replaced by each proposed plan. An optional countermelody
uses its own source motif and register limits. Seven bounded profiles vary pad
rhythm and voicing, bass motion, and whether a supplied countermelody plays.
The last two retain offbeat and metered textures with close harmony and a
root/fifth bass when open voicing or approaching bass cannot fit the supplied
register and collision limits.
The ensemble template can opt into
`restart_countermelody_at_section_entry`; the resolved proposal retains this
choice so a countermelody entrance starts its motif anew after a silent
section. Missing flags keep the prior continuous-contour behavior.
Optional sustained role dynamics in the template are copied to each candidate
and its resolved replay contract. A profile that omits countermelody also
omits its dynamic setting. Omitted levels keep the previous playback.
For a multi-part source, harmony search reads the selected melody part's first
staff; the complete arrangement retains every original part.
The engine tries candidate-plan/profile pairs in a fixed order and removes
duplicates by role, instrument, written attack position, duration, pitch and
articulation and dynamic marking. Sustained notes and reattacks have distinct identities.
When the template includes a countermelody, two answer profiles are tried
before filling a three-candidate set with accompaniment-only profiles. This
lets the smallest proposal set include an orchestration choice when one can be
realized within the caller's register and collision limits.
If it cannot realize the requested number of distinct complete scores, it
returns no candidates and an `insufficient-distinct-arrangements` diagnostic.

`difference_codes` compare actual output with the first proposal: harmony,
generated harmony notes, bass notes, countermelody, attack density and part
count. `generated_attacks_by_measure` exposes the density contour rather than
only its total or peak. These codes describe structural differences, not a
musical-quality ranking. `replay_arrangement_proposal` checks the source fingerprint and
resolved plan against the role options before rebuilding the score. It
reproduces the same pitches, timing and velocities; generated note IDs can differ.

`regenerate_arrangement_region` takes the original and replacement resolved
proposals and a half-open physical-measure range. `locked_roles` keeps selected
generated parts, and `locked_measures` keeps all generated parts in selected
measures. `locked_chord_entries` names indices in the original proposal's
plan; the replacement must keep their sounding chord throughout each original
harmonic span. Chord-entry locks on the initial proposal request are respected
before either complete proposal is built. The result retains original source
parts and generated measures outside the selected range. A countermelody can
be added for the selected measures or replaced with rests while the other
roles stay locked; `role_parts` reports the resulting part indices. It rejects
incompatible instruments, invalid ranges, mismatched sources and ties crossing
a splice boundary without returning a partial score. A global role-dynamic
change is also rejected because it could change playback outside the selected
region. When a role is added, its first inserted attack receives the requested
dynamic. Removing its only dynamic mark is rejected if later notes outside
the region would play at a different velocity. The result checks all playback
events outside the selected region and in locked measures before returning a
score. Repeating the resolved region request reproduces the same performed
pitches, timing and velocities. The returned
`changed_roles` and `changed_measures` report actual written-event differences,
ignoring regenerated note IDs.

The current proposal set has no listening-derived ranking. Use the fixed
development and holdout corpus in `arrangement-capability-roadmap.md` for a
matched source, tempo, sound source and volume comparison before claiming a
general musical improvement.
