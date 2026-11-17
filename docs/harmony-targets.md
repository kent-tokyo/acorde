# Applied dominants and destination-key progressions

Stage 4 extends `phrase_harmony` with `suggest_phrase_harmony_targets` and
`replay_phrase_harmony_target`. They return playable scores through the existing
score-connected harmony arranger. The original score parts and their key
signatures remain unchanged; source/destination keys are explicit harmonic
context in the request and saved choice.

A `PhraseHarmonyTargetRequest` supplies a `HarmonyArrangementRequest` base,
secondary-dominant targets, modulation regions, locked chord entries, a changed
harmony limit, one to four requested results, a seed, and at most 64 realization
attempts. The base retains its 32-bar, 64-harmony, register, instrument, comping
and locked-measure limits.

- A secondary target replaces the immediately preceding chord with the spelled
  V7 of the retained arrival chord. Major/minor arrivals may retain authored
  extensions and omissions; suspended, diminished and augmented arrivals that
  cannot establish a major/minor third and perfect-fifth identity are diagnosed.
  E7-Am retains G-sharp; D7-G retains F-sharp. Changed entries retain their
  original source references and gain generated-harmony provenance.
- A modulation names both source and destination keys and an inclusive region
  of harmonic entries. Common-chord approaches require at least three entries
  and a spelled triad native to both keys. The minor-key vocabulary includes
  natural, harmonic and ascending melodic-minor triads. Enharmonic respelling
  is not substituted for an exact common spelling.
- A dominant approach requires at least four entries: the destination V7
  resolves to its tonic at the region entrance, and the region ends with V7-I/i.
  Longer regions sustain the new tonic and can introduce its subdominant before
  the final cadence. A single chromatic replacement never serves as the complete
  modulation region. A requested route that is too short reports a shortage.
- Target regions must not overlap. Replacing an entry also requires that its
  entire harmonic span lies in the editable range and avoids all locked bars.
  A retained arrival is not silently changed to force a new major/minor quality.

Each candidate contains the exact changed entries, selected modulation route and
pivot, and the performed voice transitions at every promised resolution.
Persistent voice indices identify leading tones that rise a semitone to the
arrival root and sevenths that descend by step to its third. These observations
are measured from the actual realized voices, not inferred from chord labels.
Repeat routes that bypass a promised resolution are rejected. An unsupported
arranger context still returns its typed error.

Ranking uses actual melody-overlap friction, unresolved guide-tone movements,
voice-motion cost, and the number of changed harmonies. An optional
`max_melody_friction_ticks` rejects a candidate exceeding the same overlap metric
used by stage 3. These development costs and the structural V7-I/i evidence do
not by themselves establish perceptual key change or listening quality.

The Cartesian search is capped at 64 plans and realization attempts. It reports
pruning, failed voicings, register failures, locks, budgets, missing common
chords, melody friction and duplicates. `complete` means the requested number
survived; duplicate plans never fill the count. Seeded ordering resolves bounded
search ties. Global optimality or universal absence of a feasible progression
is not claimed.

Saved choices contain the original permissions, selected routes, generated
chord plan, source fingerprint and both contract/engine versions. Replay checks
the bounds and permissions again and realizes only the saved selection. It
rejects source changes, unknown versions, changed output plans or added
permissions that disagree with the saved selection. Fingerprints identify
musical content; they are not signatures.

Rust and full/viewer WASM expose the same contract. Request JSON is limited to
2 MiB and saved replay JSON to 8 MiB, including caller provenance. The tests share
a fixed score and compare actual playback and resolution evidence. MusicXML and
MSCZ tests verify the generated note spelling and sounding events.

The distinction between an applied chord and a new-key cadence, and the
requirement that a pivot belong to both keys, follow the background descriptions
in [Open Music Theory: tonicization](https://viva.pressbooks.pub/openmusictheory/chapter/tonicization/)
and [modulation](https://openmusictheory.github.io/Modulation.html). The bounded
vocabulary, numeric costs and API restrictions above are implementation choices.

Stage 4 development acceptance is complete: structural/IO checks, full/viewer
WASM, hosted CI, matched size/runtime evidence, and the fixed eight/sixteen-bar
listening comparisons. On 2026-11-17 the user accepted all presented comparisons
with "問題なし". Evidence is recorded in `harmony-targets-rhythm-progress.json`
and the pinned listening manifest. This is not a held-out multi-listener result
or a release/publication claim.

Native generation measurements are pinned in
[`arrangement-stage45-performance.json`](arrangement-stage45-performance.json).
The six fixed eight/sixteen-bar target cases each have 60 measured calls after
warmup, with unchanged playback and work counters on every call. The largest
observed p95 was 7.446 ms (sixteen-bar common-chord modulation). Applied/dominant
cases attempted one realization; common-chord cases attempted four. Reported
inner voicing-state counts cover selected candidates, not rejected candidates.
These are development-input timings on one native host, not a worst-case bound
or browser latency guarantee.

Stage 0 did not record native generation timings. Its pinned source was rebuilt
beside the current source with the same Rust 1.100.0, dependencies, release
profile, harness and unchanged sixteen-bar legacy form input. The legacy median
ratio was 0.9824 and p95 ratio 0.8133, with identical playback. New stage-4/5 APIs
did not exist in stage 0; no cross-API speed ratio is inferred. Reproduce with:

```sh
python3 scripts/benchmark_arrangement_stages.py \
  --baseline /path/to/clean-stage-0-checkout \
  --output /new/measurement-directory
```

The report pins revisions, core trees, harness/fixture/dependency hashes, all
timing samples and the balanced baseline/candidate/candidate/baseline run order.
Input preparation, playback verification, serialization, rendering and result
destruction are outside the measured API call. The script refuses changed
library/fixture inputs and mismatched legacy outputs.

The same native run emits `native-parity.json`. Its pinned copy is
`tests/fixtures/arrangement/stage45-native-parity.json`: fourteen eight/sixteen-bar
cases, four shared source scores, full native playback events and resolved
contracts. `crates/wasm/tests/arrangement_native_parity.rs` compares these values
directly with generation and replay through the bindings in native tests and
Chrome full/viewer tests. It also checks original-part preservation. The expected
values are not regenerated inside the browser. These checks complement the
smaller binding-versus-core tests, whose core call runs within the same target.
Regenerating the fixture is an explicit review step after an intentional musical
contract change; timing variation alone does not justify replacing it.

The observed legacy timing ratios vary between runs; this sample does not prove
a performance improvement. The script's standalone wrapper allows Clippy's
`duplicate_mod` lint only because both unchanged audition fixture graphs embed
the same private helper. Product code and tests retain their normal lint rules.

