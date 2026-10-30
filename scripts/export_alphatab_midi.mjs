#!/usr/bin/env node
// Generate an SMF1.0 playback projection from an independently pinned alphaTab install.
// Usage: node scripts/export_alphatab_midi.mjs PREFIX INPUT.mxl OUTPUT.mid

import fs from 'node:fs';
import path from 'node:path';
import { pathToFileURL } from 'node:url';

const [prefix, inputPath, outputPath] = process.argv.slice(2);
if (!prefix || !inputPath || !outputPath) {
    throw new Error('expected PREFIX INPUT.mxl OUTPUT.mid');
}
const packageDir = path.join(prefix, 'node_modules', '@coderline', 'alphatab');
const packageInfo = JSON.parse(fs.readFileSync(path.join(packageDir, 'package.json'), 'utf8'));
if (packageInfo.version !== '1.8.4') {
    throw new Error(`expected alphaTab 1.8.4, got ${packageInfo.version}`);
}
const alphaTab = await import(pathToFileURL(path.join(packageDir, 'dist', 'alphaTab.mjs')).href);
const settings = new alphaTab.Settings();
const score = alphaTab.importer.ScoreLoader.loadScoreFromBytes(
    new Uint8Array(fs.readFileSync(inputPath)),
    settings,
);
const midiFile = new alphaTab.midi.MidiFile();
const handler = new alphaTab.midi.AlphaSynthMidiFileHandler(midiFile, true);
new alphaTab.midi.MidiFileGenerator(score, settings, handler).generate();
fs.writeFileSync(outputPath, midiFile.toBinary());
console.log(JSON.stringify({ alphaTab: packageInfo.version, title: score.title, tracks: score.tracks.length, masterBars: score.masterBars.length }));
