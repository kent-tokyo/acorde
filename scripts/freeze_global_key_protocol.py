#!/usr/bin/env python3
"""Pin the global-key scorer and both top-choice candidates before labels."""

import argparse
import hashlib
import json
from pathlib import Path

import lxml
import music21

from score_global_key_holdout import SUPPORT_SCRIPTS, INDEX_SHA256


def sha256(path):
    return hashlib.sha256(path.read_bytes()).hexdigest()


def freeze(selection_path, inputs_path, preflight_path, binary, scorer):
    selection = json.loads(selection_path.read_text())
    inputs = json.loads(inputs_path.read_text())
    preflight = json.loads(preflight_path.read_text())
    if music21.__version__ != "10.5.2" or lxml.__version__ != "6.0.2":
        raise ValueError("comparator dependency version differs")
    if (selection["classification"] != "global-key label holdout selection"
            or inputs["selection_sha256"] != sha256(selection_path)
            or preflight["selection_sha256"] != sha256(selection_path)
            or preflight["inputs_sha256"] != sha256(inputs_path)):
        raise ValueError("selection/input/preflight chain differs")
    if (len(selection["works"]) != 12
            or preflight["summary"]["parsed_both"] != 12
            or preflight["summary"]["unequal_timeline_measures"] != 0
            or any("error" in row for row in inputs["inputs"])):
        raise ValueError("score-only preflight is incomplete")
    if not selection["label_values_unread_at_freeze"]:
        raise ValueError("selection was not frozen without labels")
    scripts = scorer.parent
    return {
        "schema_version": 1,
        "classification": "preregistered three-style global-key holdout",
        "works": 12,
        "styles": ["chorale", "keyboard", "chamber"],
        "acorde_method": "first key_estimates candidate from analyze --aarden-essen-keys",
        "music21_method": "music21 10.5.2 score.analyze('key')",
        "primary_metric": "exact tonic spelling and major/minor mode, one vote per selected score; unavailable top choice counts as wrong",
        "secondary_metric": "tonic pitch class plus mode; report each style and prediction coverage",
        "chorale_reference_rule": ("Use the Riemenschneider-numbered key-index tonic and mode "
                                   "only when the row is unique, unstarred, and unambiguous "
                                   "and the pinned human RomanText opening key agrees. "
                                   "A starred row signals possible collection transposition."),
        "reference_exclusions": [
            "nonunique chorale index row", "chorale collection transposition flag",
            "ambiguous or unsupported chorale index key",
            "missing chorale opening key or reference disagreement",
            "missing or unsupported annotated_key metadata",
        ],
        "gate": {"required_scores": 12, "max_excluded_scores": 1,
                 "min_eligible_per_group": 3,
                 "require_no_lower_acorde_prediction_coverage": True,
                 "require_more_acorde_exact_on_paired_scores": True,
                 "require_no_worse_acorde_exact_in_each_group": True},
        "stop_rule": ("No candidate, comparator, label projection, exclusion, "
                      "or threshold change after selected labels are read. "
                      "A failure remains archived; tuning requires a new "
                      "disjoint label holdout."),
        "selected_label_values_unread_at_freeze": True,
        "pinned": {
            "selection_sha256": sha256(selection_path),
            "inputs_sha256": sha256(inputs_path),
            "preflight_sha256": sha256(preflight_path),
            "acorde_binary_sha256": sha256(binary),
            "scorer_source_sha256": sha256(scorer),
            "support_script_sha256": {
                name: sha256(scripts / name) for name in SUPPORT_SCRIPTS},
            "music21_version": music21.__version__,
            "music21_discrete_source_sha256": sha256(
                Path(music21.analysis.discrete.__file__)),
            "lxml_version": lxml.__version__,
            "chorale_key_index_sha256": INDEX_SHA256,
            "cpe_metadata_git_blob": selection["works"][4]["metadata_git_blob"],
            "pleyel_metadata_git_blob": selection["works"][8]["metadata_git_blob"],
        },
    }


def main():
    parser = argparse.ArgumentParser(description=__doc__)
    parser.add_argument("selection", type=Path)
    parser.add_argument("inputs", type=Path)
    parser.add_argument("preflight", type=Path)
    parser.add_argument("acorde", type=Path)
    parser.add_argument("output", type=Path)
    args = parser.parse_args()
    result = freeze(args.selection, args.inputs, args.preflight,
                    args.acorde.resolve(),
                    Path(__file__).with_name("score_global_key_holdout.py"))
    args.output.write_text(json.dumps(result, ensure_ascii=False, indent=2) + "\n")
    print("froze three-style global-key protocol before selected labels")


if __name__ == "__main__":
    main()
