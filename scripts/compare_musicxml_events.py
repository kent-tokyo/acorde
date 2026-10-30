#!/usr/bin/env python3
"""Compare a bounded MusicXML note/rest event projection from two scores.

This independent XML projection ignores voice numbers and note order.
It checks part/measure/staff, rational onset and duration, written pitch, and lyric text.
It does not check ties, directions, articulations, engraving, or whole-score semantics.
"""

from __future__ import annotations

import argparse
import json
import xml.etree.ElementTree as ET
from collections import Counter
from fractions import Fraction
from pathlib import Path
from zipfile import ZipFile


def local_name(tag: str) -> str:
    return tag.rsplit("}", 1)[-1]


def child(node: ET.Element, name: str) -> ET.Element | None:
    return next((item for item in node if local_name(item.tag) == name), None)


def children(node: ET.Element, name: str) -> list[ET.Element]:
    return [item for item in node if local_name(item.tag) == name]


def value(node: ET.Element, name: str, default: str = "") -> str:
    found = child(node, name)
    return found.text if found is not None and found.text is not None else default


def read_score(path: Path) -> ET.Element:
    if path.suffix.lower() == ".mxl":
        with ZipFile(path) as archive:
            container = ET.fromstring(archive.read("META-INF/container.xml"))
            rootfile = next(
                (node.get("full-path") for node in container.iter() if local_name(node.tag) == "rootfile"),
                None,
            )
            if not rootfile:
                raise ValueError(f"MXL has no rootfile: {path}")
            return ET.fromstring(archive.read(rootfile))
    return ET.parse(path).getroot()


def event_projection(path: Path) -> Counter[tuple]:
    score = read_score(path)
    if local_name(score.tag) != "score-partwise":
        raise ValueError(f"expected score-partwise MusicXML: {path}")
    events: Counter[tuple] = Counter()
    for part_index, part in enumerate(children(score, "part")):
        divisions = 1
        for measure_index, measure in enumerate(children(part, "measure")):
            cursor = Fraction(0)
            previous_onset = Fraction(0)
            for item in measure:
                kind = local_name(item.tag)
                if kind == "attributes":
                    raw_divisions = value(item, "divisions")
                    if raw_divisions:
                        divisions = int(raw_divisions)
                        if divisions <= 0:
                            raise ValueError(f"invalid divisions in {path}")
                elif kind == "backup":
                    cursor -= Fraction(int(value(item, "duration", "0")), divisions)
                elif kind == "forward":
                    cursor += Fraction(int(value(item, "duration", "0")), divisions)
                elif kind == "note":
                    chord = child(item, "chord") is not None
                    onset = previous_onset if chord else cursor
                    previous_onset = onset
                    duration = Fraction(int(value(item, "duration", "0")), divisions)
                    pitch_node = child(item, "pitch")
                    if pitch_node is not None:
                        pitch = (
                            "pitch",
                            value(pitch_node, "step"),
                            value(pitch_node, "alter", "0"),
                            value(pitch_node, "octave"),
                        )
                    elif child(item, "rest") is not None:
                        pitch = ("rest",)
                    else:
                        pitch = ("unpitched",)
                    lyrics = tuple(
                        "".join(text.text or "" for text in children(lyric, "text"))
                        for lyric in children(item, "lyric")
                    )
                    events[(part_index, measure_index, value(item, "staff", "1"), onset, duration, pitch, lyrics)] += 1
                    if not chord and child(item, "grace") is None:
                        cursor += duration
    return events


def count_elements(path: Path, name: str) -> int:
    return sum(local_name(node.tag) == name for node in read_score(path).iter())


def main() -> int:
    parser = argparse.ArgumentParser(description=__doc__)
    parser.add_argument("source", type=Path)
    parser.add_argument("candidate", type=Path)
    args = parser.parse_args()
    source = event_projection(args.source)
    candidate = event_projection(args.candidate)
    print(
        json.dumps(
            {
                "source_events": source.total(),
                "candidate_events": candidate.total(),
                "missing_source_events": (source - candidate).total(),
                "added_candidate_events": (candidate - source).total(),
                "source_wedge_elements": count_elements(args.source, "wedge"),
                "candidate_wedge_elements": count_elements(args.candidate, "wedge"),
            },
            indent=2,
        )
    )
    return 0


if __name__ == "__main__":
    raise SystemExit(main())
