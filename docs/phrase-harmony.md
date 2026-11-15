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
};
```

For `two-groups-per-bar`, an empty `meter_groups` uses Acorde's default beat
grouping. An authored list gives denominator-unit group lengths and must sum
to each affected bar's meter numerator. The default midpoint is beat 2 in
4/4, beat 1.5 in 6/8 and 5/8, and beat 2 in 7/8. The chosen midpoint must fit
the existing chord-position grid. Meter changes are checked bar by bar.

If `proposal.source_plan` is supplied, its change positions must match every
generated position. The returned candidates can then preserve source chords
and `locked_chord_entries` through the existing progression search. A mismatch
returns `source-harmonic-positions-mismatch` with no candidates. With no source
plan, locks are invalid. This first API does not move existing chords to a new
rhythm; that requires an explicit reanchoring rule so chord spelling, bass,
provenance and locks cannot silently shift.

The phrase schedule must cover every written measure once, in order. Invalid
gaps, overlaps, groupings and limits return no positions or candidates. The
limits are 64 phrases and 512 change positions. Candidate results retain the
source fingerprint and resolved-plan replay contract of the underlying chord
proposal API. The next harmony work adds reanchoring, supplied local keys,
borrowed and passing chords, and cadence constraints.
