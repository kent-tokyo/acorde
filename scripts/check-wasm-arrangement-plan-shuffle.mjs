// Build full/viewer web packages into target/wasm-profiles before running.
import assert from "node:assert/strict";
import { readFile } from "node:fs/promises";
import { resolve } from "node:path";
import { pathToFileURL } from "node:url";

const root = resolve(import.meta.dirname, "..");
const abc = await readFile(resolve(root, "tests/fixtures/arrangement_plan_eight_bars.abc"), "utf8");
const plan = JSON.parse(await readFile(resolve(root, "tests/fixtures/arrangement_plan_eight_bars.json"), "utf8"));
const request = {
  chords: plan.chords,
  bass_mode: plan.bass_mode,
  sections: plan.sections.map(({start_measure, end_measure, accompaniment, melodic_voice}) => ({
    start_measure, end_measure,
    allowed_accompaniments: [accompaniment],
    allowed_melodic_voices: [melodic_voice],
  })),
  seed: "1234567890123456789",
  max_attempts: 64,
};
request.sections[0].allowed_accompaniments.push("held-bass-chord");
request.sections[1].allowed_accompaniments.push("held-bass-chord");
request.sections[1].allowed_melodic_voices.push("unchanged");
const snapshots = {};
for (const profile of ["full", "viewer"]) {
  const directory = resolve(root, "target/wasm-profiles", profile);
  const api = await import(pathToFileURL(resolve(directory, "acorde_wasm.js")));
  await api.default({ module_or_path: await readFile(resolve(directory, "acorde_wasm_bg.wasm")) });
  const source = api.parse_abc(abc);
  const select = (req) => JSON.parse(api.shuffle_accordion_plan(source, JSON.stringify(req)));
  const result = select(request);
  assert.deepEqual(result.diagnostics, []);
  assert(result.score);
  assert.equal(result.pool_size, 8);
  assert.equal(result.selection_contract_version, 1);
  const again = select(request);
  assert.equal(result.selected_pool_index, again.selected_pool_index);
  assert.deepEqual(result.applied_sections, again.applied_sections);
  assert.deepEqual(result.generated_notes, again.generated_notes);
  const reordered = structuredClone(request);
  reordered.sections[0].allowed_accompaniments.reverse();
  reordered.sections[0].allowed_accompaniments.push("held-bass-chord");
  const canonical = select(reordered);
  assert.equal(result.selected_pool_index, canonical.selected_pool_index);
  const arranged = JSON.stringify(result.score);
  const events = JSON.parse(api.to_playback_events_ex(arranged, "{}"));
  const replay = JSON.parse(api.arrange_from_resolved_plan(api.parse_abc(abc), JSON.stringify(result.resolved_config)));
  assert.deepEqual(replay.diagnostics, []);
  assert.deepEqual(JSON.parse(api.to_playback_events_ex(JSON.stringify(replay.score), "{}")), events);
  assert.deepEqual(replay.generated_notes, result.generated_notes);
  const address = result.generated_notes[0].address;
  const noteAddress = [address.part, address.staff, address.measure, address.voice, address.note].join(":");
  assert(api.render_score_svg(arranged, '{"interactive":true}').includes(`data-note-addr="${noteAddress}"`));
  const restored = api.parse_mscz(api.serialize_mscz(arranged));
  const music = (value) => value.map(({pitch_midi, time_beats, time_secs, duration_beats, duration_secs, velocity}) =>
    ({pitch_midi, time_beats, time_secs, duration_beats, duration_secs, velocity}));
  assert.deepEqual(music(JSON.parse(api.to_playback_events_ex(restored, "{}"))), music(events));
  const impossible = structuredClone(request);
  impossible.sections[1].allowed_accompaniments = ["waltz-bass-chord"];
  impossible.sections[1].allowed_melodic_voices = ["unchanged"];
  const none = select(impossible);
  assert.equal(none.diagnostics[0].code, "no-candidate");
  assert.equal(none.score, null);
  impossible.sections[1].allowed_accompaniments.push("arpeggio");
  impossible.max_attempts = 1;
  const limited = select(impossible);
  assert.equal(limited.diagnostics[0].code, "search-limit");
  snapshots[profile] = {events, config: result.resolved_config, generated: result.generated_notes,
    selection: result.selected_pool_index, attempts: result.attempted, rejected: result.rejected,
    none: none.diagnostics, limited: limited.diagnostics};
}
assert.deepEqual(snapshots.full, snapshots.viewer);
console.log(JSON.stringify({profiles: ["full", "viewer"], parity: true, events: snapshots.full.events.length,
  selection: snapshots.full.selection, replay: true, svg: true, mscz_roundtrip: true}));
