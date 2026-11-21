#!/usr/bin/env python3
"""Compare two MIDI transcriptions across a fixed manifest of aligned excerpts."""

import argparse
import hashlib
import json
import math
from pathlib import Path

import mir_eval
import numpy as np

from evaluate import extract


def summarize(reference, estimate, *, start=0.0, end=None, offset_ratio=None):
    def inside(onset):
        return bool(start <= onset and (end is None or onset < end))
    def subset(events, indices):
        return tuple(part[np.asarray(indices, dtype=int)] for part in events)
    def match(left, right):
        return mir_eval.transcription.match_notes(
            *left, *right, onset_tolerance=0.05, pitch_tolerance=50,
            offset_ratio=offset_ratio, offset_min_tolerance=0.05)
    inside_refs = [i for i, interval in enumerate(reference[0]) if inside(interval[0])]
    outside_refs = [i for i, interval in enumerate(reference[0]) if not inside(interval[0])]
    matches = match(subset(reference, inside_refs), estimate)
    matched = len(matches)
    matched_estimates = {est for _, est in matches}
    remaining_estimates = [i for i in range(len(estimate[0])) if i not in matched_estimates]
    boundary_matches = match(subset(reference, outside_refs), subset(estimate, remaining_estimates))
    matched_estimates.update(remaining_estimates[est] for _, est in boundary_matches)
    false_positive = sum(inside(interval[0]) and index not in matched_estimates
                         for index, interval in enumerate(estimate[0]))
    reference_count = len(inside_refs)
    estimate_count = matched + false_positive
    return {'matched': matched, 'reference': reference_count, 'estimated': estimate_count,
            'false_positive': false_positive, 'missed': reference_count - matched}


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
        margin_start = max(0.0, start - 0.05)
        margin_end = None if end is None else end + 0.05
        reference = extract(paths['reference'], start=margin_start, end=margin_end)
        row = {'id': label, 'start': start, 'end': end, 'sha256': hashes}
        for model in ('baseline', 'candidate'):
            estimate = extract(paths[model], start=margin_start, end=margin_end)
            counts = summarize(reference, estimate, start=start, end=end)
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
