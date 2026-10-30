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
contract version and engine version. Save it with the input Score for sharing
or replay. A seed alone is not a durable representation across engine changes.
Replay rejects another input, engine version, or tampered preset options.
Generated score and note IDs can differ between runs; note addresses, pitches,
durations and musical timing are reproducible.

The fixed 2:1 swing preset writes triplets into the score and sets
`notated_swing`. When that flag is true, pass `{}` to `to_playback_events_ex`
or leave `PlaybackOptions.swing` unset. A host may choose a separate
performance swing policy on straight notation.

## Minimal browser flow

```js
import init, {
  accordion_preset_capabilities,
  arrange_accordion_preset,
  shuffle_accordion_preset,
  arrange_from_resolved_preset,
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
