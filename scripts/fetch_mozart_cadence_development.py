#!/usr/bin/env python3
"""Fetch only development cadence CSVs from a pinned local Git clone."""

import argparse
import hashlib
import json
import subprocess
from pathlib import Path


def main():
    parser = argparse.ArgumentParser(description=__doc__)
    parser.add_argument("selection", type=Path)
    parser.add_argument("repo", type=Path)
    parser.add_argument("output_dir", type=Path)
    parser.add_argument("record", type=Path)
    args = parser.parse_args()
    selection_bytes = args.selection.read_bytes()
    selection = json.loads(selection_bytes)
    commit = subprocess.check_output(["git", "-C", str(args.repo), "rev-parse", "HEAD"],
                                     text=True).strip()
    if commit != selection["commit"]:
        parser.error("source commit mismatch")
    rows = []
    for case in selection["cases"]:
        if case["split"] != "development":
            continue
        relative = Path(case["cadence_path"])
        if relative.is_absolute() or ".." in relative.parts:
            parser.error("unsafe cadence path")
        data = subprocess.check_output(["git", "-C", str(args.repo), "show",
                                        f"{commit}:{relative.as_posix()}"])
        blob = subprocess.check_output(["git", "hash-object", "--stdin"], input=data,
                                       text=False).decode("ascii").strip()
        if blob != case["cadence_git_blob"]:
            parser.error(f"source blob mismatch: {case['id']}")
        destination = args.output_dir / relative
        destination.parent.mkdir(parents=True, exist_ok=True)
        destination.write_bytes(data)
        rows.append({"id": case["id"], "path": relative.as_posix(),
                     "git_blob": blob, "sha256": hashlib.sha256(data).hexdigest()})
    result = {"schema_version": 1, "source_commit": commit,
              "selection_sha256": hashlib.sha256(selection_bytes).hexdigest(),
              "classification": "development annotations only; holdout CSVs not fetched",
              "annotations": rows}
    args.record.write_text(json.dumps(result, ensure_ascii=False, indent=2) + "\n")
    print(f"fetched {len(rows)} development annotations")


if __name__ == "__main__":
    main()
