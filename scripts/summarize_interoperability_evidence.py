#!/usr/bin/env python3
"""Combine validated external observations without assigning a compatibility score."""

from __future__ import annotations

import argparse
import hashlib
import json
import sys
from pathlib import Path
from typing import Any

from validate_external_evidence import validate


CONTRACT_VERSION = 1


def digest(path: Path) -> str:
    return hashlib.sha256(path.read_bytes()).hexdigest()


def outcome(report: dict[str, Any]) -> dict[str, Any]:
    projection = report["projection"]
    kind = report.get("evidence_kind")
    if kind == "interoperability-comparison":
        return {
            "semantic_equivalent": projection["semantic_equivalent"],
            "analysis_equivalent": projection["analysis_equivalent"],
            "lossless": projection["lossless"],
            "change_count": len(projection["changes"]),
            "source_diagnostic_count": len(projection["source_diagnostics"]),
            "candidate_diagnostic_count": len(projection["candidate_diagnostics"]),
        }
    if kind == "svg-structure-observation":
        return {
            "svg_byte_count": projection["svg_byte_count"],
            "element_counts": projection["element_counts"],
            "expected_text_present": projection.get("expected_text_present", {}),
        }
    if kind == "midi-event-observation":
        return dict(projection["comparison"])
    raise ValueError("report has no supported evidence_kind")


def main() -> int:
    parser = argparse.ArgumentParser(description=__doc__)
    parser.add_argument("reports", nargs="+", type=Path)
    parser.add_argument("--output", type=Path)
    args = parser.parse_args()
    try:
        entries = []
        for path in args.reports:
            report = json.loads(path.read_text(encoding="utf-8"))
            validated = validate(report)
            entries.append(
                {
                    "path": str(path),
                    "sha256": digest(path),
                    "evidence_kind": report.get("evidence_kind"),
                    "execution_status": report.get("execution_status"),
                    "fixture": validated["fixture"],
                    "tool": validated["tool"],
                    "outcome": outcome(report),
                }
            )
        summary = {
            "contract_version": CONTRACT_VERSION,
            "evidence_kind": "interoperability-summary",
            "report_count": len(entries),
            "reports": entries,
            "claim_boundary": (
                "This inventory preserves each bounded observation separately and assigns no "
                "compatibility rate, parity result, or cross-format aggregate."
            ),
        }
        encoded = json.dumps(summary, indent=2, sort_keys=True) + "\n"
        if args.output:
            args.output.write_text(encoded, encoding="utf-8")
        else:
            print(encoded, end="")
    except (OSError, ValueError, json.JSONDecodeError) as error:
        print(f"interoperability summary invalid: {error}", file=sys.stderr)
        return 1
    return 0


if __name__ == "__main__":
    raise SystemExit(main())
