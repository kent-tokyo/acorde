#!/usr/bin/env python3
"""Fetch only selected global-key score sources; verify pinned Git blobs."""

import argparse
import hashlib
import json
import subprocess
from pathlib import Path
from urllib.parse import quote


def git_blob_sha1(data):
    return hashlib.sha1(f"blob {len(data)}\0".encode() + data).hexdigest()


def main():
    parser = argparse.ArgumentParser(description=__doc__)
    parser.add_argument("selection", type=Path)
    parser.add_argument("output_dir", type=Path)
    parser.add_argument("hashes", type=Path)
    parser.add_argument("--local-only", action="store_true",
                        help="verify already downloaded score files without network access")
    args = parser.parse_args()
    selection_bytes = args.selection.read_bytes()
    selected = json.loads(selection_bytes)
    if selected.get("classification") != "global-key label holdout selection":
        parser.error("unexpected selection")
    records = []
    for work in selected["works"]:
        repo = work["score_repo"]
        commit = work["score_commit"]
        path = work["score_path"]
        if repo not in ("MarkGotham/Chorale-Corpus", "DCMLab/cpe_bach_keyboard",
                        "DCMLab/pleyel_quartets"):
            raise ValueError(f"unexpected score repository: {repo}")
        if ".." in Path(path).parts or path.startswith("/"):
            raise ValueError(f"unsafe score path: {path}")
        suffix = Path(path).suffix
        target = args.output_dir / work["group"] / f"{work['piece']}{suffix}"
        target.parent.mkdir(parents=True, exist_ok=True)
        if target.exists():
            data = target.read_bytes()
        else:
            if args.local_only:
                raise FileNotFoundError(target)
            url = (f"https://raw.githubusercontent.com/{repo}/{commit}/"
                   + quote(path))
            fetched = subprocess.run(
                ["curl", "-L", "--fail", "--silent", "--show-error",
                 "--max-time", "120", url], check=False, capture_output=True)
            if fetched.returncode:
                raise RuntimeError(f"curl failed for {path}: "
                                   + fetched.stderr.decode(errors="replace")[-300:])
            data = fetched.stdout
        if git_blob_sha1(data) != work["score_git_blob"]:
            raise ValueError(f"score Git blob mismatch: {work['group']}/{work['piece']}")
        target.write_bytes(data)
        records.append({"group": work["group"], "piece": work["piece"],
                        "score_path": path, "score_git_blob": work["score_git_blob"],
                        "local_path": str(target.relative_to(args.output_dir)),
                        "source_sha256": hashlib.sha256(data).hexdigest(),
                        "source_bytes": len(data)})
    result = {"schema_version": 1, "scores_only": True,
              "selection_sha256": hashlib.sha256(selection_bytes).hexdigest(),
              "files": records}
    args.hashes.write_text(json.dumps(result, ensure_ascii=False, indent=2) + "\n")
    print(f"verified {len(records)} score sources; no key labels fetched")


if __name__ == "__main__":
    main()
