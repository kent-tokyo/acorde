#!/usr/bin/env python3
"""Score two explicit cadence transition hypotheses on development labels only."""

import argparse
import hashlib
import json
import subprocess
from collections import Counter
from pathlib import Path

from score_mozart_cadence_acorde_development import candidate_position, score_case


def sha256(path):
    return hashlib.sha256(path.read_bytes()).hexdigest()


def plain_roman(label, numeral):
    if label is None or "/" in label or not label.startswith(numeral):
        return False
    suffix = label[len(numeral):]
    return not suffix or suffix[0].isdigit()


def transitions(sonorities):
    ordered = sorted(sonorities, key=lambda item: (item["measure"], item["beat"]))
    candidates = []
    for index, current in enumerate(ordered):
        roman = current["label"].get("roman_numeral")
        if not roman:
            continue
        previous = next((item for item in reversed(ordered[max(0, index - 8):index])
                         if current["measure"] - item["measure"] <= 1
                         and item["label"].get("roman_numeral")
                         and item["label"].get("roman_numeral") != roman), None)
        if previous is None:
            continue
        prior = previous["label"].get("roman_numeral")
        if plain_roman(roman, "V") and prior in ("I64", "i64"):
            kind = "Half"
        elif roman in ("vi", "VI") and plain_roman(prior, "V"):
            kind = "Deceptive"
        else:
            continue
        candidates.append({"kind": kind, "from": previous["label"]["address"],
                           "to": current["label"]["address"],
                           "previous_roman": prior, "current_roman": roman})
    return candidates


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
    results = []
    for case in projection["cases"]:
        if case["split"] != "development":
            parser.error("reserved annotation in development diagnostic")
        name = "kv" + case["id"][1:].lower().replace("-", "_") + ".musicxml"
        path = args.input_dir / "scores" / name
        if sha256(path) != case["score_input_sha256"]:
            parser.error(f"input hash mismatch: {case['id']}")
        vertical = json.loads(subprocess.check_output([
            str(args.acorde.resolve()), "analyze-vertical", str(path),
            "--infer-key", "--arpeggiated-bars"]))
        report = json.loads(subprocess.check_output([
            str(args.acorde.resolve()), "report", str(path)]))
        candidates = transitions(vertical["sonorities"])
        for candidate in candidates:
            part, measure, beat = candidate_position(report["score"], candidate)
            candidate["position"] = [part, measure, beat.numerator, beat.denominator]
            measures = report["score"]["parts"][part]["staves"][0]["measures"]
            candidate["time_sig"] = next(
                (item["time_sig"] for item in reversed(measures[:measure + 1])
                 if item["time_sig"]), None)
        counts, rows = score_case(case, candidates, report["score"],
                                  set(checked[case["id"]]["unequal_timeline_measures"]))
        results.append({"id": case["id"], "input_sha256": case["score_input_sha256"],
                        "candidates": candidates, "counts": counts, "rows": rows})
    metrics = ("eligible", "gold_positive", "predicted_positions", "correct",
               "wrong_label", "false_positive", "missed", "true_negative",
               "ambiguous_prediction", "outside_eligible_candidates")
    summary = {key: sum(row["counts"][key] for row in results) for key in metrics}
    summary["predicted_by_kind"] = dict(Counter(
        candidate["kind"] for row in results for candidate in row["candidates"]))
    result = {"schema_version": 1,
              "classification": "development transition hypotheses, not a production candidate",
              "hypotheses": ["I64 to plain V predicts Half",
                             "plain V to vi/VI predicts Deceptive"],
              "projection_sha256": sha256(args.projection),
              "preflight_sha256": sha256(args.preflight),
              "acorde_binary_sha256": sha256(args.acorde),
              "summary": summary, "cases": results}
    args.output.write_text(json.dumps(result, ensure_ascii=False, indent=2) + "\n")
    print(json.dumps(summary, sort_keys=True))


if __name__ == "__main__":
    main()
