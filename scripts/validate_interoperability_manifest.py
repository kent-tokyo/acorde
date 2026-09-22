#!/usr/bin/env python3
"""Validate the checked-in Phase 18 external-interoperability corpus contract."""

from __future__ import annotations

import argparse
import hashlib
import json
import sys
from pathlib import Path
from typing import Any


CONTRACT_VERSION = 1
SEMANTIC_PROJECTION = {
    "semantic_equivalent",
    "analysis_equivalent",
    "lossless",
    "changes",
    "source_diagnostics",
    "candidate_diagnostics",
}
PROJECTION_BY_KIND = {
    "semantic-score": SEMANTIC_PROJECTION,
    "svg-structure": {"svg_byte_count", "element_counts", "text_content"},
    "midi-events": {"mido", "acorde", "comparison"},
}


def object_from(path: Path, description: str) -> dict[str, Any]:
    value = json.loads(path.read_text(encoding="utf-8"))
    if not isinstance(value, dict):
        raise ValueError(f"{description} root must be an object")
    return value


def sha256(path: Path) -> str:
    return hashlib.sha256(path.read_bytes()).hexdigest()


def main() -> int:
    parser = argparse.ArgumentParser(description=__doc__)
    parser.add_argument(
        "manifest",
        nargs="?",
        type=Path,
        default=Path("benchmarks/interoperability/phase18a.json"),
    )
    parser.add_argument("--root", type=Path, default=Path.cwd())
    args = parser.parse_args()
    try:
        root = args.root.resolve()
        manifest_path = args.manifest.resolve()
        manifest = object_from(manifest_path, "interoperability manifest")
        if manifest.get("contract_version") != CONTRACT_VERSION:
            raise ValueError(f"contract_version must be {CONTRACT_VERSION}")
        if not isinstance(manifest.get("name"), str) or not manifest["name"].strip():
            raise ValueError("name must be a non-empty string")
        fixture_manifest_path = root / str(manifest.get("fixture_manifest", ""))
        fixture_manifest = object_from(fixture_manifest_path, "fixture manifest")
        fixtures = fixture_manifest.get("fixtures")
        if not isinstance(fixtures, list):
            raise ValueError("fixture manifest fixtures must be an array")
        fixture_by_path = {
            fixture.get("path"): fixture for fixture in fixtures if isinstance(fixture, dict)
        }
        cases = manifest.get("cases")
        if not isinstance(cases, list) or not cases:
            raise ValueError("cases must be a non-empty array")
        seen: set[str] = set()
        for case in cases:
            if not isinstance(case, dict):
                raise ValueError("each case must be an object")
            identifier = case.get("id")
            if not isinstance(identifier, str) or not identifier.strip() or identifier in seen:
                raise ValueError("case id must be non-empty and unique")
            seen.add(identifier)
            kind = case.get("comparison_kind")
            if kind not in PROJECTION_BY_KIND:
                raise ValueError(f"{identifier}: comparison_kind is unsupported")
            source = case.get("source")
            fixture = fixture_by_path.get(Path(str(source)).name)
            if not isinstance(fixture, dict):
                raise ValueError(f"{identifier}: source is not a registered fixture")
            source_path = root / str(source)
            if not source_path.is_file() or sha256(source_path) != fixture.get("sha256"):
                raise ValueError(f"{identifier}: source bytes differ from fixture manifest")
            if case.get("source_format") != fixture.get("format"):
                raise ValueError(f"{identifier}: source_format differs from fixture manifest")
            if not isinstance(case.get("candidate_format"), str) or not case["candidate_format"].strip():
                raise ValueError(f"{identifier}: candidate_format must be a non-empty string")
            projection = case.get("required_projection")
            if not isinstance(projection, list) or not PROJECTION_BY_KIND[kind].issubset(projection):
                raise ValueError(f"{identifier}: required_projection omits a required field")
            if not isinstance(case.get("claim_boundary"), str) or not case["claim_boundary"].strip():
                raise ValueError(f"{identifier}: claim_boundary must be non-empty")
        print(json.dumps({"contract_version": CONTRACT_VERSION, "cases": len(cases), "valid": True}, sort_keys=True))
    except (OSError, ValueError, json.JSONDecodeError) as error:
        print(f"interoperability manifest invalid: {error}", file=sys.stderr)
        return 1
    return 0


if __name__ == "__main__":
    raise SystemExit(main())
