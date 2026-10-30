#!/usr/bin/env node
// Export pinned alphaTab 1.8.4 synthesized stereo float32 audio without a browser.
// Usage: node scripts/export_alphatab_audio.mjs PREFIX SCORE SF2 OUTPUT.f32le [SAMPLE_RATE]

import fs from 'node:fs';
import path from 'node:path';
import crypto from 'node:crypto';
import { pathToFileURL } from 'node:url';

const [prefix, scorePath, soundfontPath, outputPath, rateText = '8000'] = process.argv.slice(2);
if (!prefix || !scorePath || !soundfontPath || !outputPath) {
    throw new Error('expected PREFIX SCORE SF2 OUTPUT.f32le [SAMPLE_RATE]');
}
if ([scorePath, soundfontPath].some(input => path.resolve(input) === path.resolve(outputPath))) {
    throw new Error('audio output must not replace a score or SoundFont input');
}
const sampleRate = Number(rateText);
if (!Number.isInteger(sampleRate) || sampleRate < 8000 || sampleRate > 384000) {
    throw new Error('sample rate must be an integer between 8000 and 384000');
}
const packageDir = path.join(prefix, 'node_modules', '@coderline', 'alphatab');
const packageInfo = JSON.parse(fs.readFileSync(path.join(packageDir, 'package.json'), 'utf8'));
if (packageInfo.version !== '1.8.4') {
    throw new Error(`expected alphaTab 1.8.4, got ${packageInfo.version}`);
}
const alphaTab = await import(pathToFileURL(path.join(packageDir, 'dist', 'alphaTab.mjs')).href);
const scoreBytes = fs.readFileSync(scorePath);
const soundfontBytes = fs.readFileSync(soundfontPath);
const settings = new alphaTab.Settings();
const score = alphaTab.importer.ScoreLoader.loadScoreFromBytes(
    new Uint8Array(scoreBytes),
    settings,
);
const midi = new alphaTab.midi.MidiFile();
const handler = new alphaTab.midi.AlphaSynthMidiFileHandler(midi, true);
new alphaTab.midi.MidiFileGenerator(score, settings, handler).generate();

// AlphaSynth's public export method creates its own offline synthesizer. This silent
// output satisfies the live-output interface required by its constructor.
const emitter = { on() {} };
const output = {
    sampleRate,
    ready: emitter,
    sampleRequest: emitter,
    samplesPlayed: emitter,
    open() {},
};
const synthesizer = new alphaTab.synth.AlphaSynth(output, 0);
const options = new alphaTab.synth.AudioExportOptions();
options.sampleRate = sampleRate;
options.soundFonts = [new Uint8Array(soundfontBytes)];
options.useSyncPoints = false;
const exporter = synthesizer.exportAudio(options, midi, [], new Map());

let frames = 0;
let complete = false;
let nonFiniteSamples = 0;
let firstNonFiniteFrame = null;
const audioHash = crypto.createHash('sha256');
const fd = fs.openSync(outputPath, 'w');
try {
    for (let i = 0; i < 1800; i++) {
        const chunk = exporter.render(500);
        if (!chunk) {
            complete = true;
            break;
        }
        const samples = chunk.samples;
        if (samples.length % 2 !== 0) {
            throw new Error('alphaTab returned non-stereo float32 audio');
        }
        for (let index = 0; index < samples.length; index++) {
            if (!Number.isFinite(samples[index])) {
                nonFiniteSamples++;
                firstNonFiniteFrame ??= frames + Math.floor(index / 2);
            }
        }
        const bytes = Buffer.from(samples.buffer, samples.byteOffset, samples.byteLength);
        for (let offset = 0; offset < bytes.length;) {
            const written = fs.writeSync(fd, bytes, offset, bytes.length - offset);
            if (written === 0) throw new Error('audio output made no progress');
            offset += written;
        }
        audioHash.update(bytes);
        frames += samples.length / 2;
    }
} finally {
    fs.closeSync(fd);
    if (!complete) fs.rmSync(outputPath, { force: true });
}
if (!complete) throw new Error('audio export exceeded 15 minutes');
const sha256 = bytes => crypto.createHash('sha256').update(bytes).digest('hex');
console.log(JSON.stringify({
    alphaTab: packageInfo.version,
    title: score.title,
    bars: score.masterBars.length,
    sampleRate,
    channels: 2,
    frames,
    scoreSha256: sha256(scoreBytes),
    soundfontSha256: sha256(soundfontBytes),
    audioSha256: audioHash.digest('hex'),
    nonFiniteSamples,
    firstNonFiniteFrame,
}));
