# Coordinated ensemble form

`arrange_ensemble_form(&Score, &EnsembleFormRequest)` compiles a complete written
section plan into the existing harmony, bass and countermelody generators.
`replay_ensemble_form` requires the original normalized source fingerprint and
the same engine/contract marker. Full and viewer WASM expose both JSON APIs.

The `ensemble` template supplies explicit chords, instruments, ranges, melody
clearance, meter groupings, voicing and density/collision limits. Each section
has a label and adjacent half-open written measure range. Optional `harmony`
and `bass` treatments select a pattern and sustained dynamic; optional
`countermelody` selects its dynamic. An absent treatment rests that role.
Template role sections, pattern sections, phrase endings and dynamics must be
empty so there is only one source of scheduling decisions.

`phrase_end_measures` is a sorted list of written phrase-ending measures. It
holds the final metrical groups of pulsed harmony and thins the countermelody.
Already-held harmony retains a single attack through the chord region. The
countermelody pauses while inactive and restarts its contour on re-entry.
Bass pattern changes are realized in a single pass, retaining preceding pitch
context and upcoming harmonic targets across section boundaries.

Every sounding generated measure carries its requested dynamic on its first
attack, so repeat/navigation entry restores the appropriate level. No marking
is added to a rest. Source parts, note IDs, dynamics and navigation remain
unchanged. The result reports written attacks by role and owning section for
every measure, plus dynamic-mark addresses and the resolved replay contract.

Plans are limited to 64 sections, 512 written measures and the existing source,
chord, generated-note and MIDI-channel limits. Invalid gaps, overlaps, labels,
phrase endings, unsupported dynamics or missing role options return no score,
resolved contract or partial reports. Existing register, density and collision
checks also apply. Caller labels describe intent; they are not automatic form
analysis. Source chord lists remain explicit in this version.

The original 16-bar development fixture in
`tests/fixtures/arrangement/form_development_v2.rs` demonstrates a quiet opening,
offbeat contrast, arpeggiated return, metered closing phrase and quiet release.
It is shared by native, I/O and browser contract tests. See
[outcomes 0–6](arrangement-development-v2.md) for the full goal and evidence gaps.
In particular contract 1 applies the same treatment to every playback visit of
a written bar; visit-specific variation and independent phrase-aware harmony
are still required before the whole-piece outcome is complete.
