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

The shared 16-bar fixture checks section entrances, rests, dynamics, unchanged
source parts and replay. The twelve-work development corpus has ten works
with a valid countermelody base. Each of those ten has at least one compatible
five-way sectional form; 39 of the 50 attempted forms accept imitation under
their own density and collision constraints. This is structural and playback
coverage, not a listening judgment. The other two works have no accepted
countermelody base. Independent inner voices, phrase-join voice leading and
performer review remain in [the development plan](arrangement-development-v2.md).
For automatic comparison of form profiles and rhythmic treatments, use
[developed form proposals](developed-form-proposals.md).
