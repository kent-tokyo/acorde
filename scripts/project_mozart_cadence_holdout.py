#!/usr/bin/env python3
"""Fetch and project the reserved Mozart cadence CSVs, only under a frozen protocol (#502).

Usage: project_mozart_cadence_holdout.py PROTOCOL SELECTION INPUTS REPO INPUT_DIR
       ANNOTATION_DIR HASHES_OUT PROJECTION_OUT

REPO is a clone of the pinned source commit. Every pinned script, the
selection and the prepared-input record must match the protocol before any
reserved CSV is read.
"""

import argparse
from collections import Counter
import hashlib
import json
from pathlib import Path
import subprocess

from mozart_cadence_positions import extract


def sha256(data):
    return hashlib.sha256(data).hexdigest()


def main():
    parser = argparse.ArgumentParser(description=__doc__)
    for name in ("protocol", "selection", "inputs", "repo", "input_dir", "annotation_dir",
                 "hashes_out", "projection_out"):
        parser.add_argument(name, type=Path)
    args = parser.parse_args()
    protocol = json.loads(args.protocol.read_text())
    pinned = protocol["pinned"]
    scripts = Path(__file__).resolve().parent
    selection_bytes = args.selection.read_bytes()
    if (sha256(selection_bytes) != pinned["selection_sha256"]
            or sha256(args.inputs.read_bytes()) != pinned["inputs_sha256"]
            or any(sha256((scripts / name).read_bytes()) != digest
                   for name, digest in pinned["script_sha256"].items())):
        parser.error("protocol pin mismatch; refusing to read reserved annotations")
    selection = json.loads(selection_bytes)
    commit = subprocess.check_output(["git", "-C", str(args.repo), "rev-parse", "HEAD"],
                                     text=True).strip()
    if commit != selection["commit"] or commit != protocol["source_commit"]:
        parser.error("source commit mismatch")
    inputs = {item["id"]: item for item in json.loads(args.inputs.read_text())["inputs"]}
    reserved = [case for case in selection["cases"] if case["split"] != "development"]
    if [case["id"] for case in reserved] != protocol["holdout_cases"]:
        parser.error("reserved case list mismatch")
    annotations, cases, labels = [], [], Counter()
    for case in reserved:
        relative = Path(case["cadence_path"])
        if relative.is_absolute() or ".." in relative.parts:
            parser.error("unsafe cadence path")
        data = subprocess.check_output(["git", "-C", str(args.repo), "show",
                                        f"{commit}:{relative.as_posix()}"])
        blob = subprocess.check_output(["git", "hash-object", "--stdin"], input=data).decode().strip()
        if blob != case["cadence_git_blob"]:
            parser.error(f"source blob mismatch: {case['id']}")
        target = args.annotation_dir / relative
        target.parent.mkdir(parents=True, exist_ok=True)
        target.write_bytes(data)
        annotations.append({"id": case["id"], "path": relative.as_posix(), "git_blob": blob,
                            "sha256": sha256(data)})
        item = inputs[case["id"]]
        score_bytes = (args.input_dir / item["path"]).read_bytes()
        if sha256(score_bytes) != item["input_sha256"]:
            parser.error(f"prepared score hash mismatch: {case['id']}")
        projected = extract(score_bytes, data.decode("utf-8-sig"))
        labels.update(p["cadence"] for p in projected["positions"] if p["cadence"] is not None)
        cases.append({"id": case["id"], "split": case["split"],
                      "score_input_sha256": item["input_sha256"],
                      "cadence_sha256": sha256(data), **projected})
    hashes = {"schema_version": 1, "source_commit": commit,
              "selection_sha256": sha256(selection_bytes),
              "classification": "reserved holdout annotations, read after protocol freeze",
              "protocol_sha256": sha256(args.protocol.read_bytes()), "annotations": annotations}
    args.hashes_out.write_text(json.dumps(hashes, ensure_ascii=False, indent=2) + "\n")
    projection = {
        "schema_version": 1, "classification": "reserved holdout, projected after protocol freeze",
        "source_commit": commit, "selection_sha256": sha256(selection_bytes),
        "prepared_inputs_sha256": pinned["inputs_sha256"],
        "annotation_hashes_sha256": sha256(args.hashes_out.read_bytes()),
        "protocol_sha256": sha256(args.protocol.read_bytes()),
        "summary": {"cases": len(cases), "source_rows": sum(c["source_rows"] for c in cases),
                    "eligible_positions": sum(c["eligible_positions"] for c in cases),
                    "positive_positions": sum(c["positive_positions"] for c in cases),
                    "labels": dict(sorted(labels.items())),
                    "exclusions": sum(len(c["exclusions"]) for c in cases)},
        "cases": cases}
    args.projection_out.write_text(json.dumps(projection, ensure_ascii=False, indent=2) + "\n")
    print(json.dumps(projection["summary"]))


if __name__ == "__main__":
    main()
