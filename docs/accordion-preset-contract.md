# Accordion preset contract (v1)

The core and viewer WASM build expose `accordion_preset_capabilities`,
`arrange_accordion_preset`, `shuffle_accordion_preset` and
`arrange_from_resolved_preset`. Build the browser module with:

```sh
wasm-pack build crates/wasm --target web --release --no-default-features --features viewer
```

The host supplies a canonical melody `Score` and explicit positioned chords.
Capabilities return stable preset IDs and supported options without translated
display text. Choosing a preset produces one score on demand. Shuffle filters
the allowed IDs through the same arrangement operation before sampling; it
never generates a candidate the engine cannot realize. An empty eligible pool
or a request with no musically different eligible result returns a diagnostic
rather than looping.
The host may provide up to four explicit `AccordionPresetSelection` values
(one `selection` plus `alternative_selections`) for basic, color or inversion
choices; Acorde samples the compatible technique and selection pairs.

The seed is decimal text for full 64-bit precision in JavaScript. The returned
`resolved_config` includes the input fingerprint, selected ID, full options,
contract version and engine version. Fingerprints beginning with `v2:` use
version 2 and normalize generated UUIDs of the Score, Parts, Notes, spanners and views while
hashing all other score content, including note order, ties, tuplets, repeats,
chord symbols, spanner addresses and settings. It permits replay after parsing
the same source again; the replay uses the newly parsed source note IDs.
Save the source identifier and `resolved_config` for sharing, then parse that
source again before replay. A seed alone is not a durable representation across
engine changes. Replay rejects changed input, engine version, contract version,
unsupported fingerprint versions, and preset IDs whose options do not match
their defined technique. Chord selections remain caller-owned.
Legacy 16-digit fingerprints use version 1 under engine
`1.3.4+arrangement-presets.1`: they continue to replay only against the
exact original Score, including its IDs. New v1.3.5 configs use engine
`1.3.5+arrangement-presets.2` and fingerprint version 2. To migrate an old config,
load its original Score and call `arrange_accordion_preset`
again with its saved chord selection and preset ID, then save the new version 2
`resolved_config`. A legacy config cannot be upgraded from a re-parsed Score
alone.
New v1.3.6 preset configs use engine `1.3.6+arrangement-presets.2`. The engine
marker changes with the workspace release; select the saved chords and preset
again to create a v1.3.6 config before replaying a v1.3.5 result.

New v1.3.8 preset configs use engine `1.3.8+arrangement-presets.2`. Reselect
the saved chords and preset before replaying an earlier configuration.
Generated score and note IDs can differ between runs; note addresses, pitches,
durations and musical timing are reproducible.

The fixed 2:1 swing preset writes triplets into the score and sets
`notated_swing`. When that flag is true, pass `{}` to `to_playback_events_ex`
or leave `PlaybackOptions.swing` unset. A host may choose a separate
performance swing policy on straight notation.

`bass-pulse`, `alternating-bass`, and `offbeat-chords` accept a 0.5- or
1.5-beat first pickup in 6/8 without lengthening the melody or inserting a
full opening bar. Their accompaniment uses the last half-beat slots of the
nominal measure; the following full measure restarts its pattern at beat 0. Shorter
regions that the pattern cannot represent still return `unsupported-rhythm`.

In 9/8, these same three presets use three dotted-quarter pulses per complete
bar. `bass-pulse` plays bass–chord–bass, `alternating-bass` plays
bass–chord–fifth bass, and `offbeat-chords` rests an eighth then plays a quarter
chord in each pulse. A first pickup occupies the end of its nominal bar; a
short final bar starts at the beginning. Chord changes may split a pulse at an
eighth-note boundary. Finer changes return `unsupported-rhythm` with the
required alignment. Both standard-bass and free-bass selections use this
timing; the generated notes remain on the bass staff for playback and export.

`waltz-bass` and `waltz-alternating-bass` are separate 3/4 presets. Both play
bass on beat 1 and chord on beats 2 and 3. The second alternates root and fifth
bass across complete bars, beginning with root in the first complete bar.
A first pickup occupies the end of its nominal bar without advancing that
alternation; a short final bar starts on beat 1. Existing `bass-pulse` and
`alternating-bass` retain their four-beat pattern.

## Standard-bass buttons and dominant sevenths

The existing standard-bass preset selection uses a default model with all twelve
bass roots and major, minor and dominant-seventh chord buttons. Its seventh
button sounds an ideal root, third, fifth and minor seventh. This is an explicit
model, not a description of every accordion. Augmented and diminished triads,
major/minor sevenths and unsupported melodic bass patterns return a diagnostic;
a diminished triad is distinct from a diminished-seventh button. Free-bass
selection retains its separate behavior. See [migration notes](migrations.md)
for the changed acceptance of previously saved selections.

For a specific instrument, `model::standard_bass_preset::arrange_profiled_bass_pulse`
and the full/viewer WASM export `arrange_profiled_bass_pulse(scoreJson, optionsJson)`
take a standard-bass preset selection and an explicit button profile. It uses
the `bass-pulse` rhythm, returns the actual button IDs and MIDI pitches per
attack, and reports permitted omitted or added pitch classes. Missing rows or
root columns produce diagnostics with no partial score. Omission and additions
must be explicitly permitted. Bass and chord attacks are separate: the candidate
contains both button identities for context, while each action's
`sounding_midi` identifies the notes actually sounding at that attack.
The default profile's four-note voicing is not substituted for the real profile.
Movement, fingering and performer review remain separate requirements.

For example, a three-note G7 chord button that omits D can be declared as:

```js
const options = {
  selection: {
    chords: [{ measure_index: 0, offset_beats: 0,
      chord: { root: 'G', kind: 'dominant' } }],
    alternative_chords: null, inversion: null, bass_mode: 'standard-bass',
  },
  profile: {
    bass_buttons: [{ id: 'G bass', position: 0, midi: 43 }],
    chord_buttons: [{ id: 'G7 chord', position: 0, midi: [67, 71, 77] }],
    max_distance: 0,
  },
  allow_chord_omissions: true, allow_chord_additions: false,
};
const result = JSON.parse(arrange_profiled_bass_pulse(scoreJson, JSON.stringify(options)));
```

The returned action's `candidate.omitted_pitch_classes` includes D (pitch class
2). Bass and chord actions have distinct `sounding_midi` lists.

## Two 2/4 accompaniment patterns

`fine-bass-chord` and `habanera` are straight 2/4 presets, also exposed as
`fine-bass-chord` and `habanera` accompaniment values in the free-layer API.
A complete bar plays root bass, chord, fifth bass, chord. Their respective
quarter-beat durations are `0.5, 0.5, 0.5, 0.5` and
`0.75, 0.25, 0.5, 0.5`. A first pickup uses the end of the nominal bar and a
short final bar uses its beginning. Chord changes on sixteenth-note boundaries
split the current slot and rearticulate with the new chord; identical repeated
chords do not restart a slot. Other meters, shorter internal bars, finer
positions, and notated-swing combinations return diagnostics. The standard
bass chord restrictions apply as for existing presets.

## Driving 4/4 accompaniment

`driving-bass-chord` alternates a bass on each beat with a staccato chord on
its eighth-note offbeat. Bass notes alternate the selected bass and its fifth
within each two-beat cycle. The first beat of a complete bar has an accent;
chord staccato shortens playback without changing written onset positions.
`driving-bass-chord-light` is an unaccented quarter-note bass/chord pattern for
lower-density comparison. Both names are also available as free-layer
`accompaniment` values. They require 4/4 and straight notation; chord changes,
pickups and short final bars must lie on eighth-note boundaries. A first pickup
occupies the end of the nominal bar; a short final bar begins at its start.
A changed chord rearticulates the current slot without resetting the two-beat
phase; identical repeated chord symbols do not restart it.

Standard bass uses the default chord-kind model and range restrictions described above. As a
conservative configurable-by-pattern rate policy, it rejects score tempos or
ramps exceeding 200 BPM for either pattern: a half-beat chord change can split
a light-pattern quarter note, so the minimum possible attack spacing is 150 ms. This is a
product constraint, not a universal assessment of a player's technique.
Free bass is not subject to that rate policy. The check uses the written score tempo;
a host-applied playback tempo override or multiplier must be checked by the host.
Neither pattern changes tempo.

## Minimal browser flow

```js
import init, {
  accordion_preset_capabilities,
  arrange_accordion_preset,
  shuffle_accordion_preset,
  arrange_from_resolved_preset,
  arrange_accordion_layers,
  arrange_from_resolved_layers,
  shuffle_accordion_layers,
  render_score_svg,
  to_playback_events_ex,
} from './acorde_wasm.js';

await init();
const capabilities = JSON.parse(accordion_preset_capabilities());
// scoreJson is the host's authored canonical Score. Chord input is also host-owned.
const selection = {
  chords: [{ measure_index: 0, offset_beats: 0,
    chord: { root: 'C', kind: 'major', bass: null } }],
  alternative_chords: null,
  inversion: null,
  bass_mode: 'free-bass',
};
const selected = JSON.parse(arrange_accordion_preset(
  scoreJson, JSON.stringify(selection), capabilities.presets[0].id,
));
if (selected.diagnostics.length) throw new Error(selected.diagnostics[0].message);
const shuffled = JSON.parse(shuffle_accordion_preset(scoreJson, JSON.stringify({
  selection,
  alternative_selections: [],
  allowed_preset_ids: capabilities.presets.map(preset => preset.id),
  seed: '42',
  previous_config: selected.resolved_config,
})));
if (shuffled.diagnostics.length) throw new Error(shuffled.diagnostics[0].message);
const replayed = JSON.parse(arrange_from_resolved_preset(
  scoreJson, JSON.stringify(shuffled.resolved_config),
));
const arrangedJson = JSON.stringify(replayed.score);
const svg = render_score_svg(arrangedJson, '{}');
const events = JSON.parse(to_playback_events_ex(arrangedJson, '{}'));
```

The host displays the SVG, schedules the returned playback events, and owns
click-to-play permission, cancellation, history and UI. The engine owns pitches,
rhythm, score generation and random selection. Shuffle is bounded to 128
measures, 8192 source notes, 512 explicit chords per selection, four selections and 32 allowed IDs. The existing arrangement
exports remain available.

## Free layer combinations

`arrange_accordion_layers`, `arrange_from_resolved_layers`, and
`shuffle_accordion_layers` are separate Rust and viewer WASM exports for
combinations that do not have a built-in preset ID. The direct call accepts
`LayeredAccordionOptions` and returns the score, diagnostics, resolved chords,
source and generated note addresses, and a versioned `resolved_config`.
Save that config as JSON and pass it with the same source after re-parsing to
`arrange_from_resolved_layers`. Its `v2:` fingerprint, contract version, and
`arrangement-layers.1` engine version are validated before generation.
For v1.3.6 the full marker is `1.3.6+arrangement-layers.1`. Reselect the saved
free-layer options to create a v1.3.8 config with marker
`1.3.8+arrangement-layers.1`.
Built-in preset configs continue to use `arrange_from_resolved_preset`; changing
their options still returns `invalid-configuration`.

The free-layer shuffle accepts at most 12 accompaniment choices (including `null`).
The free-layer shuffle takes an explicit chord `selection`, up to three
`alternative_selections`, and allowed sets of accompaniment patterns, melodic
voices, and swing policies. Put `null` in `allowed_accompaniments` to allow no
accompaniment. Acorde tests each permitted combination, discards unsupported
ones, and samples distinct playback results with the decimal `seed`. The pool
is bounded to 64 combinations, 128 measures, 8192 source notes, and 512 chords
per selection. An empty pool returns `empty-eligible-pool`; a pool containing
only the previous musical result returns `no-different-result`. Invalid seeds,
oversized requests, and a previous config from another input or engine have
their own diagnostics.

```js
const layers = {
  ...selected.resolved_config.options,
  melodic_voice: 'sixth-below',
};
const combined = JSON.parse(arrange_accordion_layers(
  scoreJson, JSON.stringify(layers),
));
const savedConfig = JSON.stringify(combined.resolved_config);
const replay = JSON.parse(arrange_from_resolved_layers(
  reparsedScoreJson, savedConfig,
));
const next = JSON.parse(shuffle_accordion_layers(
  reparsedScoreJson, JSON.stringify({
    selection,
    alternative_selections: [],
    allowed_accompaniments: ['held-bass-chord', 'rising-arpeggio'],
    allowed_melodic_voices: ['sixth-below', 'octave-below'],
    allowed_swing_policies: ['straight'],
    seed: '42',
    previous_config: { kind: 'layers', config: combined.resolved_config },
  }),
));
```

For a previous fixed preset, use
`{ kind: 'preset', config: selected.resolved_config }`. Acorde compares the
playback events, so the same result under a different technique ID is not
selected again. The host still owns source IDs, sharing URLs, labels and UI.

### Positioned plans and instrument profiles

`model::profiled_chord_plan::profile_chord_plan` (also exposed to WASM) checks each positioned chord against the supplied button inventory and returns options for `arrange_profiled_bass_pulse` only when every entry has a supported candidate. It checks harmony and timing-grid positions, not complete score coverage or physical playability; performer review is required. Candidate matching uses the combined pitch classes of a bass and chord button. The resulting bass pulse attacks them separately, so it does not claim the simultaneous harmony sounds throughout.

### Explicit button rows and columns

`model::standard_bass_button_layout::located_standard_bass_candidates` and its WASM binding add caller-supplied row and column labels to simultaneous-button candidates. Every button must have one unique location; the API does not infer a physical layout from an instrument size. For root-position dominant seventh chords, a supplied seventh-row button containing the root, third and minor seventh is preferred when it meets the requested omission/addition policy. A missing fifth is never allowed implicitly. Slash bass remains explicit. Optional transition distance is the sum of column changes, not a physical reach or fingering guarantee; performer review is still required. This candidate search does not yet select actions for positioned chord plans or generated scores.
