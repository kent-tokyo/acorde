# Layered accordion arrangement (contract v1)

`acorde_core::arrange_with_layers` and the viewer WASM export
`arrange_with_layers(score_json, options_json)` take a validated, one-part,
one-staff monophonic melody. Chords must be positioned explicitly; the API does
not infer harmony. It returns a canonical `Score` or structured diagnostics,
resolved options and chords, and source/generated `NoteAddr` lists. Failure
returns no partial score. The source melody staff is cloned, preserving its
note identity, notation, pickup, repeats and endings.

`LayeredAccordionOptions` selects independent layers:

- `accompaniment`: `held-bass-chord`, `bass-chord-pulse`,
  `alternating-bass-chord`, `arpeggio`, `rising-arpeggio`, `alberti-bass`,
  `offbeat-chords`, `swing-accompaniment`, `walking-bass`,
  `waltz-bass-chord` or `waltz-alternating-bass-chord`; `null` leaves the
  left-hand voice empty.
- `alternative_chords`: optional explicit replacement progression. Inversion
  may be `root`, `first`, `second`, `third` or `nearby`; the latter chooses the
  nearest available bass pitch class to the prior resolved bass.
- `melodic_voice`: `unchanged`, `third-below`, `sixth-below` or
  `octave-below`. An added voice uses the resolved chord tones and remains
  separate from the melody. Incompatible chord tones produce diagnostics.
- `swing`: `straight` or `notated-accompaniment-two-to-one`. The latter writes
  quarter/eighth triplet pairs in the accompaniment, and sets `notated_swing`
  in the result. Hosts should leave `PlaybackOptions.swing` unset for this
  output to avoid applying a second timing shift. This fixed 2:1 teaching
  pattern does not describe every swing performance.
- `bass_mode`: `standard-bass` or `free-bass`. The default standard-bass
  model assumes twelve bass roots and major, minor and dominant-seventh chord
  buttons, with an ideal four-note seventh voicing. It rejects augmented and
  diminished triads, major/minor sevenths and melodic bass patterns. This is
  not an instrument inventory or fingering proof; real seventh buttons may omit
  the fifth. For explicit button pitches and omission reports, use the
  [profile-based simplified arrangement](standard-bass-simplified-arrangement.md)
  or the [profiled bass-pulse preset](accordion-preset-contract.md#standard-bass-buttons-and-dominant-sevenths).

Supported chord kinds are `major`, `minor`, `diminished`, `augmented`,
`major-seventh`, `minor-seventh` and `dominant`. Slash bass and chord changes
use the same positioned chord contract as `arrange_with_explicit_chords`.
The waltz patterns use 3/4: bass on beat 1, chord on beats 2 and 3. A first
pickup starts at the matching final beat of the nominal bar and does not count
toward root/fifth alternation; the first complete bar starts with root bass.
Other patterns use their documented measure phase. A chord change keeps the
phase; repeating the same chord within a measure leaves sound and timing
unchanged.
Generated notes are limited to the documented conservative hand ranges in
`arrange.rs`; unsupported rhythms, source structures, harmony and ranges
produce diagnostic codes with a measure index where available.

The output score feeds the ordinary SVG, playback, MusicXML and other export
paths. The API does not migrate any host application by itself. A host can
replace its local pattern and pitch algorithms after it adopts this contract.

## Accompaniment sections

`arrange_with_sections(score, options)` in Rust and
`arrange_with_sections(score_json, options_json)` in full/viewer WASM choose
accompaniment by explicit physical measure ranges. This supports an opening
with held chords, a more active middle, a swing section and a silent ending in
one canonical score. The caller selects the ranges; the API does not infer
phrase boundaries or form.

`SectionedAccordionOptions` supplies positioned `chords`, `bass_mode`, a
`default_pattern` and optional `sections`. Every section has inclusive,
zero-based `first_measure` and `last_measure` indices and a `pattern`.
`pattern: null` writes rests in the accompaniment while the melody continues.
Uncovered measures use `default_pattern`, which may also be `null`. Sections
can arrive unordered; the resolved options sort them by position. Overlaps,
reversed ranges and indices outside the score are rejected. Printed measure
numbers, including a pickup numbered 0, do not change these indices.

The generator reads the entire score in one pass. Melody notes and notation,
melody note identifiers, bar lengths, keys, tempo, repeats and endings are retained.
Harmony written during a silent section carries into the next sounding bar;
even an entirely silent accompaniment requires a valid explicit chord plan.
Patterns keep their metrical phase through chord changes. A new contiguous run
of alternating-bass waltz starts with root bass; adjacent sections requesting
the same pattern continue the run. A first pickup occupies the final beats of
its nominal bar and does not advance root/fifth alternation. Sections are tied
to written measures and repeat with them; the second traversal does not choose
a new pattern automatically.

Each active pattern keeps its existing meter, rhythm and bass-mode restrictions.
For example, driving accompaniment can play a 4/4 section, waltz a 3/4 section,
and habanera a 2/4 section in one request. The default standard-bass model accepts
major, minor and dominant harmonies; arpeggios and walking bass require free
bass. Driving sections retain their 200 BPM standard-bass screening limit,
using inherited tempo and ramps. Fixed instrument button profiles and physical
playability remain separate from this idealized model.

The result has `score`, `resolved_options`, one `resolved_patterns` entry per
measure, source/generated note addresses and structured diagnostics. Silent
accompaniment rests are excluded from `generated_notes`, as in the guided API.
Swing sections write their triplet timing into notes; `notated_swing` and
`notated_swing_measures` identify them. Leave `PlaybackOptions.swing` unset for
this output. Errors return no score, note mappings or partial resolved schedule.
Inputs are limited to 512 measures, 8192 source notes, 512 chord changes and
512 sections, with at most 32 quarter-note beats per measure and 8192 in total.

This example requires at least four source measures. A chord proposal's
`preset_selection.chords` can provide `chords`; retain its plan separately for
harmony provenance.

```js
const result = JSON.parse(arrange_with_sections(scoreJson, JSON.stringify({
  chords,
  bass_mode: "free-bass",
  default_pattern: "held-bass-chord",
  sections: [
    { first_measure: 1, last_measure: 1, pattern: "bass-chord-pulse" },
    { first_measure: 2, last_measure: 2, pattern: "swing-accompaniment" },
    { first_measure: 3, last_measure: 3, pattern: null }
  ]
})));
if (result.score) {
  const svg = render_score_svg(JSON.stringify(result.score), "{}");
  // Display svg and show result.diagnostics when score is null.
}
```

## Independent right-hand harmony sections

`arrange_sections_with_melodic_voices(score, accompaniment, melodic)` in Rust
and `arrange_sections_with_melodic_voices(score_json, accompaniment_json,
melodic_json)` in full/viewer WASM add an independent right-hand schedule to
`arrange_with_sections`. The `accompaniment` argument is the existing
`SectionedAccordionOptions`. The `melodic` argument selects `default_voice`
and optional inclusive, zero-based `sections` with `first_measure`,
`last_measure`, and `voice`. Voices are `unchanged`, `third-below`,
`sixth-below`, or `octave-below`. For example, use `unchanged` in the verse,
`third-below` in the chorus, and `octave-below` in the ending while changing
left-hand patterns on different measure ranges.

The output wraps the section result as `arrangement` and reports one
`resolved_melodic_voices` value per physical measure. An unchanged section
writes rests into the second right-hand voice when any other section uses it.
The original melody, its notation and note IDs stay intact. Added thirds and
sixths require a chord tone at the requested interval; if none fits, the
result reports `incompatible-harmony` and returns no score or note mappings.
Overlapping, reversed, out-of-score, or more than 512 melodic sections are
rejected. The selected ranges follow written measures through repeats and
endings. Octave ties stop where the selected right-hand voice changes at a
barline. The current harmony voice selection works one melody note at a time;
it does not optimize counterpoint across notes.

```js
const result = JSON.parse(arrange_sections_with_melodic_voices(
  scoreJson,
  JSON.stringify({
    chords, bass_mode: "free-bass", default_pattern: "held-bass-chord",
    sections: [{ first_measure: 2, last_measure: 3, pattern: "bass-chord-pulse" }]
  }),
  JSON.stringify({
    default_voice: "unchanged",
    sections: [{ first_measure: 2, last_measure: 3, voice: "third-below" }]
  })
));
if (result.arrangement.score) {
  render_score_svg(JSON.stringify(result.arrangement.score), "{}");
}
```

## Accompaniment dynamics by section

`arrange_sections_with_dynamics(score, accompaniment, schedule)` in Rust and
`arrange_sections_with_dynamics(score_json, accompaniment_json, dynamics_json)`
in full/viewer WASM add a staff-local loudness schedule to the existing
sectioned accompaniment generator. The `accompaniment` argument is
`SectionedAccordionOptions`. The dynamic schedule has a `default_dynamic`
and optional inclusive, zero-based `sections` with `first_measure`,
`last_measure` and `dynamic`. Use sustained levels from `Pppp` through `Ffff`
(Rust `Dynamic` names and the same JSON strings). One level is resolved for
every physical measure; sections may arrive unordered but cannot overlap.

The first sounding accompaniment note of each measure receives the selected
dynamic. Silent measures receive no marking. Marking every sounding measure
ensures that playback returns to the intended level on a repeated passage,
even when the last measure before the repeat had a different level. Under
default playback options, the marking changes only the left-hand staff's
velocity; the melody staff and its authored dynamics stay unchanged. A host
that explicitly shares dynamics across staves can override that separation.
The result wraps the usual section result as `arrangement` and reports
`resolved_dynamics` and `dynamic_marks`. Invalid ranges, overlaps, more than
512 sections and accent-only or compound dynamic symbols return no score or
note mappings. Pattern rhythm, harmony and note pitches are unchanged.

```js
const result = JSON.parse(arrange_sections_with_dynamics(
  scoreJson,
  JSON.stringify({
    chords, bass_mode: "free-bass", default_pattern: "held-bass-chord",
    sections: [{ first_measure: 2, last_measure: 3, pattern: "bass-chord-pulse" }]
  }),
  JSON.stringify({
    default_dynamic: "Mp",
    sections: [{ first_measure: 2, last_measure: 3, dynamic: "F" }]
  })
));
if (result.arrangement.score) {
  render_score_svg(JSON.stringify(result.arrangement.score), "{}");
}
```

## Responses in selected melody rests

`arrange_with_response_fills(score, accompaniment, response)` in Rust and
`arrange_with_response_fills(score_json, accompaniment_json, response_json)`
in full/viewer WASM add short, opt-in right-hand answers to the sectioned
accompaniment result. The accompaniment argument is the existing
`SectionedAccordionOptions`. The response contains `rest_notes`, an array of
source `NoteAddr` values. The API never fills unselected rests.

Each selected source note must be an ordinary, un-dotted half rest between two
pitched melody notes in the same written measure. The answer uses a quarter
rest followed by one quarter-note chord tone, so it occupies only the second
half of the authored two-beat rest. The chord active at the answer onset
supplies the pitch class. The chosen pitch is in MIDI 60–84, below the next
melody note by 1–12 semitones, and no more than 12 semitones from the previous
melody note. Equal motion costs prefer the higher chord tone. This is a
deterministic, deliberately sparse rule; it does not compose a general
countermelody or rate musical taste.

The original melody, its rests and note IDs are unchanged. Responses are
written in a separate right-hand voice; left-hand patterns, silence, chord
changes, repeats and endings remain as selected. The result wraps the ordinary
section result in `arrangement` and lists each source rest, generated note
address and pitch in `fills`. Invalid addresses, unsupported rests or a lack
of suitable chord tone return a diagnostic and no score or mappings. At most
512 rests may be selected.

```js
const result = JSON.parse(arrange_with_response_fills(
  scoreJson,
  JSON.stringify({
    chords, bass_mode: "free-bass", default_pattern: "bass-chord-pulse",
    sections: []
  }),
  JSON.stringify({
    rest_notes: [{ part: 0, staff: 0, measure: 2, voice: 0, note: 1 }]
  })
));
if (result.arrangement.score) {
  render_score_svg(JSON.stringify(result.arrangement.score), "{}");
}
```

## Opt-in accompaniment voicing


`arrange_with_voice_leading(score, layers, voicing)` in Rust and
`arrange_with_voice_leading(score_json, layers_json, voicing_json)` in full/viewer
WASM run the same layered generator, then choose octave positions for its block
chords. The result's `arrangement.score` feeds the existing render, playback and
export APIs. `arrange_with_layers` and saved preset output remain unchanged.

The melody (and any separately requested melodic voice), chord symbols, slash
bass, bass attacks, note lengths, rests, ties and articulations are preserved.
Every chord pitch class occurs exactly once, with no omissions, additions or
reharmonization. Only generated accompaniment chord pitches move by octaves.
`voicings` lists each original and chosen chord MIDI array and all affected note
addresses, including tied fragments.

`AccordionVoicingOptions` defaults to MIDI 48–84 and a maximum chord span of 12
semitones. Custom bounds must remain inside MIDI 48–84 and allow a span of
1–24 semitones. These limits apply to the chord alone, excluding the independent
bass pitch. A range that cannot retain every chord tone produces diagnostics
and no score or note mappings.

The motion cost sums, for each pair of successive chord attacks, the distance
from every tone to the nearest tone of the other chord in both directions.
For example, `[60,64,67]` to `[60,65,69]` costs 6. Repeats and alternate endings
follow the existing expanded playback order; tied continuation fragments are
not new attacks. Consecutive instances of an identical written chord share a
voicing, and repeated sections reuse their written voicings.

The search starts with the closest feasible voicing to each original chord,
then reduces the transition cost through at most 16 local-improvement passes.
Equal transition costs favor staying closer to the original register. With the
default limits and no melody gap the original voicings are feasible, so the reported
motion cost cannot increase. Tighter bounds or a melody gap may require a higher cost than the
unconstrained original. `search_limit_reached` reports an unfinished local
search; even a stabilized result is not a global optimum or a performer rating.
The C–F–G–C regression lowers this defined cost from 47 to 22; it is a synthetic
case, not evidence of general listening preference.

Only `free-bass` is supported. Held chords, bass/chord pulses, offbeat chords,
swing, waltz, fine bass, habanera and driving block-chord patterns are accepted.
Arpeggio, rising-arpeggio, Alberti and walking bass are not revoiced. Fixed
standard-bass button voicings must come from an explicit instrument profile.
The optional melody gap below constrains chord/melody register. The search does
not constrain parallel intervals, fingerings, hand span including the bass,
bellows or instrument-specific physical reach.

Inputs are bounded to 512 measures, 8192 source notes, 512 chord changes, 32
quarter-note beats per measure and 8192 beats total. A chord can have at most
128 candidate voicings; wider searches return `voicing-candidate-limit`.

For proposed harmony, pass a selected chord proposal's
`preset_selection.chords` as `layers.chords`, choose a compatible block pattern
and set `bass_mode` to `free-bass`. For example:

```js
const proposals = JSON.parse(suggest_chord_plans(scoreJson, "{}"));
const candidate = proposals.candidates.find(c => c.id === "jazz-color-v2");
if (candidate) {
  const layers = {
    chords: candidate.preset_selection.chords,
    alternative_chords: null, inversion: null,
    accompaniment: "swing-accompaniment",
    melodic_voice: "unchanged", swing: "notated-accompaniment-two-to-one",
    bass_mode: "free-bass"
  };
  const result = JSON.parse(arrange_with_voice_leading(
    scoreJson, JSON.stringify(layers), "{}"
  ));
  if (result.arrangement.score) {
    const svg = render_score_svg(JSON.stringify(result.arrangement.score), "{}");
    // Display svg and retain candidate.plan for its harmony provenance.
  }
}
```

## Keeping block chords below the melody

Both voicing APIs accept `min_melody_gap_semitones` in their
`AccordionVoicingOptions` / voicing JSON. Omit it or pass `null` to retain the
existing search. A value from 0 through 24 requires the highest accompaniment
chord tone to remain at least that many semitones below every overlapping
right-hand pitch. Zero allows unison; 1 requires the chord to be strictly lower.
For example, pass `{"min_melody_gap_semitones": 3}` as the voicing options.

The check uses complete written note durations, including rests, tuplets and
tied fragments. A lower melody note arriving during a held chord constrains
that chord; a low note that ends before a chord attack does not. An added
right-hand melodic voice is protected too. Staff and measure-instrument
transposition are included, and microtonal pitches are rounded downward when
deriving the integer-MIDI ceiling. Chords sounding only under right-hand rests
receive no additional ceiling. Consecutive identical chord groups continue to
share one voicing, using the strictest ceiling across all their targets.

`voicings[].melody_ceiling_midi` reports each group's effective upper bound,
including the requested gap. It is omitted when the group has no melody bound.
Candidate generation applies this ceiling before its 128-candidate cap.
The melody, bass attacks and chord pitch classes stay unchanged. If the range,
span and melody gap cannot retain every chord tone, the result reports
`no-voicing-below-melody` at the restricting target measure and returns no score
or mappings. A gap can force a larger motion cost than the unconstrained result.

This is a written-duration register constraint on generated block chords.
It does not constrain independent bass attacks or the unchanged arpeggio,
Alberti and walking-bass sections. It does not model pedal release, performed
ornaments or host timing changes. Pitched grace notes, unpitched right-hand
notes and pitch bends return `unsupported-melody-clearance` when the gap is
enabled; their performance timing/pitches are not silently ignored.

## Sections with accompaniment voicing

`arrange_sections_with_voice_leading(score, sections, voicing)` in Rust and
`arrange_sections_with_voice_leading(score_json, sections_json, voicing_json)`
in full/viewer WASM combine the two opt-in operations. Pass the same
`SectionedAccordionOptions` and `AccordionVoicingOptions` described above.
The section generator produces one complete score before a shared voicing
search adjusts the block chords across its sections.

Arpeggio, rising-arpeggio, Alberti and walking-bass sections retain their
generated pitches and rhythm. Silent sections retain their rests. Melody,
slash bass and other bass attacks also remain unchanged. Only the block-chord
patterns contribute targets and motion costs. Consecutive block-chord attacks
are compared even when a silent or melodic section lies between them, including
repeat-back transitions. This cost does not measure movement along the skipped
melodic bass line or discount a long pause. Written repeats share one voicing.

The `VoiceLedSectionedAccordionResult` has the same top-level voicing fields as
`VoiceLedAccordionResult`: `resolved_voicing`, `voicings`,
`original_motion_cost`, `motion_cost`, `search_limit_reached` and
`contract_version` (1). Its `arrangement` is a `SectionedAccordionResult`, so
resolved patterns, note mappings and notated-swing measures remain available.
`arrangement.generated_notes` includes the untouched melodic accompaniment;
`voicings[].targets` identifies only notes whose chord placement was selected.

Only free bass is supported, and at least one generated block chord is required.
An entirely silent or melodic accompaniment returns `no-chord-attacks`; use
`arrange_with_sections` directly for that case. Section errors and impossible
voicing constraints return no score, mappings, resolved patterns, swing flags
or voicing report. The section and voicing bounds above both apply. Existing
standalone APIs keep their behavior.

For a source of at least four measures, this generates held chords, an arpeggio,
swing and silence, choosing the held and swing chord positions together:

```js
const result = JSON.parse(arrange_sections_with_voice_leading(
  scoreJson,
  JSON.stringify({
    chords: candidate.preset_selection.chords,
    bass_mode: "free-bass",
    default_pattern: "held-bass-chord",
    sections: [
      { first_measure: 1, last_measure: 1, pattern: "arpeggio" },
      { first_measure: 2, last_measure: 2, pattern: "swing-accompaniment" },
      { first_measure: 3, last_measure: 3, pattern: null }
    ]
  }),
  JSON.stringify({ min_midi: 48, max_midi: 72, max_span_semitones: 12 })
));
if (result.arrangement.score) {
  const arrangedJson = JSON.stringify(result.arrangement.score);
  const svg = render_score_svg(arrangedJson, "{}");
  const events = JSON.parse(to_playback_events_ex(arrangedJson, "{}"));
  // Retain candidate.plan for harmony provenance; leave playback swing unset.
}
```

For a single sustained bass pitch without a chord progression, use
[`arrange_with_drone`](accordion-drone.md). Its optional `layers` setting can
add a chord-derived melodic voice while the bass pitch stays fixed.
