#!/usr/bin/env python3
"""Compare Acorde playback JSON with an external MIDI note schedule.

Requires the optional `mido` package. This checks matching pitch and onset, then records
second and duration differences. It does not compare audio waveforms, velocity, or sound quality.
"""

from __future__ import annotations

import argparse
import hashlib
import json
from collections import Counter, defaultdict, deque
from pathlib import Path

import mido


def digest(path: Path) -> str:
    return hashlib.sha256(path.read_bytes()).hexdigest()


def key(beat: float, pitch: int) -> tuple[float, int]:
    # Acorde uses floating-point beats; external MIDI uses integer ticks.
    return (round(beat, 6), pitch)


def acorde_events(path: Path) -> dict[tuple[float, int], list[tuple[float, float, bool]]]:
    data = json.loads(path.read_text(encoding="utf-8"))
    if not isinstance(data, list):
        raise ValueError("Acorde playback-report must be a JSON array")
    events: dict[tuple[float, int], list[tuple[float, float, bool]]] = defaultdict(list)
    for event in data:
        if event.get("is_metronome"):
            continue
        events[key(event["time_beats"], event["pitch_midi"])].append(
            (
                event["time_secs"],
                event["duration_beats"],
                "Fermata" in event.get("articulations", []),
            )
        )
    return events


def midi_events(path: Path) -> dict[tuple[float, int], list[tuple[float, float]]]:
    midi = mido.MidiFile(path)
    ticks_per_beat = midi.ticks_per_beat
    if ticks_per_beat <= 0:
        raise ValueError("invalid MIDI ticks per beat")
    events: dict[tuple[float, int], list[tuple[float, float]]] = defaultdict(list)
    active: dict[tuple[int, int], deque[tuple[int, float]]] = defaultdict(deque)
    tick = 0
    seconds = 0.0
    tempo = 500_000
    for message in mido.merge_tracks(midi.tracks):
        tick += message.time
        seconds += message.time * tempo / (ticks_per_beat * 1_000_000)
        if message.type == "set_tempo":
            tempo = message.tempo
        elif message.type == "note_on" and message.velocity > 0:
            active[(message.channel, message.note)].append((tick, seconds))
        elif message.type == "note_off" or (
            message.type == "note_on" and message.velocity == 0
        ):
            playing = active[(message.channel, message.note)]
            if not playing:
                raise ValueError("MIDI note-off has no matching note-on")
            start_tick, start_seconds = playing.popleft()
            events[key(start_tick / ticks_per_beat, message.note)].append(
                (start_seconds, (tick - start_tick) / ticks_per_beat)
            )
    if any(active.values()):
        raise ValueError("MIDI has unterminated notes")
    return events


def main() -> int:
    parser = argparse.ArgumentParser(description=__doc__)
    parser.add_argument("acorde_json", type=Path)
    parser.add_argument("external_midi", type=Path)
    args = parser.parse_args()
    acorde = acorde_events(args.acorde_json)
    external = midi_events(args.external_midi)
    acorde_counts = Counter({event: len(values) for event, values in acorde.items()})
    external_counts = Counter({event: len(values) for event, values in external.items()})
    onset_deltas_ms = []
    duration_differences = []
    for event in acorde_counts & external_counts:
        for own, other in zip(sorted(acorde[event]), sorted(external[event])):
            onset_deltas_ms.append(abs(own[0] - other[0]) * 1000)
            if abs(own[1] - other[1]) > 0.01:
                duration_differences.append(own[2])
    print(
        json.dumps(
            {
                "acorde_sha256": digest(args.acorde_json),
                "external_midi_sha256": digest(args.external_midi),
                "acorde_events": acorde_counts.total(),
                "external_note_events": external_counts.total(),
                "matched_onset_pitch": (acorde_counts & external_counts).total(),
                "only_acorde": (acorde_counts - external_counts).total(),
                "only_external": (external_counts - acorde_counts).total(),
                "max_onset_difference_ms": max(onset_deltas_ms, default=None),
                "duration_differences_over_0_01_beats": len(duration_differences),
                "all_duration_differences_on_acorde_fermata_notes": all(duration_differences),
            },
            indent=2,
        )
    )
    return 0


if __name__ == "__main__":
    raise SystemExit(main())
