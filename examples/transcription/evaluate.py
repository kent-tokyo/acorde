#!/usr/bin/env python3
"""Measure note transcription against an aligned reference MIDI performance."""

import argparse
import hashlib
import json
import math
from pathlib import Path

import mir_eval
import numpy as np
import pretty_midi


def extract(path, *, shift=0.0, start=0.0, end=None):
    midi = pretty_midi.PrettyMIDI(str(path))
    notes = [n for instrument in midi.instruments if not instrument.is_drum
             for n in instrument.notes if start <= n.start + shift
             and (end is None or n.start + shift < end)]
    intervals = np.array([[n.start + shift, n.end + shift] for n in notes], dtype=float).reshape(-1, 2)
    pitches = np.array([pretty_midi.note_number_to_hz(n.pitch) for n in notes])
    return intervals, pitches


def main():
    parser = argparse.ArgumentParser(description=__doc__)
    parser.add_argument('reference', type=Path)
    parser.add_argument('estimate', type=Path)
    parser.add_argument('--estimate-offset', type=float, default=0.0,
                        help='seconds to add to exported MIDI times to recover recording times')
    parser.add_argument('--start', type=float, default=0.0,
                        help='include note onsets at or after this recording time')
    parser.add_argument('--end', type=float,
                        help='include note onsets before this recording time')
    args = parser.parse_args()
    if (not math.isfinite(args.estimate_offset) or args.estimate_offset < 0
            or not math.isfinite(args.start) or args.start < 0
            or (args.end is not None and (not math.isfinite(args.end) or args.end <= args.start))):
        parser.error('offset and range must be finite, non-negative, and start must precede end')
    reference = extract(args.reference, start=args.start, end=args.end)
    estimate = extract(args.estimate, shift=args.estimate_offset, start=args.start, end=args.end)
    output = {'files': {str(path): hashlib.sha256(path.read_bytes()).hexdigest()
                        for path in (args.reference, args.estimate)},
              'reference_notes': len(reference[0]), 'estimated_notes': len(estimate[0]),
              'onset_tolerance_seconds': 0.05, 'pitch_tolerance_cents': 50,
              'estimate_offset_seconds': args.estimate_offset,
              'range_start_seconds': args.start, 'range_end_seconds': args.end}
    for name, offset_ratio in [('onset', None), ('onset_offset', 0.2)]:
        precision, recall, f1, overlap = mir_eval.transcription.precision_recall_f1_overlap(
            *reference, *estimate, onset_tolerance=0.05, pitch_tolerance=50,
            offset_ratio=offset_ratio, offset_min_tolerance=0.05)
        output[name] = dict(precision=precision, recall=recall, f1=f1, average_overlap=overlap)
    print(json.dumps(output, indent=2))


if __name__ == '__main__':
    main()
