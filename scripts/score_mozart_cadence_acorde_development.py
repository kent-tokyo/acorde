#!/usr/bin/env python3
"""Diagnostic Acorde-only cadence baseline on reserved-free Mozart development data."""

import argparse
import hashlib
import json
import subprocess
from collections import defaultdict
from fractions import Fraction
from pathlib import Path

from compare_wir_harmony_music21 import acorde_fraction_beats


GOLD_KIND = {"PAC": "Authentic", "IAC": "Authentic", "HC": "Half",
             "DC": "Deceptive", "EC": "Evaded", "PC": "Plagal"}


def sha256(path):
    return hashlib.sha256(path.read_bytes()).hexdigest()


def candidate_position(score, candidate):
    address = candidate["to"]
    voice = score["parts"][address["part"]]["staves"][address["staff"]]["measures"][address["measure"]]["voices"][address["voice"]]
    if address["note"] >= len(voice):
        raise ValueError("cadence address exceeds its voice")
    beat = sum((acorde_fraction_beats(note) for note in voice[:address["note"]]), Fraction())
    return (address["part"], address["measure"], beat)


def score_case(projected, candidates, score, excluded_measures):
    eligible = {}
    for position in projected["positions"]:
        if position["measure"] in excluded_measures:
            continue
        beat = Fraction(*position["offset_quarter"])
        key = (position["part"], position["measure"], beat)
        if key in eligible:
            raise ValueError("duplicate eligible position")
        eligible[key] = position["cadence"]
    predicted = defaultdict(set)
    outside = 0
    for candidate in candidates:
        key = candidate_position(score, candidate)
        if key not in eligible:
            outside += 1
        else:
            predicted[key].add(candidate["kind"])
    counts = {"eligible": len(eligible),
              "gold_positive": sum(label is not None for label in eligible.values()),
              "predicted_positions": len(predicted), "correct": 0,
              "wrong_label": 0, "false_positive": 0,
              "missed": 0, "true_negative": 0,
              "ambiguous_prediction": 0,
              "outside_eligible_candidates": outside}
    errors = []
    for (part, measure, beat), label in sorted(eligible.items()):
        kinds = predicted.get((part, measure, beat), set())
        expected = GOLD_KIND.get(label) if label else None
        if label and expected is None:
            raise ValueError(f"unknown gold cadence: {label}")
        if len(kinds) > 1:
            counts["ambiguous_prediction"] += 1
        if not kinds and expected is None:
            counts["true_negative"] += 1
        elif not kinds:
            counts["missed"] += 1
        elif expected is None:
            counts["false_positive"] += 1
        elif len(kinds) == 1 and expected in kinds:
            counts["correct"] += 1
        else:
            counts["wrong_label"] += 1
        if kinds or expected:
            errors.append({"part": part, "measure": measure,
                           "offset_quarter": [beat.numerator, beat.denominator],
                           "gold": label, "gold_coarse": expected,
                           "predicted": sorted(kinds)})
    return counts, errors


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
    by_preflight = {case["id"]: case for case in preflight["cases"]}
    results = []
    for case in projection["cases"]:
        if case["split"] != "development":
            parser.error("reserved case in development projection")
        checked = by_preflight[case["id"]]
        if case["score_input_sha256"] != checked["input_sha256"]:
            parser.error(f"input mismatch: {case['id']}")
        score_name = "kv" + case["id"][1:].lower().replace("-", "_") + ".musicxml"
        score_path = args.input_dir / "scores" / score_name
        if sha256(score_path) != case["score_input_sha256"]:
            parser.error(f"file hash mismatch: {case['id']}")
        try:
            analysis = json.loads(subprocess.check_output([str(args.acorde.resolve()), "analyze", str(score_path)]))
            report = json.loads(subprocess.check_output([str(args.acorde.resolve()), "report", str(score_path)]))
            counts, rows = score_case(case, analysis["cadence_candidates"],
                                      report["score"], set(checked["unequal_timeline_measures"]))
            results.append({"id": case["id"], "input_sha256": case["score_input_sha256"],
                            "excluded_timeline_measures": checked["unequal_timeline_measures"],
                            "counts": counts, "rows": rows})
        except (ValueError, subprocess.CalledProcessError) as error:
            results.append({"id": case["id"], "error": str(error)})
    keys = ("eligible", "gold_positive", "predicted_positions", "correct", "wrong_label",
            "false_positive", "missed", "true_negative", "ambiguous_prediction",
            "outside_eligible_candidates")
    summary = {key: sum(case["counts"][key] for case in results if "counts" in case)
               for key in keys}
    summary["cases"] = len(results)
    summary["failures"] = sum("error" in case for case in results)
    result = {"schema_version": 1, "classification": "development diagnostic, Acorde only",
              "source_commit": projection["source_commit"],
              "projection_sha256": sha256(args.projection),
              "preflight_sha256": sha256(args.preflight),
              "acorde_binary_sha256": sha256(args.acorde),
              "class_mapping": GOLD_KIND,
              "summary": summary, "cases": results}
    args.output.write_text(json.dumps(result, ensure_ascii=False, indent=2) + "\n")
    print(json.dumps(summary))


if __name__ == "__main__":
    main()
