# Arrangement capability: six completion gates

This document tracks the six requested outcomes as implementation and evidence,
not as a count of new pattern names. A completed gate needs the stated behavior
in the current Rust and full/viewer WASM builds, structural tests, export/playback
round trips, and the musical comparison described below. Keep the existing
unreleased APIs and unrelated checkout changes intact while iterating.

| Gate | Required behavior | Current evidence and remaining work |
| --- | --- | --- |
| 1. Comparison baseline | Fixed development and untouched holdout melodies spanning major/minor, pickup, 3/4, 4/4, 6/8, 5/8, mid-bar changes, repeats and multiple parts. Save source identity, resolved plan, note-onset/duration and pitch-class summaries for each proposal. | `tests/fixtures/arrangement/corpus-v1.json` fixes eight original cases with a 4/4 development/holdout split and one repeat. `baseline-v1.json` freezes source fingerprints, resolved plans and playback summaries for five profiles per case. `arrangement-corpus-v1-review.md` compares their structural results. The three reproducible development listening packs save exact source JSON, resolved proposal contracts, MusicXML/WAV SHA-256, and per-part written and repeat-expanded onset, duration and pitch-class summaries for all 22 unified candidates. A separate verifier recalculates these summaries from full/viewer WASM replay. The listening form binds source/candidate identities and WAV SHA-256; a review checker validates a submitted JSON against the actual WAV bytes. Listening judgments and a later untouched holdout review remain. |
| 2. Voicing and independent bass | Choose root/nearby inversions, closed/open positions and melody clearance within program ranges; add a separate bass part with root/fifth and approaching motion. Preserve source parts and report motion. | Chordal-pad `smooth` chooses close inversions and `open` spreads an inner tone in written order. Optional clearance constrains the entire harmony region beneath a selected melody staff. `global-smooth` uses bounded search over repeat-expanded chord-change edges. `arrange_bass_line` adds a separate root/fifth/approach part. Pad and bass motion are reported in written and playback order. Matched listening review remains before calling this gate complete. |
| 3. Meter and phrase | Keep accents, rests and attacks on the intended pulse across chord changes, compound/irregular meters and sections; vary density at an authored phrase ending. | Chordal-pad `meter-pulse` uses validated denominator-unit groupings, rests and accents; chord changes force attacks without shifting the next group pulse. Authored ending measures hold the last two groups, and the resolved grouping/start ticks are reported. Unit tests cover 6/8, both 5/8 groupings and section changes; the fixed five-profile corpus also covers 3/4, 4/4, 7/8 and a pickup. The 4/4 eight-measure development phrase authors 2+2 groups and endings at bars four/eight, producing five distinct harmony-onset sequences. A separate 6/8 eight-measure development phrase uses 3+3 groups and rests its countermelody at both endings. Matched listening review remains. |
| 4. Harmony and reharmonization | Distinguish structural melody tones from passing, neighboring and suspended tones; realize supported added/suspended/half-diminished chords; offer bounded change amount while respecting locks and provenance. | Progression search, caller positions and locked entries exist. Opt-in contextual weighting distinguishes weak passing/neighbor tones and resolved suspensions. Free-bass presets, chordal pad and independent bass realize sus2, sus4, major/minor add9 and half-diminished qualities; the five kinds round-trip through MusicXML and MSCZ, and locked plans replay. `max_changed_spans` bounds source-region changes while preserving locks/provenance. Matched listening review remains before calling this gate complete. |
| 5. Whole-piece roles | Arrange melody, bass, harmony and countermelody as independently switchable parts/sections, with register, density and collision checks. | `arrange_ensemble_roles` composes separate harmony, bass and motif-derived countermelody parts with independent measure activity, optional per-role sustained dynamics, melody clearance, pairwise register checks and generated-attack limits while preserving source parts. Opt-in `arrange_ensemble_with_melody` creates a performance copy with independently scheduled melody rests and a countermelody handoff while leaving the input source untouched; a muted measure closes adjacent surviving melody ties, including partial chord ties, while preserving ties into an active alternate ending. Authored phrase endings thin countermelody attacks and invert the next phrase contour. An opt-in section-entry flag pauses countermelody development while silent and restarts the motif at its next entrance; omitted flags retain prior replay behavior. The 4/4 eight-measure regression covers two separated countermelody rests, thinner endings, complete-proposal replay and locked region regeneration. A 6/8 eight-measure fixture tests one-bar countermelody answers at measures three and seven with silent endings. Each listening pack includes five distinct profiles, two with countermelody, across three harmonic plans. Rust, WASM JSON and MusicXML/MSCZ round trips cover the handoff, including a tied melody. Matched listening review and longer phrase-shape listening judgment remain. |
| 6. Distinct proposal set | Return 3–5 reproducible, musically different complete arrangements; deduplicate by harmony, bass, attacks, density and orchestration; explain differences and regenerate a selected region with locks. | `suggest_arrangement_proposals` combines bounded chord plans with seven pad/bass/countermelody profiles, including close-voicing/root-fifth alternatives when narrow ranges or bass collisions reject open/approaching textures. It returns exactly 3–5 complete scores or an atomic failure, deduplicates written role/instrument/attack/duration/pitch/articulation events, reports structural difference codes, and replays from a source-bound contract. A three-candidate request tries answer textures early when a countermelody is supplied. Region regeneration preserves locked roles/measures/chord spans and all outside measures; it can add or rest a countermelody locally. All eight fixed development/holdout sources, including multipart and irregular-meter cases, produce three structural variants and pass replay and region checks; Rust/WASM JSON and MusicXML/MSCZ checks cover focused examples. Separate 4/4 and 6/8 eight-measure development packs each produce five profiles across three harmonic plans, with two countermelody textures. Full and viewer Chrome WASM suites pass. Matched listening review and further phrase-level/orchestration judgment remain before completion. |

## Next execution sequence

1. **Freeze the comparison contract.** Keep the eight source scores and their
   development/holdout split fixed. For every proposal, retain the source
   fingerprint, resolved harmony and role options, per-measure attacks, written
   and repeat-expanded note events, and export identity. Re-run focused tests,
   the full Rust suite, Clippy and both WASM build configurations after edits.
2. **Review the development audio.** Compare each source with its three
   proposals at the same tempo, sound source and gain. Record for each case the
   clearest and weakest proposal, the exact measure and role of any melody
   masking, register clash, awkward bass leap, misplaced accent or weak phrase
   ending, any distracting volume change, and whether the proposals sound
   meaningfully different. The verifier reports full-mix RMS relative to the
   source as a signal-level cue, not a substitute for listening.
3. **Improve the weakest musical axis first.** Use the review to choose a
   bounded change in harmony, voicing/bass, meter/phrase, role handoff or
   countermelody. Keep it opt-in, preserve source parts and locks, and add a
   focused regression case for the reported measure. A new pattern name alone
   does not count as improved diversity.
4. **Verify the complete proposal workflow.** Require 3–5 deterministic,
   complete candidates; compare harmony, bass, per-measure density, attacks,
   articulation and orchestration; replay each candidate from its resolved
   contract; regenerate a selected region with chord, role and measure locks;
   verify MusicXML/MSCZ round trips and no change outside the selection.
5. **Open the holdout once.** After development choices are fixed, compare the
   untouched four cases under the same listening conditions and record any
   failures without tuning to them. Run the full/viewer browser WASM test in
   Chrome. Mark a gate complete only when its structural, export and listening
   evidence all pass; otherwise retain the concrete failure and next change.

The first comparison uses the same melody, tempo, playback sound source and
volume for each candidate. Mechanical checks cover unchanged source note IDs,
exact written and playback timing, chord pitch classes, register, MIDI channel,
repeat expansion, no partial result on error, and MusicXML/MSCZ round trips.
Motion and difference measures establish what changed; they do not establish
musical quality. A listening review of the fixed development set and untouched
holdout is required before claiming a style or general improvement.

Implement new generation in `acorde-core` using `Score` and `ChordPlan`.
`acorde-wasm` exposes matching bounded JSON APIs. Keep I/O, filesystem access,
app settings and AI commands out of core. Each opt-in request needs explicit
limits, a stable resolved contract, and replay that reproduces pitches and
timing even if generated note IDs differ.
