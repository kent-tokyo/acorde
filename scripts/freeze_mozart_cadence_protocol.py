#!/usr/bin/env python3
"""Pin the Mozart cadence comparison before the reserved holdout CSVs are read (#502)."""

import argparse
import hashlib
import json
from pathlib import Path

import music21

PINNED_SCRIPTS = ("compare_mozart_cadence_tools.py", "adapt_mozart_cadence_report.py",
                  "project_mozart_cadence_holdout.py", "mozart_cadence_positions.py",
                  "score_mozart_cadence_acorde_development.py", "compare_wir_harmony_music21.py",
                  "evaluate_analysis_labels.py")


def sha256(path):
    return hashlib.sha256(path.read_bytes()).hexdigest()


def main():
    parser = argparse.ArgumentParser(description=__doc__)
    for name in ("selection", "inputs", "preflight", "development_report", "acorde", "output"):
        parser.add_argument(name, type=Path)
    args = parser.parse_args()
    if args.output.exists():
        parser.error("refusing to overwrite a frozen protocol")
    if music21.__version__ != "10.5.2":
        parser.error("requires music21 10.5.2")
    selection = json.loads(args.selection.read_text())
    preflight = json.loads(args.preflight.read_text())
    development = json.loads(args.development_report.read_text())
    binary = args.acorde.resolve()
    scripts = Path(__file__).resolve().parent
    if (preflight["selection_sha256"] != sha256(args.selection)
            or preflight["prepared_inputs_sha256"] != sha256(args.inputs)
            or development["split"] != "development"
            or development["acorde_binary_sha256"] != sha256(binary)
            or development["script_sha256"] != sha256(scripts / "compare_mozart_cadence_tools.py")):
        parser.error("selection, inputs, development report, binary, or comparator drift")
    holdout = [case for case in selection["cases"] if case["split"] != "development"]
    result = {
        "schema_version": 1,
        "classification": "preregistered Mozart cadence label holdout",
        "issue": "https://github.com/kent-tokyo/acorde/issues/502",
        "source_commit": selection["commit"],
        "holdout_cases": [case["id"] for case in holdout],
        "task": "coarse cadence class at part-0 note onsets of the projected cadence CSV",
        "class_mapping": development["class_mapping"],
        "acorde_command": development["acorde_command"],
        "music21_recipe": development["recipe"],
        "metrics": "precision, recall and F1 per tool from evaluate_analysis_labels.py, development and holdout reported separately; per-class counts and parse failures reported",
        "gate": None,
        "gate_reason": "issue #502 asks for measured precision/recall/F1 for both tools, not a superiority claim",
        "stop_rule": "Score the reserved split once. No change to either tool, the recipe, positions, exclusions or class mapping after its CSVs are read. Afterwards these 18 movements are development data.",
        "holdout_annotations_unread_at_freeze": True,
        "development_summary": development["summary"],
        "pinned": {"selection_sha256": sha256(args.selection),
                   "inputs_sha256": sha256(args.inputs),
                   "preflight_sha256": sha256(args.preflight),
                   "development_report_sha256": sha256(args.development_report),
                   "acorde_binary_sha256": sha256(binary),
                   "script_sha256": {name: sha256(scripts / name) for name in PINNED_SCRIPTS},
                   "music21_version": music21.__version__,
                   "music21_roman_source_sha256": sha256(Path(music21.roman.__file__)),
                   "music21_discrete_source_sha256": sha256(Path(music21.analysis.discrete.__file__))},
    }
    args.output.write_text(json.dumps(result, ensure_ascii=False, indent=2) + "\n")
    print(f"froze protocol for {len(holdout)} reserved movements")


if __name__ == "__main__":
    main()
