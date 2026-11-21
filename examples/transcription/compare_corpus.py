#!/usr/bin/env python3
"""Compare two MIDI transcriptions across a fixed manifest of aligned excerpts."""

import argparse
import hashlib
import json
import math
from pathlib import Path

import mir_eval

from evaluate import extract


def summarize(reference, estimate):
    matches = mir_eval.transcription.match_notes(
        *reference, *estimate, onset_tolerance=0.05, pitch_tolerance=50, offset_ratio=None)
    matched = len(matches)
    reference_count = len(reference[0])
    estimate_count = len(estimate[0])
    return {'matched': matched, 'reference': reference_count, 'estimated': estimate_count,
            'false_positive': estimate_count - matched, 'missed': reference_count - matched}


def rates(counts):
    precision = counts['matched'] / counts['estimated'] if counts['estimated'] else 0.0
    recall = counts['matched'] / counts['reference'] if counts['reference'] else 0.0
    f1 = 2 * precision * recall / (precision + recall) if precision + recall else 0.0
    return {**counts, 'precision': precision, 'recall': recall, 'f1': f1}


def compare(manifest_path):
    manifest = json.loads(manifest_path.read_text())
    if not isinstance(manifest, list) or not manifest:
        raise ValueError('manifest must contain a non-empty list')
    totals = {model: dict.fromkeys(('matched', 'reference', 'estimated', 'false_positive', 'missed'), 0)
              for model in ('baseline', 'candidate')}
    results = []
    seen = set()
    for item in manifest:
        label = item['id']
        if not isinstance(label, str) or not label or label in seen:
            raise ValueError('each excerpt must have a unique non-empty id')
        seen.add(label)
        start, end = item.get('start', 0.0), item.get('end')
        if (not isinstance(start, (int, float)) or isinstance(start, bool) or not math.isfinite(start)
                or start < 0 or (end is not None and (not isinstance(end, (int, float))
                    or isinstance(end, bool) or not math.isfinite(end) or end <= start))):
            raise ValueError(f'invalid excerpt range: {label}')
        paths = {key: (manifest_path.parent / item[key]).resolve()
                 for key in ('reference', 'baseline', 'candidate')}
        if 'audio' in item:
            paths['audio'] = (manifest_path.parent / item['audio']).resolve()
        hashes = {key: hashlib.sha256(path.read_bytes()).hexdigest() for key, path in paths.items()}
        expected_hashes = item.get('sha256', {})
        if not isinstance(expected_hashes, dict) or any(
                key not in hashes or value != hashes[key] for key, value in expected_hashes.items()):
            raise ValueError(f'file hash does not match manifest: {label}')
        reference = extract(paths['reference'], start=start, end=end)
        row = {'id': label, 'start': start, 'end': end, 'sha256': hashes}
        for model in ('baseline', 'candidate'):
            counts = summarize(reference, extract(paths[model], start=start, end=end))
            for key in counts:
                totals[model][key] += counts[key]
            row[model] = rates(counts)
        row['recall_change'] = row['candidate']['recall'] - row['baseline']['recall']
        results.append(row)
    return {'onset_tolerance_seconds': 0.05, 'pitch_tolerance_cents': 50,
            'excerpts': results, 'aggregate': {key: rates(value) for key, value in totals.items()}}


def main():
    parser = argparse.ArgumentParser(description=__doc__)
    parser.add_argument('manifest', type=Path, help='JSON list of reference, baseline, and candidate MIDI paths')
    args = parser.parse_args()
    print(json.dumps(compare(args.manifest.resolve()), indent=2))


if __name__ == '__main__':
    main()
