#!/usr/bin/env python3
"""Check answer-free local-key holdout inputs and both pitched timelines."""

import argparse
import json
import subprocess
from pathlib import Path

import music21

from compare_wir_harmony_music21 import (acorde_events, music21_events,
                                         music21_measures, score_has_harmony,
                                         timeline_equal)
from diagnose_wir_local_keys import digest


def check_case(entry, root, binary):
    path = root / entry["path"]
    if digest(path) != entry["input_sha256"]:
        raise ValueError(f"prepared score hash mismatch: {entry['work']}")
    if score_has_harmony(path):
        raise ValueError(f"embedded harmony answer remains: {entry['work']}")
    parsed = json.loads(subprocess.check_output([str(binary), "report", str(path)]))["score"]
    source = music21.converter.parse(str(path), forceSource=True)
    m_measures = music21_measures(source, music21.stream)
    a_measures = [staff["measures"] for part in parsed["parts"] for staff in part["staves"]]
    counts = {len(measures) for measures in (*m_measures, *a_measures)}
    if len(counts) != 1:
        raise ValueError(f"different physical measure counts among staves: {sorted(counts)}")
    measure_count = counts.pop()
    unequal = []
    for index in range(measure_count):
        if not timeline_equal(music21_events(m_measures, index), acorde_events(parsed, index)):
            unequal.append(index)
    return {"work": entry["work"], "input_sha256": entry["input_sha256"],
            "source_sha256": entry["source_sha256"],
            "removed_harmony_elements": entry["removed_harmony_elements"],
            "music21_parts": len(m_measures), "acorde_staves": len(a_measures),
            "physical_measures": measure_count,
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
        parser.error(f"requires music21 10.5.2; found {music21.__version__}")
    selection = json.loads(args.selection.read_text())
    inputs = json.loads(args.inputs.read_text())
    if inputs["selection_sha256"] != digest(args.selection):
        parser.error("selection hash mismatch")
    if selection["commit"] != inputs["source_commit"]:
        parser.error("source commit mismatch")
    if len(selection["works"]) != len(inputs["inputs"]):
        parser.error("selection/input count mismatch")
    binary = args.acorde.resolve()
    cases = []
    for work, entry in zip(selection["works"], inputs["inputs"]):
        if work["work"] != entry["work"] or work["score_path"] != entry["path"]:
            parser.error("selection/input work mismatch")
        try:
            cases.append(check_case(entry, args.input_dir, binary))
        except (ValueError, subprocess.CalledProcessError,
                music21.exceptions21.Music21Exception) as error:
            cases.append({"work": entry["work"], "input_sha256": entry["input_sha256"],
                          "error": f"{type(error).__name__}: {error}"})
    result = {"schema_version": 1, "classification": "input-only, no gold content read",
              "source_commit": selection["commit"],
              "selection_sha256": digest(args.selection),
              "inputs_sha256": digest(args.inputs),
              "acorde_binary_sha256": digest(binary),
              "music21_version": music21.__version__,
              "summary": {"cases": len(cases),
                          "parsed_both": sum("error" not in case for case in cases),
                          "physical_measures": sum(case.get("physical_measures", 0) for case in cases),
                          "unequal_timeline_measures": sum(len(case.get("unequal_timeline_measures", []))
                                                           for case in cases),
                          "removed_harmony_elements": sum(case.get("removed_harmony_elements", 0)
                                                          for case in cases)},
              "cases": cases}
    args.output.write_text(json.dumps(result, ensure_ascii=False, indent=2) + "\n")
    print(result["summary"])


if __name__ == "__main__":
    main()
