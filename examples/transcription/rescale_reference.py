#!/usr/bin/env python3
"""Rescale MIDI tick resolution by rounding absolute event positions."""

import argparse
from pathlib import Path

import mido


def rescale(source_path, output_path, ticks_per_beat=960):
    midi = mido.MidiFile(str(source_path))
    if midi.ticks_per_beat <= 0 or not 1 <= ticks_per_beat <= 32767:
        raise ValueError('positive PPQ tick resolutions are required')
    result = mido.MidiFile(type=midi.type, ticks_per_beat=ticks_per_beat)
    for track in midi.tracks:
        scaled = mido.MidiTrack()
        source_tick = previous_tick = 0
        for message in track:
            source_tick += message.time
            tick = round(source_tick * ticks_per_beat / midi.ticks_per_beat)
            scaled.append(message.copy(time=tick - previous_tick))
            previous_tick = tick
        result.tracks.append(scaled)
    result.save(str(output_path))


def main():
    parser = argparse.ArgumentParser(description=__doc__)
    parser.add_argument('source', type=Path)
    parser.add_argument('output', type=Path)
    parser.add_argument('--ticks-per-beat', type=int, default=960)
    args = parser.parse_args()
    rescale(args.source, args.output, args.ticks_per_beat)


if __name__ == '__main__':
    main()
