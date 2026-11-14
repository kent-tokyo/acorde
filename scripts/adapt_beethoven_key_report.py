#!/usr/bin/env python3
"""Project a saved Beethoven key-comparison report into the label evaluator.

This preserves all selected movements, including conversion and parse failures.
The published 2026-11-07 report is now development evidence because its results
have been inspected. This adapter assigns the development split accordingly.
"""

from __future__ import annotations

import argparse
import json
from pathlib import Path


def projection(report, work_dir=None):
    required = ("corpus", "archive_sha256", "music21_version", "acorde_binary_sha256",
                "mscore_version", "selection", "rows")
    if not isinstance(report, dict) or any(key not in report for key in required):
        raise ValueError("not a Beethoven key-comparison report")
    if not isinstance(report["rows"], list) or not report["rows"]:
        raise ValueError("report has no rows")
    cases = []
    for row in report["rows"]:
        piece, gold = row.get("piece"), row.get("gold")
        if not isinstance(piece, str) or not piece or not isinstance(gold, str) or not gold:
            raise ValueError("row needs piece and gold key")
        digest = row.get("analysis_xml_sha256") or row.get("source_sha256")
        if not isinstance(digest, str):
            raise ValueError(f"{piece}: missing input hash")
        case = {
            "id": piece,
            "input_sha256": digest,
            "split": "development",
            "gold": [{"task": "global-key", "at": "score", "labels": [gold]}],
            "systems": {},
        }
        if work_dir is not None and "analysis_xml_sha256" in row:
            case["input_path"] = str((work_dir / f"{piece}.analysis.musicxml").resolve())
        for system in ("acorde", "music21"):
            observation = row.get(system)
            if "export_error" in row:
                case["systems"][system] = {"error": "conversion: " + row["export_error"]}
            elif not isinstance(observation, dict):
                raise ValueError(f"{piece}: invalid {system} observation")
            elif "error" in observation:
                case["systems"][system] = {"error": observation["error"]}
            else:
                key = observation.get("key")
                case["systems"][system] = (
                    [{"task": "global-key", "at": "score", "label": key}] if key else []
                )
        cases.append(case)
    return {
        "schema_version": 1,
        "provenance": {
            "corpus": report["corpus"],
            "archive_sha256": report["archive_sha256"],
            "mscore_version": report["mscore_version"],
            "selection": report["selection"],
            "source_report_kind": "compare_beethoven_keys.py",
            "historical_holdout_reused_for_development": True,
        },
        "system_profiles": {
            "acorde": {
                "version": "binary-sha256:" + report["acorde_binary_sha256"],
                "command": "acorde analyze INPUT --aarden-essen-keys",
                "options": {"key_estimator": "aarden-essen"},
            },
            "music21": {
                "version": report["music21_version"],
                "command": "score.analyze('key')",
                "options": {"key_estimator": "aarden-essen"},
            },
        },
        "cases": cases,
    }


def main():
    parser = argparse.ArgumentParser(description=__doc__)
    parser.add_argument("report", type=Path)
    parser.add_argument("output", type=Path)
    parser.add_argument("--work-dir", type=Path,
                        help="retained converter output, for checking common input hashes")
    args = parser.parse_args()
    try:
        converted = projection(json.loads(args.report.read_text(encoding="utf-8")),
                               args.work_dir)
        args.output.write_text(json.dumps(converted, ensure_ascii=False, indent=2) + "\n",
                               encoding="utf-8")
    except (OSError, ValueError, TypeError) as error:
        parser.error(str(error))


if __name__ == "__main__":
    main()
