#!/usr/bin/env python3
"""Compare pitch and beat onsets with a narrow, independent MusicXML source oracle.

The oracle reads MusicXML duration, backup, forward, chord, and tie elements directly.
It rejects unsupported note forms and navigation instead of guessing their timing.
Requires the optional `mido` package for the external MIDI input.
"""

from __future__ import annotations

import argparse
import hashlib
import json
import xml.etree.ElementTree as ET
import zipfile
from collections import Counter, defaultdict
from pathlib import Path

import mido

from compare_playback_onsets import acorde_events, midi_events

STEP = {"C": 0, "D": 2, "E": 4, "F": 5, "G": 7, "A": 9, "B": 11}


def source_attacks(path: Path) -> tuple[Counter[tuple[float, int]], list[float]]:
    with zipfile.ZipFile(path) as archive:
        container = ET.fromstring(archive.read("META-INF/container.xml"))
        rootfile = container.find(".//{*}rootfile")
        if rootfile is None:
            raise ValueError("MXL has no rootfile")
        root = ET.fromstring(archive.read(rootfile.attrib["full-path"]))
    if root.tag != "score-partwise":
        raise ValueError("only unnamespaced score-partwise MusicXML is supported")
    parts = [part.findall("measure") for part in root.findall("part")]
    if not parts or len({len(measures) for measures in parts}) != 1:
        raise ValueError("parts must have the same physical measure count")

    attacks: Counter[tuple[float, int]] = Counter()
    lengths: list[float] = []
    divisions = [1] * len(parts)
    score_beat = 0.0
    for index in range(len(parts[0])):
        measure_lengths = []
        measure_attacks = []
        for part_index, measures in enumerate(parts):
            measure = measures[index]
            if measure.find(".//repeat") is not None or measure.find(".//transpose") is not None:
                raise ValueError("navigation and transposition are outside this oracle")
            cursor = 0.0
            last_note_start = 0.0
            max_cursor = 0.0
            for item in measure:
                if item.tag == "attributes" and item.findtext("divisions"):
                    divisions[part_index] = int(item.findtext("divisions"))
                    if divisions[part_index] <= 0:
                        raise ValueError("invalid divisions")
                elif item.tag in {"backup", "forward"}:
                    duration = int(item.findtext("duration")) / divisions[part_index]
                    cursor += duration if item.tag == "forward" else -duration
                    if cursor < 0:
                        raise ValueError("backup before measure start")
                    max_cursor = max(max_cursor, cursor)
                elif item.tag == "note":
                    if item.find("grace") is not None or item.find("cue") is not None:
                        raise ValueError("grace and cue notes are outside this oracle")
                    duration = int(item.findtext("duration")) / divisions[part_index]
                    chord = item.find("chord") is not None
                    if not chord:
                        last_note_start = cursor
                    pitch = item.find("pitch")
                    if pitch is not None:
                        alter = float(pitch.findtext("alter") or 0)
                        if not alter.is_integer():
                            raise ValueError("microtonal pitch is outside this MIDI oracle")
                        midi = 12 * (int(pitch.findtext("octave")) + 1)
                        midi += STEP[pitch.findtext("step")] + int(alter)
                        if not any(tie.attrib.get("type") == "stop" for tie in item.findall("tie")):
                            measure_attacks.append((last_note_start, midi))
                    elif item.find("rest") is None:
                        raise ValueError("unpitched note is outside this oracle")
                    if not chord:
                        cursor += duration
                        max_cursor = max(max_cursor, cursor)
            measure_lengths.append(max_cursor)
        length = max(measure_lengths)
        if length <= 0:
            raise ValueError("empty or invalid physical measure")
        lengths.append(length)
        attacks.update((round(score_beat + beat, 6), pitch) for beat, pitch in measure_attacks)
        score_beat += length
    return attacks, lengths


def fingerprint(attacks: Counter[tuple[float, int]]) -> str:
    lines = "".join(
        f"{round(beat * 1_000_000)}:{pitch}\n"
        for (beat, pitch), count in sorted(attacks.items())
        for _ in range(count)
    )
    return hashlib.sha256(lines.encode()).hexdigest()


def match_onsets(
    source: Counter[tuple[float, int]],
    actual: Counter[tuple[float, int]],
    tolerance_beats: float,
) -> tuple[int, float]:
    """Match each pitch in time order, allowing MIDI's tick rounding on the beat axis."""
    def by_pitch(attacks: Counter[tuple[float, int]]) -> dict[int, list[float]]:
        grouped: defaultdict[int, list[float]] = defaultdict(list)
        for (beat, pitch), count in attacks.items():
            grouped[pitch].extend([beat] * count)
        return {pitch: sorted(beats) for pitch, beats in grouped.items()}

    expected = by_pitch(source)
    observed = by_pitch(actual)
    matches = 0
    max_difference = 0.0
    for pitch in expected.keys() | observed.keys():
        reference = expected.get(pitch, [])
        produced = observed.get(pitch, [])
        i = j = 0
        while i < len(reference) and j < len(produced):
            difference = reference[i] - produced[j]
            if abs(difference) <= tolerance_beats:
                matches += 1
                max_difference = max(max_difference, abs(difference))
                i += 1
                j += 1
            elif difference < 0:
                i += 1
            else:
                j += 1
    return matches, max_difference


def main() -> None:
    parser = argparse.ArgumentParser(description=__doc__)
    parser.add_argument("source_mxl", type=Path)
    parser.add_argument("acorde_json", type=Path)
    parser.add_argument("external_midi", type=Path)
    args = parser.parse_args()
    source, lengths = source_attacks(args.source_mxl)
    acorde = Counter({key: len(values) for key, values in acorde_events(args.acorde_json).items()})
    external = Counter({key: len(values) for key, values in midi_events(args.external_midi).items()})
    ticks_per_beat = mido.MidiFile(args.external_midi).ticks_per_beat
    if ticks_per_beat <= 0:
        raise ValueError("invalid MIDI ticks per beat")
    tolerance_beats = 2 / ticks_per_beat
    acorde_matches, acorde_max_difference = match_onsets(source, acorde, tolerance_beats)
    external_matches, external_max_difference = match_onsets(source, external, tolerance_beats)
    print(json.dumps({
        "source_attacks": source.total(),
        "source_onset_fingerprint_sha256": fingerprint(source),
        "physical_measure_lengths_beats": lengths,
        "onset_tolerance_beats": tolerance_beats,
        "acorde_source_matches": acorde_matches,
        "acorde_only": acorde.total() - acorde_matches,
        "acorde_max_matched_difference_beats": acorde_max_difference,
        "alphatab_source_matches": external_matches,
        "alphatab_only": external.total() - external_matches,
        "alphatab_max_matched_difference_beats": external_max_difference,
    }, indent=2))


if __name__ == "__main__":
    main()
