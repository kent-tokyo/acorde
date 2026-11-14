#!/usr/bin/env python3
"""Project a complete When in Rome comparison into the common label evaluator."""

import argparse
from fractions import Fraction
import hashlib
import json
from pathlib import Path

from compare_wir_harmony_music21 import canonical


TASK = "roman_at_primary_annotation_position"


def position(row):
    beat = Fraction(str(row["beat"])).limit_denominator(10_000)
    return {"part": 0, "staff": 0, "measure": row["measure_index"], "beat": str(beat)}


def adapt(report, selection, hashes, *, report_bytes, selection_bytes,
          source_dir=None, prepared=None, prepared_bytes=None):
    if report.get("schema_version") != 1 or report.get("classification") not in {"development", "holdout"}:
        raise ValueError("expected complete classified report")
    if hashlib.sha256(selection_bytes).hexdigest() != report["selection_sha256"]:
        raise ValueError("selection/report hash mismatch")
    if hashes["selection_sha256"] != report["selection_sha256"]:
        raise ValueError("selection/hash-record mismatch")
    if report["commit"] != selection["commit"] or report["commit"] != hashes["commit"]:
        raise ValueError("commit mismatch")
    selected = {work["work"]: work for work in selection["works"]}
    hashed = {work["work"]: work for work in hashes["files"]}
    observed = {work["work"]: work for work in report["works"]}
    if not selected or set(selected) != set(hashed) or set(selected) != set(observed):
        raise ValueError("work list mismatch")
    if prepared is None:
        if report.get("prepared_inputs_sha256") is not None:
            raise ValueError("missing prepared input record")
        prepared_by_work = {}
    else:
        if (prepared_bytes is None
                or report.get("prepared_inputs_sha256") != hashlib.sha256(prepared_bytes).hexdigest()
                or prepared.get("source_commit") != report["commit"]
                or prepared.get("selection_sha256") != report["selection_sha256"]):
            raise ValueError("prepared input provenance mismatch")
        prepared_by_work = {item["work"]: item for item in prepared["inputs"]}
        if set(prepared_by_work) != set(selected):
            raise ValueError("prepared work set mismatch")
    cases = []
    for work in selection["works"]:
        name = work["work"]
        source = observed[name]
        digest = (prepared_by_work[name]["input_sha256"] if prepared_by_work
                  else hashed[name]["score_sha256"])
        if prepared_by_work and (
            prepared_by_work[name]["source_sha256"] != hashed[name]["score_sha256"]
            or prepared_by_work[name]["gold_sha256"] != hashed[name]["gold_sha256"]
            or prepared_by_work[name]["path"] != work["score_path"]
        ):
            raise ValueError(f"prepared input mismatch: {name}")
        if source["score_sha256"] != digest or source["gold_sha256"] != hashed[name]["gold_sha256"]:
            raise ValueError(f"input or gold hash mismatch: {name}")
        if source.get("source_labels", 0) != len(source["rows"]) + sum(
            item["labels"] for item in source["exclusions"]
        ):
            raise ValueError(f"source-label conservation failed: {name}")
        case = {"id": name, "input_sha256": digest, "split": report["classification"],
                "gold_sha256": source["gold_sha256"], "exclusions": source["exclusions"]}
        if source_dir is not None:
            case["input_path"] = str((source_dir / work["score_path"]).resolve())
        gold = []
        acorde = []
        music21 = []
        for row in source["rows"]:
            at = position(row)
            label = canonical(row["gold"])
            if not label:
                raise ValueError(f"empty gold label: {name}")
            gold.append({"task": TASK, "at": at, "labels": [label]})
            for key, destination in (("acorde", acorde), ("music21", music21)):
                prediction = canonical(row[key])
                if (prediction == label) != row[f"{key}_correct"]:
                    raise ValueError(f"saved correctness mismatch: {name} {row['measure']} {row['beat']}")
                if prediction:
                    destination.append({"task": TASK, "at": at, "label": prediction})
        case["gold"] = gold
        case["systems"] = {
            "acorde": {"error": source["acorde_error"]} if "acorde_error" in source else acorde,
            "music21": {"error": source["music21_error"]} if "music21_error" in source else music21,
        }
        cases.append(case)
    summary = report["summary"]
    if (sum(len(case["gold"]) for case in cases) != summary["scored_labels"]
            or sum(len(case["systems"]["acorde"]) for case in cases) != summary["acorde_predicted"]
            or sum(len(case["systems"]["music21"]) for case in cases) != summary["music21_predicted"]):
        raise ValueError("saved summary counts mismatch")
    return {
        "schema_version": 1,
        "provenance": {"corpus_commit": report["commit"],
                       "source_report_sha256": hashlib.sha256(report_bytes).hexdigest(),
                       "selection_sha256": report["selection_sha256"],
                       "task": TASK,
                       "exclusion_policy": "unequal pitched timeline, ambiguous measure or duplicate source position"},
        "system_profiles": {
            "acorde": {"version": report["acorde_binary_sha256"],
                       "command": "acorde analyze-vertical INPUT --infer-key" +
                       (" --arpeggiated-bars" if report.get("arpeggiated_bars") else ""),
                       "options": {"key": "independently estimated global key",
                                   "arpeggiated_bars": bool(report.get("arpeggiated_bars"))}},
            "music21": {"version": report["music21_version"],
                        "command": "converter.parse(INPUT).analyze('key'); roman.romanNumeralFromChord(active, key)",
                        "options": {"key": "independently estimated global key"}},
        },
        "cases": cases,
    }


def main():
    parser = argparse.ArgumentParser(description=__doc__)
    parser.add_argument("report", type=Path)
    parser.add_argument("selection", type=Path)
    parser.add_argument("hashes", type=Path)
    parser.add_argument("output", type=Path)
    parser.add_argument("--source-dir", type=Path)
    parser.add_argument("--prepared-inputs", type=Path)
    args = parser.parse_args()
    report_bytes = args.report.read_bytes()
    selection_bytes = args.selection.read_bytes()
    prepared_bytes = args.prepared_inputs.read_bytes() if args.prepared_inputs else None
    result = adapt(json.loads(report_bytes), json.loads(selection_bytes),
                   json.loads(args.hashes.read_bytes()),
                   report_bytes=report_bytes, selection_bytes=selection_bytes,
                   source_dir=args.source_dir,
                   prepared=json.loads(prepared_bytes) if prepared_bytes else None,
                   prepared_bytes=prepared_bytes)
    args.output.write_text(json.dumps(result, ensure_ascii=False, indent=2) + "\n")
    print(f"projected {len(result['cases'])} works")


if __name__ == "__main__":
    main()
