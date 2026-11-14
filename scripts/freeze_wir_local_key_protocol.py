#!/usr/bin/env python3
"""Pin the local-key holdout evaluator and candidate before opening labels."""

import argparse
import json
from pathlib import Path

import music21

from diagnose_wir_local_keys import digest
from score_wir_local_key_holdout import SUPPORT_SCRIPTS


def freeze(selection_path, inputs_path, preflight_path, binary, scorer_path):
    selection = json.loads(selection_path.read_text())
    inputs = json.loads(inputs_path.read_text())
    preflight = json.loads(preflight_path.read_text())
    if music21.__version__ != "10.5.2":
        raise ValueError(f"requires music21 10.5.2, found {music21.__version__}")
    if (selection["split"] != "local-key label holdout"
            or selection["commit"] != inputs["source_commit"]
            or selection["commit"] != preflight["source_commit"]
            or digest(selection_path) != inputs["selection_sha256"]
            or preflight["selection_sha256"] != digest(selection_path)
            or preflight["inputs_sha256"] != digest(inputs_path)):
        raise ValueError("selection, inputs, or preflight drift")
    if (len(selection["works"]) != 12 or preflight["summary"]["parsed_both"] != 12
            or preflight["summary"]["unequal_timeline_measures"] != 0
            or not inputs["inputs"]):
        raise ValueError("incomplete score-only holdout preflight")
    if any("gold_sha256" in item for item in inputs["inputs"]):
        raise ValueError("gold content was read before protocol freeze")
    scripts_dir = scorer_path.parent
    return {"schema_version": 1,
            "classification": "preregistered local-key label holdout",
            "source_commit": selection["commit"],
            "works": 12,
            "acorde_radius_measures": 4,
            "music21_window_measures": 2,
            "primary_metric": "exact tonic spelling and major/minor mode at each aligned RomanText annotation position; unavailable predictions count as wrong",
            "secondary_metric": "tonic pitch class and mode; report coverage, exclusions, and per-work exact counts",
            "exclusions": ["duplicate gold labels at the same source beat",
                           "missing or ambiguous source measure number",
                           "Acorde/source measure-number mismatch",
                           "predeclared unequal pitched timeline measure"],
            "gate": {"required_works": 12,
                     "max_excluded_fraction": 0.05,
                     "min_acorde_work_wins": 7,
                     "require_no_lower_parse_and_prediction_coverage": True,
                     "require_more_exact_labels_on_paired_positions": True},
            "stop_rule": "No candidate, comparator, source-position, or threshold change after gold is read. A failure stays a failure; any subsequent tuning requires a new disjoint label holdout.",
            "gold_files_unread_at_freeze": True,
            "pinned": {"selection_sha256": digest(selection_path),
                       "inputs_sha256": digest(inputs_path),
                       "preflight_sha256": digest(preflight_path),
                       "acorde_binary_sha256": digest(binary),
                       "scorer_source_sha256": digest(scorer_path),
                       "support_script_sha256": {
                           name: digest(scripts_dir / name) for name in SUPPORT_SCRIPTS},
                       "music21_version": music21.__version__,
                       "music21_floating_key_source_sha256": digest(
                           Path(music21.analysis.floatingKey.__file__)),
                       "music21_discrete_source_sha256": digest(
                           Path(music21.analysis.discrete.__file__))}}


def main():
    parser = argparse.ArgumentParser(description=__doc__)
    parser.add_argument("selection", type=Path)
    parser.add_argument("inputs", type=Path)
    parser.add_argument("preflight", type=Path)
    parser.add_argument("acorde", type=Path)
    parser.add_argument("output", type=Path)
    args = parser.parse_args()
    result = freeze(args.selection, args.inputs, args.preflight,
                    args.acorde.resolve(), Path(__file__).with_name("score_wir_local_key_holdout.py"))
    args.output.write_text(json.dumps(result, ensure_ascii=False, indent=2) + "\n")
    print(f"froze {result['works']} works and both local-key candidates before gold read")


if __name__ == "__main__":
    main()
