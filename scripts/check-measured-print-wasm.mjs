// Compare the same canonical Score, PrintConfig and font bytes on native and WASM.
// Build with wasm-pack --target nodejs --features viewer,musicxml,measured-print.
import assert from 'node:assert/strict';
import { createHash } from 'node:crypto';
import { readFileSync } from 'node:fs';
import { createRequire } from 'node:module';
import { resolve } from 'node:path';

const [modulePath, fontPath, ...cases] = process.argv.slice(2);
assert(modulePath && fontPath && cases.length, 'usage: check-measured-print-wasm.mjs WASM.js FONT.ttf NATIVE.json ...');
const api = createRequire(import.meta.url)(resolve(modulePath));
const font = readFileSync(fontPath);
const tolerance = 0.0001;
let maximumDelta = 0;
function compare(a, b, location) {
  if (typeof a === 'number' && typeof b === 'number') {
    assert(Number.isFinite(a) && Number.isFinite(b), location);
    maximumDelta = Math.max(maximumDelta, Math.abs(a - b));
    assert(Math.abs(a - b) <= tolerance, `${location}: ${a} != ${b}`);
  } else if (typeof a === 'string' && location.includes('.pages[')) {
    // Keep every nonnumeric SVG byte identical and compare the coordinates separately.
    const number = /[-+]?(?:\d*\.\d+|\d+\.?\d*)(?:[eE][-+]?\d+)?/g;
    assert.equal(a.replace(number, '#'), b.replace(number, '#'), location);
    compare([...a.matchAll(number)].map(x => +x[0]), [...b.matchAll(number)].map(x => +x[0]), `${location}.geometry`);
  } else if (Array.isArray(a)) {
    assert(Array.isArray(b), location);
    assert.equal(a.length, b.length, location);
    a.forEach((v, i) => compare(v, b[i], `${location}[${i}]`));
  } else if (a && typeof a === 'object') {
    assert.deepEqual(Object.keys(a).sort(), Object.keys(b).sort(), location);
    for (const key of Object.keys(a)) compare(a[key], b[key], `${location}.${key}`);
  } else assert.equal(a, b, location);
}
const results = cases.map(path => {
  const input = readFileSync(path);
  const evidence = JSON.parse(input);
  const start = performance.now();
  const output = JSON.parse(api.engrave_print_pages(JSON.stringify(evidence.score), JSON.stringify(evidence.config), font));
  const elapsedMs = performance.now() - start;
  compare(evidence.rendered, output, 'rendered');
  return { path, sha256: createHash('sha256').update(input).digest('hex'), pages: output.pages.length, elapsed_ms: elapsedMs };
});
console.log(JSON.stringify({ status: 'pass', tolerance, maximum_delta: maximumDelta, font_sha256: createHash('sha256').update(font).digest('hex'), cases: results }, null, 2));
