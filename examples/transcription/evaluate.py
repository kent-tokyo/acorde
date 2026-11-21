#!/usr/bin/env python3
"""Measure note transcription against an aligned reference MIDI performance."""

import argparse
import hashlib
import json
from pathlib import Path

import mir_eval
import numpy as np
import pretty_midi


def extract(path):
    midi = pretty_midi.PrettyMIDI(str(path))
    notes = [n for instrument in midi.instruments if not instrument.is_drum for n in instrument.notes]
    if not notes:
        raise ValueError(f'No notes in {path}')
    intervals = np.array([[n.start, n.end] for n in notes])
    pitches = np.array([pretty_midi.note_number_to_hz(n.pitch) for n in notes])
    return intervals, pitches


def main():
    parser = argparse.ArgumentParser(description=__doc__)
    parser.add_argument('reference', type=Path)
    parser.add_argument('estimate', type=Path)
    args = parser.parse_args()
    reference = extract(args.reference)
    estimate = extract(args.estimate)
    output = {'files': {str(path): hashlib.sha256(path.read_bytes()).hexdigest()
                        for path in (args.reference, args.estimate)},
              'reference_notes': len(reference[0]), 'estimated_notes': len(estimate[0]),
              'onset_tolerance_seconds': 0.05, 'pitch_tolerance_cents': 50}
    for name, offset_ratio in [('onset', None), ('onset_offset', 0.2)]:
        precision, recall, f1, overlap = mir_eval.transcription.precision_recall_f1_overlap(
            *reference, *estimate, onset_tolerance=0.05, pitch_tolerance=50,
            offset_ratio=offset_ratio, offset_min_tolerance=0.05)
        output[name] = dict(precision=precision, recall=recall, f1=f1, average_overlap=overlap)
    print(json.dumps(output, indent=2))


if __name__ == '__main__':
    main()
