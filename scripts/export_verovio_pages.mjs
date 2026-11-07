#!/usr/bin/env node
// Render every page with an independently pinned Verovio npm package.
// Usage: node scripts/export_verovio_pages.mjs PREFIX INPUT.musicxml OUTPUT_PREFIX [VERSION]

import fs from 'node:fs';
import path from 'node:path';
import { pathToFileURL } from 'node:url';

const [prefix, inputPath, outputPrefix, expectedVersion = '6.4.1'] = process.argv.slice(2);
if (!prefix || !inputPath || !outputPrefix) {
    throw new Error('expected PREFIX INPUT.musicxml OUTPUT_PREFIX [VERSION]');
}

const packageDir = path.join(prefix, 'node_modules', 'verovio');
const packageInfo = JSON.parse(fs.readFileSync(path.join(packageDir, 'package.json'), 'utf8'));
if (packageInfo.version !== expectedVersion) {
    throw new Error(`expected Verovio ${expectedVersion}, got ${packageInfo.version}`);
}
const moduleUrl = pathToFileURL(path.join(packageDir, 'dist', 'verovio-module.mjs')).href;
const toolkitUrl = pathToFileURL(path.join(packageDir, 'dist', 'verovio.mjs')).href;
const { default: createVerovioModule } = await import(moduleUrl);
const { VerovioToolkit } = await import(toolkitUrl);
const toolkit = new VerovioToolkit(await createVerovioModule());
toolkit.setOptions({
    inputFrom: 'xml',
    pageWidth: 2100,
    pageHeight: 2970,
    pageMarginTop: 160,
    pageMarginBottom: 160,
    pageMarginLeft: 140,
    pageMarginRight: 140,
});
const input = fs.readFileSync(inputPath);
const loaded = path.extname(inputPath).toLowerCase() === '.mxl'
    ? toolkit.loadZipDataBuffer(Uint8Array.from(input).buffer)
    : toolkit.loadData(input.toString('utf8'));
if (!loaded) {
    throw new Error(`Verovio could not load ${inputPath}`);
}
const pageCount = toolkit.getPageCount();
if (pageCount < 1) {
    throw new Error(`Verovio produced no pages for ${inputPath}`);
}
for (let page = 1; page <= pageCount; page++) {
    fs.writeFileSync(`${outputPrefix}-${page}.svg`, toolkit.renderToSVG(page));
}
console.log(JSON.stringify({version: packageInfo.version, pages: pageCount}));
