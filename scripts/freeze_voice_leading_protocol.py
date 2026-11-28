#!/usr/bin/env python3
"""Pin the Bach chorale fifth/octave comparison before the holdout chorales are run (#502)."""

import argparse
import hashlib
import json
from pathlib import Path

import music21
from music21.figuredBass import checker

PINNED_SCRIPTS = ("freeze_voice_leading_dahn_labels.py", "compare_voice_leading_dahn.py",
                  "adapt_voice_leading_dahn_report.py", "evaluate_analysis_labels.py")


def sha256(path):
    return hashlib.sha256(path.read_bytes()).hexdigest()


def main():
    parser = argparse.ArgumentParser(description=__doc__)
    for name in ("labels", "development_report", "acorde", "output"):
        parser.add_argument(name, type=Path)
    args = parser.parse_args()
    if args.output.exists():
        parser.error("refusing to overwrite a frozen protocol")
    if music21.__version__ != "10.5.2":
        parser.error("requires music21 10.5.2")
    labels = json.loads(args.labels.read_text())
    development = json.loads(args.development_report.read_text())
    binary = args.acorde.resolve()
    scripts = Path(__file__).resolve().parent
    if (development["split"] != "development"
            or development["labels_sha256"] != sha256(args.labels)
            or development["acorde_binary_sha256"] != sha256(binary)
            or development["script_sha256"] != sha256(scripts / "compare_voice_leading_dahn.py")):
        parser.error("labels, development report, binary, or comparator drift")
    holdout = [c["bwv"] for c in labels["chorales"] if c["split"] == "holdout"]
    exposed = {c["bwv"]: c["exposed_before_freeze"] for c in labels["chorales"]
               if "exposed_before_freeze" in c}
    result = {
        "schema_version": 1,
        "classification": "preregistered Bach chorale consecutive fifth/octave label holdout",
        "issue": "https://github.com/kent-tokyo/acorde/issues/502",
        "labels_source": labels["source"],
        "holdout_chorales": holdout,
        "moved_to_development_before_freeze": exposed,
        "task": "consecutive fifths and octaves (parallel or antiparallel) per voice pair, matched when interval and voices agree and either state's measure number is listed",
        "acorde_command": development["acorde_command"],
        "music21_recipe": development["recipe"],
        "metrics": "recall and F1 per tool and interval from evaluate_analysis_labels.py, development and holdout separately; precision is a lower bound because the source list is not exhaustive; unlisted detections and failures reported",
        "gate": None,
        "gate_reason": "issue #502 asks for measured precision/recall/F1 for both tools, not a superiority claim",
        "stop_rule": "Run the holdout chorales once. No change to either tool, the recipe, the matching rule or the labels afterwards. Afterwards these chorales are development data.",
        "holdout_tool_output_unseen_at_freeze": True,
        "development_summary": development["summary"],
        "pinned": {"labels_sha256": sha256(args.labels),
                   "development_report_sha256": sha256(args.development_report),
                   "acorde_binary_sha256": sha256(binary),
                   "script_sha256": {name: sha256(scripts / name) for name in PINNED_SCRIPTS},
                   "music21_version": music21.__version__,
                   "music21_checker_source_sha256": sha256(Path(checker.__file__)),
                   "music21_voiceLeading_source_sha256": sha256(Path(music21.voiceLeading.__file__))},
    }
    args.output.write_text(json.dumps(result, ensure_ascii=False, indent=2) + "\n")
    print(f"froze protocol for {len(holdout)} holdout chorales")


if __name__ == "__main__":
    main()
