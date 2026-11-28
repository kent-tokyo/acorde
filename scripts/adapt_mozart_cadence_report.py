#!/usr/bin/env python3
"""Project compare_mozart_cadence_tools.py reports into the common label evaluator (#502).

Gold rows are the eligible positions with an annotated cadence, labeled with
the coarse class (Authentic, Half, Deceptive, Evaded). Each tool's
predictions at eligible positions become prediction rows; a prediction at an
unannotated position counts as an extra (false-positive) row. Pass one report
per split; their cases keep their split.
"""

import argparse
from fractions import Fraction
import hashlib
import json
from pathlib import Path

TASK = "cadence_class_at_onset"
TOOLS = ("acorde", "music21")


def sha256(path):
    return hashlib.sha256(path.read_bytes()).hexdigest()


def at(row):
    beat = Fraction(*row["offset_quarter"])
    return {"part": 0, "staff": 0, "measure": row["measure"], "beat": str(beat)}


def adapt(paths):
    cases, provenance, profiles = [], {}, None
    for path in paths:
        report = json.loads(path.read_text())
        provenance[report["split"]] = sha256(path)
        profile = {"acorde": {"version": report["acorde_binary_sha256"],
                              "command": "acorde " + report["acorde_command"], "options": {}},
                   "music21": {"version": report["music21_version"],
                               "command": report["recipe"],
                               "options": {"key": "score.analyze('key')"}}}
        if profiles not in (None, profile):
            raise ValueError("reports use different tool versions")
        profiles = profile
        for case in report["cases"]:
            gold = [{"task": TASK, "at": at(row), "labels": [row["gold_coarse"]]}
                    for row in case["rows"] if row["gold_coarse"]]
            if len(gold) != case["gold_positive"]:
                raise ValueError(f"gold row count mismatch: {case['id']}")
            systems = {}
            for tool in TOOLS:
                if f"{tool}_error" in case:
                    systems[tool] = {"error": case[f"{tool}_error"]}
                    continue
                predicted = []
                for row in case["rows"]:
                    kinds = row.get(tool, [])
                    if len(kinds) > 1:
                        raise ValueError(f"ambiguous {tool} prediction: {case['id']}")
                    if kinds:
                        predicted.append({"task": TASK, "at": at(row), "label": kinds[0]})
                systems[tool] = predicted
            cases.append({"id": case["id"], "input_sha256": case["input_sha256"],
                          "split": report["split"], "gold": gold, "systems": systems})
    return {"schema_version": 1,
            "provenance": {"task": TASK, "reports_sha256": provenance,
                           "eligible_positions": "part-0 note onsets of the projected cadence CSV, minus preflight unequal-timeline measures",
                           "class_mapping": {"PAC": "Authentic", "IAC": "Authentic", "HC": "Half",
                                             "DC": "Deceptive", "EC": "Evaded"}},
            "system_profiles": profiles, "cases": cases}


def main():
    parser = argparse.ArgumentParser(description=__doc__)
    parser.add_argument("output", type=Path)
    parser.add_argument("reports", type=Path, nargs="+")
    args = parser.parse_args()
    result = adapt(args.reports)
    args.output.write_text(json.dumps(result, ensure_ascii=False, indent=2) + "\n")
    print(f"projected {len(result['cases'])} cases")


if __name__ == "__main__":
    main()
