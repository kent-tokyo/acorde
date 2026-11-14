// Bind a human listening review to the exact development audio it judged.
// Usage: node scripts/check-arrangement-listening-review.mjs <pack> <review.json>
import assert from 'node:assert/strict';
import { createHash } from 'node:crypto';
import { readFileSync } from 'node:fs';
import { resolve } from 'node:path';

const [, , packArg, reviewArg] = process.argv;
if (!packArg || !reviewArg || process.argv.length !== 4) {
  throw new Error('usage: check-arrangement-listening-review.mjs <pack> <review.json>');
}
const pack = resolve(packArg);
const manifest = JSON.parse(readFileSync(resolve(pack, 'manifest.json'), 'utf8'));
const review = JSON.parse(readFileSync(resolve(reviewArg), 'utf8'));
assert.equal(manifest.schema_version, 2);
assert.equal(review.schema_version, 2);
assert.equal(review.partition, manifest.partition);

function wavHash(name) {
  assert.match(name, /^[A-Za-z0-9.-]+\.wav$/);
  return createHash('sha256').update(readFileSync(resolve(pack, name))).digest('hex');
}

const cases = new Map();
for (const candidate of manifest.candidates) {
  assert.equal(candidate.partition, manifest.partition);
  if (!cases.has(candidate.case_id)) cases.set(candidate.case_id, []);
  cases.get(candidate.case_id).push(candidate);
}
assert.ok(cases.size > 0);
assert.ok(Array.isArray(review.cases));
assert.equal(review.cases.length, cases.size, 'every source case needs one review');

let judged = 0;
let negative = 0;
let uncertain = 0;
for (const [index, [caseId, proposals]] of [...cases].entries()) {
  const entry = review.cases[index];
  assert.equal(new Set(proposals.map(item => item.candidate_id)).size, proposals.length,
    `${caseId}: duplicate proposal ID`);
  for (const proposal of proposals) {
    assert.equal(proposal.source_fingerprint, proposals[0].source_fingerprint,
      `${caseId}: inconsistent source fingerprint`);
    assert.equal(proposal.source_wav, proposals[0].source_wav,
      `${caseId}: inconsistent source WAV`);
    assert.equal(proposal.source_wav_sha256, proposals[0].source_wav_sha256,
      `${caseId}: inconsistent source WAV hash`);
  }
  assert.equal(entry?.case_id, caseId, `case ${index + 1} identity`);
  assert.equal(entry.source_fingerprint, proposals[0].source_fingerprint, `${caseId}: source fingerprint`);
  assert.equal(entry.source_wav_sha256, proposals[0].source_wav_sha256, `${caseId}: source WAV hash`);
  assert.equal(entry.source_wav_sha256, wavHash(proposals[0].source_wav), `${caseId}: source WAV bytes`);
  assert.deepEqual(entry.proposal_ids, proposals.map(item => item.candidate_id), `${caseId}: proposal IDs`);
  assert.deepEqual(entry.proposal_wav_sha256, proposals.map(item => item.wav_sha256), `${caseId}: proposal WAV hashes`);
  for (const proposal of proposals) {
    assert.equal(proposal.wav_sha256, wavHash(proposal.wav), `${caseId}: ${proposal.candidate_id} WAV bytes`);
  }
  const best = String(entry.best_proposal_index ?? '');
  const weakest = String(entry.weakest_proposal_index ?? '');
  const validChoice = value => value === '' ||
    (/^[1-9][0-9]*$/.test(value) && Number(value) <= proposals.length);
  assert.ok(validChoice(best), `${caseId}: invalid best proposal`);
  assert.ok(validChoice(weakest), `${caseId}: invalid weakest proposal`);
  assert.ok(best === '' || weakest === '' || best !== weakest, `${caseId}: best and weakest are identical`);
  assert.ok(['yes', 'no', 'unsure'].includes(entry.distinctness), `${caseId}: distinctness unanswered`);
  if (entry.distinctness === 'yes') {
    assert.ok(best && weakest, `${caseId}: select both best and weakest proposals`);
  }
  assert.equal(typeof entry.issue_note, 'string', `${caseId}: issue note type`);
  if (entry.distinctness !== 'yes') {
    assert.ok(entry.issue_note.trim(), `${caseId}: explain indistinct or uncertain proposals`);
  }
  if (best && weakest && entry.distinctness !== 'unsure') judged++;
  if (entry.distinctness === 'no') negative++;
  if (entry.distinctness === 'unsure') uncertain++;
}
console.log(`${manifest.partition}: ${cases.size} reviewed cases and ${manifest.candidates.length} proposals match the saved WAV bytes`);
console.log(`${manifest.partition}: best/weakest selected for ${judged}/${cases.size}; indistinct ${negative}; uncertain ${uncertain}`);
