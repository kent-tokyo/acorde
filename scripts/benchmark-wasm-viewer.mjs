// Run after building both profiles into target/wasm-profiles/{full,viewer}.
// PLAYWRIGHT_MODULE may point to an installed playwright/index.mjs.
import { createServer } from "node:http";
import { readFile } from "node:fs/promises";
import { brotliCompressSync, constants, gzipSync } from "node:zlib";
import { resolve } from "node:path";
import { execFileSync } from "node:child_process";

const playwright = await import(process.env.PLAYWRIGHT_MODULE ?? "playwright");
const root = resolve(import.meta.dirname, "..");
const toolShell = process.platform === "darwin" ? "/bin/zsh" : "/bin/sh";
const toolShellFlag = process.platform === "darwin" ? "-lc" : "-c";
const toolVersion = (command) => execFileSync(toolShell, [toolShellFlag, command], {
  encoding: "utf8",
}).trim();
const profiles = ["full", "viewer"];
const files = Object.fromEntries(
  await Promise.all(profiles.map(async (profile) => {
    const dir = resolve(root, "target/wasm-profiles", profile);
    const wasm = await readFile(resolve(dir, "acorde_wasm_bg.wasm"));
    const js = await readFile(resolve(dir, "acorde_wasm.js"));
    const gzip = gzipSync(wasm, { level: 9 });
    const brotli = brotliCompressSync(wasm, {
      params: { [constants.BROTLI_PARAM_QUALITY]: 11 },
    });
    return [profile, { wasm, js, gzip, brotli }];
  })),
);

const server = createServer((req, res) => {
  res.setHeader("Cache-Control", "no-store");
  if (req.url === "/") {
    res.setHeader("Content-Type", "text/html");
    res.end("<!doctype html><meta charset=utf-8><title>Acorde WASM benchmark</title>");
    return;
  }
  const match = /^\/(full|viewer)\/acorde_wasm(\.js|_bg\.wasm)$/.exec(req.url ?? "");
  if (!match) { res.writeHead(404).end(); return; }
  const artifact = files[match[1]];
  if (match[2] === ".js") {
    res.setHeader("Content-Type", "text/javascript");
    res.end(artifact.js);
  } else {
    res.setHeader("Content-Type", "application/wasm");
    res.setHeader("Content-Encoding", "br");
    res.end(artifact.brotli);
  }
});
await new Promise((done) => server.listen(0, "127.0.0.1", done));
const port = server.address().port;
const base = `http://127.0.0.1:${port}`;

const tinyAbc = "X:1\nT:Original tiny tune\nM:4/4\nL:1/8\nK:C\nCDEF GABc|cBAG FEDC|\n";
const browser = await playwright.chromium.launch({ channel: "chrome", headless: true });
const chromeVersion = browser.version();

async function freshPage() {
  const context = await browser.newContext({ serviceWorkers: "block" });
  const page = await context.newPage();
  const cdp = await context.newCDPSession(page);
  await cdp.send("Network.enable");
  await cdp.send("Network.setCacheDisabled", { cacheDisabled: true });
  await cdp.send("Network.emulateNetworkConditions", {
    offline: false, latency: 80, downloadThroughput: 1_000_000, uploadThroughput: 1_000_000,
  });
  await page.goto(base);
  return { context, page };
}

let largeBytes;
try {
  // Fixture setup is outside timed runs and uses the exact bytes in both profiles.
  const setup = await freshPage();
  largeBytes = await setup.page.evaluate(async ({ base, tinyAbc }) => {
    const wasm = await import(`${base}/viewer/acorde_wasm.js`);
    await wasm.default(await (await fetch(`${base}/viewer/acorde_wasm_bg.wasm`)).arrayBuffer());
    const score = JSON.parse(wasm.parse_abc(tinyAbc));
    const original = score.parts[0];
    score.parts = Array.from({ length: 4 }, (_, partIndex) => {
      const part = structuredClone(original);
      part.id = crypto.randomUUID();
      part.name = `Generated part ${partIndex + 1}`;
      part.staves = Array.from({ length: 2 }, () => {
        const staff = structuredClone(original.staves[0]);
        staff.measures = Array.from({ length: 32 }, (_, index) => {
          const measure = structuredClone(original.staves[0].measures[index % 2]);
          measure.number = index + 1;
          measure.id = crypto.randomUUID();
          for (const voice of measure.voices) {
            for (const note of voice) note.id = crypto.randomUUID();
          }
          return measure;
        });
        return staff;
      });
      return part;
    });
    return Array.from(wasm.serialize_mscz(JSON.stringify(score)));
  }, { base, tinyAbc });
  await setup.context.close();

  const results = {};
  for (const profile of profiles) {
    results[profile] = {};
    for (const fixture of ["tiny-abc", "generated-4-part-8-staff-32-bar-mscz"]) {
      const trials = [];
      for (let trial = 0; trial < 3; trial++) {
        const { context, page } = await freshPage();
        const measurement = await page.evaluate(async ({ base, profile, fixture, tinyAbc, largeBytes }) => {
          const wasm = await import(`${base}/${profile}/acorde_wasm.js`);
          const url = `${base}/${profile}/acorde_wasm_bg.wasm`;
          const transferStart = performance.now();
          const bytes = await (await fetch(url, { cache: "no-store" })).arrayBuffer();
          const transferMs = performance.now() - transferStart;
          const initStart = performance.now();
          await wasm.default(bytes);
          const initMs = performance.now() - initStart;
          const parseStart = performance.now();
          const scoreJson = fixture === "tiny-abc" ? wasm.parse_abc(tinyAbc) :
            wasm.parse_mscz(new Uint8Array(largeBytes));
          const parseMs = performance.now() - parseStart;
          const svgStart = performance.now();
          const svg = wasm.render_score_svg(scoreJson, "{}");
          const svgMs = performance.now() - svgStart;
          if (!svg.includes("data-note-addr") || !svg.includes("acorde-beam")) {
            throw new Error("SVG lost note addresses or beaming");
          }
          const score = JSON.parse(scoreJson);
          const events = JSON.parse(wasm.to_playback_events_ex(scoreJson, "{}"));
          if (!events.length) throw new Error("playback events missing");
          const exported = wasm.serialize_mscz(scoreJson);
          if (!exported.length) throw new Error("MSCZ export missing");
          const resource = performance.getEntriesByName(url).at(-1);
          return {
            transferMs, initMs, parseMs, svgMs,
            transferBytes: resource?.transferSize ?? null,
            encodedBytes: resource?.encodedBodySize ?? null,
            decodedBytes: resource?.decodedBodySize ?? null,
            scoreParts: score.parts.length, scoreStaves: score.parts.reduce((n, p) => n + p.staves.length, 0),
            playbackEvents: events.length, msczBytes: exported.length,
          };
        }, { base, profile, fixture, tinyAbc, largeBytes });
        trials.push(measurement);
        await context.close();
      }
      results[profile][fixture] = trials;
    }
  }
  const sizes = Object.fromEntries(profiles.map((profile) => {
    const { wasm, gzip, brotli } = files[profile];
    return [profile, { raw: wasm.length, gzip: gzip.length, brotli: brotli.length }];
  }));
  console.log(JSON.stringify({
    gitHead: execFileSync("git", ["rev-parse", "HEAD"], { cwd: root, encoding: "utf8" }).trim(),
    rustc: execFileSync("rustc", ["--version"], { encoding: "utf8" }).trim(),
    wasmPack: toolVersion("wasm-pack --version"),
    wasmOpt: toolVersion("wasm-opt --version"),
    browser: `Chrome ${chromeVersion}`,
    conditions: "cold context, cache disabled, 80 ms latency, 1 MB/s download/upload, three runs",
    fixture: { tinyAbc, generatedMsczBytes: largeBytes.length, generatedParts: 4, generatedStaves: 8, generatedBars: 32 },
    sizes, results,
  }, null, 2));
} finally {
  await browser.close();
  server.close();
}
