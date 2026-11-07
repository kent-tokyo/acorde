#!/usr/bin/env python3
"""Compare note attacks and conservative written ends with a MusicXML source oracle.

The oracle reads MusicXML duration, backup, forward, chord, tie, and silent cue
elements directly. It rejects unsupported note forms and navigation instead
of guessing their timing. Slurs, articulation, pedal and ambiguous equal-pitch
overlaps are reported but excluded from the strict note-end metric. Pedal
caveats follow the written active interval, and chord-wide release markings
apply to every sounding member of the chord.
Requires the optional `mido` package for the external MIDI input.
"""

from __future__ import annotations

import argparse
import hashlib
import json
import math
import xml.etree.ElementTree as ET
import zipfile
from collections import Counter, defaultdict
from dataclasses import dataclass
from pathlib import Path

import mido

from compare_playback_onsets import acorde_events, midi_events

STEP = {"C": 0, "D": 2, "E": 4, "F": 5, "G": 7, "A": 9, "B": 11}


def load_score(path: Path) -> ET.Element:
    with zipfile.ZipFile(path) as archive:
        container = ET.fromstring(archive.read("META-INF/container.xml"))
        rootfile = container.find(".//{*}rootfile")
        if rootfile is None:
            raise ValueError("MXL has no rootfile")
        root = ET.fromstring(archive.read(rootfile.attrib["full-path"]))
    if root.tag != "score-partwise":
        raise ValueError("only unnamespaced score-partwise MusicXML is supported")
    return root


def source_attacks(path: Path) -> tuple[Counter[tuple[float, int]], list[float]]:
    root = load_score(path)
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
                    if item.find("grace") is not None:
                        raise ValueError("grace notes are outside this oracle")
                    cue = item.find("cue") is not None
                    duration = int(item.findtext("duration")) / divisions[part_index]
                    chord = item.find("chord") is not None
                    if not chord:
                        last_note_start = cursor
                    pitch = item.find("pitch")
                    if pitch is not None and not cue:
                        alter = float(pitch.findtext("alter") or 0)
                        if not alter.is_integer():
                            raise ValueError("microtonal pitch is outside this MIDI oracle")
                        midi = 12 * (int(pitch.findtext("octave")) + 1)
                        midi += STEP[pitch.findtext("step")] + int(alter)
                        if not any(tie.attrib.get("type") == "stop" for tie in item.findall("tie")):
                            measure_attacks.append((last_note_start, midi))
                    elif pitch is None and item.find("rest") is None:
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


@dataclass
class SourceSpan:
    start: float
    end: float
    pitch: int
    release_caveats: set[str]
    part: int


def source_spans(path: Path) -> list[SourceSpan]:
    """Read written note ends; join sound ties across physical measures.

    Articulation, slur and pedal marks can change a performed note-off. Their
    spans remain visible in the report but are excluded from the strict
    written-duration count.
    """
    root = load_score(path)
    parts = [part.findall("measure") for part in root.findall("part")]
    if not parts or len({len(measures) for measures in parts}) != 1:
        raise ValueError("parts must have the same physical measure count")
    pedal_events: list[list[tuple[float, int, str, str]]] = [[] for _ in parts]
    divisions = [1] * len(parts)
    open_ties: dict[tuple[int, str, str, int], SourceSpan] = {}
    active_slurs: set[tuple[int, str, str, str]] = set()
    spans: list[SourceSpan] = []
    score_beat = 0.0
    for index in range(len(parts[0])):
        measure_lengths = []
        for part_index, measures in enumerate(parts):
            measure = measures[index]
            if measure.find(".//repeat") is not None or measure.find(".//transpose") is not None:
                raise ValueError("navigation and transposition are outside this oracle")
            cursor = last_note_start = max_cursor = 0.0
            chord_spans: list[SourceSpan] = []
            chord_release_reasons: set[str] = set()
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
                elif item.tag == "direction":
                    pedals = item.findall("./direction-type/pedal")
                    if not pedals:
                        continue
                    offset = float(item.findtext("offset") or 0) / divisions[part_index]
                    if not math.isfinite(offset):
                        raise ValueError("invalid pedal direction offset")
                    for pedal in pedals:
                        kind = pedal.attrib.get("type", "")
                        if kind not in {
                            "start", "stop", "change", "continue", "resume", "discontinue"
                        }:
                            raise ValueError(f"unsupported pedal type: {kind}")
                        events = pedal_events[part_index]
                        events.append((
                            round(score_beat + cursor + offset, 6), len(events),
                            kind, pedal.attrib.get("number", "1"),
                        ))
                elif item.tag == "note":
                    if item.find("grace") is not None:
                        raise ValueError("grace notes are outside this oracle")
                    duration = int(item.findtext("duration")) / divisions[part_index]
                    if duration <= 0:
                        raise ValueError("nonpositive note duration")
                    chord = item.find("chord") is not None
                    if not chord:
                        last_note_start = cursor
                        chord_spans = []
                        chord_release_reasons = set()
                    pitch = item.find("pitch")
                    if pitch is not None and item.find("cue") is None:
                        alter = float(pitch.findtext("alter") or 0)
                        if not alter.is_integer():
                            raise ValueError("microtonal pitch is outside this MIDI oracle")
                        midi = 12 * (int(pitch.findtext("octave")) + 1)
                        midi += STEP[pitch.findtext("step")] + int(alter)
                        voice = item.findtext("voice") or "1"
                        staff = item.findtext("staff") or "1"
                        slurs = item.findall(".//notations/slur")
                        slur_keys = [
                            (part_index, voice, staff, mark.attrib.get("number", "1"))
                            for mark in slurs
                        ]
                        reasons = set()
                        if slurs or any(
                            key[:3] == (part_index, voice, staff) for key in active_slurs
                        ):
                            reasons.add("slur")
                        for tag in ("articulations", "fermata", "ornaments"):
                            if item.find(f".//notations/{tag}") is not None:
                                reasons.add(tag)
                        start = round(score_beat + last_note_start, 6)
                        end = round(start + duration, 6)
                        tie_types = {tie.attrib.get("type") for tie in item.findall("tie")}
                        if tie_types - {"start", "stop"}:
                            raise ValueError("unsupported tie type")
                        tie_key = (
                            part_index,
                            voice,
                            staff,
                            midi,
                        )
                        if "stop" in tie_types:
                            if tie_key not in open_ties:
                                # MusicXML may change voice at a measure boundary.
                                # Resolve only an unambiguous same-part/staff/pitch
                                # continuation; never guess between two voices.
                                candidates = [
                                    key for key, previous in open_ties.items()
                                    if key[0] == part_index and key[2:] == tie_key[2:]
                                    and abs(previous.end - start) <= 1e-6
                                ]
                                if len(candidates) != 1:
                                    raise ValueError(f"tie stop without unique start: {tie_key}")
                                tie_key = candidates[0]
                            span = open_ties.pop(tie_key)
                            if abs(span.end - start) > 1e-6:
                                raise ValueError(f"noncontiguous tie: {tie_key}")
                            span.end = end
                            span.release_caveats.update(reasons)
                        else:
                            span = SourceSpan(start, end, midi, reasons, part_index)
                        if "start" in tie_types:
                            if tie_key in open_ties:
                                raise ValueError(f"overlapping tie starts: {tie_key}")
                            open_ties[tie_key] = span
                        else:
                            spans.append(span)
                        # MusicXML often writes a chord articulation only on
                        # its first note; it affects every sounding member.
                        chord_release_reasons.update(reasons)
                        span.release_caveats.update(chord_release_reasons)
                        for member in chord_spans:
                            member.release_caveats.update(reasons)
                        chord_spans.append(span)
                        for mark, key in zip(slurs, slur_keys):
                            if mark.attrib.get("type") == "stop":
                                active_slurs.discard(key)
                            elif mark.attrib.get("type") == "start":
                                active_slurs.add(key)
                    elif pitch is None and item.find("rest") is None:
                        raise ValueError("unpitched note is outside this oracle")
                    if not chord:
                        cursor += duration
                        max_cursor = max(max_cursor, cursor)
            measure_lengths.append(max_cursor)
        length = max(measure_lengths)
        if length <= 0:
            raise ValueError("empty or invalid physical measure")
        score_beat += length
    if open_ties:
        raise ValueError(f"unterminated ties: {sorted(open_ties)}")
    # The pedal can prolong a note-off only near its actual active interval.
    # Directions can follow backup/forward, so resolve them by beat after all
    # parts and measures have supplied their physical lengths.
    pedal_intervals: list[list[tuple[float, float]]] = [[] for _ in parts]
    uncertain_pedal: set[int] = set()
    for part_index, events in enumerate(pedal_events):
        active: dict[str, float] = {}
        for beat, _, kind, number in sorted(events):
            if kind in {"start", "resume"}:
                if number in active:
                    pedal_intervals[part_index].append((active[number], beat))
                active[number] = beat
            elif kind == "change":
                if number in active:
                    pedal_intervals[part_index].append((active[number], beat))
                else:
                    uncertain_pedal.add(part_index)
                active[number] = beat
            elif kind in {"stop", "discontinue"}:
                if number in active:
                    pedal_intervals[part_index].append((active.pop(number), beat))
                else:
                    uncertain_pedal.add(part_index)
            elif kind == "continue" and number not in active:
                uncertain_pedal.add(part_index)
        pedal_intervals[part_index].extend((start, score_beat) for start in active.values())
    for span in spans:
        if span.part in uncertain_pedal:
            span.release_caveats.add("pedal_ambiguous")
        elif any(
            start <= span.end and span.start <= end
            for start, end in pedal_intervals[span.part]
        ):
            span.release_caveats.add("pedal_overlap")
    # The MIDI reader intentionally drops part/channel identity. A note-off
    # cannot be attributed uniquely when equal pitches overlap in the source.
    by_pitch: defaultdict[int, list[SourceSpan]] = defaultdict(list)
    for span in spans:
        by_pitch[span.pitch].append(span)
    for same_pitch in by_pitch.values():
        same_pitch.sort(key=lambda span: span.start)
        for index, earlier in enumerate(same_pitch):
            for later in same_pitch[index + 1:]:
                if later.start >= earlier.end:
                    break
                earlier.release_caveats.add("overlapping_same_pitch")
                later.release_caveats.add("overlapping_same_pitch")
    return spans


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


def match_written_durations(
    spans: list[SourceSpan],
    actual: dict[tuple[float, int], list[tuple]],
    tolerance_beats: float,
) -> dict:
    """Pair equal-pitch attacks, then compare unmarked written note ends.

    This reports a score-duration contract. Marked notes are excluded because
    their performed note-off can intentionally differ from the written value.
    """
    by_pitch: defaultdict[int, list[tuple[float, float]]] = defaultdict(list)
    for (start, pitch), events in actual.items():
        by_pitch[pitch].extend((start, start + event[1]) for event in events)
    for events in by_pitch.values():
        events.sort()
    matched = eligible = within = 0
    max_difference = 0.0
    mismatches = []
    for span in sorted(spans, key=lambda item: (item.start, item.pitch, item.end)):
        events = by_pitch[span.pitch]
        candidates = [
            (i, event) for i, event in enumerate(events)
            if abs(event[0] - span.start) <= tolerance_beats
        ]
        if not candidates:
            continue
        # Equal-pitch unisons can share a tick; match the closest note-off.
        index, event = min(
            candidates,
            key=lambda pair: (abs(pair[1][0] - span.start), abs(pair[1][1] - span.end)),
        )
        del events[index]
        matched += 1
        if span.release_caveats:
            continue
        eligible += 1
        difference = abs(span.end - event[1])
        max_difference = max(max_difference, difference)
        if difference <= tolerance_beats:
            within += 1
        else:
            mismatches.append({
                "source_start_beat": span.start,
                "pitch_midi": span.pitch,
                "source_end_beat": span.end,
                "actual_end_beat": round(event[1], 6),
                "difference_beats": round(difference, 6),
            })
    mismatches.sort(key=lambda item: -item["difference_beats"])
    return {
        "matched_source_spans": matched,
        "strict_eligible_matched_spans": eligible,
        "strict_written_duration_matches": within,
        "strict_max_note_end_difference_beats": round(max_difference, 6) if eligible else None,
        "strict_note_end_mismatch_examples": mismatches[:10],
    }


def main() -> None:
    parser = argparse.ArgumentParser(description=__doc__)
    parser.add_argument("source_mxl", type=Path)
    parser.add_argument("acorde_json", type=Path)
    parser.add_argument("external_midi", type=Path)
    args = parser.parse_args()
    source, lengths = source_attacks(args.source_mxl)
    spans = source_spans(args.source_mxl)
    if Counter((span.start, span.pitch) for span in spans) != source:
        raise ValueError("source note spans and source attacks disagree")
    acorde_schedule = acorde_events(args.acorde_json)
    external_schedule = midi_events(args.external_midi)
    acorde = Counter({key: len(values) for key, values in acorde_schedule.items()})
    external = Counter({key: len(values) for key, values in external_schedule.items()})
    ticks_per_beat = mido.MidiFile(args.external_midi).ticks_per_beat
    if ticks_per_beat <= 0:
        raise ValueError("invalid MIDI ticks per beat")
    tolerance_beats = 2 / ticks_per_beat
    acorde_matches, acorde_max_difference = match_onsets(source, acorde, tolerance_beats)
    external_matches, external_max_difference = match_onsets(source, external, tolerance_beats)
    acorde_durations = match_written_durations(spans, acorde_schedule, tolerance_beats)
    external_durations = match_written_durations(spans, external_schedule, tolerance_beats)
    print(json.dumps({
        "source_sha256": hashlib.sha256(args.source_mxl.read_bytes()).hexdigest(),
        "acorde_schedule_sha256": hashlib.sha256(args.acorde_json.read_bytes()).hexdigest(),
        "external_midi_sha256": hashlib.sha256(args.external_midi.read_bytes()).hexdigest(),
        "source_attacks": source.total(),
        "source_note_spans": len(spans),
        "source_spans_with_release_caveats": sum(bool(span.release_caveats) for span in spans),
        "release_caveat_counts": dict(sorted(Counter(
            reason for span in spans for reason in span.release_caveats
        ).items())),
        "source_onset_fingerprint_sha256": fingerprint(source),
        "physical_measure_lengths_beats": lengths,
        "onset_tolerance_beats": tolerance_beats,
        "acorde_source_matches": acorde_matches,
        "acorde_events": acorde.total(),
        "acorde_only": acorde.total() - acorde_matches,
        "acorde_max_matched_difference_beats": acorde_max_difference,
        "acorde_written_durations": acorde_durations,
        "alphatab_source_matches": external_matches,
        "alphatab_events": external.total(),
        "alphatab_only": external.total() - external_matches,
        "alphatab_max_matched_difference_beats": external_max_difference,
        "alphatab_written_durations": external_durations,
    }, indent=2))


if __name__ == "__main__":
    main()
