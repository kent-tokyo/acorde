# Select distinct harmony and comping arrangements

`suggest_harmony_groove_arrangements(&Score, &HarmonyGrooveRequest)` returns
plain, colored and rhythmic scores, their resolved permissions, actual differences,
and development costs. Both this API and `replay_harmony_groove_arrangement` are
available in Rust and full/viewer WASM. Existing arrangement entry points are
unchanged. The implementation is on the harmony development branch. The final code at
`ff5e1c95` passed all 20 hosted checks; listening acceptance remains pending.
See `harmony-groove-acceptance.md` for the tested contracts and evidence.

The request's `base` is a concrete `HarmonyArrangementRequest`: root policy
`include`, held comping, no anticipation or accent. This is the literal plain
anchor and supplies all shared ranges, source roles, locks, density bounds and
instrument settings. Other values in these four anchor fields are errors rather
than silently changed preferences.

Explicit permission fields authorize the alternatives:

- `tensions`: chord entry and allowed `{ degree, alter }` additions. Degrees are
  9, 11 or 13, with alterations from -2 through 2. The base chord's kind, root,
  bass, existing degrees and authored omissions are retained. An already defined
  or explicitly removed degree cannot be overridden by a permission.
- `allow_rootless`: permit the score arranger's automatic root policy. It still
  requires root support over the complete actual sounding interval.
- `allow_offbeats`: permit half-beat pulses on quarter-note offbeats.
- `anticipation`: optional allowed eighth/sixteenth leads, tie/reattack joins and
  maximum number of anticipated entries. The selector finds eligible entries;
  actual scheduling diagnoses locks, pickups and ambiguous repeat destinations.
- `rhythmic_accent`: none, anticipations or all attacks. Color keeps the anchor's
  attacks and dynamics, allowing its harmonic change to be compared directly.

`max_added_tensions_per_chord` is 0–2 and `max_changed_harmonies` is 0–64.
`count` is 3–5, `max_realizations` is at least count and at most 128, and `seed`
resolves bounded harmony-search ties. Source/score limits of the arranger also
apply, including 32 bars and 64 harmonic positions. Refer to the fixed request
builder in `tests/fixtures/arrangement/harmony_groove_source.rs` for a complete
Rust example.

WASM accepts at most 2 MiB of request JSON and 8 MiB of resolved JSON. Resolved
contracts contain both the original permissions and the selected chord plan,
including caller provenance. Their limit therefore differs from the generic
64 KiB options limit. Oversized inputs are rejected before JSON parsing; the
score JSON limit remains 16 MiB.

The harmonic beam holds at most 16 sets of additions, preserving alternatives at
different change counts. This is a bounded heuristic, not an exhaustive search
or proof that no better arrangement exists. Up to half the realization budget
is reserved for color exploration, leaving room for rhythmic candidates. Only
eight successful candidates per profile are retained for ranking. The rhythm
search prioritizes the chosen color's harmony and can use offbeats, anticipations
or both. It currently uses that one anchor; this is another search boundary,
not a guarantee of finding every feasible rhythmic alternative.

Costs report sustained semitone/seventh pitch-class friction with the melody,
low-register clusters, and voice-motion cost. Friction is overlap time measured
in 1/960000 quarter-note units, weighted twice for accented velocities above 64.
These costs guide selection; they do not certify listening quality. The color
must change actual pitches relative to the plain anchor. Rhythm must change
actual attack times or durations relative to its anchor. UUIDs and labels do not
count as diversity, and duplicates never fill a requested count.

`complete` requires the requested number and all three profiles. Otherwise the
result includes available candidates and `missing_profiles`. Rejections distinguish
register, voicing, comping and duplicate failures. `realizations` includes failed
attempts; `realization_limit_reached`, `harmony_search_pruned` and
`voicing_search_limited` distinguish bounded search from lack of observed options.
Invalid source/request data and unsupported score context remain typed errors.

A resolved candidate saves the original request, selected additions, selected
score-arrangement request, source fingerprint, contract and engine versions.
Replay rechecks permissions, locks and bounds before realizing the saved choice.
It does not rerun proposal selection. Musical events and notation provenance,
not regenerated UUIDs, define reproducibility. Source fingerprints are identity
checks, not signatures against malicious modification.

## Fixed comparison artifacts

`harmony-groove-listening.json` defines an eight-bar turnaround and a sixteen-bar
answer phrase. They are development fixtures, not held-out evaluation works.
Generate the pack with:

```sh
cargo run -p acorde-cli --example harmony_groove_listening_pack -- /path/to/new-directory
```

The generator refuses an existing output directory. It validates all three
profiles, source-bound replay, MusicXML/MSCZ playback round trips and WAV clipping.
It writes Score JSON, resolved settings, MusicXML, MSCZ, WAV and render manifests
for the same source/SoundFont/program/tempo conditions, plus an HTML audio index.
The comparison uses the pinned repository piano SF2, 22,050 Hz mono and 500 ms
release tail, with no per-candidate normalization. The manifest records file and
source hashes, checkout identity, timing, search limits and a pending listening
review. Producing these artifacts does not complete that review.
