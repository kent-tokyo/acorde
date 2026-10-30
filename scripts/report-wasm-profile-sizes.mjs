import { readFile } from "node:fs/promises";
import { resolve } from "node:path";
import { brotliCompressSync, constants, gzipSync } from "node:zlib";

const root = resolve(import.meta.dirname, "..");
const profiles = [
  "viewer", "viewer-musicxml", "viewer-midi", "viewer-gp",
  "viewer-mei", "viewer-soundfont", "full",
];
const sizes = {};
for (const profile of profiles) {
  const dir = resolve(root, "target/wasm-profiles", profile);
  const wasm = await readFile(resolve(dir, "acorde_wasm_bg.wasm"));
  const declaration = await readFile(resolve(dir, "acorde_wasm.d.ts"), "utf8");
  const exports = [...declaration.matchAll(/^export function (\w+)\(/gm)].map((match) => match[1]);
  sizes[profile] = {
    raw: wasm.length,
    gzip: gzipSync(wasm, { level: 9 }).length,
    brotli: brotliCompressSync(wasm, {
      params: { [constants.BROTLI_PARAM_QUALITY]: 11 },
    }).length,
    exports,
  };
}
console.log(JSON.stringify(sizes, null, 2));
