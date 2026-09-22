#!/usr/bin/env python3
"""Validate Phase 19A print fixtures and deterministic logical-layout reports."""

from __future__ import annotations

import argparse
import hashlib
import json
import math
import subprocess
import sys
import tempfile
from pathlib import Path
from typing import Any


CONTRACT_VERSION = 1


def sha256(path: Path) -> str:
    return hashlib.sha256(path.read_bytes()).hexdigest()


def load_json(path: Path) -> dict[str, Any]:
    value = json.loads(path.read_text(encoding="utf-8"))
    if not isinstance(value, dict):
        raise ValueError(f"{path} root must be an object")
    return value


def fixture_index(manifest: dict[str, Any]) -> dict[str, dict[str, Any]]:
    fixtures = manifest.get("fixtures")
    if not isinstance(fixtures, list):
        raise ValueError("fixture manifest fixtures must be an array")
    return {item["path"]: item for item in fixtures if isinstance(item, dict) and isinstance(item.get("path"), str)}


def validate_manifest(corpus: dict[str, Any], root: Path) -> list[dict[str, Any]]:
    if corpus.get("contract_version") != CONTRACT_VERSION:
        raise ValueError(f"contract_version must be {CONTRACT_VERSION}")
    cases = corpus.get("cases")
    if not isinstance(cases, list) or not cases:
        raise ValueError("cases must be a non-empty array")
    inventory = fixture_index(load_json(root / corpus["fixture_manifest"]))
    seen: set[str] = set()
    checked = []
    for case in cases:
        if not isinstance(case, dict):
            raise ValueError("each case must be an object")
        identifier = case.get("id")
        source = case.get("source")
        if not isinstance(identifier, str) or not identifier or identifier in seen:
            raise ValueError("case ids must be unique non-empty strings")
        if not isinstance(source, str) or not source.startswith("tests/fixtures/"):
            raise ValueError(f"{identifier}: source must be under tests/fixtures")
        fixture_path = root / source
        registered = inventory.get(source.removeprefix("tests/fixtures/"))
        if registered is None:
            raise ValueError(f"{identifier}: source is not registered in the fixture manifest")
        if registered.get("format") != "musicxml" or not registered.get("license"):
            raise ValueError(f"{identifier}: source must be a licensed MusicXML fixture")
        if not fixture_path.is_file() or registered.get("sha256") != sha256(fixture_path):
            raise ValueError(f"{identifier}: source checksum does not match fixture manifest")
        if not isinstance(case.get("coverage"), list) or not case["coverage"]:
            raise ValueError(f"{identifier}: coverage must be a non-empty array")
        for key in ("measures_per_system", "systems_per_page", "minimum_pages", "minimum_systems", "minimum_span_segments"):
            if not isinstance(case.get(key), int) or case[key] < 0:
                raise ValueError(f"{identifier}: {key} must be a non-negative integer")
        if not isinstance(case.get("preset"), str) or not case["preset"]:
            raise ValueError(f"{identifier}: preset must be a non-empty string")
        seen.add(identifier)
        checked.append(case)
    return checked


def canonical(value: dict[str, Any]) -> str:
    return json.dumps(value, sort_keys=True, separators=(",", ":"), allow_nan=False)


def inspect_report(case: dict[str, Any], report: dict[str, Any]) -> dict[str, int]:
    identifier = case["id"]
    if report.get("import_error_count") != 0:
        raise ValueError(f"{identifier}: print report has import errors")
    if report.get("renderer_issues") != []:
        raise ValueError(f"{identifier}: print report has renderer issues")
    layout = report.get("layout")
    if not isinstance(layout, dict) or not isinstance(layout.get("pages"), list):
        raise ValueError(f"{identifier}: print report lacks layout pages")
    pages = layout["pages"]
    systems = [system for page in pages for system in page.get("systems", [])]
    spans = [span for page in pages for span in page.get("span_segments", [])]
    if len(pages) < case["minimum_pages"] or len(systems) < case["minimum_systems"] or len(spans) < case["minimum_span_segments"]:
        raise ValueError(f"{identifier}: report falls below declared layout minima")
    for page in pages:
        height = page.get("height_mm")
        if not isinstance(height, (int, float)) or not math.isfinite(height) or height <= 0:
            raise ValueError(f"{identifier}: page height must be finite and positive")
        if not page.get("publication", {}).get("is_title_page") and not page.get("systems"):
            raise ValueError(f"{identifier}: non-title page has no systems")
        for system in page.get("systems", []):
            top, system_height = system.get("top_mm"), system.get("height_mm")
            if not all(isinstance(value, (int, float)) and math.isfinite(value) for value in (top, system_height)):
                raise ValueError(f"{identifier}: system geometry must be finite")
            if top < 0 or system_height <= 0 or top + system_height > height:
                raise ValueError(f"{identifier}: system geometry falls outside page bounds")
    return {"pages": len(pages), "systems": len(systems), "span_segments": len(spans)}


def run_case(case: dict[str, Any], root: Path, acorde: str) -> dict[str, int]:
    command = [
        acorde,
        "print-report",
        str(root / case["source"]),
        "--preset",
        case["preset"],
        "--measures-per-system",
        str(case["measures_per_system"]),
        "--systems-per-page",
        str(case["systems_per_page"]),
    ]
    outputs = []
    for _ in range(2):
        completed = subprocess.run(command, check=False, capture_output=True, text=True)
        if completed.returncode:
            raise ValueError(f"{case['id']}: print-report failed: {completed.stderr.strip()}")
        outputs.append(json.loads(completed.stdout))
    if canonical(outputs[0]) != canonical(outputs[1]):
        raise ValueError(f"{case['id']}: print-report is not deterministic")
    result = inspect_report(case, outputs[0])
    with tempfile.TemporaryDirectory(prefix="acorde-phase19a-") as directory:
        reports = []
        for index in range(2):
            output = Path(directory) / f"{case['id']}-{index}.svg"
            command = [
                acorde,
                "render-report",
                str(root / case["source"]),
                str(output),
                "--width",
                "900",
                "--staff-size",
                "24",
                "--measures-per-system",
                str(case["measures_per_system"]),
                "--no-interactive",
                "--fail-on-issues",
            ]
            completed = subprocess.run(command, check=False, capture_output=True, text=True)
            if completed.returncode:
                raise ValueError(f"{case['id']}: SVG preflight failed: {completed.stderr.strip()}")
            report = json.loads(completed.stdout)
            svg = output.read_text(encoding="utf-8")
            if not report.get("rendered") or report.get("render_error") is not None:
                raise ValueError(f"{case['id']}: SVG renderer did not produce an artifact")
            if report.get("import_error_count") != 0 or report.get("renderer_issues") != []:
                raise ValueError(f"{case['id']}: SVG preflight reported issues")
            if not report.get("svg_fingerprint") or not svg.lstrip().startswith("<svg"):
                raise ValueError(f"{case['id']}: SVG artifact is structurally invalid")
            if "NaN" in svg or "Infinity" in svg:
                raise ValueError(f"{case['id']}: SVG artifact has non-finite geometry")
            reports.append((report, hashlib.sha256(svg.encode("utf-8")).hexdigest()))
        if reports[0][0]["svg_fingerprint"] != reports[1][0]["svg_fingerprint"] or reports[0][1] != reports[1][1]:
            raise ValueError(f"{case['id']}: SVG render is not deterministic")
    return {**result, "svg_fingerprint": reports[0][0]["svg_fingerprint"]}


def main() -> int:
    parser = argparse.ArgumentParser(description=__doc__)
    parser.add_argument("--manifest", type=Path, default=Path("benchmarks/print/phase19a.json"))
    parser.add_argument("--root", type=Path, default=Path.cwd())
    parser.add_argument("--acorde", default="acorde")
    parser.add_argument("--check-only", action="store_true")
    args = parser.parse_args()
    try:
        root = args.root.resolve()
        cases = validate_manifest(load_json(root / args.manifest), root)
        results = [] if args.check_only else [{"id": case["id"], **run_case(case, root, args.acorde)} for case in cases]
        print(json.dumps({"contract_version": CONTRACT_VERSION, "valid": True, "cases": results}, sort_keys=True))
    except (OSError, ValueError, json.JSONDecodeError) as error:
        print(f"print corpus invalid: {error}", file=sys.stderr)
        return 1
    return 0


if __name__ == "__main__":
    raise SystemExit(main())
