#!/usr/bin/env python3
"""Select score/cadence pairs from a pinned Batik-plays-Mozart Git tree.

Selection uses tree paths and blob IDs only. It never reads score or annotation
contents, and it assigns complete sonatas to one split.
"""

import argparse
import json
import re
import subprocess
from pathlib import Path


SOURCE = "https://github.com/huispaty/batik_plays_mozart"
SCORE_RE = re.compile(r"scores/kv(\d+)_(\d+)\.musicxml\Z")


def freeze(commit, tree_entries):
    if not re.fullmatch(r"[0-9a-f]{40}", commit):
        raise ValueError("expected a full pinned Git commit")
    blobs = dict(tree_entries)
    scores = []
    for path, score_blob in blobs.items():
        match = SCORE_RE.fullmatch(path)
        if not match:
            continue
        sonata, movement = map(int, match.groups())
        cadence_path = f"score_parts_annotated/kv{sonata}_{movement}_spart_cadence.csv"
        cadence_blob = blobs.get(cadence_path)
        if cadence_blob is None:
            raise ValueError(f"no cadence annotation for {path}")
        scores.append((sonata, movement, path, score_blob, cadence_path, cadence_blob))
    sonatas = sorted({row[0] for row in scores})
    if len(sonatas) != 12 or len(scores) != 36:
        raise ValueError(f"expected 12 sonatas and 36 movements, found {len(sonatas)} and {len(scores)}")
    for sonata in sonatas:
        if sorted(row[1] for row in scores if row[0] == sonata) != [1, 2, 3]:
            raise ValueError(f"incomplete movement set for K{sonata}")
    development = set(sonatas[:6])
    cases = [
        {"id": f"K{sonata}-{movement}",
         "split": "development" if sonata in development else "holdout",
         "score_path": score_path, "score_git_blob": score_blob,
         "cadence_path": cadence_path, "cadence_git_blob": cadence_blob}
        for sonata, movement, score_path, score_blob, cadence_path, cadence_blob
        in sorted(scores)
    ]
    return {
        "schema_version": 1,
        "source": SOURCE,
        "commit": commit,
        "selection": "All paired MusicXML and cadence-CSV movements, sorted by sonata number. The first six complete sonatas are development, the last six are untouched cadence-label holdout. The selection reads only Git paths and blob IDs.",
        "prior_exposure": "Some of these score files or movements were previously inspected for global-key work. Cadence labels have not been read for this task; this is a label-level holdout, not a previously unseen-score claim.",
        "cases": cases,
    }


def tree_entries(repo, commit):
    raw = subprocess.check_output(["git", "-C", str(repo), "ls-tree", "-r", "-z", commit])
    for entry in raw.split(b"\0"):
        if not entry:
            continue
        meta, path = entry.split(b"\t", 1)
        mode, kind, blob = meta.split(b" ", 2)
        if kind == b"blob":
            yield path.decode("utf-8"), blob.decode("ascii")


def main():
    parser = argparse.ArgumentParser(description=__doc__)
    parser.add_argument("repo", type=Path)
    parser.add_argument("commit")
    parser.add_argument("output", type=Path)
    args = parser.parse_args()
    result = freeze(args.commit, tree_entries(args.repo, args.commit))
    args.output.write_text(json.dumps(result, ensure_ascii=False, indent=2) + "\n")
    print(f"froze {len(result['cases'])} movements")


if __name__ == "__main__":
    main()
