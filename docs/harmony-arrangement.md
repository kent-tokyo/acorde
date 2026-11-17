# Spelled harmony accompaniment from a source score

`arrange_harmony_voicings(&Score, &HarmonyArrangementRequest)` derives the bass
and melody constraints, selects ordered voices, and appends a held accompaniment
part. `replay_harmony_arrangement` regenerates it from the source-bound resolved
request. Both operations are exported by full and viewer WASM with score/request
JSON arguments. The [shared fixture](../tests/fixtures/arrangement/harmony_arrangement_source.rs)
builds a three-bar Dm9 → G13 → Cmaj9 example with separate melody and bass.

The request identifies the melody staff and optional bass staff, a half-open
written-measure range, locked measures, the chord plan, register and clearance
limits, three to five voices, root policy, leap limit, parallel-perfect cost,
and MIDI program. The first chord must start at score position zero; later
entries must be strictly ordered and fall inside their measures. Chords before
the selected range supply carried harmony. A locked or unselected bar receives
rests in the added part. Source parts, note IDs and score metadata are preserved.

The result includes the score, added part index, resolved request, the exact
written spans, generated-note addresses referring to those spans, the derived
voicing request, and the per-voice transitions and selection report. Every
performed transition, including repeat returns, enters the graph. Repeated
visits use the same written voicing. Each bar reattacks its held harmony; ties
join duration fragments inside that bar. Anticipation and cross-bar comping ties
remain stage-3 work.

Bass support must cover the entire sounding interval. A rest or shortened
staccato leaves a gap and therefore cannot authorize automatic root omission.
The highest overlapping bass pitch determines the accompaniment floor; the
lowest overlapping melody pitch determines its ceiling. These constraints
include staff/instrument transposition and staff-wide ottava. Instrument-change
context follows the existing playback engine's measure-local resolution. A
channel used by an instrument change is reserved when choosing the new part's
channel. Bass support below the effective floor allows Dm9 to use F–A–C–E.

Written source durations and span boundaries use checked, normalized fractions.
Tuplet source notes participate without rounding. Generated durations currently
use the existing 1/32-quarter-beat notation grid; a source bar length or requested
harmonic position outside that grid is rejected. Pitches are emitted with their
original spelling, including double accidentals, rather than being reconstructed
from MIDI numbers. MusicXML and MSCZ tests compare spelling, ties and repeated
playback after export and import.

Context analysis currently rejects pedal, grace, ornaments, arpeggiation,
glissandi, authored pitch bends, microtones, unpitched notes and sustain/sostenuto
or all-notes-off controllers on selected context parts/staves. These effects
require additional context handling; they are not silently ignored. Other source
parts are copied unchanged. External playback options such as muting the source
bass are not part of the request and invalidate its bass-support assumption.

Input is bounded to 32 written measures, 64 harmonic positions, 32 parts,
8 staves per part, 8,192 source notes and 32,768 source pitches. All staves must
share the same measure count and exact lengths. Generated notes are capped at
8,192 and performed graph edges at 512. The lower-level planner's candidate and
search limits also apply. Invalid input, unsupported context and unsatisfied
voicing constraints return typed errors with no partial score.

The resolved contract stores version 1, a versioned engine marker, the existing
UUID-normalized `v2:` source fingerprint, and the request. Replay revalidates the
source and request before generation. Musical events and provenance reproduce;
newly generated UUIDs need not match. The fingerprint is an identity check, not
an authenticity mechanism. Complete stage acceptance still requires the
checks recorded in `harmony-groove-progress.json`; this API alone does not provide
the stage-3 three-candidate listening comparison.
