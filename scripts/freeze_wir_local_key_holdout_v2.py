#!/usr/bin/env python3
"""Select the second local-key label holdout (#501) from a pinned Git tree only.

Usage: freeze_wir_local_key_holdout_v2.py WIR_CLONE OUTPUT --previous SELECTION...

WIR_CLONE is a blob-less clone of When in Rome at the pinned commit. The
script reads only `git ls-tree` paths and blob ids; it opens no score or
annotation. Every work, collection and song cycle in a previous selection is
excluded; for the quartet, sonata and song groups every parent collection is
excluded too, so no movement or song of an earlier piece or cycle can repeat.
Bach keyboard works are separate preludes and fugues and are excluded by work.
"""

import argparse
import hashlib
import json
import subprocess
from pathlib import Path, PurePosixPath

SOURCE = "https://github.com/MarkGotham/When-in-Rome"
COMMIT = "1c61fe41b8c2910296d7d2bcbf6476c7c1f2fe35"
SEED = "acorde-wir-local-key-holdout-v2-20261128"
# (group, path prefix, works, one work per collection, one work per composer)
GROUPS = (
    ("quartet", "Corpus/Quartets/Beethoven,_Ludwig_van/", 4, True, False),
    ("keyboard", "Corpus/Keyboard_Other/Bach,_Johann_Sebastian/", 4, False, False),
    ("sonata", "Corpus/Piano_Sonatas/Mozart,_Wolfgang_Amadeus/", 4, True, False),
    ("song", "Corpus/OpenScore-LiederCorpus/", 4, True, True),
)


def sha256(data):
    return hashlib.sha256(data).hexdigest()


def collection(work):
    """The piece or cycle a work belongs to: its parent directory."""
    return str(PurePosixPath(work).parent)


def composer(work):
    return PurePosixPath(work).parts[2]


def select(entries, previous):
    paths = {path: sha for path, sha in entries}
    used_works = {work["work"] for old in previous for work in old["works"]}
    used_collections = {collection(work) for work in used_works}
    works = []
    for group, prefix, count, per_collection, per_composer in GROUPS:
        candidates = []
        for path in paths:
            if not (path.startswith(prefix) and path.endswith("/score.mxl")):
                continue
            work = path.removesuffix("/score.mxl")
            if work + "/analysis.txt" not in paths:
                continue
            if work in used_works or (per_collection and collection(work) in used_collections):
                continue
            rank = sha256(f"{SEED}\n{group}\n{work}".encode())
            candidates.append((rank, work))
        seen_collections, seen_composers, chosen = set(), set(), 0
        for rank, work in sorted(candidates):
            if per_collection and collection(work) in seen_collections:
                continue
            if per_composer and composer(work) in seen_composers:
                continue
            seen_collections.add(collection(work))
            seen_composers.add(composer(work))
            score, gold = work + "/score.mxl", work + "/analysis.txt"
            works.append({"group": group, "work": work, "selection_rank": rank,
                          "score_path": score, "score_git_blob": paths[score],
                          "gold_path": gold, "gold_git_blob": paths[gold]})
            chosen += 1
            if chosen == count:
                break
        if chosen != count:
            raise ValueError(f"not enough unused score/analysis pairs under {prefix}")
    return works


def main():
    parser = argparse.ArgumentParser(description=__doc__)
    parser.add_argument("clone", type=Path)
    parser.add_argument("output", type=Path)
    parser.add_argument("--previous", type=Path, action="append", required=True)
    args = parser.parse_args()
    if args.output.exists():
        parser.error("refusing to overwrite a frozen selection")
    git = ["git", "-c", "gc.auto=0", "-C", str(args.clone)]
    head = subprocess.run([*git, "rev-parse", "HEAD"], check=True,
                          capture_output=True, text=True).stdout.strip()
    if head != COMMIT:
        parser.error(f"clone is at {head}, not {COMMIT}")
    tree = subprocess.run([*git, "ls-tree", "-r", "HEAD"], check=True,
                          capture_output=True, text=True).stdout
    entries = []
    for line in tree.splitlines():
        meta, path = line.split("\t", 1)
        _, kind, sha = meta.split()
        if kind == "blob":
            entries.append((path, sha))
    previous = []
    for path in args.previous:
        old = json.loads(path.read_text())
        if old["source"] != SOURCE or old["commit"] != COMMIT:
            parser.error(f"previous selection has another source: {path}")
        previous.append(old)
    works = select(entries, previous)
    result = {
        "schema_version": 1, "source": SOURCE, "commit": COMMIT, "seed": SEED,
        "split": "local-key label holdout v2",
        "selected_on": "2026-11-28",
        "tree_listing_sha256": sha256(tree.encode()),
        "selection": "Ascending SHA-256(seed, group, work) among complete score.mxl/analysis.txt pairs, four per group: Beethoven string quartet movements from distinct quartets, Bach keyboard works, Mozart piano sonata movements from distinct sonatas, and OpenScore Lieder songs by distinct composers from distinct collections. Every work of the earlier When in Rome selections is excluded, and for quartets, sonatas and songs every parent collection too. Selection reads only the pinned Git tree listing.",
        "musical_input": "score.mxl with MusicXML harmony elements removed; no analysis_on_score.mxl or analysis_automatic.rntxt",
        "gold": "analysis.txt, unopened until the candidate, music21 baseline and scorer are frozen",
        "candidate": "estimate_local_key_path_with_aarden_essen_profile (analyze-local-keys --method path)",
        "stop_rule": "Score once. Do not tune against these labels. If they inform any algorithm change, reclassify all sixteen works as development and freeze a new disjoint holdout.",
        "prior_selection_sha256": {path.name: sha256(path.read_bytes()) for path in args.previous},
        "works": works,
    }
    args.output.write_text(json.dumps(result, ensure_ascii=False, indent=2) + "\n")
    print(f"froze {len(works)} works")
    for work in works:
        print(work["group"], work["work"])


if __name__ == "__main__":
    main()
