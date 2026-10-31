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
Built-in preset configs continue to use `arrange_from_resolved_preset`; changing
their options still returns `invalid-configuration`.

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
