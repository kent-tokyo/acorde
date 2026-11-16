# Rhythmic countermelody in a written ensemble form

`arrange_ensemble_form_development` combines an explicit `EnsembleFormRequest`
with one `CountermelodyDevelopmentKind`. It composes harmony and bass through
the existing form engine, then develops the source melody's note/rest rhythm
for the countermelody only in sections that enable that role. Inactive bars
contain rests. Section dynamics and per-bar reports describe the resulting
parts, and the existing density and vertical collision rules apply to all
three generated roles.

The request must supply countermelody options and at least one active
countermelody section. Invalid form, unsupported motif rhythm, unplayable
counterpoint, collisions and excess density return a typed error with no
partial score. A `Diminution` request can therefore fail where `Imitation`
succeeds under the same density limit. The original source parts remain
unchanged. The existing `arrange_ensemble_form` and
`arrange_countermelody_development` contracts retain their prior behavior.

The result contains a source-bound, versioned
`ResolvedEnsembleFormDevelopment`. Use
`replay_ensemble_form_development` with that outer contract to reproduce the
developed score; its nested form contract records the form request but does
not by itself represent the rhythmic treatment. Rust and full/viewer WASM
expose arrangement and replay as JSON operations.

`arrange_ensemble_form_development_with_phrase_joins` adds a maximum leap of
0–24 semitones between the final countermelody pitch of an authored phrase and
the first pitch of the next active phrase. The generator chooses a permitted
chord tone within the instrument range and below the overlapping melody. A
silent countermelody section resets the link. If no pitch meets the bound, the
request fails atomically with `phrase-join-unavailable` and a measure index.
The optional bound is stored in the resolved contract and honored by the same
replay function. The original call still omits the field and keeps its prior
playback. This rule applies to adjacent written phrases; repeated-visit edges
and independent inner voices still need separate work.

The shared 16-bar fixture checks section entrances, rests, dynamics, unchanged
source parts and replay. The twelve-work development corpus has ten works
with a valid countermelody base. Each of those ten has at least one compatible
five-way sectional form; 39 of the 50 attempted forms accept imitation under
their own density and collision constraints. This is structural and playback
coverage, not a listening judgment. The other two works have no accepted
countermelody base. With a four-semitone phrase-entry bound, each of the ten
works has at least one compatible form. In the 16-bar fixture, a return that
previously entered three semitones below the last phrase note can instead
enter on the same pitch. A contrasting F-major return needs two semitones;
a one-semitone bound rejects it at measure 8. This is a concrete register
constraint, not a listener judgment. Independent inner voices, repeated-visit
joins and performer review remain in
[the development plan](arrangement-development-v2.md).
For automatic comparison of form profiles and rhythmic treatments, use
[developed form proposals](developed-form-proposals.md).
