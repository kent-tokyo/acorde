#!/usr/bin/env python3
"""Compare timed right-hand, bass and chord events against an annotated performance."""

import argparse
from collections import defaultdict
import hashlib
import json
import math
from pathlib import Path

import mir_eval
import numpy as np

PARTS = ('right', 'bass', 'chord')
CHORD_QUALITIES = {'major', 'minor', 'seventh', 'diminished'}


def read_events(path):
    data = json.loads(path.read_text())
    if not isinstance(data, dict) or set(data) != set(PARTS):
        raise ValueError('events must contain right, bass and chord arrays')
    groups = {}
    for part in PARTS:
        if not isinstance(data[part], list):
            raise ValueError(f'{part} must be an array')
        grouped = defaultdict(list)
        for event in data[part]:
            if not isinstance(event, dict):
                raise ValueError(f'{part} events must be objects')
            time = event.get('time')
            if (isinstance(time, bool) or not isinstance(time, (int, float))
                    or not math.isfinite(time) or time < 0):
                raise ValueError(f'{part} event time must be finite and non-negative')
            label = event.get('pitch' if part == 'right' else 'root')
            limit = 127 if part == 'right' else 11
            if isinstance(label, bool) or not isinstance(label, int) or not 0 <= label <= limit:
                raise ValueError(f'{part} event pitch or root is invalid')
            if part == 'chord':
                quality = event.get('quality')
                if quality not in CHORD_QUALITIES:
                    raise ValueError('invalid chord quality')
                label = (label, quality)
            grouped[label].append(float(time))
        groups[part] = grouped
    return groups


def compare(reference, estimate, tolerance):
    results = {}
    for part in PARTS:
        ref, est = reference[part], estimate[part]
        reference_count = sum(len(times) for times in ref.values())
        estimate_count = sum(len(times) for times in est.values())
        matched = sum(len(mir_eval.util.match_events(
            np.array(ref[label]), np.array(est[label]), window=tolerance))
            for label in ref.keys() & est.keys())
        precision = matched / estimate_count if estimate_count else 0.0
        recall = matched / reference_count if reference_count else 0.0
        f1 = 2 * precision * recall / (precision + recall) if precision + recall else 0.0
        results[part] = {'reference': reference_count, 'estimated': estimate_count,
                         'matched': matched, 'false_positive': estimate_count - matched,
                         'missed': reference_count - matched, 'precision': precision,
                         'recall': recall, 'f1': f1}
    return results


def main():
    parser = argparse.ArgumentParser(description=__doc__)
    parser.add_argument('reference', type=Path)
    parser.add_argument('estimate', type=Path)
    parser.add_argument('--tolerance', type=float, default=0.1, help='onset tolerance in seconds')
    args = parser.parse_args()
    if not math.isfinite(args.tolerance) or args.tolerance <= 0:
        parser.error('tolerance must be finite and positive')
    result = {'files': {str(path): hashlib.sha256(path.read_bytes()).hexdigest()
                        for path in (args.reference, args.estimate)},
              'onset_tolerance_seconds': args.tolerance,
              'parts': compare(read_events(args.reference), read_events(args.estimate), args.tolerance)}
    print(json.dumps(result, indent=2))


if __name__ == '__main__':
    main()
