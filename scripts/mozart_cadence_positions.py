#!/usr/bin/env python3
"""Map development cadence CSV rows to folded MusicXML note-onset positions.

The reserved split is rejected unless --allow-holdout is explicitly supplied
after a candidate and evaluation protocol have been frozen.
"""

import argparse
import csv
import hashlib
import io
import json
import re
import subprocess
import xml.etree.ElementTree as ET
from collections import defaultdict
from fractions import Fraction
from pathlib import Path


STEPS = {"C": 0, "D": 2, "E": 4, "F": 5, "G": 7, "A": 9, "B": 11}
REQUIRED = {"id", "xml_mn", "pitch", "cadence", "cadence_type"}


def child(parent, name):
    return next((item for item in parent if item.tag.rsplit("}", 1)[-1] == name), None)


def value(parent, name):
    item = child(parent, name)
    return item.text.strip() if item is not None and item.text else None


def midi_pitch(note):
    pitch = child(note, "pitch")
    if pitch is None:
        return None
    step, octave = value(pitch, "step"), value(pitch, "octave")
    if step not in STEPS or octave is None:
        return None
    return 12 * (int(octave) + 1) + STEPS[step] + int(value(pitch, "alter") or 0)


def note_index(xml_bytes):
    root = ET.fromstring(xml_bytes)
    notes = {}
    for part_index, part in enumerate(item for item in root if item.tag.rsplit("}", 1)[-1] == "part"):
        divisions = 1
        for measure_index, measure in enumerate(item for item in part if item.tag.rsplit("}", 1)[-1] == "measure"):
            cursor = Fraction(0)
            previous_start = None
            for item in measure:
                tag = item.tag.rsplit("}", 1)[-1]
                if tag == "attributes":
                    new_divisions = value(item, "divisions")
                    if new_divisions is not None:
                        divisions = int(new_divisions)
                        if divisions <= 0:
                            raise ValueError("invalid divisions")
                elif tag in {"backup", "forward"}:
                    duration = Fraction(int(value(item, "duration")), divisions)
                    cursor += -duration if tag == "backup" else duration
                    previous_start = None
                elif tag == "note":
                    chord = child(item, "chord") is not None
                    onset = previous_start if chord else cursor
                    if onset is None:
                        raise ValueError("orphan chord marker in prepared input")
                    if not chord:
                        previous_start = onset
                    note_id = item.attrib.get("id")
                    if note_id:
                        if note_id in notes:
                            raise ValueError(f"duplicate source note ID: {note_id}")
                        notes[note_id] = {
                            "part": part_index, "measure": measure_index,
                            "xml_mn": measure.attrib.get("number"),
                            "offset": onset, "pitch": midi_pitch(item),
                        }
                    if not chord and child(item, "grace") is None:
                        duration = value(item, "duration")
                        if duration is None:
                            raise ValueError("missing note duration")
                        cursor += Fraction(int(duration), divisions)
    return notes


def extract(xml_bytes, csv_text):
    notes = note_index(xml_bytes)
    reader = csv.DictReader(io.StringIO(csv_text))
    if not reader.fieldnames or not REQUIRED.issubset(reader.fieldnames):
        raise ValueError("cadence CSV fields missing")
    grouped = defaultdict(list)
    exclusions = []
    source_rows = 0
    for row in reader:
        source_rows += 1
        match = re.fullmatch(r"(.+)-[0-9]+", row["id"])
        base = match.group(1) if match else None
        note = notes.get(base)
        if note is None or note["xml_mn"] != row["xml_mn"] or note["pitch"] != int(row["pitch"]):
            exclusions.append({"id": row["id"], "reason": "missing_id_or_note_mismatch"})
            continue
        grouped[base].append(row["cadence"].strip())
    located = defaultdict(list)
    for note_id, labels in grouped.items():
        distinct = set(labels)
        if len(distinct) != 1:
            exclusions.append({"id": note_id, "reason": "conflicting_repeat_labels"})
            continue
        note = notes[note_id]
        key = (note["part"], note["measure"], note["offset"])
        located[key].append((note_id, labels[0]))
    positions = []
    for (part, measure, offset), events in sorted(located.items()):
        positive = {label for _, label in events if label}
        if len(positive) > 1:
            exclusions.append({"id": ",".join(note_id for note_id, _ in events),
                               "reason": "conflicting_position_labels"})
            continue
        positions.append({
            "part": part, "measure": measure,
            "offset_quarter": [offset.numerator, offset.denominator],
            "cadence": next(iter(positive), None),
            "source_note_ids": [note_id for note_id, _ in events],
        })
    return {
        "schema_version": 1,
        "source_rows": source_rows,
        "source_note_ids": len(notes),
        "mapped_note_ids": len(grouped),
        "eligible_positions": len(positions),
        "positive_positions": sum(item["cadence"] is not None for item in positions),
        "exclusions": exclusions,
        "positions": positions,
    }


def main():
    parser = argparse.ArgumentParser(description=__doc__)
    parser.add_argument("selection", type=Path)
    parser.add_argument("inputs", type=Path)
    parser.add_argument("input_dir", type=Path)
    parser.add_argument("case_id")
    parser.add_argument("cadence_csv", type=Path)
    parser.add_argument("output", type=Path)
    parser.add_argument("--allow-holdout", action="store_true")
    args = parser.parse_args()
    selection = json.loads(args.selection.read_text())
    inputs = json.loads(args.inputs.read_text())
    if inputs["selection_sha256"] != hashlib.sha256(args.selection.read_bytes()).hexdigest():
        parser.error("selection/input provenance mismatch")
    cases = [case for case in selection["cases"] if case["id"] == args.case_id]
    prepared = [item for item in inputs["inputs"] if item["id"] == args.case_id]
    if len(cases) != 1 or len(prepared) != 1:
        parser.error("case must appear exactly once")
    case, item = cases[0], prepared[0]
    if case["split"] == "holdout" and not args.allow_holdout:
        parser.error("holdout annotations require --allow-holdout after protocol freeze")
    if item["path"] != case["score_path"] or item["split"] != case["split"]:
        parser.error("prepared input mismatch")
    xml_path = args.input_dir / item["path"]
    xml_bytes = xml_path.read_bytes()
    if hashlib.sha256(xml_bytes).hexdigest() != item["input_sha256"]:
        parser.error("prepared input SHA-256 mismatch")
    csv_bytes = args.cadence_csv.read_bytes()
    if subprocess.check_output(["git", "hash-object", str(args.cadence_csv)], text=True).strip() != case["cadence_git_blob"]:
        parser.error("cadence source Git blob mismatch")
    result = extract(xml_bytes, csv_bytes.decode("utf-8-sig"))
    result.update({"case_id": args.case_id, "split": case["split"],
                   "score_input_sha256": item["input_sha256"],
                   "cadence_sha256": hashlib.sha256(csv_bytes).hexdigest()})
    args.output.write_text(json.dumps(result, ensure_ascii=False, indent=2) + "\n")
    print(json.dumps({key: result[key] for key in ("source_rows", "eligible_positions",
                                                 "positive_positions", "exclusions")}))


if __name__ == "__main__":
    main()
