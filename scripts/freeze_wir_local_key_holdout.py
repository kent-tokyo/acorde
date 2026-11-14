#!/usr/bin/env python3
"""Select a disjoint local-key label holdout from a pinned Git file index only."""

import argparse
import hashlib
import json
from pathlib import Path


SOURCE = "https://github.com/MarkGotham/When-in-Rome"
GROUPS = (
    ("Corpus/Quartets/Beethoven,_Ludwig_van/", 6, 3, True),
    ("Corpus/Keyboard_Other/Bach,_Johann_Sebastian/", 9, 3, False),
    ("Corpus/Piano_Sonatas/Mozart,_Wolfgang_Amadeus/", 6, 3, True),
    ("Corpus/OpenScore-LiederCorpus/Reichardt,_Louise/", 0, 3, True),
)


def sha256(path):
    return hashlib.sha256(path.read_bytes()).hexdigest()


def select(tree, commit_record, previous):
    commit = commit_record["sha"]
    if tree.get("truncated") or tree.get("sha") != commit:
        raise ValueError("tree must be complete and rooted at the pinned commit")
    previous_works = set()
    for old in previous:
        if old["commit"] != commit or old["source"] != SOURCE:
            raise ValueError("previous selection has a different source commit")
        previous_works.update(work["work"] for work in old["works"])
    paths = {entry["path"]: entry for entry in tree["tree"] if entry["type"] == "blob"}
    works = []
    for prefix, skip, count, distinct_collection in GROUPS:
        candidates = sorted(
            path.removesuffix("/score.mxl")
            for path in paths
            if path.startswith(prefix) and path.endswith("/score.mxl")
            and path.removesuffix("score.mxl") + "analysis.txt" in paths
        )
        if distinct_collection:
            seen = set()
            unique = []
            for work in candidates:
                collection = work[len(prefix):].split("/", 1)[0]
                if collection not in seen:
                    seen.add(collection)
                    unique.append(work)
            candidates = unique
        if len(candidates) < skip + count:
            raise ValueError(f"not enough score/analysis pairs under {prefix}")
        for work in candidates[skip:skip + count]:
            if work in previous_works:
                raise ValueError(f"prior work leaked into local-key holdout: {work}")
            score = work + "/score.mxl"
            gold = work + "/analysis.txt"
            works.append({"work": work,
                          "score_path": score, "score_git_blob": paths[score]["sha"],
                          "gold_path": gold, "gold_git_blob": paths[gold]["sha"]})
    if len(works) != 12 or len({work["work"] for work in works}) != 12:
        raise ValueError("local-key selection must contain 12 distinct works")
    return {"schema_version": 1, "source": SOURCE, "commit": commit,
            "tree_sha": commit_record["commit"]["tree"]["sha"],
            "split": "local-key label holdout",
            "selection": "Three lexicographic score.mxl/analysis.txt pairs in each fixed group after the recorded skip: distinct Beethoven Op.59 quartets, Bach WTC I 10-12, distinct Mozart K309-311 sonatas, and distinct Louise Reichardt song collections. Selection reads only the pinned complete Git tree index and prior work names.",
            "prior_score_exposure": "Some Mozart sonata scores were used in a separate global-key task; this is a local-key label holdout, not an unseen-score claim.",
            "musical_input": "score.mxl only; no analysis_on_score.mxl or analysis_automatic.rntxt",
            "gold": "analysis.txt, unopened until candidate and scoring protocol are frozen",
            "unit": "work directory; no work overlaps the three prior When in Rome selections",
            "task": "local-key label at aligned human RomanText positions",
            "stop_rule": "Do not tune against these twelve local-key labels. If they inform an algorithm change, reclassify all twelve works as development and freeze a new disjoint label holdout.",
            "works": works}


def main():
    parser = argparse.ArgumentParser(description=__doc__)
    parser.add_argument("tree", type=Path)
    parser.add_argument("commit_record", type=Path)
    parser.add_argument("output", type=Path)
    parser.add_argument("--previous", type=Path, action="append", required=True)
    args = parser.parse_args()
    if len(args.previous) != 3:
        parser.error("provide the three earlier selection files")
    previous = [json.loads(path.read_text()) for path in args.previous]
    result = select(json.loads(args.tree.read_text()),
                    json.loads(args.commit_record.read_text()), previous)
    result["prior_selection_sha256"] = [sha256(path) for path in args.previous]
    args.output.write_text(json.dumps(result, ensure_ascii=False, indent=2) + "\n")
    print(f"froze {len(result['works'])} local-key label-holdout works at {result['commit']}")


if __name__ == "__main__":
    main()
