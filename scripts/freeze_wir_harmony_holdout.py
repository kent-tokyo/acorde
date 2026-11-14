#!/usr/bin/env python3
"""Freeze a score-first When in Rome harmony holdout from a pinned Git tree.

This script only reads the repository file index. It never reads analyses or
analyzer predictions, so selection cannot depend on their content.
"""

import argparse
import json
from pathlib import Path


GROUPS = (
    ("Corpus/Keyboard_Other/Bach,_Johann_Sebastian/", 3),
    ("Corpus/Keyboard_Other/Chopin,_Frédéric/", 3),
    ("Corpus/OpenScore-LiederCorpus/Brahms,_Johannes/", 3),
    ("Corpus/OpenScore-LiederCorpus/Schubert,_Franz/", 3),
)

SECOND_GROUPS = (
    ("Corpus/Quartets/Beethoven,_Ludwig_van/", 0, 3, True),
    ("Corpus/Keyboard_Other/Bach,_Johann_Sebastian/", 3, 3, False),
    ("Corpus/OpenScore-LiederCorpus/Hensel,_Fanny_(Mendelssohn)/", 0, 3, True),
    ("Corpus/OpenScore-LiederCorpus/Schumann,_Clara/", 0, 3, True),
)

THIRD_GROUPS = (
    ("Corpus/Quartets/Beethoven,_Ludwig_van/", 3, 3, True),
    ("Corpus/Keyboard_Other/Bach,_Johann_Sebastian/", 6, 3, False),
    ("Corpus/OpenScore-LiederCorpus/Schumann,_Robert/", 0, 3, True),
    ("Corpus/OpenScore-LiederCorpus/Lang,_Josephine/", 0, 3, True),
)


def freeze(tree, commit_record, cohort=1):
    commit = commit_record["sha"]
    if tree.get("truncated") or tree.get("sha") != commit:
        raise ValueError("tree must be complete and rooted at the pinned commit")
    paths = {entry["path"]: entry for entry in tree["tree"] if entry["type"] == "blob"}
    works = []
    groups = {
        1: ((prefix, 0, count, False) for prefix, count in GROUPS),
        2: SECOND_GROUPS,
        3: THIRD_GROUPS,
    }[cohort]
    for prefix, skip, count, distinct_collection in groups:
        candidates = sorted(
            path.removesuffix("/score.mxl")
            for path in paths
            if path.startswith(prefix) and path.endswith("/score.mxl")
            and path.removesuffix("score.mxl") + "analysis.txt" in paths
        )
        if distinct_collection:
            seen_collections = set()
            distinct = []
            for work in candidates:
                collection = work[len(prefix):].split("/", 1)[0]
                if collection not in seen_collections:
                    seen_collections.add(collection)
                    distinct.append(work)
            candidates = distinct
        if len(candidates) < skip + count:
            raise ValueError(f"not enough score/analysis pairs under {prefix}")
        for work in candidates[skip:skip + count]:
            score = work + "/score.mxl"
            gold = work + "/analysis.txt"
            works.append({
                "work": work,
                "score_path": score,
                "score_git_blob": paths[score]["sha"],
                "gold_path": gold,
                "gold_git_blob": paths[gold]["sha"],
            })
    selection = {
        "schema_version": 1,
        "source": "https://github.com/MarkGotham/When-in-Rome",
        "commit": commit,
        "tree_sha": commit_record["commit"]["tree"]["sha"],
        "split": "holdout",
        "selection": {
            1: "First three lexicographic work directories per fixed composer prefix with score.mxl and analysis.txt, chosen from the pinned complete Git tree without reading file contents or predictions.",
            2: "Three lexicographic score.mxl/analysis.txt pairs at each fixed composer prefix; one per distinct collection for Beethoven/Hensel/Clara Schumann, and Bach pairs 04-06 after skipping the three inspected ones. Selected without reading contents or predictions.",
            3: "Three lexicographic score.mxl/analysis.txt pairs at each fixed composer prefix; one per distinct collection for Beethoven/Robert Schumann/Josephine Lang, skipping the first three inspected Beethoven collections, and Bach pairs 07-09 after skipping the six inspected ones. Selected without reading contents or predictions.",
        }[cohort],
        "musical_input": "score.mxl only; never analysis_on_score.mxl or analysis_automatic.rntxt",
        "gold": "analysis.txt only; never automatic or alternate analyses",
        "unit": "work directory; no movement or transposition leaks between splits",
        "task": "Roman numeral at human-annotated positions; score only at source positions; report all parser/alignment exclusions",
        "normalization": "Freeze before evaluating: Roman glyph (%/ø, °/o), inversion 42/2, and whitespace only; preserve root, quality, applied chord, key, and chromatic spelling distinctions.",
        "stop_rule": "Do not tune against these works. If a result informs a fix, reclassify all twelve works as development and freeze a new untouched holdout before a superiority claim.",
        "works": works,
    }
    if cohort > 1:
        selection["cohort"] = cohort
    return selection


def main():
    parser = argparse.ArgumentParser(description=__doc__)
    parser.add_argument("tree", type=Path)
    parser.add_argument("commit_record", type=Path)
    parser.add_argument("output", type=Path)
    parser.add_argument("--cohort", type=int, choices=(1, 2, 3), default=1)
    args = parser.parse_args()
    result = freeze(json.loads(args.tree.read_text()), json.loads(args.commit_record.read_text()), args.cohort)
    args.output.write_text(json.dumps(result, ensure_ascii=False, indent=2) + "\n")
    print(f"froze {len(result['works'])} works at {result['commit']}")


if __name__ == "__main__":
    main()
