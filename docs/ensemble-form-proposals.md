# Sectional ensemble form proposals

`suggest_ensemble_form_proposals` returns three to five complete arrangements
from a source `Score`, a selected chord plan and instrument ranges. It varies
role entrances, harmony and bass patterns, countermelody activity and dynamics
at caller-supplied section boundaries. The source's original parts are retained.

The request contains `ensemble: EnsembleRoleOptions`, adjacent labeled
`sections: Vec<FormProposalBoundary>` covering every written measure,
optional zero-based `phrase_end_measures`, and `count` from 3 to 5. Supply
harmony and bass options in `ensemble`; countermelody is optional. Existing
role-activity and pattern sections in the template conflict with form
generation. An empty phrase-end list uses the last measure of each section.
The API returns a typed error and no partial result when it cannot produce the
requested number of distinct valid candidates.

Each candidate includes a score, a source-bound `ResolvedFormProposal`,
per-measure attack counts and difference codes relative to the first candidate.
Distinctness is checked using generated playback events, not just names or
request settings. `replay_ensemble_form_proposal` rebuilds a selected score
against the same source; the contract version and source fingerprint are
checked. The same two operations are exposed as JSON functions in full and
viewer WASM builds.

When an independently generated countermelody collides with harmony, the
generator can omit it in that section and retry. The indices of omitted
sections are recorded in the resolved contract. Other realization failures
remain errors. This bounded repair is a structural constraint, not a claim
that the resulting texture is musically preferred.

The development corpus contains twelve independently authored 16–32-bar
sources. Mechanical tests require five distinct forms for every source and
preserve source playback, validity and replay. MusicXML/MSCZ tests cover the
16-bar fixture. Instrument-specific playback and blinded listening review
remain required by [the development plan](arrangement-development-v2.md).
