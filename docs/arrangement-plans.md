# Arrangement plans over written measures

For the dated development record behind harmony, voicing, groove and sound
extensions, see the [historical plan](development/arrangement-harmony-groove-plan.md)
(Japanese). The [changelog](../CHANGELOG.md) identifies released work.

`arrange_accordion_plan` (Rust and full/viewer WASM) applies existing accompaniment
and added melodic voices to explicit sections of one monophonic melody. It returns
one `Score` for rendering, playback and export. The source is never sliced:
pickup phase, carried harmony, tempo changes, source note addresses, ties,
tuplets, repeats and endings keep their full-score context.

This is the explicit-plan stage of [#433](https://github.com/kent-tokyo/acorde/issues/433),
with bounded seeded section selection from [#435](https://github.com/kent-tokyo/acorde/issues/435).
It does not infer phrases, alter the melody, choose a genre or certify musical
quality or physical playability. Instrument-specific patterns remain separate.

## Coordinates and repeat behavior

Each section is a zero-based, half-open range `[start_measure, end_measure)` in
the source staff's **written measure array**. A pickup occupies index 0 when
present. Printed measure numbers do not determine the range. Sections must be
ordered, nonempty, adjacent and cover the entire score exactly once. Use
`accompaniment: null` and `melodic_voice: "unchanged"` to explicitly leave a
section without generated layers.

Every repeated visit to a written measure uses the same arrangement. First and
second endings retain their written indices. A different arrangement for each
performance-order visit is outside contract 1. Pattern phase and carried chords
do not restart at a section boundary; walking-bass lookahead also retains the
complete explicit progression.

## Eight-bar example

```json
{
  "chords": [
    {"measure_index": 0, "offset_beats": 0,
     "chord": {"root": "C", "kind": "major", "bass": null}}
  ],
  "bass_mode": "standard-bass",
  "sections": [
    {"start_measure": 0, "end_measure": 4,
     "accompaniment": "bass-chord-pulse", "melodic_voice": "unchanged"},
    {"start_measure": 4, "end_measure": 7,
     "accompaniment": "driving-bass-chord", "melodic_voice": "third-below"},
    {"start_measure": 7, "end_measure": 8,
     "accompaniment": "held-bass-chord", "melodic_voice": "unchanged"}
  ]
}
```

These are `AccordionPattern` names: the existing preset names `bass-pulse` and
`held-chord` correspond to `bass-chord-pulse` and `held-bass-chord`. The third-below
section requires source pitches with compatible chord tones (the checked fixture
uses G over C major and A minor). Unsupported harmony is diagnosed, not replaced
by a different interval or a free-bass setting.

```javascript
const result = JSON.parse(wasm.arrange_accordion_plan(sourceJson, optionsJson));
if (result.score) {
  const arranged = JSON.stringify(result.score);
  const svg = wasm.render_score_svg(arranged, "{}");
  const events = JSON.parse(wasm.to_playback_events_ex(arranged, "{}"));
  const archive = wasm.serialize_mscz(arranged);
  const saved = JSON.stringify(result.resolved_config);
  // After re-parsing the same source:
  const replay = JSON.parse(wasm.arrange_from_resolved_plan(sourceJson, saved));
}
```

Rust exposes the same functions with `&Score`, `AccordionPlanOptions` and
`AccordionPlanConfig`, returning `AccordionPlanResult`. No new `Score` fields or
dependencies are required.

## Preservation, provenance and replay

Voice 0 of the original melody staff, including pitch spelling, duration and
notation, remains unchanged. Voice 1 contains the selected added melody only in
its sections. Staff 1 carries the accompaniment; silent sections have an empty
accompaniment voice. As with the existing layered API, the output part is an
accordion part, with new score/part/generated-note IDs. It retains source note
IDs and spanner addresses; part groups and saved views are cleared by the existing
accordion conversion.

`applied_sections` reports the actual requested techniques and ranges.
`source_notes` contains the original non-rest note addresses. Each entry of
`generated_notes` has an output `address`, `section_index` and `origin`:

- `accompaniment`: `chord_index` in `resolved_chords`, including carried harmony.
- `melodic-voice`: original melody `source` address plus `chord_index`.

Generated-note entries include tied continuations but exclude rests. Indices are
notation addresses, not playback-event indices; repeats can produce multiple
events from one address. Hosts can derive text such as “Add a third-below voice
in measures 5–7” directly from the section and origin data.

Save `resolved_config` for replay. Contract version 1, the workspace-versioned
`+arrangement-plan.1` engine marker and a `v2:` source-content fingerprint are
checked before generation. The existing fingerprint normalizes regenerated UUIDs
while retaining musical content and address relationships; it is a content check,
not a cryptographic authenticity check. Re-parsing the same source can therefore
replay a plan. Changed music/tempo or a different engine is rejected. Generated
UUIDs are not reproducible; musical events and provenance addresses are.

## Constraints and diagnostics

The existing pattern meter, rhythm, pitch-range, chord and standard-bass tempo
constraints apply in the measures where each pattern is used. Tempo and meter
changes before a section remain in force. Mid-bar chords use quarter-note beat
offsets and the existing 1/64-beat input grid; each pattern's finer restrictions
still apply. Swing is selected explicitly as `swing-accompaniment`; no other
pattern is substituted. If `notated_swing` is true, leave playback swing unset
because the generated score already contains triplet timing.

Changing the melodic layer across a source tie or tuplet is rejected with
`unsafe-tie-boundary` or `unsafe-tuplet-boundary`. Keeping the same layer across
the boundary is allowed. Unmarked tuplets touching a layer cut are conservatively
rejected; explicitly closed/opened groups permit a cut between groups. Contract 1
rejects third/sixth layers on tied source notes (`unsupported-melodic-tie`), since
the existing interval generator does not preserve those ties. A continuous octave
layer retains source ties and tuplets. Accompaniment-only changes do not cut the
source melody.

Diagnostics include `section_index`, `measure_index` and, where applicable, a
source-note address. Invalid ranges, overlaps and uncovered measures have distinct
codes. On any failure `score` and `resolved_config` are null and no partial
provenance is returned.

The explicit engine makes one generation pass, with at most 64 sections, 512
written measures, 2,048 supplied chords and 8,192 source notes (including rests).
Requests exceeding these budgets return `limit-exceeded` before generation.
The explicit `arrange_accordion_plan` call makes no candidate search. The seeded
selector below calls it for each attempted combination, so these constraints
and diagnostics remain authoritative.

## Seeded selection from section choices

`shuffle_accordion_plan` accepts the same explicit chords and bass mode, a
decimal unsigned 64-bit seed string, `max_attempts`, and ordered, adjacent
`sections`. Each section has `start_measure`, `end_measure`,
`allowed_accompaniments` and `allowed_melodic_voices` arrays. Include `null`
in the accompaniment array to allow silence. The arrays are sets: order and
duplicates have no selection weight. All ranges must cover the written score.

```json
{
  "chords": [{"measure_index": 0, "offset_beats": 0,
    "chord": {"root": "C", "kind": "major", "bass": null}}],
  "bass_mode": "standard-bass",
  "sections": [
    {"start_measure": 0, "end_measure": 4,
      "allowed_accompaniments": ["bass-chord-pulse", "held-bass-chord"],
      "allowed_melodic_voices": ["unchanged"]},
    {"start_measure": 4, "end_measure": 7,
      "allowed_accompaniments": ["driving-bass-chord", "held-bass-chord"],
      "allowed_melodic_voices": ["third-below", "unchanged"]},
    {"start_measure": 7, "end_measure": 8,
      "allowed_accompaniments": ["held-bass-chord"],
      "allowed_melodic_voices": ["unchanged"]}
  ],
  "seed": "1234567890123456789",
  "max_attempts": 64
}
```

The Rust function takes `&Score` and `AccordionPlanShuffleRequest`. Full and
viewer WASM accept score/request JSON and return JSON. A successful result
contains the explicit plan fields, including `score`, `applied_sections`,
`resolved_config`, chord and note provenance, plus
`selection_contract_version`, `seed`, `pool_size`, `attempted`,
`selected_pool_index` and `rejected` diagnostic counts. Save `resolved_config`
and call `arrange_from_resolved_plan` to replay on the same source with the
same engine marker. The seed identifies the choice order, while the saved
resolved plan identifies the actual techniques. Fresh generated UUIDs can vary.

The selection order is fixed for the same source content, options, seed and
engine version. It uses the version 2 source fingerprint, sorted unique
choices and a seeded permutation without replacement. Version 1 permits
1–8 sections, 1–8 listed choices per axis per section, at most 65,536 distinct
combinations, and 1–64 attempted candidates. Each attempt runs the explicit
plan engine. It never changes an allowed technique, chord or bass mode.

Failures return no score or partial provenance. `unsupported` means the seed,
range, input or bounded request cannot be processed; `no-candidate` means every
combination was tried and rejected; `search-limit` means the attempt budget
ended before the pool was exhausted. `attempted` and `rejected` show how far
the search got and why candidates failed. A candidate rejected for meter,
harmony, ties, tuplets, tempo or standard-bass constraints may be followed by
another permitted candidate. These outcomes do not imply a musical-quality
ranking or a physical playability check.

## Regression evidence

- `crates/core/tests/arrangement_plan.rs`: source preservation, legacy-pattern
  equivalence, ranges, bounds, meter/tempo context, pickup/final bars, unsafe ties
  and tuplets, and source/version-checked replay.
- `crates/wasm/tests/issue_433_arrangement_plan.rs`: native and browser bindings,
  section provenance, SVG note addresses, MSCZ roundtrip, repeats with first/second
  endings, source spanners and absolute playback times.
- `scripts/check-wasm-arrangement-plan.mjs`: executes both built full and viewer
  web WASM packages and compares events, configurations, provenance and errors.
- `crates/core/tests/arrangement_plan_shuffle.rs` and
  `crates/wasm/tests/issue_435_arrangement_plan_shuffle.rs`: deterministic
  selection, bounded failure outcomes, source and replay invariants, and
  playback, SVG and MSCZ checks.
- `scripts/check-wasm-arrangement-plan-shuffle.mjs`: full/viewer built WASM
  parity for seeded choices and replay.

The committed ABC and JSON fixtures under `tests/fixtures/arrangement_plan_*`
require no downloads. These checks establish the covered examples, not broad
real-score or instrument-playability guarantees.
