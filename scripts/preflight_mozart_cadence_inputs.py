#!/usr/bin/env python3
"""Check answer-free Mozart MusicXML parse coverage without reading cadence gold."""

import argparse
import hashlib
import json
import subprocess
import xml.etree.ElementTree as ET
from pathlib import Path

from compare_wir_harmony_music21 import acorde_events, music21_events, music21_measures


FORBIDDEN = {"harmony", "direction", "lyric", "credit"}


def sha256(path):
    return hashlib.sha256(path.read_bytes()).hexdigest()


def main():
    parser = argparse.ArgumentParser(description=__doc__)
    parser.add_argument("selection", type=Path)
    parser.add_argument("inputs", type=Path)
    parser.add_argument("input_dir", type=Path)
    parser.add_argument("acorde", type=Path)
    parser.add_argument("output", type=Path)
    args = parser.parse_args()
    from music21 import converter, stream
    import music21

    if music21.__version__ != "10.5.2":
        parser.error(f"requires music21 10.5.2, found {music21.__version__}")
    selection = json.loads(args.selection.read_text())
    prepared = json.loads(args.inputs.read_text())
    if (prepared["source_commit"] != selection["commit"]
            or prepared["selection_sha256"] != sha256(args.selection)):
        parser.error("selection/input provenance mismatch")
    by_id = {row["id"]: row for row in prepared["inputs"]}
    if len(by_id) != len(prepared["inputs"]) or set(by_id) != {case["id"] for case in selection["cases"]}:
        parser.error("selected input set mismatch")
    rows = []
    for case in selection["cases"]:
        item = by_id[case["id"]]
        if (item["path"] != case["score_path"]
                or item["source_git_blob"] != case["score_git_blob"]
                or item["split"] != case["split"]):
            parser.error(f"input provenance mismatch: {case['id']}")
        score = args.input_dir / item["path"]
        if sha256(score) != item["input_sha256"]:
            parser.error(f"input SHA-256 mismatch: {case['id']}")
        if any(element.tag.rsplit("}", 1)[-1] in FORBIDDEN
               for _, element in ET.iterparse(score, events=("start",))):
            parser.error(f"answer-bearing element survived: {case['id']}")
        acorde = subprocess.run([str(args.acorde.resolve()), "report", str(score)],
                                stdout=subprocess.PIPE, stderr=subprocess.PIPE, text=True)
        acorde_score = json.loads(acorde.stdout)["score"] if acorde.returncode == 0 else None
        try:
            music21_score = converter.parse(str(score), forceSource=True)
            music21_error = None
        except Exception as error:
            music21_score = None
            music21_error = f"{type(error).__name__}: {error}"
        unequal = []
        total_measures = None
        if acorde_score is not None and music21_score is not None:
            m_measures = music21_measures(music21_score, stream)
            a_measures = [staff["measures"] for part in acorde_score["parts"]
                          for staff in part["staves"]]
            total_measures = max((len(part) for part in m_measures), default=0)
            total_measures = max(total_measures, *(len(staff) for staff in a_measures))
            for index in range(total_measures):
                m_events = music21_events(m_measures, index)
                a_events = acorde_events(acorde_score, index)
                if (m_events is None or a_events is None
                        or sorted((beat, duration, pitches) for beat, duration, pitches, _ in m_events)
                        != sorted(a_events)):
                    unequal.append(index)
        rows.append({"id": case["id"], "split": case["split"],
                     "input_sha256": item["input_sha256"],
                     "acorde_parsed": acorde.returncode == 0,
                     "acorde_error": acorde.stderr.strip() if acorde.returncode else None,
                     "music21_parsed": music21_error is None,
                     "music21_error": music21_error,
                     "total_measures": total_measures,
                     "unequal_timeline_measures": unequal})
    result = {
        "schema_version": 1,
        "classification": "input-only preflight; no cadence annotations read",
        "source_commit": selection["commit"],
        "selection_sha256": sha256(args.selection),
        "prepared_inputs_sha256": sha256(args.inputs),
        "acorde_binary_sha256": sha256(args.acorde),
        "music21_version": music21.__version__,
        "summary": {"cases": len(rows),
                    "acorde_parsed": sum(row["acorde_parsed"] for row in rows),
                    "music21_parsed": sum(row["music21_parsed"] for row in rows),
                    "equal_timeline_cases": sum(row["total_measures"] is not None
                                                and not row["unequal_timeline_measures"] for row in rows),
                    "compared_measures": sum(row["total_measures"] or 0 for row in rows),
                    "unequal_timeline_measures": sum(len(row["unequal_timeline_measures"])
                                                     for row in rows)},
        "cases": rows,
    }
    args.output.write_text(json.dumps(result, ensure_ascii=False, indent=2) + "\n")
    print(json.dumps(result["summary"]))


if __name__ == "__main__":
    main()
