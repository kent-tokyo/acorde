#!/usr/bin/env python3
"""Project the local-key path development sweep and holdout v2 into the common label evaluator.

Development rows come from diagnose_wir_local_key_path.py; holdout rows from
score_wir_local_key_holdout_v2.py. Each report's row file is hash-checked
and each case keeps its split, so the evaluator reports them separately.
"""

import argparse
from collections import defaultdict
from fractions import Fraction
import hashlib
import json
from pathlib import Path

TASK = "local_key_at_annotation_position"


def sha256(path):
    return hashlib.sha256(path.read_bytes()).hexdigest()


def rows_for(report_path, report):
    rows_path = report_path.with_name(report["rows_file"])
    if sha256(rows_path) != report["rows_sha256"]:
        raise ValueError(f"row file hash mismatch: {rows_path}")
    grouped = defaultdict(list)
    for line in rows_path.read_text().splitlines():
        row = json.loads(line)
        grouped[row["work"]].append(row)
    return grouped


def case(name, digest, split, rows):
    gold, acorde, music21 = [], [], []
    for row in rows:
        beat = Fraction(str(row["beat"])).limit_denominator(10_000)
        at = {"part": 0, "staff": 0, "measure": row["measure_index"], "beat": str(beat)}
        gold.append({"task": TASK, "at": at, "labels": [row["gold"]]})
        for key, destination in (("acorde", acorde), ("music21", music21)):
            if row[key] is not None:
                destination.append({"task": TASK, "at": at, "label": row[key]})
    return {"id": name, "input_sha256": digest, "split": split, "gold": gold,
            "systems": {"acorde": acorde, "music21": music21}}


def adapt(development_path, holdout_path):
    development = json.loads(development_path.read_text())
    holdout = json.loads(holdout_path.read_text())
    if (development["classification"] != "development diagnostic on inspected works, not a holdout"
            or holdout["classification"] != "frozen local-key label holdout v2"):
        raise ValueError("unexpected report classification")
    cases = []
    dev_rows = rows_for(development_path, development)
    for work in development["works"]:
        if len(dev_rows[work["work"]]) != work["positions"]:
            raise ValueError(f"development row count mismatch: {work['work']}")
        cases.append(case(work["work"], work["input_sha256"], "development", dev_rows[work["work"]]))
    hold_rows = rows_for(holdout_path, holdout)
    for work in holdout["works"]:
        if "error" in work:
            cases.append({"id": work["work"], "input_sha256": work["input_sha256"],
                          "split": "holdout", "gold": [],
                          "systems": {"acorde": {"error": work["error"]},
                                      "music21": {"error": work["error"]}}})
            continue
        if len(hold_rows[work["work"]]) != work["counts"]["positions"]:
            raise ValueError(f"holdout row count mismatch: {work['work']}")
        cases.append(case(work["work"], work["input_sha256"], "holdout", hold_rows[work["work"]]))
    return {"schema_version": 1,
            "provenance": {"task": TASK,
                           "development_report_sha256": sha256(development_path),
                           "holdout_report_sha256": sha256(holdout_path),
                           "exclusion_policy": "duplicate source beat, missing or ambiguous source measure, measure-number mismatch, predeclared unequal pitched timeline"},
            "system_profiles": {
                "acorde": {"version": holdout["acorde_binary_sha256"],
                           "command": "acorde analyze-local-keys INPUT --method path",
                           "options": {"radius_measures": 1, "change_cost": 1.5, "fifth_cost": 0.1}},
                "music21": {"version": holdout["music21_version"],
                            "command": "analysis.floatingKey.KeyAnalyzer(score).smoothInterpretationByMeasure()",
                            "options": {"window_measures": holdout["music21_window_measures"]}}},
            "cases": cases}


def main():
    parser = argparse.ArgumentParser(description=__doc__)
    parser.add_argument("development", type=Path)
    parser.add_argument("holdout", type=Path)
    parser.add_argument("output", type=Path)
    args = parser.parse_args()
    result = adapt(args.development, args.holdout)
    args.output.write_text(json.dumps(result, ensure_ascii=False, indent=2) + "\n")
    print(f"projected {len(result['cases'])} works")


if __name__ == "__main__":
    main()
