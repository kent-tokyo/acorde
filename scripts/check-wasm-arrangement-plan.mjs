// Build full/viewer web packages into target/wasm-profiles before running.
import assert from "node:assert/strict";
import { readFile } from "node:fs/promises";
import { resolve } from "node:path";
import { pathToFileURL } from "node:url";

const root = resolve(import.meta.dirname, "..");
const abc = await readFile(resolve(root, "tests/fixtures/arrangement_plan_eight_bars.abc"), "utf8");
const plan = await readFile(resolve(root, "tests/fixtures/arrangement_plan_eight_bars.json"), "utf8");
const snapshots = {};
for (const profile of ["full", "viewer"]) {
  const directory = resolve(root, "target/wasm-profiles", profile);
  const api = await import(pathToFileURL(resolve(directory, "acorde_wasm.js")));
  await api.default({ module_or_path: await readFile(resolve(directory, "acorde_wasm_bg.wasm")) });
  const source = api.parse_abc(abc);
  const result = JSON.parse(api.arrange_accordion_plan(source, plan));
  assert.deepEqual(result.diagnostics, []);
  assert(result.score);
  const arranged = JSON.stringify(result.score);
  const events = JSON.parse(api.to_playback_events_ex(arranged, "{}"));
  const replay = JSON.parse(api.arrange_from_resolved_plan(api.parse_abc(abc), JSON.stringify(result.resolved_config)));
  assert.deepEqual(replay.diagnostics, []);
  assert.deepEqual(JSON.parse(api.to_playback_events_ex(JSON.stringify(replay.score), "{}")), events);
  assert.deepEqual(replay.generated_notes, result.generated_notes);
  for (let mi = 0; mi < 8; mi++) {
    assert.deepEqual(result.score.parts[0].staves[0].measures[mi].voices[0], JSON.parse(source).parts[0].staves[0].measures[mi].voices[0]);
    assert.equal(result.score.parts[0].staves[0].measures[mi].voices[1].length > 0, mi >= 4 && mi < 7);
  }
  const svg = api.render_score_svg(arranged, '{"interactive":true}');
  assert(svg.includes('data-note-addr="0:0:4:1:0"'));
  assert(!svg.includes('data-note-addr="0:0:3:1:0"'));
  const restored = api.parse_mscz(api.serialize_mscz(arranged));
  const music = (events) => events.map(({pitch_midi, time_beats, time_secs, duration_beats, duration_secs, velocity}) =>
    ({pitch_midi, time_beats, time_secs, duration_beats, duration_secs, velocity}));
  assert.deepEqual(music(JSON.parse(api.to_playback_events_ex(restored, "{}"))), music(events));
  const invalid = JSON.parse(plan);
  invalid.sections[1].start_measure = 3;
  const rejected = JSON.parse(api.arrange_accordion_plan(source, JSON.stringify(invalid)));
  assert.equal(rejected.score, null);
  assert.equal(rejected.diagnostics[0].code, "overlapping-range");
  const changed = JSON.parse(source);
  changed.settings.tempo_bpm += 1;
  const mismatch = JSON.parse(api.arrange_from_resolved_plan(JSON.stringify(changed), JSON.stringify(result.resolved_config)));
  assert.equal(mismatch.score, null);
  assert.equal(mismatch.diagnostics[0].code, "input-mismatch");
  snapshots[profile] = {events, generated_notes: result.generated_notes, resolved_config: result.resolved_config,
    sections: result.applied_sections, rejected: rejected.diagnostics, mismatch: mismatch.diagnostics};
}
assert.deepEqual(snapshots.full, snapshots.viewer);
console.log(JSON.stringify({profiles: ["full", "viewer"], events: snapshots.full.events.length,
  generated_notes: snapshots.full.generated_notes.length, parity: true, replay: true, svg: true, mscz_roundtrip: true}));
