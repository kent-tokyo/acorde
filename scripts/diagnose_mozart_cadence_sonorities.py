#!/usr/bin/env python3
"""Inspect only Mozart cadence development labels against vertical chords."""

import argparse
import hashlib
import json
import subprocess
from collections import Counter, defaultdict
from fractions import Fraction
from pathlib import Path


def sha256(path):
    return hashlib.sha256(path.read_bytes()).hexdigest()


def score_path(input_dir, case_id):
    return input_dir / "scores" / ("kv" + case_id[1:].lower().replace("-", "_")
                                     + ".musicxml")


def inspect_case(case, checked, input_dir, binary):
    if case["split"] != "development":
        raise ValueError("reserved label case in development diagnostic")
    if case["score_input_sha256"] != checked["input_sha256"]:
        raise ValueError("projection/preflight input hash mismatch")
    path = score_path(input_dir, case["id"])
    if sha256(path) != case["score_input_sha256"]:
        raise ValueError("prepared score hash mismatch")
    result = subprocess.run([str(binary), "analyze-vertical", str(path),
                             "--infer-key", "--arpeggiated-bars"],
                            capture_output=True, text=True, timeout=120,
                            check=False)
    if result.returncode:
        raise RuntimeError(result.stderr.strip()[-300:] or
                           f"Acorde exit status {result.returncode}")
    sonorities = json.loads(result.stdout)["sonorities"]
    by_measure = defaultdict(list)
    for sonority in sonorities:
        by_measure[sonority["measure"]].append(sonority)
    for values in by_measure.values():
        values.sort(key=lambda item: item["beat"])
    rows = []
    excluded = set(checked["unequal_timeline_measures"])
    for position in case["positions"]:
        label = position["cadence"]
        if label is None or position["measure"] in excluded:
            continue
        measure = position["measure"]
        beat = float(Fraction(*position["offset_quarter"]))
        current = [item for item in by_measure[measure]
                   if abs(item["beat"] - beat) < 1e-6]
        earlier = [item for index in (measure - 1, measure)
                   if index >= 0 for item in by_measure[index]
                   if index < measure or item["beat"] < beat - 1e-6]
        earlier.sort(key=lambda item: (item["measure"], item["beat"]), reverse=True)
        current_roman = sorted({item["label"].get("roman_numeral") for item in current
                                if item["label"].get("roman_numeral")})
        previous_roman = next((item["label"].get("roman_numeral")
                               for item in earlier
                               if item["label"].get("roman_numeral")
                               and item["label"].get("roman_numeral") not in current_roman), None)
        rows.append({"id": case["id"], "gold": label,
                     "part": position["part"], "measure": measure,
                     "offset_quarter": position["offset_quarter"],
                     "vertical_at_gold": bool(current),
                     "current_roman": current_roman,
                     "previous_distinct_roman": previous_roman})
    return rows


def summarize(rows):
    by_label = {}
    for label in sorted({row["gold"] for row in rows}):
        selected = [row for row in rows if row["gold"] == label]
        by_label[label] = {
            "gold_positions": len(selected),
            "vertical_at_gold": sum(row["vertical_at_gold"] for row in selected),
            "roman_at_gold": sum(bool(row["current_roman"]) for row in selected),
            "current_roman": dict(Counter(
                roman for row in selected for roman in (row["current_roman"] or ["<none>"]))
            ),
            "previous_distinct_roman": dict(Counter(
                row["previous_distinct_roman"] or "<none>" for row in selected)),
        }
    return by_label


def main():
    parser = argparse.ArgumentParser(description=__doc__)
    parser.add_argument("projection", type=Path)
    parser.add_argument("preflight", type=Path)
    parser.add_argument("input_dir", type=Path)
    parser.add_argument("acorde", type=Path)
    parser.add_argument("output", type=Path)
    args = parser.parse_args()
    projection = json.loads(args.projection.read_text())
    preflight = json.loads(args.preflight.read_text())
    if (projection["source_commit"] != preflight["source_commit"]
            or projection["prepared_inputs_sha256"] != preflight["prepared_inputs_sha256"]):
        parser.error("projection/preflight provenance mismatch")
    checked = {item["id"]: item for item in preflight["cases"]}
    rows = []
    for case in projection["cases"]:
        rows.extend(inspect_case(case, checked[case["id"]], args.input_dir,
                                 args.acorde.resolve()))
    result = {"schema_version": 1,
              "classification": "development diagnostic, not a held-out comparison",
              "projection_sha256": sha256(args.projection),
              "preflight_sha256": sha256(args.preflight),
              "acorde_binary_sha256": sha256(args.acorde),
              "method": "analyze-vertical --infer-key --arpeggiated-bars",
              "summary": summarize(rows), "rows": rows}
    args.output.write_text(json.dumps(result, ensure_ascii=False, indent=2) + "\n")
    print({key: {k: v for k, v in values.items()
                 if k in ("gold_positions", "vertical_at_gold", "roman_at_gold")}
           for key, values in result["summary"].items()})


if __name__ == "__main__":
    main()
