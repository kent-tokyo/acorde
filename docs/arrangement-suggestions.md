# One entry point for arrangement proposals

`suggest_arrangements(&Score, &ArrangementRequest)` runs one of the six proposal engines
and returns its candidates in one shape. `replay_arrangement(&Score, &ResolvedArrangement)`
rebuilds a candidate from its contract. Both are behind the `unstable-arrangement`
feature of `acorde-core` and carry no SemVer promise
([#497](https://github.com/kent-tokyo/acorde/issues/497)).

The engines keep their own ranking, limits and source checks. This entry point only routes
the request and normalizes the result, so the scores are the same as each engine's own
API returns. The existing per-engine functions are unchanged.

| `engine` | Engine function | Request type |
| --- | --- | --- |
| `roles` | `suggest_arrangement_proposals` | `ArrangementProposalRequest` |
| `integrated` | `suggest_integrated_arrangements` | `IntegratedArrangementRequest` |
| `harmony-groove` | `suggest_harmony_groove_arrangements` | `HarmonyGrooveRequest` |
| `ensemble-form` | `suggest_ensemble_form_proposals` | `EnsembleFormProposalRequest` |
| `developed-form` | `suggest_developed_form_proposals` | `EnsembleFormProposalRequest` |
| `cross-plan-form` | `suggest_cross_plan_forms` | `CrossPlanFormRequest` |

## Request and result

```json
{
  "engine": "integrated",
  "request": { "...": "the engine's own request" },
  "assessment": {
    "request": { "melody_part": 0 },
    "limits": { "max_melody_semitone_clash_ticks": 960, "max_outer_parallels": 0 }
  }
}
```

Each candidate has an `id`, the `score`, a tagged `resolved` contract
(`{"engine": "...", "resolved": {...}}`) and the engine's `difference_codes`. Engines without
candidate IDs get `<engine>-<index>`. The harmony/groove engine reports
`profile-plain`, `profile-color` or `profile-rhythm` as its difference code.

With `assessment`, every candidate gets an [arrangement assessment](arrangement-assessment.md).
A candidate over any limit moves to `set_aside` with the names of the exceeded limits. The
others stay in `candidates` in the engine's order. `engine_rejections` counts the
rejections or diagnostics the engine reported.

An engine error, an invalid assessment request or a failed replay returns
`ArrangementSuggestionError` and no partial result. Replay checks that the score is the
source the contract was made from.

## Tests

`crates/core/tests/arrangement_suggestions.rs` checks for every engine that the unified
call returns the same sounding events in the same order as the engine's own API. It also
checks that each candidate replays from its contract after a JSON round trip, that limits
set aside exactly the candidates over them, and that errors are atomic.

Not yet included: WASM and CLI exposure. Region regeneration still uses the per-engine
functions.
