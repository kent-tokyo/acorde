// Build full, viewer and playback-only into target/wasm-profiles before running.
import assert from "node:assert/strict";
import { createHash } from "node:crypto";
import { readFile } from "node:fs/promises";
import { resolve } from "node:path";
import { pathToFileURL } from "node:url";
import { brotliCompressSync, constants, gzipSync } from "node:zlib";

const root = resolve(import.meta.dirname, "..");
const profiles = {};
for (const name of ["full", "viewer", "playback-only"]) {
  const dir = resolve(root, "target/wasm-profiles", name);
  const wasmBytes = await readFile(resolve(dir, "acorde_wasm_bg.wasm"));
  const declaration = await readFile(resolve(dir, "acorde_wasm.d.ts"), "utf8");
  const bindings = [...declaration.matchAll(/^export function (\w+)\(/gm)]
    .map((match) => match[1])
    .filter((binding) => binding !== "initSync");
  const api = await import(pathToFileURL(resolve(dir, "acorde_wasm.js")));
  await api.default({ module_or_path: wasmBytes });
  profiles[name] = {
    api,
    size: {
      raw: wasmBytes.length,
      gzip: gzipSync(wasmBytes, { level: 9 }).length,
      brotli: brotliCompressSync(wasmBytes, {
        params: { [constants.BROTLI_PARAM_QUALITY]: 11 },
      }).length,
    },
    bindings,
  };
}

assert.deepEqual(profiles["playback-only"].bindings, [
  "parse_mscz", "to_playback_events_ex",
]);
for (const binding of ["parse_mscz", "to_playback_events_ex"]) {
  assert(profiles.viewer.bindings.includes(binding));
  assert(profiles.full.bindings.includes(binding));
}
assert(profiles.viewer.bindings.includes("render_score_svg"));
assert(profiles.full.bindings.includes("parse_musicxml"));

const tinyAbc = "X:1\nT:Playback profile\nM:4/4\nL:1/8\nK:C\nCDEF GABc|cBAG FEDC|\n";
const tinyScore = profiles.viewer.api.parse_abc(tinyAbc);
const makeArchive = (score) => profiles.viewer.api.serialize_mscz(JSON.stringify(score));
const tiny = makeArchive(JSON.parse(tinyScore));
const denseScore = JSON.parse(tinyScore);
const originalPart = denseScore.parts[0];
let nextId = 1;
const id = () => `00000000-0000-4000-8000-${String(nextId++).padStart(12, "0")}`;
denseScore.parts = Array.from({ length: 4 }, (_, partIndex) => {
  const part = structuredClone(originalPart);
  part.id = id();
  part.name = `Generated part ${partIndex + 1}`;
  part.staves = Array.from({ length: 2 }, () => {
    const staff = structuredClone(originalPart.staves[0]);
    staff.measures = Array.from({ length: 32 }, (_, index) => {
      const measure = structuredClone(originalPart.staves[0].measures[index % 2]);
      measure.id = id();
      measure.number = index + 1;
      for (const voice of measure.voices) {
        for (const note of voice) note.id = id();
      }
      return measure;
    });
    return staff;
  });
  return part;
});
const dense = makeArchive(denseScore);

const selectedEvent = (event) => ({
  time_beats: event.time_beats,
  time_secs: event.time_secs,
  pitch_midi: event.pitch_midi,
  duration_beats: event.duration_beats,
  duration_secs: event.duration_secs,
  part_index: event.part_index,
  address: event.address,
});
const parity = {};
for (const [name, archive] of [["tiny", tiny], ["dense-4-part-8-staff-32-bar", dense]]) {
  const results = {};
  for (const profile of ["full", "viewer", "playback-only"]) {
    const api = profiles[profile].api;
    const score = api.parse_mscz(archive);
    const events = JSON.parse(api.to_playback_events_ex(score, "{}"));
    assert(events.length > 0, `${profile} ${name} has no playback events`);
    results[profile] = events.map(selectedEvent);
  }
  assert.deepEqual(results["playback-only"], results.viewer);
  assert.deepEqual(results["playback-only"], results.full);
  parity[name] = {
    archiveBytes: archive.length,
    archiveSha256: createHash("sha256").update(archive).digest("hex"),
    events: results["playback-only"].length,
    eventFieldsEqual: true,
  };
}

console.log(JSON.stringify({
  profiles: Object.fromEntries(Object.entries(profiles).map(([name, value]) => [
    name, { size: value.size, bindings: value.bindings },
  ])),
  parity,
}, null, 2));
