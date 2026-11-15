# Arrangement development: outcomes 0–6

The active objective is all seven outcomes below, including musical evaluation.
The first implementation milestone (a 16-bar phrase-aware arrangement) does not
replace the full objective. Do not mark an outcome complete from a build or
structural difference alone.

## Integration identity

- Arrangement baseline: `fadb1e892946c8482e8a5b8a89ddef8fdbc1a0ba`.
- Analysis baseline: `f4894402`, containing main/release `4cb88658` (v1.3.12).
- Integration: `5798066b` in `codex/arrangement-v2-20261115`.
- Workspace: `/private/tmp/acorde-arrangement-v2-20261115`. The primary
  checkout's uncommitted analysis documentation is not part of this work.
- The separate issue-433 worktree has an in-progress accordion-only written
  section plan. Its files were read to avoid ownership/API overlap, not copied
  or changed. This work introduces an ensemble form across independent parts.

`cargo test --all --offline --quiet` passed on the integration commit using an
isolated target directory and disabled debug information/incremental builds.
That includes the fixed v1 arrangement corpus. The new form changes require
their own test evidence. Hosted CI, optimized WASM size and listening review
remain separate gates.

The first form implementation passes the full native workspace suite, Clippy
with warnings denied, formatting and diff checks. Its focused tests cover four
native cases, MusicXML and repeated MSCZ playback round trips, and the actual
Chrome form/replay JSON contract in both full and viewer builds. In the 16-bar
fixture, the first bar of each texture has respectively 1, 4, 8 and 4 harmony
attacks; the final release has one. The contrasting F bass approaches G through
F-sharp, while the returning C bass alternates C/G. Source playback is unchanged.
The complete full and viewer Chrome WASM suites also pass after the form
implementation. Logs are kept at `/private/tmp/acorde-arrangement-v2-full-wasm.log`
and `/private/tmp/acorde-arrangement-v2-viewer-wasm.log`. Hosted CI, optimized
artifact size, reviewed audio and publication gates remain pending. The CI size
limit stays at v1.3.12's 7,170,000 bytes; an older provisional macOS-based budget
increase was removed because its optimization settings differ.

The visit-specific event extension passes the full native workspace suite,
Clippy with warnings denied, formatting and both full/viewer Chrome WASM suites.
Its fixture repeats measures 0–7: visit 0 retains the opening chord while visit
8 has an eight-attack arpeggio. Tests compare unchanged source melody events,
all unselected visits, and source-bound replay. The full/viewer logs are at
`/private/tmp/acorde-arrangement-v2-visits-full-wasm.log` and
`/private/tmp/acorde-arrangement-v2-visits-viewer-wasm.log`. A separate
performance-score API now unfolds visits for notation/export. Its focused core
tests cover the 16-bar return, a structurally doubled 32-bar form, source-bound
replay, key/clef restoration at repeat edges and atomic rejection of unresolved
view references. MusicXML and MSCZ round trips preserve the exported sound.
The full native suite, warning-free Clippy and complete full/viewer Chrome WASM
suites pass; the browser contract test checks the new JSON APIs in both builds.
Logs are `/private/tmp/acorde-performance-all.log`,
`/private/tmp/acorde-performance-clippy.log`,
`/private/tmp/acorde-performance-full-wasm.log` and
`/private/tmp/acorde-performance-viewer-wasm.log`. The doubled form checks
scaling and repeat traversal; a separately composed 32-bar development piece
and listening review remain.

The phrase-harmony API passes four focused core tests, the full native
workspace suite, warning-free Clippy, and the complete full/viewer Chrome WASM
suites. The WASM JSON test confirms native parity and atomic rejection of an
overlapping phrase schedule. Its logs are `/private/tmp/acorde-phrase-native.log`,
`/private/tmp/acorde-phrase-full-wasm.log` and
`/private/tmp/acorde-phrase-viewer-wasm.log`. Source-plan positions must match
the generated positions exactly unless the caller supplies an explicit mapping.
The mapping extension passes six focused core tests, including source-bound
replay, the full native workspace suite, warning-free Clippy and full/viewer
Chrome WASM. The latter exercises locked chord and provenance preservation.
Its logs are `/private/tmp/acorde-reanchor-native.log`,
`/private/tmp/acorde-reanchor-full-wasm.log` and
`/private/tmp/acorde-reanchor-viewer-wasm.log`. The original source-plan change
budget is enforced after reanchoring. Borrowed/passing chords, cadence
constraints and listening acceptance still require implementation.

Supplied local-key regions now affect chord search without rewriting the
source score. Focused Rust tests compare a conservative plan in the written
key and an explicit G-major passage, then replay the selected plan against the
original source fingerprint. The full native suite, warning-free Clippy and
complete full/viewer Chrome WASM suites pass. Chrome verifies the same JSON
contract in both profiles. Logs are `/private/tmp/acorde-local-key-native.log`,
`/private/tmp/acorde-local-key-full-wasm.log` and
`/private/tmp/acorde-local-key-viewer-wasm.log`. Borrowed/passing chords and
listening acceptance remain.

Explicit cadence root-motion targets now constrain the search and filter
supplied source plans. Ten focused core tests cover the three motions, source
locks, one-candidate requests and replay. Full native tests, warning-free
Clippy, and the full/viewer Chrome WASM suites pass; the Chrome JSON test
combines a G-major key region with a dominant-to-tonic arrival. Logs are
`/private/tmp/acorde-cadence-native.log`,
`/private/tmp/acorde-cadence-full-wasm.log` and
`/private/tmp/acorde-cadence-viewer-wasm.log`. These tests prove requested
chord-root motion, not a complete classical cadence or listener acceptance.

Caller-selected chord color positions now add parallel-mode borrowing in major
and minor keys and short, semitone-connected diminished passing chords on free
bass. Focused core tests cover actual candidate generation, source-bound preset
realization, minor-key vocabulary, invalid positions, unavailable instrument
mode, conflicting locks and zero-change budgets. The full native suite, Clippy
with warnings denied, and full/viewer Chrome WASM suites pass. Both browser
builds compare the borrowed/passing JSON contracts with native results. Logs
are `/private/tmp/acorde-color-focused.log`,
`/private/tmp/acorde-color-all.log`, `/private/tmp/acorde-color-clippy.log`,
`/private/tmp/acorde-color-full-wasm.log` and
`/private/tmp/acorde-color-viewer-wasm.log`. This verifies the requested chord
relationships and realization; it does not establish that listeners prefer the
chromatic harmony. A longer local-key example, stronger cadence-quality checks
and listening notes remain.

Authored bass rhythm sections now compose note/rest durations, chord-relative
tones and accents. Focused core tests cover walking, syncopation, ostinato,
anticipation, a phrase-ending fill, pickup and 5/8 lengths, mid-bar chord
changes, repeat-back motion, atomic rejection and source-bound form replay.
MusicXML/MSCZ round trips and the WASM JSON test pass natively. `cargo test
-p acorde-core`, the full native workspace suite and warning-free Clippy pass.
An earlier full-suite run was stopped during compilation when shared disk free
space fell below 150 MiB; the completed rerun is recorded at
`/private/tmp/acorde-rhythm-all.log`. Complete full/viewer Chrome WASM suites
also pass, including authored-bass JSON/replay cases. Their logs are
`/private/tmp/acorde-rhythm-full-wasm.log` and
`/private/tmp/acorde-rhythm-viewer-wasm.log`. The dedicated generated native
target was removed after process/open-file checks to make room for browser
verification. Other related logs are `/private/tmp/acorde-rhythm-core-all.log`,
`/private/tmp/acorde-rhythm-io.log`, `/private/tmp/acorde-rhythm-wasm-native.log`
and `/private/tmp/acorde-rhythm-clippy-final.log`. These checks prove written rhythms
and playback events, not listener preference or ensemble playability.

The new `development-v2.json` adds twelve independently authored 16–32-bar
source works with major/minor keys, five meters, a pickup, a written repeat, a
local key-signature passage and a two-part source. Core and I/O tests validate
all twelve and preserve MusicXML playback. The source-pack example generated
score JSON, MusicXML and reference WAV files twice with identical SHA-256
manifests; its pinned manifest is `development-v2-manifest.json`. Reference
audio uses a documented all-piano copy because the bundled SoundFont lacks
the other programs. These sources have not received listening review. Six
independent sealed final works and hosted CI
remain outstanding for the comparison baseline.

The twelve works now each produce three replayable complete proposals under
an explicit, source-specific request. The proposal pack pins 36 MusicXML and
WAV identities and offers a numbered local listening sheet. A core test
checks three distinct generated-event sequences per source, source-part
preservation and source-bound replay. The lower cello melody needs an authored
upper-harmony placement; this is a request choice, not automatic orchestration.
At least one candidate in ten works adds a countermelody, diversifying its
orchestration. `five-step` and `low-tide` still have no countermelody candidate.
Whole-piece form differences are not established by this pack.
Listener judgments, instrument-specific audio and six sealed final works remain.

A separate opt-in sectional proposal API now varies role entrances, harmony
and bass texture, dynamics and attack shape across caller-supplied boundaries.
It produces five different generated playback signatures on each of the twelve
development sources, retains source parts and replays from a saved contract.
When harmony and countermelody collide, it can omit the latter only in the
affected section and records the omission. Focused native, MusicXML/MSCZ and
WASM JSON tests pass. These form proposals are not yet included in the pinned
audio listening pack, and structural diversity is not a listener judgment.
The complete native workspace suite, Clippy with warnings denied, formatting,
MusicXML/MSCZ tests and complete full/viewer Chrome WASM suites pass for this
stage. Logs are `/private/tmp/acorde-form-proposals-all-final.log`,
`/private/tmp/acorde-form-proposals-clippy-final.log`,
`/private/tmp/acorde-form-proposals-full-wasm-all.log` and
`/private/tmp/acorde-form-proposals-viewer-wasm-all.log`. The final Clippy-only
edit removed an explicit dereference with no behavior change; the Chrome suites
exercise the final source. Hosted CI, optimized WASM size and listening review
remain separate gates.

The sectional-form listening pack separately fixes 60 five-way proposals for
the same twelve development works. It holds each work's selected chord and
instrument template constant across five section profiles, and uses the same
all-piano render conditions as the source and earlier proposal packs. Every
candidate replays, preserves source playback and survives MusicXML parsing.
All twelve groups have five distinct WAV hashes; 24 candidates document a
sectional countermelody omission made to resolve a collision. The pinned
manifest is `development-v2-form-manifest.json`, and the local sheet is
`/private/tmp/acorde-arrangement-v2-form-listening-final/index.html`.
The sheet varies the A–E display order by work, while the manifest preserves
the identity mapping. Two generations of 206 files are byte-identical;
readback checks all 204 referenced score, audio and resolved-contract hashes.
The manifest SHA-256 is
`beab90959fbf8c2d834cdd8115eb579ab2c00cee5066292ef4a4919521849f27`.
No human listening judgment is recorded yet.

An opt-in countermelody-development API now retains the first eight source
motif notes' note/rest rhythm in a bounded 1/32-quarter tick form. It generates
imitation, inversion, augmentation, diminution and delayed-answer variants.
The four-bar fixture checks traceable onset/duration changes, source
preservation, atomic melody-clearance and unsupported-subdivision rejection,
and source-bound replay. Ten long-form development works with an existing
countermelody base each produce five valid, distinct generated playback
signatures (50/50 attempts). MusicXML/MSCZ round trips, native WASM JSON,
full/viewer headless Chrome focused tests and full-workspace Clippy pass.
`cargo test -p acorde-core` passes. A full-workspace native test attempt was
stopped during compilation at 302 MiB of shared free disk space; its generated
target was removed after an open-file check. The full native and full browser
suites, rhythmic motifs with ties/tuplets, phrase-join voice leading,
independent inner voices and listener acceptance remain open. Logs are
`/private/tmp/acorde-counter-development-core.log`,
`/private/tmp/acorde-counter-development-clippy.log`,
`/private/tmp/acorde-counter-development-full-wasm.log`,
`/private/tmp/acorde-counter-development-viewer-direct-final.log` and
`/private/tmp/acorde-counter-development-all.log` (interrupted).

An opt-in ensemble-form development API now applies source-rhythm
countermelody transformations only in sections where that role is active.
Harmony, bass, density and vertical-collision rules still run for the complete
arrangement, and section dynamics mark the developed part. Its 16-bar fixture
checks entrances, rests, source preservation, replay and atomic density
rejection; MusicXML/MSCZ and native/full/viewer WASM JSON round trips pass. In the twelve
development works, all ten with an existing countermelody base have at least
one compatible five-way form, and imitation succeeds on 39 of 50 attempted
forms. The two other works still lack an accepted countermelody base. Full
native and listening gates for this integration remain open. Clippy passes
for the changed libraries and new tests; the workspace-wide `--all-targets`
run stopped on an existing `phrase_harmony.rs` test lint, after an earlier run
exhausted shared disk space. The existing form, form-proposal and standalone
countermelody regression tests pass with the new tests.

Developed form proposal selection now evaluates up to 25 combinations of five
section profiles and five source-rhythm treatments, keeps distinct generated
playback and reports failed combinations with measure/role diagnostics. All ten
countermelody-capable development works yield three source-bound, replayable
selections with three different rhythmic treatments; all ten also admit five
distinct selected playback sequences. Eight works use three
form profiles; `new-horizon` uses two and `two-voices` one because other
combinations fail their constraints. The 16-bar selected scores survive
MusicXML/MSCZ playback round trips; focused Rust and full/viewer Chrome JSON
tests pass. This joins form and contrapuntal variation in selection, but it
does not yet combine independently searched harmony plans or support local
intent-directed edits. The full native workspace suite and warning-free
workspace Clippy and complete full/viewer Chrome WASM suites pass. Hosted CI,
optimized size and listening gates remain.

The developed-form selector now measures six distances between generated
candidates: harmony pitch classes per bar, event rhythm, exact bass events,
register bands, sounding role activity and section treatment. It retains a
new countermelody treatment when one is available, then chooses the candidate
farthest from its nearest selected neighbor on those measured axes. The result
reports each axis's distance from the first proposal and the nearest selected
distance. All six axes show a difference somewhere in the ten compatible
development works, and their three selected treatments remain distinct.
These distances compare proposals under one supplied chord plan; cross-plan
harmony ranking, local intent mapping and listener acceptance remain open.

An opt-in phrase-entry constraint now carries the last sounding
countermelody pitch across an authored phrase end and selects the next chord
tone within a caller-supplied 0–24-semitone leap. A silent role section resets
that link. The 16-bar fixture contrasts the original three-semitone return
with a same-pitch entry and checks a two-semitone F-major return; a one-semitone
limit fails atomically at measure 8. All ten countermelody-capable development
works have at least one sectional imitation form with a four-semitone limit.
Focused core, MusicXML/MSCZ and native WASM JSON tests pass. Full native,
workspace Clippy and complete full/viewer Chrome WASM suites also pass. The
viewer run needed a scoped cleanup of generated WASM artifacts after a disk
space failure; the complete rerun passed. Hosted CI, optimized size and
listening gates remain.

## Required outcomes and evidence

An opt-in inner-voice API now adds two independently timed chord-tone lines on
one part. The caller supplies separate rhythms, ranges and maximum leaps, plus
source-melody clearance, active bars and phrase endings. The 16-bar development
fixture uses first-half lower entries against upper offbeat entries. Focused
tests verify separate onset grids, source preservation, phrase-to-phrase leap
bounds, rejection of impossible requests, source-bound replay, 5/8 bar length,
and MusicXML/MSCZ playback round trips. The full native workspace suite,
warning-free Clippy, and full/viewer Chrome WASM suites pass with stripped
test artifacts to fit local disk space. The browser runner warns that stripping
removes its `target_features` detection section; the individual tests still
execute and pass. Logs are `/private/tmp/acorde-inner-voices-all-stripped.log`,
`/private/tmp/acorde-inner-voices-clippy.log`,
`/private/tmp/acorde-inner-voices-full-chrome.log` and
`/private/tmp/acorde-inner-voices-viewer-chrome.log`. Optimized release size
and hosted CI are separate gates. This is a source-melody and chord-plan
operation; it does not yet coordinate inner voices with every ensemble role or
prove repeat-back continuity and instrument-specific playability. Listening
review remains required.

A form-level option now uses the two inner lines in place of the chordal pad
while retaining the existing bass and developed countermelody. It derives
inner-line activity from the form's sections, marks the first sounding attack
of each bar with that section's dynamic, and checks the combined bass/inner/
countermelody vertical order and total attack density. The 16-bar development
fixture checks inactive final bars, source preservation, distinct part events,
source-bound replay, an actual density rejection and a countermelody crossing.
MusicXML/MSCZ round trips cover both lower-first and upper-first dynamic
entrances; the WASM JSON contract passes in focused tests.
The full native workspace suite, warning-free Clippy, and complete full/viewer
Chrome WASM suites also pass without stripping the WASM test artifacts. Logs
are `/private/tmp/acorde-form-inner-all.log`,
`/private/tmp/acorde-form-inner-clippy.log`,
`/private/tmp/acorde-form-inner-full-chrome.log` and
`/private/tmp/acorde-form-inner-viewer-chrome.log`.
This replaces one harmony texture; it does not yet choose among these textures
inside one form.

An opt-in performance-score API now unfolds repeat visits before choosing
inner-voice pitches. It keeps the compact written score and supplies a separate
exportable score whose second visit may start on another chord tone. A two-bar
development repeat moves the lower voice from E4 on the first entrance to G4
on the return, keeping its two-semitone playback leap bound. The original
melody events, MusicXML/MSCZ playback, source-bound replay and native/WASM JSON
results agree in focused tests. Source scores with unresolved cross-measure
references or mismatched unfolded playback fail atomically. This does not yet
prove musical quality on the broader corpus. The full native workspace suite,
warning-free Clippy, and complete full/viewer Chrome WASM suites pass on this
change. The browser runs use the direct `wasm-bindgen-test-runner` with Chrome;
optimized release size, hosted CI and listening remain separate gates.

The sectioned inner-voice form now also returns a compact written score and an
exportable performance score with visit-specific inner pitches. On the 16-bar
fixture, a repeated first eight bars produce 24 playback visits. Each visit
retains the section activity and dynamics while checking bass/inner/
countermelody spacing and total attack density; the return from visit 7 to 8
also obeys the inner-voice leap bounds. Written and performance parts retain
matching IDs. MusicXML/MSCZ playback round trips, source-bound replay and
native/WASM JSON comparison cover this form. The full native workspace suite,
warning-free Clippy, and complete full/viewer Chrome WASM suites pass.
Form-specific listening and broader corpus acceptance remain open.

An opt-in instrument plan now assigns named instruments and MIDI programs to
selected arrangement parts only after checking caller-supplied performance
limits on the complete playback traversal. A keyboard-hand envelope checks
range, simultaneous pitch count and span across overlapping notes; a wind-line
envelope checks monophony and continuous playing time, with short rests kept
inside the same breath run. Focused tests cover a sectioned 16-bar arrangement,
repeated-visit breath rejection, range/chord/span rejection, source-bound replay,
MusicXML/MSCZ name/program and event round trips, and native/WASM JSON parity.
MusicXML and MSCZ currently preserve the exported part name and MIDI program,
but do not reconstruct the full `InstrumentDefinition` on import. These
explicit envelopes are screens, not fingering or breathing certificates.
The full native workspace suite, warning-free Clippy, formatting and complete
full/viewer Chrome WASM suites pass on this increment. Hosted CI and optimized
size remain separate gates.
Piano two-hand allocation, accordion button control, string sustain, role
redistribution, simplified versions and performer acceptance remain open.

| Outcome | Required final behavior | Current evidence / remaining work |
| --- | --- | --- |
| 0. Comparison baseline | Integrate prior arrangement and analysis work; preserve reproducible old outputs and fix comparison conditions. | Integration, native corpus and full/viewer Chrome suites pass. Twelve new long-form development sources, 36 complete-proposal and 60 sectional-form piano-reference audio identities are fixed. Review the development works, create six independent sealed final works and complete hosted CI/size gates. |
| 1. Whole-piece development | Coordinate phrases, A/B sections, entrances, pauses, density and ending shape over 16–32 bars; vary a repeated passage on a later visit. | `arrange_ensemble_form` compiles an explicit written form, coordinates phrase endings, varies bass patterns without resetting bass pitch continuity, and restores dynamics at every sounding bar. The visit API selects a different generated event sequence on a later repeat visit. A new API unfolds those visits into an exportable performance score when its playback matches the event contract. A new bounded proposal API produces five distinct sectional forms on each of twelve 16–32-bar development works. The 16-bar fixture, doubled 32-bar traversal, MusicXML/MSCZ round trips and source-bound replay pass. Source-level references still need relocation, plus form-specific listening acceptance. |
| 2. Harmony | Choose harmonic rhythm by phrase; add borrowed/passing harmony and supplied local-key movement, bounded change amount and cadence intent while preserving melody/locks. | `suggest_phrase_harmony` selects two-bar, one-bar or two-group pace. Its explicit mapping can split or merge an existing source plan while preserving locks and comparing change budgets with the original regions. `local_keys` supplies major/minor movement; `cadences` constrains three named root motions. Explicit `colors` selects parallel-mode borrowing or a short chromatic passing chord. Native and full/viewer Chrome suites pass. A longer local-key example, stronger cadence-quality checks and listening acceptance remain. Use explicit source/key/phrase input before relying on uncertain analysis. |
| 3. Rhythm and bass | Composable pulse/rest/accent/duration rules, anticipation, syncopation, walking/ostinato bass and phrase-ending fills; coherent transitions. | Form can switch the existing three bass patterns by section. Authored bass rhythm sections now express the named techniques and keep exact bar lengths and chord transitions in focused tests. The full native workspace and full/viewer Chrome WASM suites pass; matched-instrument listening and broader playability checks remain. |
| 4. Countermelody and voice leading | Capture motif rhythm as well as contour; inversion, augmentation/diminution, imitation, question/answer and independent inner voices across phrases. | An opt-in source-rhythm API extracts note/rest durations and generates imitation, inversion, augmentation, diminution and delayed answer treatments. Ten long-form development works produce five valid distinct standalone event sequences each. Form development coordinates this role with section activity, harmony, bass, dynamics and collision checks. The selector chooses three distinct rhythmic treatments in each of the ten countermelody-capable works. A new opt-in bound selects the next phrase's chord tone near the prior ending pitch and rejects impossible joins; all ten works have at least one form accepting four semitones. An independent two-voice API adds separate rhythm grids, bounded cross-phrase motion, fixed voice ordering and melody clearance. A form-level option replaces the chordal pad with those lines and checks section activity, dynamics, cross-role order and total density. The sectioned form now also chooses inner pitches in playback order, checks every repeat visit's spacing and density, and bounds the return's inner-voice leaps. Texture choice within a form, instrument playability and listening acceptance remain. |
| 5. Instruments and difficulty | Piano hand allocation, accordion buttons, wind breathing, string sustain; solo/duet/small ensemble and simplified versions. | An opt-in instrument plan assigns names and MIDI programs to selected parts, then screens sounding range, one-hand keyboard chords and wind breath runs across repeats. Existing accordion button infrastructure remains separate. Two-hand allocation, accordion integration, string sustain, role redistribution, simplified versions and performer review remain. |
| 6. Proposal selection and editing | Three to five musically distinct complete proposals measured across harmony/rhythm/bass/register/roles/form; intent-directed local changes respecting locks. | Twelve long-form works produce three replayable complete proposals and five distinct sectional forms with pinned piano-reference WAV identities. The developed-form selector combines five form profiles with five rhythmic countermelody treatments and ranks distinct playback using six observed axes. A cross-plan selector now combines distinct chord plans with sectioned forms, preserving harmony locks and changed-span budgets in source-bound replay. It yields three proposals with at least two chord plans in ten of eleven countermelody-capable development works; `five-step` has fewer than two plans that survive form realization. Intent-to-constraint mapping, final-set review and listening acceptance remain. |

## Execution order and completion gates

Complete each numbered outcome before treating the next one as a release
candidate. Development fixtures can grow alongside implementation, but the
final six-work set stays unopened until the algorithm and review protocol are
frozen. Each stage gets a focused commit or PR and a written failure list with
measure and role references.

| Step | Work to ship | Exit gate |
| --- | --- | --- |
| 0. Freeze the comparison | Pin the integrated commit, old/new API requests, renderer, soundfont, tempo, audio hashes and source identities. Add 12 development works and six sealed final works with explicit key, meter, form and instrumentation coverage. | Old eight-score regressions reproduce; corpus manifest and comparison settings are reviewed before any final-set scoring. Hosted CI and optimized WASM size are recorded under the same build settings as the v1.3.12 limit. |
| 1. Complete whole-piece form | Turn visit-specific generated events into an exportable performance score; test 16- and 32-bar forms with repeats, entrances, rests, density and final release. | Every visit has an unambiguous written or performance representation; source parts, repeat traversal and MusicXML/MSCZ round trips agree with playback; three development pieces receive listening notes. |
| 2. Complete harmony | Reanchor an existing chord plan only through an explicit old-to-new region mapping that preserves locked spelling, bass and provenance. Add supplied local-key regions, borrowed/passing chord candidates and cadence targets to the phrase request and resolved replay contract. | Melody and locked regions are exact; invalid mappings fail atomically; changed-span budget and cadence are checked on development works in major, minor and local-key passages; native/full/viewer outputs agree. |
| 3. Build rhythm and bass grammar | Compose pulse, rest, accent and duration rules; add anticipation, syncopation, walking and ostinato bass, plus phrase-ending fills and transitions. | Generated note/rest durations cover each measure, including pickup and irregular meters; cross-section and repeat edges preserve bass continuity; rhythm and bass produce audible differences at matched tempo/instruments. |
| 4. Develop contrapuntal voices | Retain motif rhythm as well as contour, and add inversion, augmentation, diminution, imitation and answer phrases. Apply voice-leading rules to independent inner voices across phrase boundaries. | No source melody edits or forbidden register clashes; transformed motifs can be traced to their input; every generated voice is independently playable and musically reviewed at phrase joins. |
| 5. Add instrument and difficulty plans | Allocate piano hands, accordion buttons, wind breathing and string sustain, then redistribute roles for solo, duet and small ensemble versions with a simplified option. | Each exported part passes its explicit range, physical-control, breath or sustain constraints; simplified versions retain melody and intended cadence; performers review the relevant instrument parts. |
| 6. Select and edit full proposals | Rank three to five whole-piece candidates using measured harmony, rhythm, bass, register, roles and form differences. Convert local user intent into bounded regeneration constraints and preserve all locks and untouched regions. | Candidates are genuinely different in playback, replayable and source-bound; local edits are confined to their requested region; the frozen six-work set passes the listening gate below without tuning to that set. |

For every code stage, run focused invariants, `cargo test --all`, Clippy with
warnings denied, formatting and the full/viewer Chrome WASM suites. Run
MusicXML/MSCZ round trips when the stage changes written score or playback.
The relevant CI job and optimized size receipt are required before a release
claim; local checks alone do not satisfy that gate.

## Evaluation protocol to implement

Retain the existing eight original v1 scores as structural regressions. Create
12 new development works and six final evaluation works, spanning major/minor,
local-key changes, pickups, compound/irregular meters, repeated sections and
long phrases. The 16-bar `form_development_v2.rs` is explicitly development
material; it must never be counted as untouched evaluation data.

Before opening the final set, freeze source and annotation identities, the
development-selected algorithm, comparison conditions and acceptance thresholds.
Reviewers judge anonymized candidates for melody audibility, phrase continuity,
cadence/ending, audible differences and playability. Compare harmony/rhythm at
matched instrumentation and tempo; instrument-specific comparisons use the
intended instruments with recorded playback settings. Retain failures at exact
measures/roles. Final-set feedback cannot be used to tune and then re-score the
same set as unseen evidence.

Use at least three independent listeners for the final set. Each scores melody
audibility, phrase continuity, cadence/ending and playability from 1 to 5,
with a short measure-specific note for any score below 4. Before opening that
set, freeze the acceptance rule: at least five of six works must have a median
of at least 4 on every axis, and at least four of six must have a majority
preference for a new complete proposal over the pinned baseline. Record all
individual scores and exact audio identities. Six works give descriptive
evidence only; do not present this as a statistically significant superiority
claim. If the gate fails, keep those results as final-set evidence and use new
works for any later tuned comparison.

Every implementation step also checks source/lock preservation, full-duration
notes/rests, repeat transitions, source-bound replay, no partial result on error,
MusicXML/MSCZ round trips, native/full/viewer WASM parity and bounded resource
use. These mechanical checks establish correctness and observed differences,
not listener acceptance. No completed listening review is claimed yet.

Generation remains in core; analysis data is supplied by callers. No filesystem,
network, application settings or external AI provider is introduced into core.
Preserve old opt-in request/replay contracts and use versioned new APIs for new
behavior. Separate commits/PRs should correspond to these reviewable milestones.
