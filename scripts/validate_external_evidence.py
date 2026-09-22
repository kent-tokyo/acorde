#!/usr/bin/env python3
"""Validate a bounded external-tool evidence report without running external software.

The report may contain tool-specific projection data, but must identify the tool, version,
fixture bytes, and options used to produce it. This prevents a local observation from being
mistaken for an unpinned or general compatibility claim.
"""

from __future__ import annotations

import argparse
import json
import re
import sys
from pathlib import Path
from typing import Any


SHA256 = re.compile(r"^[0-9a-f]{64}$")
CONTRACT_VERSION = 1


def read_report(path: str) -> dict[str, Any]:
    text = sys.stdin.read() if path == "-" else Path(path).read_text(encoding="utf-8")
    value = json.loads(text)
    if not isinstance(value, dict):
        raise ValueError("report root must be an object")
    return value


def nonempty_string(value: Any, field: str) -> str:
    if not isinstance(value, str) or not value.strip():
        raise ValueError(f"{field} must be a non-empty string")
    return value


def validate(report: dict[str, Any]) -> dict[str, str]:
    if report.get("contract_version") != CONTRACT_VERSION:
        raise ValueError(f"contract_version must be {CONTRACT_VERSION}")
    tool = report.get("tool")
    fixture = report.get("fixture")
    if not isinstance(tool, dict):
        raise ValueError("tool must be an object")
    if not isinstance(fixture, dict):
        raise ValueError("fixture must be an object")
    name = nonempty_string(tool.get("name"), "tool.name")
    version = nonempty_string(tool.get("version"), "tool.version")
    path = nonempty_string(fixture.get("path"), "fixture.path")
    sha256 = nonempty_string(fixture.get("sha256"), "fixture.sha256")
    if not SHA256.fullmatch(sha256):
        raise ValueError("fixture.sha256 must be 64 lowercase hexadecimal characters")
    if not isinstance(report.get("options"), dict):
        raise ValueError("options must be an object")
    if not isinstance(report.get("projection"), dict):
        raise ValueError("projection must be an object")
    evidence_kind = report.get("evidence_kind")
    supported_kinds = {"interoperability-comparison", "svg-structure-observation", "midi-event-observation"}
    if evidence_kind is not None and evidence_kind not in supported_kinds:
        raise ValueError("evidence_kind is unsupported")
    if evidence_kind == "interoperability-comparison":
        candidate = report.get("candidate")
        if not isinstance(candidate, dict):
            raise ValueError("candidate must be an object for an interoperability comparison")
        candidate_path = nonempty_string(candidate.get("path"), "candidate.path")
        candidate_sha256 = nonempty_string(candidate.get("sha256"), "candidate.sha256")
        if not SHA256.fullmatch(candidate_sha256):
            raise ValueError("candidate.sha256 must be 64 lowercase hexadecimal characters")
        if not isinstance(candidate.get("format"), str) or not candidate["format"].strip():
            raise ValueError("candidate.format must be a non-empty string")
        status = report.get("execution_status")
        if status not in {"success", "output-produced-with-nonzero-exit"}:
            raise ValueError("execution_status must identify the external process result")
        projection = report["projection"]
        for key in ("semantic_equivalent", "analysis_equivalent", "lossless"):
            if not isinstance(projection.get(key), bool):
                raise ValueError(f"projection.{key} must be a boolean")
        if not isinstance(projection.get("changes"), list):
            raise ValueError("projection.changes must be an array")
        if not isinstance(projection.get("source_diagnostics"), list):
            raise ValueError("projection.source_diagnostics must be an array")
        if not isinstance(projection.get("candidate_diagnostics"), list):
            raise ValueError("projection.candidate_diagnostics must be an array")
        _ = candidate_path
    if evidence_kind == "svg-structure-observation":
        candidate = report.get("candidate")
        if not isinstance(candidate, dict) or candidate.get("format") != "svg":
            raise ValueError("SVG observation requires an SVG candidate")
        candidate_sha256 = nonempty_string(candidate.get("sha256"), "candidate.sha256")
        if not SHA256.fullmatch(candidate_sha256):
            raise ValueError("candidate.sha256 must be 64 lowercase hexadecimal characters")
        projection = report["projection"]
        if not isinstance(projection.get("svg_byte_count"), int) or projection["svg_byte_count"] <= 0:
            raise ValueError("projection.svg_byte_count must be positive")
        if not isinstance(projection.get("element_counts"), dict):
            raise ValueError("projection.element_counts must be an object")
        if not isinstance(projection.get("text_content"), list):
            raise ValueError("projection.text_content must be an array")
    if evidence_kind == "midi-event-observation":
        projection = report["projection"]
        if not isinstance(projection.get("mido"), dict) or not isinstance(projection.get("acorde"), dict):
            raise ValueError("MIDI observation requires mido and acorde projections")
        comparison = projection.get("comparison")
        if not isinstance(comparison, dict):
            raise ValueError("MIDI observation requires a comparison object")
        for key in ("all_channel_events_match", "note_count_match", "percussion_note_count_match", "initial_tempo_bpm_match", "time_signature_match"):
            if not isinstance(comparison.get(key), bool):
                raise ValueError(f"comparison.{key} must be a boolean")
    return {"fixture": path, "sha256": sha256, "tool": f"{name} {version}"}


def main() -> int:
    parser = argparse.ArgumentParser(description=__doc__)
    parser.add_argument("report", help="JSON report path, or - for stdin")
    args = parser.parse_args()
    try:
        summary = validate(read_report(args.report))
    except (OSError, ValueError, json.JSONDecodeError) as error:
        print(f"external evidence invalid: {error}", file=sys.stderr)
        return 1
    print(json.dumps({"contract_version": CONTRACT_VERSION, "valid": True, **summary}, sort_keys=True))
    return 0


if __name__ == "__main__":
    raise SystemExit(main())
