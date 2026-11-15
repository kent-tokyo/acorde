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
`/private/tmp/acorde-arrangement-v2-visits-viewer-wasm.log`. It does not yet
provide a notation/export score for the changed visit.

## Required outcomes and evidence

| Outcome | Required final behavior | Current evidence / remaining work |
| --- | --- | --- |
| 0. Comparison baseline | Integrate prior arrangement and analysis work; preserve reproducible old outputs and fix comparison conditions. | Integration, native corpus and full/viewer Chrome suites pass. Freeze reviewed baseline audio identities, complete hosted CI/size gates, and establish the new evaluation corpus. |
| 1. Whole-piece development | Coordinate phrases, A/B sections, entrances, pauses, density and ending shape over 16–32 bars; vary a repeated passage on a later visit. | `arrange_ensemble_form` compiles an explicit written form, coordinates phrase endings, varies bass patterns without resetting bass pitch continuity, and restores dynamics at every sounding bar. A second API selects a different generated event sequence on a later repeat visit; its written score still displays the base form. Source-bound replay and the 16-bar development fixture pass native, MusicXML/MSCZ and full/viewer browser checks. Full performance-score export for visit changes, longer examples and listening acceptance remain. |
| 2. Harmony | Choose harmonic rhythm by phrase; add borrowed/passing harmony and supplied local-key movement, bounded change amount and cadence intent while preserving melody/locks. | Existing progression search/locks are integrated. Phrase-aware rhythm, expanded harmonic language and cadence constraints remain. Use explicit source/key/phrase input before relying on uncertain analysis. |
| 3. Rhythm and bass | Composable pulse/rest/accent/duration rules, anticipation, syncopation, walking/ostinato bass and phrase-ending fills; coherent transitions. | Form can switch the existing three bass patterns by section. New rhythmic grammar and the named techniques remain. |
| 4. Countermelody and voice leading | Capture motif rhythm as well as contour; inversion, augmentation/diminution, imitation, question/answer and independent inner voices across phrases. | Existing contour-based countermelody and section activity are integrated. Rhythmic development and new transformations remain. Apply style-specific voice-leading rules; analysis flags are not a general musical-quality verdict. |
| 5. Instruments and difficulty | Piano hand allocation, accordion buttons, wind breathing, string sustain; solo/duet/small ensemble and simplified versions. | Existing register/button infrastructure is available. Joint playability constraints, role redistribution and instrument-specific performance evaluation remain. |
| 6. Proposal selection and editing | Three to five musically distinct complete proposals measured across harmony/rhythm/bass/register/roles/form; intent-directed local changes respecting locks. | Prior proposal replay/regeneration exists. Whole-form proposal search, quality/diversity selection and intent-to-constraint mapping remain. |

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

Every implementation step also checks source/lock preservation, full-duration
notes/rests, repeat transitions, source-bound replay, no partial result on error,
MusicXML/MSCZ round trips, native/full/viewer WASM parity and bounded resource
use. These mechanical checks establish correctness and observed differences,
not listener acceptance. No completed listening review is claimed yet.

Generation remains in core; analysis data is supplied by callers. No filesystem,
network, application settings or external AI provider is introduced into core.
Preserve old opt-in request/replay contracts and use versioned new APIs for new
behavior. Separate commits/PRs should correspond to these reviewable milestones.
