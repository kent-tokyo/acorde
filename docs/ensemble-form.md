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
An active section's optional `FormBass.rhythm` supplies authored bass slots
instead of its base bass pattern. Sections can therefore place walking motion,
syncopated rests, an anticipation or a phrase-ending fill in selected bars;
the same slot lengths must cover every bar of that section. The resulting form
and its rhythm instructions remain in the source-bound replay request.

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
[outcomes 0–6](development/arrangement-development-v2.md) for the full goal and evidence gaps.
Contract 1 applies the same treatment to every playback visit of a written
bar. The separate visit-specific API below can change a later visit.

`arrange_ensemble_form_visits` adds a second, opt-in performance-order contract.
Its ordered variations select zero-based visits in the repeat-expanded sequence.
Each variation supplies an alternative complete written plan. For its selected
visit, generated playback events replace the base plan's generated events;
source-part playback events remain those of the base. A written base score is
returned separately for notation and ordinary MusicXML/MSCZ export. The event
stream carries the visit-specific sound. It is replayable against the same
source and engine marker. Unchanged alternatives, invalid visit indices,
incompatible part layouts and timing that cannot be assigned unambiguously to
visits fail atomically.

The 16-bar development score repeats its first eight bars. Visit 0 retains a
held chord, while visit 8 can play an eight-attack arpeggio over the same first
bar. All source-melody events and the other 23 visits retain their prior
pitch, onset, duration and velocity. The returned written score shows the base
form.

`arrange_ensemble_form_visits_score` returns a separate performance score whose
bars follow playback order, with visit-specific generated notes written into
their selected bars. `written_measures_by_visit` maps each performance bar back
to its original written bar. Repeat and navigation marks are removed from the
unfolded bars, and distinct note IDs are assigned per visit. The score can be
serialized with MusicXML or MSCZ. The API verifies its complete playback against
`arrange_ensemble_form_visits` and returns no score on a mismatch. Replay checks
the original source fingerprint and contract marker. Source scores with
spanners, views or object style overrides currently return
`unsupported-form-performance-refs`, since those references need explicit
relocation. The compact written base and event-only APIs remain available.
