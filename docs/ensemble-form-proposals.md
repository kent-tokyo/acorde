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

To regenerate the pinned development listening pack, run:

```sh
cargo run -p acorde-cli --example arrangement_v2_form_pack --offline -- \
  /private/tmp/acorde-arrangement-v2-sources-sheet \
  /private/tmp/acorde-arrangement-v2-form-new
```

The source pack can be regenerated with `arrangement_v2_source_pack`; both
output directories must be new. The form example checks the pinned source and
form manifests, source playback, source preservation, score validity, distinct
generated events, replay and MusicXML playback. Its `index.html` compares each
source with five anonymized labels in a fixed, work-specific shuffled order and
exports a JSON listening note. The manifest retains the label-to-candidate
mapping and all audio hashes. WAVs use
the same piano SoundFont, original tempo, 22,050 Hz mono and no normalization.
This isolates section treatment in the audio comparison; it does not establish
instrument-specific sound or performer approval.
