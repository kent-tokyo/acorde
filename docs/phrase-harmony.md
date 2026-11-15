# Phrase harmonic rhythm

`suggest_phrase_harmony` builds chord-change positions from a complete,
nonoverlapping schedule of written measure ranges. Each phrase chooses one of
three paces:

| Pace | Change positions |
| --- | --- |
| `every-second-bar` | First bar and every other bar within the phrase |
| `every-bar` | Start of each bar |
| `two-groups-per-bar` | Start and middle meter-group boundary of each bar |

The caller supplies the phrases and the existing `ChordProposalRequest`; the
engine returns the selected positions, the ordinary bounded chord proposals,
and diagnostics. The score is never changed. Rust uses
`acorde_core::suggest_phrase_harmony(&score, &request)`; full and viewer WASM
use `suggest_phrase_harmony(score_json, request_json)`.

For example, a four-bar 4/4 passage can put one chord across the first two
bars, then use two changes in each of the last two bars:

```rust
let request = PhraseHarmonyRequest {
    phrases: vec![
        PhraseHarmonySection {
            start_measure: 0,
            end_measure_exclusive: 2,
            pace: PhraseHarmonicPace::EverySecondBar,
            meter_groups: vec![],
        },
        PhraseHarmonySection {
            start_measure: 2,
            end_measure_exclusive: 4,
            pace: PhraseHarmonicPace::TwoGroupsPerBar,
            meter_groups: vec![],
        },
    ],
    proposal: ChordProposalRequest::default(),
    locked_chord_entries: vec![],
    source_reanchor: None,
    local_keys: vec![],
};
```

For `two-groups-per-bar`, an empty `meter_groups` uses Acorde's default beat
grouping. An authored list gives denominator-unit group lengths and must sum
to each affected bar's meter numerator. The default midpoint is beat 2 in
4/4, beat 1.5 in 6/8 and 5/8, and beat 2 in 7/8. The chosen midpoint must fit
the existing chord-position grid. Meter changes are checked bar by bar.

If `proposal.source_plan` is supplied with no `source_reanchor`, its change
positions must match every generated position. The returned candidates can
then preserve source chords and `locked_chord_entries` through the existing
progression search. A mismatch returns `source-harmonic-positions-mismatch`
with no candidates. With no source plan, locks and reanchoring are invalid.

To change an existing plan's harmonic rhythm, set `source_reanchor` to one
source-entry index per generated position. For four source chords at bar starts,
`[0, 0, 1, 1, 2, 2, 3, 3]` splits each bar into two chord regions. A slower
two-bar pace can use `[0, 2]`; the result reports source entries 1 and 3 in
`omitted_source_entries`. Indices must be in source order, and the first must
be zero. The result includes the provisional `reanchored_source_plan` so a
caller can inspect every moved boundary before choosing a proposal.

The original chord spelling, slash bass and source references are copied to
each mapped entry. An entry moved to another boundary is marked
`generated-reharmonization`; an unchanged boundary keeps its original origin.
For each locked original entry, its starting boundary and the next original
boundary remain fixed. Inserted boundaries inside that interval inherit the
locked chord and source reference. A mapping that drops or moves a locked
region fails atomically with `locked-source-region-moved`.

`max_changed_spans` is checked against the **original** source plan after
reanchoring. Every original harmonic region whose sounding chord changes counts
once, even if the new plan splits it into several regions. Returned
`changed_spans` also compare with the original plan. A request that cannot
meet its budget returns no candidate and
`change-budget-exceeded-after-reanchor`.

`local_keys` supplies key regions for chord search without editing the source
score. For example, `ChordKeyRegion { start_measure: 2,
end_measure_exclusive: 4, key: KeySignature { fifths: 1,
mode: "major".into() } }` treats bars 2–3 as G major. Gaps use the score's
written key signature, including written local changes. Regions must be in
order, nonoverlapping and begin and end at bar-start chord boundaries (the
score end is also allowed). Major and minor keys with −7 to +7 fifths are
supported. Invalid regions return no positions or candidates. The result
records `applied_local_keys`; candidate reason codes mark the supplied key.
Selected chord plans remain bound to the original score fingerprint and replay
against the unmodified score.

The phrase schedule must cover every written measure once, in order. Invalid
gaps, overlaps, groupings and limits return no positions or candidates. The
limits are 64 phrases and 512 change positions. Candidate results retain the
source fingerprint and resolved-plan replay contract of the underlying chord
proposal API. The next harmony work adds borrowed and passing chords, and
explicit cadence constraints.
