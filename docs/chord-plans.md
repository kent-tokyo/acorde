# Positioned chord plans and proposals

`extract_chord_plan` reads harmony from a canonical `Score` without moving its source notes. The default policy reads written chord symbols from all voices of part 0, staff 0. Set `ChordPlanExtractionOptions.mode` to `AnalyzeWrittenAccompaniment` and select exactly one voice to infer harmony from written block chords. That inference is opt-in and remains distinct from a written symbol.

Each `ChordPlanEntry` records a physical measure index, an exact fraction of a quarter-note beat, a convenience `offset_beats` value, the complete `ChordSymbol`, source note addresses and optional caller-supplied references. A display annotation can snap a symbol to a later melody note; the extracted plan continues to use the source onset. The caller-supplied reference is retained verbatim and is not evidence of a tune's historical origin.

```rust
use acorde_core::{
    ChordPlanExtractionOptions, ChordProposalRequest, arrange_from_chord_proposal,
    extract_chord_plan, suggest_chord_plans,
};

# fn example(melody: &acorde_core::Score) {
let extracted = extract_chord_plan(melody, &ChordPlanExtractionOptions::default());
if let (Some(plan), Some(selection)) = (extracted.plan, extracted.preset_selection) {
    // `selection` can also go directly to `arrange_accordion_preset`.
    let proposals = suggest_chord_plans(melody, &ChordProposalRequest {
        source_plan: Some(plan),
        ..Default::default()
    });
    if let Some(candidate) = proposals.candidates.first() {
        let replay = arrange_from_chord_proposal(melody, &candidate.resolved, "held-chord");
        assert!(replay.diagnostics.is_empty());
    }
    let _ = selection;
}
# }
```

The result has no complete `plan` when source selection, timing, initial coverage or conflicting voices are unresolved. `observations` and machine-readable diagnostics retain partial evidence. If a complete plan contains an offset or measure length outside the preset engine's 1/64-beat grid, or a chord quality/extension it cannot play, `preset_selection` is absent and the source plan remains intact. A triplet's `1/3`-beat position is therefore reported exactly; it is never rounded to a playable position.

`suggest_chord_plans` accepts one monophonic melody part/staff and an optional complete source plan. It returns at most four stable IDs: `provided-v1` for a supplied plan, and distinct `conservative-v2`, `simplified-v2`, and `jazz-color-v2` proposals when they can be made. The supplied plan's origin categories and references are retained. Changed harmony is marked `generated-reharmonization`, with physical changed spans and reason codes; generated chords do not inherit source references from a different chord. `provided-v1` means the host supplied the plan; Acorde does not authenticate that provenance. No provided candidate is produced from a melody alone.

The proposal engine checks note pitches and onsets, local key changes, pickup and meter lengths, and the final cadence. It accepts up to 512 written measures, 8192 source notes, and 512 source chord changes. Above those bounds it returns `source-limit` for the melody or `invalid-source-plan` for an oversized source plan; shorten the input or provide a smaller plan. Rust callers can inspect `CHORD_PROPOSAL_MAX_MEASURES`. Its chord choices are heuristic suggestions, not recovered original harmony. The `bass_mode`, `allow_jazz_color`, `max_candidates`, optional `max_changed_spans`, and `contextual_nonchord_tones` request fields bound the result. Swing belongs to the independently selected accompaniment preset; it does not change chord provenance.

Persist a candidate's `resolved` value to replay it with `arrange_from_chord_proposal`. Replay verifies the input score fingerprint and returns the preset result together with the plan metadata. The original melody staff, including spelling, rhythm, ties, repeats and endings, is preserved by the preset engine. Browser hosts can call the matching viewer WASM exports `extract_chord_plan`, `suggest_chord_plans`, and `arrange_from_chord_proposal` with JSON strings. Proposal and replay JSON inputs are limited to 2 MiB.

## Progression search (v2)

New proposals use dynamic programming across all written harmonic regions,
with fewer than 40 chord choices per region. A later cadence can influence an
earlier ambiguous choice. Melody evidence is weighted by sounding duration and
metrical position, clipped to each region; splitting a sustained pitch into
several equal-total-duration notes does not multiply its influence. Pickups use
their position at the end of the nominal first bar. Compound meters use dotted
quarter pulses. Simplified proposals penalize chord changes and coalesce
adjacent identical harmony. Standard-bass search excludes qualities unavailable
in the default button model before choosing the progression.

Jazz proposals use a dominant seventh on V, rather than changing every major
triad to a major seventh. Applied dominants are additional candidates only
when they resolve to their target root in the next written region in the same
key. For example, the C-major regression melody
`F#4 C5 | G4 B4 D5 F5 | C5` selects
`D7 | G7 | Cmaj7`. The first bar has two half notes, the second has four
quarters, and the final C is a whole note. This candidate can feed the existing
held-chord or swing-pulse preset, preserving the melody. No new accompaniment
pattern is required.

Harmony changes remain at the supplied plan's boundaries, or at each bar when
no plan is supplied to the default API. The opt-in position API below accepts
caller-selected boundaries. The key at each region's start governs its candidates.
The search scores written order; it does not optimize repeat-back transitions,
infer phrase boundaries, or perform voice-leading/fingering
optimization. These are bounded editorial suggestions; the synthetic regression
cases do not establish general musical quality on real repertoire. The search
does not yet propose suspended, added-ninth or half-diminished qualities on its
own. A supplied or locked chord of these kinds is playable in free-bass mode;
degree alterations remain unsupported by the preset engine.

Saved `*-v1` resolved
plans still replay their stored harmony without rerunning the new search.
Consumers that select newly generated candidates by ID should adopt the v2 IDs.

## Contextual melody tones

Set `contextual_nonchord_tones: true` to reduce the harmony-search weight of
short passing and neighboring notes on a weak metrical position. A passing
note moves by one or two semitones between two notes in the same direction;
a neighboring note returns by step to the preceding pitch. The notes must be
contiguous and the middle note no longer than one quarter-note beat. A tied
note carried across a harmonic boundary, or a note already sounding at that
boundary, is treated as a suspension when the next contiguous note resolves
down by one or two semitones within the new region. The carried note keeps its
full weight before the boundary. These roles receive 25% or 40% of the usual
duration-and-meter evidence; all other notes retain full weight. The selected
option is stored in `resolved`, and candidates report
`contextual-nonchord-tones`.

This is a contour-and-timing heuristic for chord proposals. It does not
identify every ornament or prove the intended harmony. With the default
`false`, existing duration-weighted results and serialized request fields stay
as before. The supplied source plan and any locked chord remain hard inputs;
tone weighting cannot overwrite them.

## Caller-selected harmonic rhythm

`suggest_chord_plans_at_positions(score, request, positions)` in Rust and
`suggest_chord_plans_at_positions(score_json, request_json, positions_json)` in
full/viewer WASM evaluate melody and progression at explicit chord-change
positions. This lets a caller request two changes within a 4/4 bar, for example
on beats 1 and 3, when the default one-chord-per-bar proposal is too sparse.
The function accepts a melody-only request: an existing `source_plan` already
defines its own harmonic boundaries and produces
`positions-conflict-source-plan` here. Use the ordinary or locked proposal API
for that plan.

Pass 1–512 ordered, distinct `ChordPlanPosition` values. Indices are zero-based
physical measures; `offset` is an exact fraction of a quarter-note beat, so
`{"measure_index":0,"offset":{"numerator":2,"denominator":1}}` means beat 3 of
the first written bar. The first position must be `{measure_index:0, offset:0/1}`.
Every position must lie inside its measure on the current 1/64-beat preset grid.
Pickups use their actual length; metre changes use each measure's own duration.
The positions are editorial input, not automatically inferred phrase boundaries.

The existing bounded progression search picks chords across these regions. Its
`simplified-v2` candidate may merge adjacent regions assigned the same chord;
other candidates keep the requested boundaries. `caller-supplied-harmonic-rhythm`
appears in candidate reason codes. Invalid, duplicate, off-grid or out-of-score
positions return `invalid-harmonic-positions` and no candidates. Candidate
`resolved` values replay through existing presets, and their
`preset_selection.chords` can feed sectioned accompaniment and voicing.

```js
const positions = [
  { measure_index: 0, offset: { numerator: 0, denominator: 1 } },
  { measure_index: 0, offset: { numerator: 2, denominator: 1 } },
  { measure_index: 1, offset: { numerator: 0, denominator: 1 } }
];
const proposals = JSON.parse(suggest_chord_plans_at_positions(
  scoreJson, "{}", JSON.stringify(positions)
));
```

## Reharmonization with locked source chords

`suggest_chord_plans_with_locks(score, request, locked_entries)` in Rust and
`suggest_chord_plans_with_locks(score_json, request_json, locks_json)` in
full/viewer WASM keep selected source harmonies while proposing the rest.
The request is the existing `ChordProposalRequest`. `locked_entries` is a list of zero-based indices into
`request.source_plan.entries`, **not measure indices**. In WASM, pass a JSON
array such as `[0, 2]`. An empty list gives the exact existing proposal result.

A lock covers the source entry's full region: its position through the next
source entry, or through the score end. This includes changes within a measure
and regions spanning several measures. Every returned candidate retains that
entry's chord spelling, quality, slash bass, position, origin and references.
Simplified harmony keeps locked boundaries even beside an identical chord.
Locks enter the progression search as hard constraints, so the surrounding
unlocked choices adapt to them. A generated applied dominant must still resolve
to the next region's root; a host-authored locked chord is retained regardless
of its melodic fit or inferred function.

The source plan must be complete and satisfy the existing validation rules.
Locked chords must fit the selected bass mode and preset vocabulary. Unsupported
extensions, standard-bass-incompatible qualities or off-grid locked positions
are rejected rather than replaced. Lock indices may be unordered or repeated;
at most 512 supplied indices are accepted, counting duplicates. Invalid locks
return no candidates, including no `provided-v1` fallback, even when
`max_candidates` is 1. Diagnostics include `locks-require-source-plan`,
`invalid-chord-lock`, `chord-lock-limit`, `locked-chord-unsupported` and
`locked-chord-grid-unsupported`.

Candidate IDs, deduplication and the maximum of four candidates stay the same.
Nonempty locks add the reason code `locked-source-harmony`. Locking every entry
leaves just the unchanged `provided-v1` candidate. Persist `resolved`
to replay the stored plan with the existing API; replay does not regenerate
harmony or require the locks. Retain the source plan and lock indices separately
when offering another round of editing, and update indices if that source plan
is reordered or edited. The input fingerprint still identifies the score only.

## Limit the amount of reharmonization

Set `max_changed_spans` in `ChordProposalRequest` to an integer from 0 through
16 when a `source_plan` is present. This is a hard limit on the number of source
harmonic regions whose sounding chord differs from the supplied plan. The
progression search keeps the source chord available at each region and tracks
the number of changes while choosing the complete progression. A limit of zero
keeps every source chord; a larger limit lets the search choose which regions
benefit most from change. Simplified proposals may still merge adjacent
boundaries that carry the same chord, changing accompaniment attacks without
counting that as a changed harmony region. Compare `changed_spans` and the
resolved plan to see both effects.

The limit works with `suggest_chord_plans_with_locks`: locked entries cannot
change and do not spend the budget. Unchanged entries retain their source
origin and references; changed entries receive generated provenance. Persisted
`resolved` proposals record the selected limit, but replay uses their stored
plan and does not search again. Candidates report `bounded-harmony-change`.
No source plan, a limit above 16, or a source chord unavailable to the selected
bass mode returns `change-budget-requires-source-plan`,
`invalid-change-budget`, or `change-budget-source-unsupported` with no
candidates. Omit the field to keep the previous unconstrained search.

For a source plan with at least three entries, this keeps its first and third
harmonic regions and allows at most one change elsewhere:

```js
const request = {
  source_plan: sourcePlan,
  bass_mode: "free-bass",
  max_changed_spans: 1
};
const proposals = JSON.parse(suggest_chord_plans_with_locks(
  scoreJson, JSON.stringify(request), JSON.stringify([0, 2])
));
const candidate = proposals.candidates.find(c => c.id === "jazz-color-v2");
if (candidate) {
  const replay = JSON.parse(arrange_from_chord_proposal(
    scoreJson, JSON.stringify(candidate.resolved), "held-chord"
  ));
  // candidate.preset_selection.chords also feeds sectioned accompaniment/voicing.
}
```

## Preset availability diagnostics

Chord proposals describe harmony, not preset availability. Call `assess_chord_presets(score, request)` in Rust or the matching viewer WASM function with score and request JSON strings to check every built-in preset against every proposed harmony. Each assessment identifies the candidate and preset, whether the engine accepted it, and the original diagnostic codes, messages, and zero-based measure indices. Each preset reports the first failure encountered by the arrangement engine, so another issue may become visible after that failure is addressed. The `source_range_violations` list identifies each offending source note, MIDI pitch and strict right-hand range, even when another failure prevents a preset from reaching its range check. If all assessed combinations fail, the result includes `no-compatible-preset`; if proposal generation fails, inspect `proposals.diagnostics`. An accepted result does not certify a particular physical instrument. The existing strict preset ranges and rhythm rules remain in effect; this API does not transpose, alter the source, or create partial arrangements.

Assessment is opt-in and limited to 128 measures and 8192 notes across the score; exceeding either returns `assessment-limit-exceeded` without running the presets. To create an arrangement, replay an accepted candidate with `arrange_from_chord_proposal` and retain its resolved metadata.
