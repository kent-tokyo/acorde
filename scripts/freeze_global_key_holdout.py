#!/usr/bin/env python3
"""Freeze score paths for a three-style global-key holdout without reading labels.

The DCML metadata files are used only for piece IDs and score paths. Their
``annotated_key`` values and the Bach key-index rows must remain unopened until
the common score inputs and scoring protocol have been fixed.
"""

import argparse
import csv
import hashlib
import json
import subprocess
from pathlib import Path


WIR_COMMIT = "1c61fe41b8c2910296d7d2bcbf6476c7c1f2fe35"
CHORALE_COMMIT = "50fd22c7c69f6b70c5baa609758b640ad95982e1"
CPE_COMMIT = "4b3511eab12b5fbfc7aa5c75819ee02225430da8"
PLEYEL_COMMIT = "8b3d7f5e966290631571274067de6d3a9206a737"
CPE_METADATA_BLOB = "1c4eb31d4ae087ac0cc1087ae00213019686eaab"
PLEYEL_METADATA_BLOB = "2b28f963816fbbc8d528086c710c78178610e907"
CHORALE_KEY_INDEX_SHA256 = "d8849022d85f59ade61d84e911fd439fe8a42ad920c9a7748129c08863487d5e"

# Source blobs were checked at the pinned commits, before reading any label.
CHORALE_SCORES = {
    "200": "3f4642907e11ca621abb28834a4fcc5a4197baf0",
    "201": "309dbab493b0a6e869e6c0e48038cc1d2391a82e",
    "202": "20a393a8c28e869185cfeb1d27c5d654846e3c1c",
    "203": "6c9df7e7484741c525ca12a4ae14e04ad0136bcd",
}
CPE_SCORES = {
    "wq112n02": "c89ec42e816ccdd444823a3cf0e54373a23acba3",
    "wq112n08": "21d2f6144fc39c75264149ed0fa8391134371195",
    "wq112n15": "4ed742988d8122cdd2a30d70a3b7f7558ce4f0b4",
    "wq113n03": "01d66612bbde4a74997e1ea13e818678b12bf13b",
}
PLEYEL_SCORES = {
    "b307op2n1b": "340999b6eef65d4e14c37a51c051f19a225c7585",
    "b307op2n1c": "16b9e125f8d3e1ca4bd6f44e7dd730bffe519c7b",
    "b309op2n3a": "9eabf0a14a12292e03f1bfbb68501015efb243d8",
    "b309op2n3c": "fae1bf7efe9b0d2735af5ce1eb83be9f5f65f6c2",
}


def sha256(path):
    return hashlib.sha256(path.read_bytes()).hexdigest()


def git_blob(path):
    result = subprocess.run(["git", "hash-object", str(path)], capture_output=True,
                            text=True, check=True)
    return result.stdout.strip()


def metadata_paths(path, expected_blob):
    if git_blob(path) != expected_blob:
        raise ValueError(f"metadata blob differs: {path}")
    with path.open(newline="") as stream:
        rows = list(csv.DictReader(stream, delimiter="\t"))
    if not rows or not {"piece", "rel_path", "annotated_key"}.issubset(rows[0]):
        raise ValueError(f"metadata columns differ: {path}")
    # Do not inspect annotated_key values, even for eligibility.
    result = {row["piece"]: row["rel_path"] for row in rows}
    if len(result) != len(rows):
        raise ValueError(f"duplicate metadata piece: {path}")
    return result


def select(tree, cpe_paths, pleyel_paths):
    if tree.get("truncated") or tree.get("sha") != WIR_COMMIT:
        raise ValueError("When in Rome tree must be complete and pinned")
    blobs = {item["path"]: item["sha"] for item in tree["tree"]
             if item["type"] == "blob"}
    works = []
    for number, score_blob in CHORALE_SCORES.items():
        prefix = ("Corpus/Early_Choral/Bach,_Johann_Sebastian/"
                  f"Chorales/{number}/")
        analysis_path = prefix + "analysis.txt"
        remote_path = prefix + "remote.json"
        if analysis_path not in blobs or remote_path not in blobs:
            raise ValueError(f"missing chorale source index: {number}")
        works.append({
            "group": "chorale", "piece": number,
            "score_repo": "MarkGotham/Chorale-Corpus",
            "score_commit": CHORALE_COMMIT,
            "score_path": ("Bach,_Johann_Sebastian/Chorales/"
                           f"{number}/short_score.mxl"),
            "score_git_blob": score_blob,
            "gold_source": "bach-chorales.com ChoraleKeyIndex.htm, Riemenschneider column",
            "corroboration_repo": "MarkGotham/When-in-Rome",
            "corroboration_commit": WIR_COMMIT,
            "corroboration_path": analysis_path,
            "corroboration_git_blob": blobs[analysis_path],
            "remote_metadata_path": remote_path,
            "remote_metadata_git_blob": blobs[remote_path],
        })
    for group, paths, selected, repo, commit, metadata_blob in (
        ("keyboard", cpe_paths, CPE_SCORES, "DCMLab/cpe_bach_keyboard",
         CPE_COMMIT, CPE_METADATA_BLOB),
        ("chamber", pleyel_paths, PLEYEL_SCORES, "DCMLab/pleyel_quartets",
         PLEYEL_COMMIT, PLEYEL_METADATA_BLOB),
    ):
        for piece, score_blob in selected.items():
            expected_path = f"MS3/{piece}.mscx"
            if paths.get(piece) != expected_path:
                raise ValueError(f"missing or unexpected {group} path: {piece}")
            works.append({
                "group": group, "piece": piece, "score_repo": repo,
                "score_commit": commit, "score_path": expected_path,
                "score_git_blob": score_blob,
                "gold_source": "annotated_key in metadata.tsv",
                "metadata_git_blob": metadata_blob,
            })
    if len(works) != 12 or len({(w["group"], w["piece"]) for w in works}) != 12:
        raise ValueError("expected twelve distinct works")
    return {
        "schema_version": 1, "classification": "global-key label holdout selection",
        "task": "one tonic spelling and major/minor mode per score",
        "selection": ("Bach chorales 200-203, beyond the inspected BCMH 001-101; "
                      "four C. P. E. Bach keyboard pieces and two movements "
                      "from each of two Pleyel quartets. IDs and score paths "
                      "were fixed without consulting label values or predictions."),
        "chorale_key_index_url": "https://bach-chorales.com/ChoraleKeyIndex.htm",
        "chorale_key_index_sha256": CHORALE_KEY_INDEX_SHA256,
        "metadata_repo_paths": {"keyboard": "metadata.tsv", "chamber": "metadata.tsv"},
        "label_values_unread_at_freeze": True,
        "score_inputs_unprepared_at_freeze": True,
        "stop_rule": ("Freeze score preparation, label projection, comparator, "
                      "eligibility and success thresholds before reading selected "
                      "key labels; after any tuning, use new disjoint works."),
        "works": works,
    }


def main():
    parser = argparse.ArgumentParser(description=__doc__)
    parser.add_argument("tree", type=Path)
    parser.add_argument("cpe_metadata", type=Path)
    parser.add_argument("pleyel_metadata", type=Path)
    parser.add_argument("chorale_key_index", type=Path)
    parser.add_argument("output", type=Path)
    args = parser.parse_args()
    if sha256(args.chorale_key_index) != CHORALE_KEY_INDEX_SHA256:
        parser.error("chorale key index SHA-256 differs")
    result = select(json.loads(args.tree.read_text()),
                    metadata_paths(args.cpe_metadata, CPE_METADATA_BLOB),
                    metadata_paths(args.pleyel_metadata, PLEYEL_METADATA_BLOB))
    args.output.write_text(json.dumps(result, ensure_ascii=False, indent=2) + "\n")
    print(f"selected {len(result['works'])} works without inspecting key labels")


if __name__ == "__main__":
    main()
