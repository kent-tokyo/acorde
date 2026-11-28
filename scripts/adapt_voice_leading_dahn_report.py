#!/usr/bin/env python3
"""Project compare_voice_leading_dahn.py reports into the common label evaluator (#502).

Tasks are `consecutive_fifth` and `consecutive_octave`, each with the single
label "present". A position is the voice pair and the label's first listed
measure, encoded as {"part": upper voice, "voice": lower voice, "measure": m,
"beat": "0"} with voices numbered S=0, A=1, T=2, B=3. Predictions not covering
a gold label keep their arrival measure and count as extras; the source list
is not exhaustive, so precision is a lower bound.
"""

import argparse
import hashlib
import json
from pathlib import Path

TOOLS = ("acorde", "music21")
VOICES = "SATB"


def sha256(path):
    return hashlib.sha256(path.read_bytes()).hexdigest()


def at(upper, lower, measure):
    return {"part": VOICES.index(upper), "staff": 0, "measure": measure, "beat": "0",
            "voice": VOICES.index(lower)}


def adapt(paths):
    cases, provenance, profiles = [], {}, None
    for path in paths:
        report = json.loads(path.read_text())
        provenance[report["split"]] = sha256(path)
        profile = {"acorde": {"version": report["acorde_binary_sha256"],
                              "command": "acorde " + report["acorde_command"], "options": {}},
                   "music21": {"version": report["music21_version"], "command": report["recipe"],
                               "options": {}}}
        if profiles not in (None, profile):
            raise ValueError("reports use different tool versions")
        profiles = profile
        for case in report["cases"]:
            gold, seen = [], set()
            for label in case["labels"]:
                key = (label["interval"], label["upper"], label["lower"], label["measures"][0])
                if key in seen:
                    raise ValueError(f"duplicate gold position: {case['bwv']} {key}")
                seen.add(key)
                gold.append({"task": f"consecutive_{label['interval']}",
                             "at": at(*key[1:]), "labels": ["present"]})
            systems = {}
            for tool in TOOLS:
                if f"{tool}_error" in case:
                    systems[tool] = {"error": case[f"{tool}_error"]}
                    continue
                systems[tool] = [{"task": f"consecutive_{p['interval']}",
                                  "at": at(p["upper"], p["lower"], p["measure"]), "label": "present"}
                                 for p in case[tool]["positions"]]
            cases.append({"id": f"bwv{case['bwv']}", "input_sha256": case["input_sha256"],
                          "split": report["split"], "gold": gold, "systems": systems})
    return {"schema_version": 1,
            "provenance": {"tasks": ["consecutive_fifth", "consecutive_octave"],
                           "reports_sha256": provenance,
                           "position_encoding": "part = upper voice, voice = lower voice (S=0 A=1 T=2 B=3), measure = label's first listed measure number, beat 0",
                           "precision_note": "Gold lists are not exhaustive; extras are unlisted detections, so precision is a lower bound."},
            "system_profiles": profiles, "cases": cases}


def main():
    parser = argparse.ArgumentParser(description=__doc__)
    parser.add_argument("output", type=Path)
    parser.add_argument("reports", type=Path, nargs="+")
    args = parser.parse_args()
    result = adapt(args.reports)
    args.output.write_text(json.dumps(result, ensure_ascii=False, indent=2) + "\n")
    print(f"projected {len(result['cases'])} cases")


if __name__ == "__main__":
    main()
