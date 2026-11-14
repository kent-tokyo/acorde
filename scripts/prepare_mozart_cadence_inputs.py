#!/usr/bin/env python3
"""Remove possible analytical answers from selected Mozart MusicXML inputs.

This reads scores only, never cadence CSVs. Both systems must receive the same
prepared bytes. Harmony, directions, lyrics, and credits are conservatively
removed; note, rest, voice, measure, key, time, and repeat data are retained.
"""

import argparse
import hashlib
import json
import subprocess
import xml.etree.ElementTree as ET
from collections import Counter
from pathlib import Path


STRIP = frozenset(("harmony", "direction", "lyric", "credit"))


def digest(data):
    return hashlib.sha256(data).hexdigest()


def local_name(tag):
    return tag.rsplit("}", 1)[-1]


def prepare(data):
    root = ET.fromstring(data)
    if local_name(root.tag) not in {"score-partwise", "score-timewise"}:
        raise ValueError("expected a MusicXML score")
    removed = Counter()
    repaired = Counter()
    for parent in root.iter():
        for child in list(parent):
            tag = local_name(child.tag)
            if tag in STRIP:
                parent.remove(child)
                removed[tag] += 1
    for element in root.iter():
        if local_name(element.tag) == "type" and (element.text or "").strip() == "eigth":
            element.text = "eighth"
            repaired["eigth_to_eighth"] += 1
        if local_name(element.tag) != "measure":
            continue
        seen_note = False
        for child in element:
            tag = local_name(child.tag)
            if tag in {"backup", "forward"}:
                seen_note = False
            elif tag == "note":
                chord = next((item for item in child if local_name(item.tag) == "chord"), None)
                if chord is not None and not seen_note:
                    child.remove(chord)
                    repaired["orphan_chord_removed"] += 1
                    seen_note = True
                if chord is None:
                    seen_note = True
    output = ET.tostring(root, encoding="utf-8", xml_declaration=True)
    return output, dict(sorted(removed.items())), dict(sorted(repaired.items()))


def source_blob(path):
    return subprocess.check_output(["git", "hash-object", str(path)], text=True).strip()


def prepare_selection(selection, selection_bytes, source_dir, output_dir):
    if selection.get("schema_version") != 1:
        raise ValueError("selection schema mismatch")
    rows = []
    seen = set()
    for case in selection["cases"]:
        name, relative = case["id"], Path(case["score_path"])
        if name in seen or relative.is_absolute() or ".." in relative.parts:
            raise ValueError("duplicate or unsafe score path")
        seen.add(name)
        source = source_dir / relative
        source_bytes = source.read_bytes()
        if source_blob(source) != case["score_git_blob"]:
            raise ValueError(f"source Git blob mismatch: {name}")
        clean, removed, repaired = prepare(source_bytes)
        destination = output_dir / relative
        destination.parent.mkdir(parents=True, exist_ok=True)
        destination.write_bytes(clean)
        rows.append({
            "id": name, "split": case["split"], "path": relative.as_posix(),
            "source_git_blob": case["score_git_blob"],
            "source_sha256": digest(source_bytes), "input_sha256": digest(clean),
            "removed_elements": removed, "repaired_elements": repaired,
        })
    return {
        "schema_version": 1,
        "source_commit": selection["commit"],
        "selection_sha256": digest(selection_bytes),
        "input_rule": "Strip harmony, direction, lyric, and credit elements. Correct an eigth note-type typo to eighth and remove a chord marker without a preceding note since the last backup/forward. Retain all notes, pitches, durations, and rhythmic structure. Give identical prepared bytes to both analyzers.",
        "inputs": rows,
    }


def main():
    parser = argparse.ArgumentParser(description=__doc__)
    parser.add_argument("selection", type=Path)
    parser.add_argument("source_dir", type=Path)
    parser.add_argument("output_dir", type=Path)
    parser.add_argument("record", type=Path)
    args = parser.parse_args()
    selected_bytes = args.selection.read_bytes()
    result = prepare_selection(json.loads(selected_bytes), selected_bytes,
                               args.source_dir, args.output_dir)
    args.record.write_text(json.dumps(result, ensure_ascii=False, indent=2) + "\n")
    print(f"prepared {len(result['inputs'])} MusicXML inputs")


if __name__ == "__main__":
    main()
