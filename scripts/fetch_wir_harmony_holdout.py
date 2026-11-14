#!/usr/bin/env python3
"""Fetch pinned holdout files and verify their Git blobs and SHA-256 hashes."""

import argparse
import hashlib
import json
from pathlib import Path
import subprocess
from urllib.parse import quote


def git_blob_sha1(data):
    return hashlib.sha1(f"blob {len(data)}\0".encode() + data).hexdigest()


def main():
    parser = argparse.ArgumentParser(description=__doc__)
    parser.add_argument("selection", type=Path)
    parser.add_argument("output_dir", type=Path)
    parser.add_argument("hashes", type=Path)
    parser.add_argument("--scores-only", action="store_true",
                        help="fetch and hash score inputs without reading annotation files")
    args = parser.parse_args()
    selected = json.loads(args.selection.read_text())
    if selected["source"] != "https://github.com/MarkGotham/When-in-Rome":
        raise ValueError("unexpected source")
    root = f"https://raw.githubusercontent.com/MarkGotham/When-in-Rome/{selected['commit']}/"
    entries = []
    for work in selected["works"]:
        entry = {"work": work["work"]}
        kinds = (("score", "score_path"),) if args.scores_only else (
            ("score", "score_path"), ("gold", "gold_path"))
        for kind, field in kinds:
            path = work[field]
            if not path.startswith("Corpus/") or ".." in Path(path).parts:
                raise ValueError(f"invalid source path: {path}")
            target = args.output_dir / path
            target.parent.mkdir(parents=True, exist_ok=True)
            if target.exists():
                data = target.read_bytes()
            else:
                data = subprocess.run(
                    ["curl", "-L", "--fail", "--silent", "--show-error", "--max-time", "60", root + quote(path)],
                    check=True, capture_output=True,
                ).stdout
            if git_blob_sha1(data) != work[f"{kind}_git_blob"]:
                raise ValueError(f"Git blob mismatch: {path}")
            target.write_bytes(data)
            entry[f"{kind}_sha256"] = hashlib.sha256(data).hexdigest()
            entry[f"{kind}_bytes"] = len(data)
        entries.append(entry)
    result = {
        "schema_version": 1,
        "selection_sha256": hashlib.sha256(args.selection.read_bytes()).hexdigest(),
        "commit": selected["commit"],
        "files": entries,
    }
    if args.scores_only:
        result["scores_only"] = True
    args.hashes.write_text(json.dumps(result, ensure_ascii=False, indent=2) + "\n")
    print(f"verified {len(entries)} {'score inputs' if args.scores_only else 'score/gold pairs'}")


if __name__ == "__main__":
    main()
