#!/usr/bin/env python3
"""Compact an evaluate_print_holdout.py evaluation.json for the archive.

Usage: compact_print_holdout_evaluation.py EVALUATION OUTPUT
Candidate lists and per-staff samples are replaced by their counts; every other
field is copied. The full file's sha256 is recorded so the compact copy can be
traced to the original kept outside the repository.
"""
import hashlib
import json
import pathlib
import sys


def compact(engine):
    out = {}
    for key, value in engine.items():
        if isinstance(value, list) and (key.endswith("_candidates") or key.endswith("_samples_mm")):
            out[key + "_count"] = len(value)
        else:
            out[key] = value
    return out


def main():
    source, output = map(pathlib.Path, sys.argv[1:])
    rows = json.loads(source.read_text())
    for row in rows:
        for engine in ("acorde", "verovio"):
            if isinstance(row.get(engine), dict):
                row[engine] = compact(row[engine])
    report = {"classification": "compact copy of the v3 print holdout evaluation (#499)",
              "full_evaluation_sha256": hashlib.sha256(source.read_bytes()).hexdigest(),
              "full_evaluation_bytes": source.stat().st_size, "rows": rows}
    output.write_text(json.dumps(report, indent=2) + "\n")


if __name__ == "__main__":
    main()
