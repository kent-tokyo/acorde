#!/usr/bin/env python3
"""Record one version-pinned external score interchange comparison.

The external application is deliberately *not* launched here.  Invoke it yourself, then give
this script the original fixture and the produced artifact.  The script records SHA-256 values,
the external tool identity/options, and Acorde's typed semantic comparison in one portable JSON
envelope.  This makes a produced file useful evidence without making an installed application a
test-suite dependency.
"""

from __future__ import annotations

import argparse
import hashlib
import json
import subprocess
import sys
from pathlib import Path
from typing import Any


CONTRACT_VERSION = 1


def sha256(path: Path) -> str:
    return hashlib.sha256(path.read_bytes()).hexdigest()


def format_for(path: Path) -> str:
    extension = path.suffix.lower().lstrip(".")
    aliases = {"xml": "musicxml", "mid": "midi"}
    return aliases.get(extension, extension)


def read_json_object(value: str, name: str) -> dict[str, Any]:
    try:
        parsed = json.loads(value)
    except json.JSONDecodeError as error:
        raise ValueError(f"{name} is not valid JSON: {error}") from error
    if not isinstance(parsed, dict):
        raise ValueError(f"{name} must be a JSON object")
    return parsed


def relative_display(path: Path, root: Path) -> str:
    try:
        return path.resolve().relative_to(root.resolve()).as_posix()
    except ValueError:
        return path.resolve().as_posix()


def fixture_provenance(path: Path, manifest_path: Path, root: Path) -> dict[str, Any]:
    manifest = read_json_object(manifest_path.read_text(encoding="utf-8"), "fixture manifest")
    fixtures = manifest.get("fixtures")
    if not isinstance(fixtures, list):
        raise ValueError("fixture manifest fixtures must be an array")
    display_path = relative_display(path, root)
    fixture_path = relative_display(path, manifest_path.parent)
    for fixture in fixtures:
        if isinstance(fixture, dict) and fixture.get("path") == fixture_path:
            if fixture.get("sha256") != sha256(path):
                raise ValueError(f"fixture SHA-256 differs from manifest for {fixture_path}")
            return {
                key: fixture[key]
                for key in ("license", "source_url", "source_revision", "source_path")
                if key in fixture
            }
    raise ValueError(f"fixture is not present in manifest: {fixture_path} ({display_path})")


def compatibility(acorde: str, source: Path, candidate: Path) -> dict[str, Any]:
    completed = subprocess.run(
        [acorde, "compatibility-report", str(source), str(candidate)],
        check=False,
        capture_output=True,
        text=True,
    )
    if completed.returncode != 0:
        detail = completed.stderr.strip() or completed.stdout.strip()
        raise ValueError(f"acorde compatibility-report failed: {detail}")
    try:
        report = json.loads(completed.stdout)
    except json.JSONDecodeError as error:
        raise ValueError(f"acorde compatibility-report returned invalid JSON: {error}") from error
    if not isinstance(report, dict) or report.get("contract_version") != 1:
        raise ValueError("acorde compatibility-report has an unsupported contract")
    return report


def main() -> int:
    parser = argparse.ArgumentParser(description=__doc__)
    parser.add_argument("source", type=Path, help="checked-in source fixture")
    parser.add_argument("candidate", type=Path, help="artifact emitted by an external tool")
    parser.add_argument("--acorde", default="acorde", help="acorde CLI executable")
    parser.add_argument("--fixture-manifest", type=Path, default=Path("tests/fixtures/manifest.json"))
    parser.add_argument("--root", type=Path, default=Path.cwd(), help="repository root for paths")
    parser.add_argument("--tool-name", required=True)
    parser.add_argument("--tool-version", required=True)
    parser.add_argument(
        "--options-json",
        default="{}",
        help="JSON object of external-tool options used to create candidate",
    )
    parser.add_argument(
        "--execution-status",
        choices=("success", "output-produced-with-nonzero-exit"),
        default="success",
        help="external process result; nonzero output remains evidence but never a passing gate",
    )
    parser.add_argument("--output", type=Path, help="write JSON to this path instead of stdout")
    args = parser.parse_args()

    try:
        source = args.source.resolve()
        candidate = args.candidate.resolve()
        if not source.is_file() or not candidate.is_file():
            raise ValueError("source and candidate must be readable files")
        options = read_json_object(args.options_json, "options")
        root = args.root.resolve()
        provenance = fixture_provenance(source, args.fixture_manifest.resolve(), root)
        result = {
            "contract_version": CONTRACT_VERSION,
            "evidence_kind": "interoperability-comparison",
            "tool": {"name": args.tool_name, "version": args.tool_version},
            "fixture": {
                "path": relative_display(source, root),
                "format": format_for(source),
                "sha256": sha256(source),
                **provenance,
            },
            "candidate": {
                "path": relative_display(candidate, root),
                "format": format_for(candidate),
                "sha256": sha256(candidate),
            },
            "options": options,
            "execution_status": args.execution_status,
            "projection": compatibility(args.acorde, source, candidate),
        }
        text = json.dumps(result, indent=2, sort_keys=True) + "\n"
        if args.output:
            args.output.write_text(text, encoding="utf-8")
        else:
            print(text, end="")
    except (OSError, ValueError) as error:
        print(f"interoperability comparison invalid: {error}", file=sys.stderr)
        return 1
    return 0


if __name__ == "__main__":
    raise SystemExit(main())
