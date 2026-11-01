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

`suggest_chord_plans` accepts one monophonic melody part/staff and an optional complete source plan. It returns at most four stable IDs: `provided-v1` for a supplied plan, and distinct `conservative-v1`, `simplified-v1`, and `jazz-color-v1` proposals when they can be made. The supplied plan's origin categories and references are retained. Changed harmony is marked `generated-reharmonization`, with physical changed spans and reason codes; generated chords do not inherit source references from a different chord. `provided-v1` means the host supplied the plan; Acorde does not authenticate that provenance. No provided candidate is produced from a melody alone.

The proposal engine checks note pitches and onsets, local key changes, pickup and meter lengths, and the final cadence. It accepts up to 512 written measures, 8192 source notes, and 512 source chord changes. Above those bounds it returns `source-limit` for the melody or `invalid-source-plan` for an oversized source plan; shorten the input or provide a smaller plan. Rust callers can inspect `CHORD_PROPOSAL_MAX_MEASURES`. Its chord choices are heuristic suggestions, not recovered original harmony. The `bass_mode`, `allow_jazz_color`, and `max_candidates` request fields bound the result. Swing belongs to the independently selected accompaniment preset; it does not change chord provenance.

Persist a candidate's `resolved` value to replay it with `arrange_from_chord_proposal`. Replay verifies the input score fingerprint and returns the preset result together with the plan metadata. The original melody staff, including spelling, rhythm, ties, repeats and endings, is preserved by the preset engine. Browser hosts can call the matching viewer WASM exports `extract_chord_plan`, `suggest_chord_plans`, and `arrange_from_chord_proposal` with JSON strings. Proposal and replay JSON inputs are limited to 2 MiB.
