#!/usr/bin/env python3
"""Project only the development cadence annotations to folded score positions."""

import argparse
import hashlib
import json
from collections import Counter
from pathlib import Path

from mozart_cadence_positions import extract


def sha256(data):
    return hashlib.sha256(data).hexdigest()


def main():
    parser = argparse.ArgumentParser(description=__doc__)
    parser.add_argument("selection", type=Path)
    parser.add_argument("inputs", type=Path)
    parser.add_argument("annotation_hashes", type=Path)
    parser.add_argument("input_dir", type=Path)
    parser.add_argument("annotation_dir", type=Path)
    parser.add_argument("output", type=Path)
    args = parser.parse_args()
    selection_bytes = args.selection.read_bytes()
    selection = json.loads(selection_bytes)
    inputs = json.loads(args.inputs.read_text())
    annotations = json.loads(args.annotation_hashes.read_text())
    selected_hash = sha256(selection_bytes)
    if (inputs["selection_sha256"] != selected_hash
            or annotations["selection_sha256"] != selected_hash
            or inputs["source_commit"] != selection["commit"]
            or annotations["source_commit"] != selection["commit"]):
        parser.error("provenance mismatch")
    by_input = {item["id"]: item for item in inputs["inputs"]}
    by_annotation = {item["id"]: item for item in annotations["annotations"]}
    development = [case for case in selection["cases"] if case["split"] == "development"]
    if (len(development) != 18 or len(by_input) != 36
            or set(by_annotation) != {case["id"] for case in development}):
        parser.error("development split mismatch")
    cases = []
    labels = Counter()
    for case in development:
        item = by_input[case["id"]]
        annotation = by_annotation[case["id"]]
        if (item["path"] != case["score_path"]
                or annotation["path"] != case["cadence_path"]
                or annotation["git_blob"] != case["cadence_git_blob"]):
            parser.error(f"case provenance mismatch: {case['id']}")
        score_bytes = (args.input_dir / item["path"]).read_bytes()
        csv_bytes = (args.annotation_dir / annotation["path"]).read_bytes()
        if sha256(score_bytes) != item["input_sha256"] or sha256(csv_bytes) != annotation["sha256"]:
            parser.error(f"file SHA-256 mismatch: {case['id']}")
        projected = extract(score_bytes, csv_bytes.decode("utf-8-sig"))
        labels.update(position["cadence"] for position in projected["positions"]
                      if position["cadence"] is not None)
        cases.append({"id": case["id"], "split": case["split"],
                      "score_input_sha256": item["input_sha256"],
                      "cadence_sha256": annotation["sha256"], **projected})
    result = {
        "schema_version": 1,
        "classification": "development only; holdout annotation files untouched",
        "source_commit": selection["commit"],
        "selection_sha256": selected_hash,
        "prepared_inputs_sha256": sha256(args.inputs.read_bytes()),
        "annotation_hashes_sha256": sha256(args.annotation_hashes.read_bytes()),
        "summary": {"cases": len(cases), "source_rows": sum(case["source_rows"] for case in cases),
                    "eligible_positions": sum(case["eligible_positions"] for case in cases),
                    "positive_positions": sum(case["positive_positions"] for case in cases),
                    "labels": dict(sorted(labels.items())),
                    "exclusions": sum(len(case["exclusions"]) for case in cases)},
        "cases": cases,
    }
    args.output.write_text(json.dumps(result, ensure_ascii=False, indent=2) + "\n")
    print(json.dumps(result["summary"]))


if __name__ == "__main__":
    main()
