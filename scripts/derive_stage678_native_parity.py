#!/usr/bin/env python3
"""Embed native listening-pack requests, choices, and events for browser parity."""

import json
from pathlib import Path
import sys


def main() -> None:
    if len(sys.argv) != 3:
        raise SystemExit("usage: derive_stage678_native_parity.py <pack-dir> <output.json>")
    root = Path(sys.argv[1])
    manifest = json.loads((root / "manifest.json").read_text())
    if manifest["working_tree_dirty"] or len(manifest["cases"]) != 2:
        raise SystemExit("expected a clean 8/16-bar native listening pack")
    cases = []
    for case in manifest["cases"]:
        directory = root / case["id"]
        candidates = []
        if len(case["candidates"]) != 3:
            raise SystemExit(f"expected three candidates: {case['id']}")
        for candidate in case["candidates"]:
            candidates.append({
                "resolved": json.loads((directory / candidate["resolved"]["file"]).read_text()),
                "difference_codes": candidate["difference_codes"],
                "playback_events": candidate["playback_events"],
            })
        cases.append({
            "id": case["id"],
            "measures": case["measures"],
            "source": json.loads((directory / case["source"]["file"]).read_text()),
            "request": json.loads((directory / case["request"]["file"]).read_text()),
            "candidates": candidates,
        })
    receipt = {
        "schema_version": 1,
        "native_commit": manifest["source_commit"],
        "cases": cases,
    }
    Path(sys.argv[2]).write_text(json.dumps(receipt, ensure_ascii=False, separators=(",", ":")) + "\n")


if __name__ == "__main__":
    main()
