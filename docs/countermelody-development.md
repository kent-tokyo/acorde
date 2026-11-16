# Source-rhythm countermelody development

`arrange_countermelody_development` adds one separate, chord-grounded part
without changing the source parts. It is opt-in and leaves the existing
`arrange_countermelody` contract unchanged. The request contains existing
`CountermelodyOptions` and one `CountermelodyDevelopmentKind`:

| Kind | Treatment |
| --- | --- |
| `imitation` | Source note/rest rhythm after a one-meter-unit entry delay. |
| `inversion` | Source rhythm with directed motif intervals reversed. |
| `augmentation` | Double note and rest durations. |
| `diminution` | Halve note and rest durations. |
| `answer` | Enter halfway through the bar with inverted contour. |

The result reports the source and transformed rhythm as integer 1/32-quarter
ticks, the bounded pitch intervals, generated note addresses and attack count.
It contains a versioned source fingerprint and resolved request;
`replay_countermelody_development` rejects a changed source or engine version.
Full and viewer WASM expose both operations as JSON functions.

The first eight sounding motif notes and intervening rests supply the rhythm.
The generated rhythm restarts at each written measure. A chord change inside
a sounding cell splits that cell into fresh chord-tone attacks. An authored
phrase ending leaves only the first sounding segment in that measure. The
realizer keeps pitches within the requested instrument range and below
overlapping source melody notes by the specified gap. Tied or tuplet source
motifs are rejected by this version because their rhythmic subdivision cannot
be represented by the current 1/32-quarter tick contract. Errors are atomic.

The shared four-bar test traces a quarter note, eighth rest, eighth note and
half note into five distinct outputs. Core tests preserve original parts and
source-bound replay; MusicXML/MSCZ and WASM JSON tests preserve playback.
Ten 16–32-bar development works that already admit a countermelody produce
all five distinct transformations each. The two other works have no accepted
countermelody base and are not counted as coverage. This proves structural
diversity and export fidelity, not counterpoint quality or instrument
playability. Phrase-join voice leading, independent inner voices and listener
review remain in [the development plan](arrangement-development-v2.md).
For written section activity and dynamics alongside harmony and bass, use
[ensemble form development](ensemble-form-development.md).
