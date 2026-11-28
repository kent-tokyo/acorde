#!/usr/bin/env python3
"""Pin the second local-key holdout's candidate, comparator and gate before opening labels."""

import argparse
import json
from pathlib import Path

import music21

from diagnose_wir_local_keys import digest
from score_wir_local_key_holdout_v2 import SUPPORT_SCRIPTS


def freeze(selection_path, inputs_path, preflight_path, binary, scripts_dir, analysis_source):
    selection = json.loads(selection_path.read_text())
    inputs = json.loads(inputs_path.read_text())
    preflight = json.loads(preflight_path.read_text())
    works = len(selection["works"])
    if music21.__version__ != "10.5.2":
        raise ValueError(f"requires music21 10.5.2, found {music21.__version__}")
    if (selection["split"] != "local-key label holdout v2"
            or selection["commit"] != inputs["source_commit"]
            or selection["commit"] != preflight["source_commit"]
            or digest(selection_path) != inputs["selection_sha256"]
            or preflight["selection_sha256"] != digest(selection_path)
            or preflight["inputs_sha256"] != digest(inputs_path)
            or preflight["acorde_binary_sha256"] != digest(binary)):
        raise ValueError("selection, inputs, preflight, or binary drift")
    if (works != 16 or preflight["summary"]["parsed_both"] != works
            or preflight["summary"]["unequal_timeline_measures"] != 0):
        raise ValueError("incomplete score-only holdout preflight")
    if any("gold_sha256" in item for item in inputs["inputs"]):
        raise ValueError("gold content was read before protocol freeze")
    scorer = scripts_dir / "score_wir_local_key_holdout_v2.py"
    return {"schema_version": 1,
            "classification": "preregistered local-key label holdout v2",
            "issue": "https://github.com/kent-tokyo/acorde/issues/501",
            "source_commit": selection["commit"],
            "works": works,
            "acorde_method": "path",
            "acorde_parameters": {"radius_measures": 1, "change_cost": 1.5, "fifth_cost": 0.1,
                                  "chosen_on": "48 inspected When in Rome development works (the three harmony cohorts and the failed first local-key holdout)"},
            "music21_window_measures": 2,
            "music21_window_reason": "the first local-key holdout's preregistered comparator window, unchanged",
            "primary_metric": "exact tonic spelling and major/minor mode at each aligned RomanText annotation position, on positions where both tools give an estimate",
            "secondary_metric": "all-position exact labels with unavailable predictions counted wrong; tonic pitch class and mode; coverage, exclusions, per-group and per-work exact counts and work wins (reported, not gated)",
            "exclusions": ["duplicate gold labels at the same source beat",
                           "missing or ambiguous source measure number",
                           "Acorde/source measure-number mismatch",
                           "predeclared unequal pitched timeline measure"],
            "gate": {"required_works": works,
                     "max_excluded_fraction": 0.05,
                     "require_no_lower_prediction_coverage": True,
                     "require_at_least_as_many_exact_labels_on_paired_positions": True},
            "gate_source": "issue #501 acceptance: exact-label count at least music21's, with no lower parse coverage",
            "stop_rule": "Score once. No candidate, comparator, source-position, or threshold change after gold is read. A failure stays a failure; any later tuning requires a new disjoint holdout.",
            "gold_files_unread_at_freeze": True,
            "pinned": {"selection_sha256": digest(selection_path),
                       "inputs_sha256": digest(inputs_path),
                       "preflight_sha256": digest(preflight_path),
                       "acorde_binary_sha256": digest(binary),
                       "acorde_analysis_source_sha256": digest(analysis_source),
                       "scorer_source_sha256": digest(scorer),
                       "support_script_sha256": {
                           name: digest(scripts_dir / name) for name in SUPPORT_SCRIPTS},
                       "music21_version": music21.__version__,
                       "music21_floating_key_source_sha256": digest(
                           Path(music21.analysis.floatingKey.__file__)),
                       "music21_discrete_source_sha256": digest(
                           Path(music21.analysis.discrete.__file__))}}


def main():
    parser = argparse.ArgumentParser(description=__doc__)
    for name in ("selection", "inputs", "preflight", "acorde", "output"):
        parser.add_argument(name, type=Path)
    args = parser.parse_args()
    if args.output.exists():
        parser.error("refusing to overwrite a frozen protocol")
    scripts_dir = Path(__file__).resolve().parent
    result = freeze(args.selection, args.inputs, args.preflight, args.acorde.resolve(),
                    scripts_dir, scripts_dir.parent / "crates/analysis/src/lib.rs")
    args.output.write_text(json.dumps(result, ensure_ascii=False, indent=2) + "\n")
    print(f"froze {result['works']} works before any gold read")


if __name__ == "__main__":
    main()
