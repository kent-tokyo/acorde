#!/usr/bin/env python3
"""Align a phone-recorded audio excerpt and MIDI using their first nine seconds.

This research helper is specific to the Sight to Sound MIDI test recordings.
Evaluate on a later, separate portion of the recording.
"""

import argparse
from pathlib import Path

import librosa
import numpy as np
import pretty_midi


def align(audio_path, reference_path, output_path):
    audio, sample_rate = librosa.load(audio_path, sr=16000, mono=True)
    flux = librosa.onset.onset_strength(
        y=audio, sr=sample_rate, hop_length=80, n_fft=512, center=True)
    midi = pretty_midi.PrettyMIDI(str(reference_path))
    starts = np.array([note.start for inst in midi.instruments for note in inst.notes
                       if 1 <= note.start < 9])
    if not len(starts):
        raise ValueError('no reference onsets in alignment interval')
    shifts = np.arange(-0.8, 0.201, 0.005)
    scores = []
    for shift in shifts:
        frames = np.rint((starts + shift) * sample_rate / 80).astype(int)
        scores.append(np.mean(flux[np.clip(frames, 0, len(flux) - 1)]))
    shift = float(shifts[np.argmax(scores)])
    for inst in midi.instruments:
        for note in inst.notes:
            note.start = max(0.0, note.start + shift)
            note.end = max(note.start + 0.0001, note.end + shift)
        for event in inst.control_changes:
            event.time = max(0.0, event.time + shift)
        for event in inst.pitch_bends:
            event.time = max(0.0, event.time + shift)
    midi.write(str(output_path))
    return shift, len(starts)


def main():
    parser = argparse.ArgumentParser(description=__doc__)
    parser.add_argument('audio', type=Path)
    parser.add_argument('reference', type=Path)
    parser.add_argument('output', type=Path)
    args = parser.parse_args()
    shift, count = align(args.audio, args.reference, args.output)
    print(f'shift={shift:.3f} seconds; calibration onsets={count}')


if __name__ == '__main__':
    main()
