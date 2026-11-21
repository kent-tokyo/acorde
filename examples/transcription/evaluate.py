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


def extract_playback_report(path, *, shift=0.0, start=0.0, end=None):
    events = json.loads(path.read_text())
    notes = [(event['time_secs'] + shift, event['duration_secs'], event['pitch_midi'])
             for event in events if event.get('pitch_midi') is not None
             and not event.get('is_metronome', False)
             and start <= event['time_secs'] + shift
             and (end is None or event['time_secs'] + shift < end)]
    intervals = np.array([[onset, onset + duration] for onset, duration, _ in notes],
                         dtype=float).reshape(-1, 2)
    pitches = np.array([pretty_midi.note_number_to_hz(pitch) for _, _, pitch in notes])
    return intervals, pitches


def window_events(reference, estimate, *, start, end):
    """Match boundary context before selecting notes for a half-open window."""
    def inside(onset):
        return bool(start <= onset and (end is None or onset < end))
    def subset(events, indices):
        return tuple(part[np.asarray(indices, dtype=int)] for part in events)
    def match(left, right):
        return mir_eval.transcription.match_notes(
            *left, *right, onset_tolerance=0.05, pitch_tolerance=50, offset_ratio=None)

    inside_refs = [i for i, interval in enumerate(reference[0]) if inside(interval[0])]
    outside_refs = [i for i, interval in enumerate(reference[0]) if not inside(interval[0])]
    matches = match(subset(reference, inside_refs), estimate)
    selected_estimates = {est for _, est in matches}
    remaining = [i for i in range(len(estimate[0])) if i not in selected_estimates]
    outside_matches = match(subset(reference, outside_refs), subset(estimate, remaining))
    matched_outside = {remaining[est] for _, est in outside_matches}
    selected_estimates.update(i for i, interval in enumerate(estimate[0])
                              if inside(interval[0]) and i not in matched_outside)
    return subset(reference, inside_refs), subset(estimate, sorted(selected_estimates))


def error_details(reference, estimate):
    """List unmatched notes and onset-matched notes with offset errors.

    Octave and duplicate flags are proximity hints, not inferred causes.
    """
    ref_intervals, ref_pitches = reference
    est_intervals, est_pitches = estimate
    matches = mir_eval.transcription.match_notes(
        ref_intervals, ref_pitches, est_intervals, est_pitches,
        onset_tolerance=0.05, pitch_tolerance=50, offset_ratio=None)
    matched_ref = {ref for ref, _ in matches}
    matched_est = {est for _, est in matches}

    def note(interval, frequency):
        return {'start': float(interval[0]), 'end': float(interval[1]),
                'pitch': int(round(pretty_midi.hz_to_note_number(frequency)))}

    missed = [note(interval, pitch) for index, (interval, pitch)
              in enumerate(zip(ref_intervals, ref_pitches)) if index not in matched_ref]
    extra = []
    for index, (interval, pitch) in enumerate(zip(est_intervals, est_pitches)):
        if index in matched_est:
            continue
        item = note(interval, pitch)
        item['near_reference_octave'] = any(
            abs(interval[0] - ref_interval[0]) <= 0.05
            and abs(abs(12 * math.log2(pitch / ref_pitch)) - 12) <= 0.5
            for ref_interval, ref_pitch in zip(ref_intervals, ref_pitches))
        item['near_matched_same_pitch'] = any(
            abs(interval[0] - est_intervals[other][0]) <= 0.1
            and abs(12 * math.log2(pitch / est_pitches[other])) <= 0.5
            for other in matched_est)
        extra.append(item)
    offsets = []
    for ref_index, est_index in matches:
        ref_interval = ref_intervals[ref_index]
        difference = float(est_intervals[est_index][1] - ref_interval[1])
        tolerance = max(0.05, 0.2 * (ref_interval[1] - ref_interval[0]))
        if abs(difference) > tolerance:
            offsets.append({'reference': note(ref_interval, ref_pitches[ref_index]),
                            'estimate': note(est_intervals[est_index], est_pitches[est_index]),
                            'offset_error_seconds': difference})
    return {'missed': missed, 'extra': extra, 'offset_errors': offsets}


def main():
    parser = argparse.ArgumentParser(description=__doc__)
    parser.add_argument('reference', type=Path)
    parser.add_argument('estimate', type=Path)
    parser.add_argument('--estimate-playback-report', action='store_true',
                        help='read estimate as JSON from acorde playback-report')
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
    margin_start = max(0.0, args.start - 0.05)
    margin_end = None if args.end is None else args.end + 0.05
    reference = extract(args.reference, start=margin_start, end=margin_end)
    estimate_reader = extract_playback_report if args.estimate_playback_report else extract
    estimate = estimate_reader(args.estimate, shift=args.estimate_offset,
                               start=margin_start, end=margin_end)
    reference, estimate = window_events(reference, estimate, start=args.start, end=args.end)
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
    output['errors'] = error_details(reference, estimate)
    print(json.dumps(output, indent=2))


if __name__ == '__main__':
    main()
