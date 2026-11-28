// One isolated Node process per engine/input. Both engines parse the same plain
// MusicXML and render every page on each iteration. No result is a quality pass.
// Usage: bench_print_parity.mjs acorde|verovio MODULE_OR_PREFIX INPUT.xml FONT CONFIG [ROUNDS]
// VEROVIO_VERSION selects the expected npm version (default 6.4.1).
import fs from 'node:fs';
import path from 'node:path';
import { createRequire } from 'node:module';
import { pathToFileURL } from 'node:url';
import { createHash } from 'node:crypto';

const [engine, modulePath, inputPath, fontPath, configPath, count = '9'] = process.argv.slice(2);
const rounds = Number(count);
if (!['acorde', 'verovio'].includes(engine) || !Number.isInteger(rounds) || rounds < 3 || rounds > 100) {
  throw new Error('expected engine and 3..100 rounds');
}
const input = fs.readFileSync(inputPath);
const xml = input.toString('utf8');
const font = fs.readFileSync(fontPath);
const config = fs.readFileSync(configPath, 'utf8');
const initializationStart = performance.now();
let render;
if (engine === 'acorde') {
  const api = createRequire(import.meta.url)(path.resolve(modulePath));
  render = () => JSON.parse(api.engrave_print_pages(api.parse_musicxml(xml), config, font)).pages;
} else {
  const dir = path.join(modulePath, 'node_modules/verovio');
  const version = JSON.parse(fs.readFileSync(path.join(dir, 'package.json'), 'utf8')).version;
  const expected = process.env.VEROVIO_VERSION || '6.4.1';
  if (version !== expected) throw new Error(`unexpected Verovio version: ${version}`);
  const { default: createModule } = await import(pathToFileURL(path.join(dir, 'dist/verovio-module.mjs')));
  const { VerovioToolkit } = await import(pathToFileURL(path.join(dir, 'dist/verovio.mjs')));
  const toolkit = new VerovioToolkit(await createModule());
  toolkit.setOptions({ inputFrom: 'xml', pageWidth: 2100, pageHeight: 2970,
    pageMarginTop: 160, pageMarginBottom: 160, pageMarginLeft: 140, pageMarginRight: 140 });
  render = () => {
    if (!toolkit.loadData(xml)) throw new Error('MusicXML load failed');
    return Array.from({ length: toolkit.getPageCount() }, (_, i) => toolkit.renderToSVG(i + 1));
  };
}
const initializationMs = performance.now() - initializationStart;
const initialRss = process.memoryUsage().rss;
const samples = [];
let pages = 0, outputBytes = 0;
for (let i = -2; i < rounds; i++) {
  const start = performance.now();
  const output = render();
  const elapsed = performance.now() - start;
  if (!output.length) throw new Error('no pages');
  if (i >= 0) samples.push(elapsed);
  pages = output.length;
  outputBytes = output.reduce((sum, svg) => sum + Buffer.byteLength(svg), 0);
}
const ordered = [...samples].sort((a, b) => a - b);
console.log(JSON.stringify({ engine, node: process.version, platform: process.platform, arch: process.arch,
  input_sha256: createHash('sha256').update(input).digest('hex'), warmups: 2, rounds,
  operation: 'parse MusicXML and render all page SVGs; Acorde also serializes canonical JSON and outlines text',
  initialization_ms: initializationMs, samples_ms: samples,
  median_ms: ordered[Math.floor(ordered.length / 2)], p95_ms: ordered[Math.ceil(ordered.length * 0.95) - 1],
  rss_after_initialization_bytes: initialRss, rss_after_iterations_bytes: process.memoryUsage().rss,
  process_max_rss_kib: process.resourceUsage().maxRSS, pages, svg_bytes: outputBytes }, null, 2));
