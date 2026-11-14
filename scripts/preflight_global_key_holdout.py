#!/usr/bin/env python3
"""Check prepared global-key scores without reading any selected key labels."""

import argparse
import hashlib
import json
import subprocess
import xml.etree.ElementTree as ET
from collections import Counter
from pathlib import Path

import music21

from compare_wir_harmony_music21 import (acorde_events, music21_events,
                                         music21_measures, timeline_equal)


def sha256(path):
    return hashlib.sha256(path.read_bytes()).hexdigest()


def check_one(entry, root, binary):
    path = root / entry["input_path"]
    if sha256(path) != entry["input_sha256"]:
        raise ValueError("prepared input SHA-256 differs")
    xml = ET.parse(path).getroot()
    prohibited = {"harmony", "direction", "lyric", "credit"}
    if any(node.tag.rsplit("}", 1)[-1] in prohibited for node in xml.iter()):
        raise ValueError("analyst annotation element remains")
    result = subprocess.run([str(binary), "report", str(path)],
                            capture_output=True, text=True, timeout=120,
                            check=False)
    if result.returncode:
        raise RuntimeError(result.stderr.strip()[-300:] or
                           f"Acorde exit status {result.returncode}")
    report = json.loads(result.stdout)
    parsed = report["score"]
    score = music21.converter.parse(str(path), forceSource=True)
    m_measures = music21_measures(score, music21.stream)
    a_measures = [staff["measures"] for part in parsed["parts"]
                  for staff in part["staves"]]
    lengths = {len(measures) for measures in (*m_measures, *a_measures)}
    if len(lengths) != 1:
        raise ValueError(f"unequal physical measure counts across parts: {sorted(lengths)}")
    measure_count = lengths.pop()
    unequal = []
    for index in range(measure_count):
        if not timeline_equal(music21_events(m_measures, index),
                              acorde_events(parsed, index)):
            unequal.append(index)
    return {"group": entry["group"], "piece": entry["piece"],
            "input_sha256": entry["input_sha256"],
            "music21_parts": len(m_measures), "acorde_staves": len(a_measures),
            "physical_measures": measure_count,
            "acorde_import_diagnostics_by_code": dict(sorted(Counter(
                item["code"] for item in report["diagnostics"]).items())),
            "unequal_timeline_measures": unequal}


def main():
    parser = argparse.ArgumentParser(description=__doc__)
    parser.add_argument("selection", type=Path)
    parser.add_argument("inputs", type=Path)
    parser.add_argument("input_dir", type=Path)
    parser.add_argument("acorde", type=Path)
    parser.add_argument("output", type=Path)
    args = parser.parse_args()
    if music21.__version__ != "10.5.2":
        parser.error(f"requires music21 10.5.2, found {music21.__version__}")
    selected = json.loads(args.selection.read_text())
    inputs = json.loads(args.inputs.read_text())
    if inputs["selection_sha256"] != sha256(args.selection):
        parser.error("selection hash differs")
    if len(selected["works"]) != len(inputs["inputs"]):
        parser.error("selection/input count differs")
    cases = []
    binary = args.acorde.resolve()
    for work, entry in zip(selected["works"], inputs["inputs"]):
        if (work["group"], work["piece"]) != (entry["group"], entry["piece"]):
            parser.error("selection/input order differs")
        if "error" in entry:
            cases.append({"group": entry["group"], "piece": entry["piece"],
                          "error": "input preparation: " + entry["error"]})
            continue
        try:
            cases.append(check_one(entry, args.input_dir, binary))
        except Exception as error:
            cases.append({"group": entry["group"], "piece": entry["piece"],
                          "input_sha256": entry["input_sha256"],
                          "error": f"{type(error).__name__}: {error}"})
        print(f"{entry['group']}/{entry['piece']}: "
              f"{'error' if 'error' in cases[-1] else 'parsed'}", flush=True)
    result = {"schema_version": 1,
              "classification": "score-only global-key preflight; labels unread",
              "selection_sha256": sha256(args.selection),
              "inputs_sha256": sha256(args.inputs),
              "acorde_binary_sha256": sha256(binary),
              "music21_version": music21.__version__,
              "summary": {
                  "cases": len(cases),
                  "parsed_both": sum("error" not in case for case in cases),
                  "physical_measures": sum(case.get("physical_measures", 0)
                                           for case in cases),
                  "unequal_timeline_measures": sum(
                      len(case.get("unequal_timeline_measures", []))
                      for case in cases),
              }, "cases": cases}
    args.output.write_text(json.dumps(result, ensure_ascii=False, indent=2) + "\n")
    print(result["summary"])


if __name__ == "__main__":
    main()
