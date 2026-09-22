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
