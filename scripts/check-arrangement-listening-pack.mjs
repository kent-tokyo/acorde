// Verify that saved arrangement contracts replay to the exported MusicXML in
// both the full and viewer WASM builds. Usage:
// node scripts/check-arrangement-listening-pack.mjs <pack> <full-pkg> <viewer-pkg>
import assert from 'node:assert/strict';
import { createHash } from 'node:crypto';
import { readFileSync } from 'node:fs';
import { resolve } from 'node:path';
import { pathToFileURL } from 'node:url';

const [, , packArg, fullArg, viewerArg] = process.argv;
if (!packArg || !fullArg || !viewerArg || process.argv.length !== 5) {
  throw new Error('usage: check-arrangement-listening-pack.mjs <pack> <full-pkg> <viewer-pkg>');
}

const pack = resolve(packArg);
const manifest = JSON.parse(readFileSync(resolve(pack, 'manifest.json'), 'utf8'));
assert.ok([1, 2].includes(manifest.schema_version));
assert.ok(Array.isArray(manifest.candidates) && manifest.candidates.length > 0);

function packBytes(name) {
  assert.match(name, /^[A-Za-z0-9.-]+$/);
  return readFileSync(resolve(pack, name));
}

function packFile(name) {
  return packBytes(name).toString('utf8');
}

function sha256(name) {
  return createHash('sha256').update(packBytes(name)).digest('hex');
}

const wavCache = new Map();
function wavStats(name) {
  if (wavCache.has(name)) return wavCache.get(name);
  const bytes = packBytes(name);
  assert.equal(bytes.toString('ascii', 0, 4), 'RIFF', `${name}: RIFF`);
  assert.equal(bytes.toString('ascii', 8, 12), 'WAVE', `${name}: WAVE`);
  let format;
  let channels;
  let sampleRate;
  let bitsPerSample;
  let dataStart;
  let dataSize;
  for (let offset = 12; offset + 8 <= bytes.length;) {
    const size = bytes.readUInt32LE(offset + 4);
    const start = offset + 8;
    const end = start + size;
    assert.ok(end <= bytes.length, `${name}: truncated WAV chunk`);
    const kind = bytes.toString('ascii', offset, offset + 4);
    if (kind === 'fmt ') {
      assert.ok(size >= 16, `${name}: short WAV format chunk`);
      format = bytes.readUInt16LE(start);
      channels = bytes.readUInt16LE(start + 2);
      sampleRate = bytes.readUInt32LE(start + 4);
      bitsPerSample = bytes.readUInt16LE(start + 14);
    } else if (kind === 'data') {
      dataStart = start;
      dataSize = size;
    }
    offset = end + (size & 1);
  }
  assert.equal(format, 1, `${name}: PCM format`);
  assert.equal(channels, manifest.channels, `${name}: channels`);
  assert.equal(sampleRate, manifest.sample_rate_hz, `${name}: sample rate`);
  assert.equal(bitsPerSample, 16, `${name}: sample depth`);
  assert.ok(dataStart !== undefined && dataSize > 0 && dataSize % (channels * 2) === 0, `${name}: sample data`);
  const frames = dataSize / (channels * 2);
  let peak = 0;
  let sumSquares = 0;
  for (let offset = dataStart; offset < dataStart + dataSize; offset += 2) {
    const sample = bytes.readInt16LE(offset);
    peak = Math.max(peak, Math.abs(sample));
    const normalized = sample / 32768;
    sumSquares += normalized * normalized;
  }
  assert.ok(peak < 32767, `${name}: sample rail hit`);
  const stats = { frames, peak, rms: Math.sqrt(sumSquares / (dataSize / 2)) };
  wavCache.set(name, stats);
  return stats;
}

async function loadWasm(directory) {
  const pkg = resolve(directory);
  const api = await import(pathToFileURL(resolve(pkg, 'acorde_wasm.js')).href);
  api.initSync({ module: readFileSync(resolve(pkg, 'acorde_wasm_bg.wasm')) });
  return api;
}

function playbackEvents(api, score) {
  const events = JSON.parse(api.to_playback_events(JSON.stringify(score), 0, '[]'));
  assert.ok(Array.isArray(events));
  return events.map(event => [
    event.source?.part,
    event.time_beats,
    event.duration_beats,
    event.pitch_midi,
  ]).sort((left, right) => JSON.stringify(left).localeCompare(JSON.stringify(right)));
}

function eventSummary(api, score) {
  const baseBeats = {
    Longa: 16, Breve: 8, Whole: 4, Half: 2, Quarter: 1,
    Eighth: 0.5, Sixteenth: 0.25, ThirtySecond: 0.125,
    SixtyFourth: 0.0625, HundredTwentyEighth: 0.03125,
  };
  const semitone = { C: 0, D: 2, E: 4, F: 5, G: 7, A: 9, B: 11 };
  const tick = beats => Math.round(beats * 480);
  const counts = values => Object.fromEntries(
    [...new Set(values)].sort((a, b) => a - b).map(value => [value, values.filter(item => item === value).length]),
  );
  const pitchClassCounts = pitches => Array.from({ length: 12 }, (_, pc) =>
    pitches.filter(pitch => ((pitch % 12) + 12) % 12 === pc).length);
  const played = JSON.parse(api.to_playback_events(JSON.stringify(score), 0, '[]'));
  return { parts: score.parts.map((part, partIndex) => {
    const writtenOnsets = Array.from({ length: Math.max(0, ...part.staves.map(staff => staff.measures.length)) }, () => []);
    const writtenDurations = [];
    const writtenPitches = [];
    let writtenAttacks = 0;
    for (const staff of part.staves) {
      for (const [measureIndex, measure] of staff.measures.entries()) {
        for (const voice of measure.voices) {
          let beat = 0;
          for (const note of voice) {
            let duration = baseBeats[note.duration];
            assert.ok(duration !== undefined, `${partIndex}: unknown duration`);
            let dot = duration / 2;
            for (let index = 0; index < note.dot_count; index++) {
              duration += dot;
              dot /= 2;
            }
            if (note.is_grace || note.is_cue) duration = 0;
            if (note.tuplet) duration *= note.tuplet.normal_notes / note.tuplet.actual_notes;
            if (!note.is_rest && !note.is_unpitched) {
              for (const pitch of note.pitches) {
                const midi = Math.round((pitch.octave + 1) * 12 + semitone[pitch.step]
                  + pitch.alter + (pitch.microtone_cents ?? 0) / 100);
                writtenDurations.push(tick(duration));
                writtenPitches.push(midi);
                if (!note.tie_end) {
                  writtenOnsets[measureIndex].push(tick(beat));
                  writtenAttacks++;
                }
              }
            }
            beat += duration;
          }
        }
      }
    }
    const events = played.filter(event => event.source?.part === partIndex && !event.is_metronome)
      .map(event => [tick(event.time_beats), tick(event.duration_beats), event.pitch_midi, event.velocity])
      .sort((a, b) => {
        for (let index = 0; index < a.length; index++) {
          if (a[index] !== b[index]) return a[index] - b[index];
        }
        return 0;
      });
    const signature = Buffer.alloc(events.length * 18);
    for (const [index, [onset, duration, pitch, velocity]] of events.entries()) {
      signature.writeBigUInt64LE(BigInt(onset), index * 18);
      signature.writeBigUInt64LE(BigInt(duration), index * 18 + 8);
      signature.writeUInt8(pitch, index * 18 + 16);
      signature.writeUInt8(velocity, index * 18 + 17);
    }
    const playedPitches = events.map(event => event[2]);
    return {
      part_index: partIndex,
      written_attack_count: writtenAttacks,
      written_attack_onsets_480_by_measure: writtenOnsets.map(onsets => [...new Set(onsets)].sort((a, b) => a - b)),
      written_fragment_duration_counts_480: counts(writtenDurations),
      written_fragment_pitch_classes: pitchClassCounts(writtenPitches),
      playback_event_count: events.length,
      playback_onsets_480: [...new Set(events.map(event => event[0]))],
      playback_duration_counts_480: counts(events.map(event => event[1])),
      playback_pitch_classes: pitchClassCounts(playedPitches),
      playback_lowest_midi: playedPitches.length ? Math.min(...playedPitches) : null,
      playback_highest_midi: playedPitches.length ? Math.max(...playedPitches) : null,
      playback_event_sha256: createHash('sha256').update(signature).digest('hex'),
    };
  }) };
}

const full = await loadWasm(fullArg);
const viewer = await loadWasm(viewerArg);
const loudnessRatios = [];
for (const candidate of manifest.candidates) {
  assert.equal(candidate.candidate_id, candidate.resolved.id);
  assert.equal(candidate.source_fingerprint, candidate.resolved.source_fingerprint);
  if (manifest.schema_version === 2) {
    assert.equal(candidate.wav_sha256, sha256(candidate.wav));
    assert.equal(candidate.source_wav_sha256, sha256(candidate.source_wav));
  }
  // Schema 1 named the source only through its JSON file. Keep archived packs
  // verifiable without changing their manifests.
  const sourceWav = candidate.source_wav ?? candidate.source_json.replace(/\.json$/, '.wav');
  const sourceAudio = wavStats(sourceWav);
  const candidateAudio = wavStats(candidate.wav);
  assert.equal(candidateAudio.frames, sourceAudio.frames, `${candidate.candidate_id}: WAV duration`);
  assert.ok(sourceAudio.rms > 0, `${candidate.candidate_id}: silent source WAV`);
  loudnessRatios.push({ id: candidate.candidate_id, ratio: candidateAudio.rms / sourceAudio.rms });
  const source = packFile(candidate.source_json);
  const exported = JSON.parse(full.parse_musicxml(packFile(candidate.score)));
  if (manifest.event_summary_schema_version === 1) {
    assert.equal(candidate.score_sha256, sha256(candidate.score));
    assert.equal(candidate.source_score_sha256, sha256(candidate.source_score));
    const sourceScore = JSON.parse(source);
    assert.equal(candidate.source_part_count, sourceScore.parts.length);
    assert.deepEqual(candidate.source_event_summary, eventSummary(full, sourceScore), `${candidate.candidate_id}: source event summary`);
    const exportedSource = JSON.parse(full.parse_musicxml(packFile(candidate.source_score)));
    assert.deepEqual(playbackEvents(full, exportedSource), playbackEvents(full, sourceScore),
      `${candidate.candidate_id}: source export playback`);
    assert.deepEqual(candidate.arrangement_event_summary.parts.slice(0, candidate.source_part_count),
      candidate.source_event_summary.parts, `${candidate.candidate_id}: original part events`);
  }
  for (const [name, api] of [['full', full], ['viewer', viewer]]) {
    const replay = JSON.parse(api.replay_arrangement_proposal(source, JSON.stringify(candidate.resolved)));
    assert.deepEqual(replay.diagnostics, [], `${candidate.candidate_id}: ${name} diagnostics`);
    assert.ok(replay.score, `${candidate.candidate_id}: ${name} has no score`);
    if (manifest.event_summary_schema_version === 1) {
      assert.deepEqual(candidate.arrangement_event_summary, eventSummary(api, replay.score),
        `${candidate.candidate_id}: ${name} arrangement event summary`);
    }
    assert.deepEqual(
      playbackEvents(api, replay.score),
      playbackEvents(api, exported),
      `${candidate.candidate_id}: ${name} export playback`,
    );
  }
}
console.log(`${manifest.partition}: ${manifest.candidates.length} candidates replay in full/viewer WASM and match MusicXML playback`);
loudnessRatios.sort((left, right) => left.ratio - right.ratio);
const quietest = loudnessRatios[0];
const loudest = loudnessRatios[loudnessRatios.length - 1];
console.log(`${manifest.partition}: WAV RMS/source range ${quietest.ratio.toFixed(2)}–${loudest.ratio.toFixed(2)}x (highest ${loudest.id}); equal frames, PCM format and no sample-rail hits`);
