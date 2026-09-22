#!/usr/bin/env python3
"""Record a hash-pinned external MEI-to-SVG structural observation."""

from __future__ import annotations

import argparse
import hashlib
import json
import sys
import xml.etree.ElementTree as etree
from collections import Counter
from pathlib import Path
from typing import Any


CONTRACT_VERSION = 1


def sha256(path: Path) -> str:
    return hashlib.sha256(path.read_bytes()).hexdigest()


def fixture(path: Path, manifest_path: Path, root: Path) -> dict[str, Any]:
    manifest = json.loads(manifest_path.read_text(encoding="utf-8"))
    relative = path.resolve().relative_to(manifest_path.parent.resolve()).as_posix()
    for item in manifest.get("fixtures", []):
        if item.get("path") == relative:
            if item.get("sha256") != sha256(path):
                raise ValueError("source bytes differ from fixture manifest")
            return {
                "path": path.resolve().relative_to(root.resolve()).as_posix(),
                "format": item["format"],
                "sha256": item["sha256"],
                **{key: item[key] for key in ("license", "source_url", "source_revision") if key in item},
            }
    raise ValueError(f"source is not a registered fixture: {relative}")


def local_name(tag: str) -> str:
    return tag.rsplit("}", 1)[-1]


def main() -> int:
    parser = argparse.ArgumentParser(description=__doc__)
    parser.add_argument("source", type=Path)
    parser.add_argument("svg", type=Path)
    parser.add_argument("--fixture-manifest", type=Path, default=Path("tests/fixtures/manifest.json"))
    parser.add_argument("--root", type=Path, default=Path.cwd())
    parser.add_argument("--tool-name", required=True)
    parser.add_argument("--tool-version", required=True)
    parser.add_argument("--options-json", default="{}")
    parser.add_argument("--expect-text", action="append", default=[])
    parser.add_argument("--output", type=Path)
    args = parser.parse_args()
    try:
        source = args.source.resolve()
        svg = args.svg.resolve()
        root = args.root.resolve()
        options = json.loads(args.options_json)
        if not isinstance(options, dict):
            raise ValueError("options must be a JSON object")
        tree = etree.parse(svg)
        counts = Counter(local_name(node.tag) for node in tree.iter())
        text_content = [
            node.text.strip()
            for node in tree.iter()
            if local_name(node.tag) in {"text", "tspan"} and node.text and node.text.strip()
        ]
        result = {
            "contract_version": CONTRACT_VERSION,
            "evidence_kind": "svg-structure-observation",
            "tool": {"name": args.tool_name, "version": args.tool_version},
            "fixture": fixture(source, args.fixture_manifest.resolve(), root),
            "candidate": {
                "path": svg.resolve().as_posix(),
                "format": "svg",
                "sha256": sha256(svg),
            },
            "options": options,
            "execution_status": "success",
            "projection": {
                "svg_byte_count": svg.stat().st_size,
                "element_counts": dict(sorted(counts.items())),
                "text_content": text_content,
                "expected_text_present": {value: value in text_content for value in args.expect_text},
            },
        }
        text = json.dumps(result, indent=2, sort_keys=True) + "\n"
        if args.output:
            args.output.write_text(text, encoding="utf-8")
        else:
            print(text, end="")
    except (OSError, ValueError, json.JSONDecodeError, etree.ParseError) as error:
        print(f"SVG observation invalid: {error}", file=sys.stderr)
        return 1
    return 0


if __name__ == "__main__":
    raise SystemExit(main())
