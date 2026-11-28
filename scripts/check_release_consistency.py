#!/usr/bin/env python3
"""Check that release version markers agree with the workspace version.

The workspace `Cargo.toml` is the source of truth. This check fails when any
of the following disagree with it:

- internal `acorde-*` path dependencies in `crates/*/Cargo.toml`;
- `release` in `docs/scorecard.json`, and `base_release` against the previous
  release heading in `CHANGELOG.md`;
- the newest released heading in `CHANGELOG.md` (`## X.Y.Z - YYYY-MM-DD`);
- the "Version X.Y.Z" baseline sentence in `ROADMAP.md`.

Run from the repository root, or pass `--root`:

    python3 -B scripts/check_release_consistency.py
"""

from __future__ import annotations

import argparse
import json
import re
import sys
from pathlib import Path

WORKSPACE_VERSION = re.compile(
    r"^\[workspace\.package\]\s*$.*?^version\s*=\s*\"([^\"]+)\"",
    re.MULTILINE | re.DOTALL,
)
INTERNAL_DEP = re.compile(
    r"^(acorde(?:-[a-z0-9-]+)?)\s*=\s*\{[^}\n]*\bpath\s*=[^}\n]*\}",
    re.MULTILINE,
)
DEP_VERSION = re.compile(r"\bversion\s*=\s*\"([^\"]+)\"")
CHANGELOG_RELEASE = re.compile(
    r"^## \[?(\d+\.\d+\.\d+(?:-[0-9A-Za-z.-]+)?)\]? - \d{4}-\d{2}-\d{2}\s*$",
    re.MULTILINE,
)
ROADMAP_VERSION = re.compile(r"^Version (\d+\.\d+\.\d+(?:-[0-9A-Za-z.-]+)?) ", re.MULTILINE)


def workspace_version(root: Path) -> str:
    match = WORKSPACE_VERSION.search((root / "Cargo.toml").read_text(encoding="utf-8"))
    if not match:
        raise ValueError("Cargo.toml: no [workspace.package] version")
    return match.group(1)


def check(root: Path) -> list[str]:
    """Return human-readable mismatch descriptions; empty means consistent."""
    errors: list[str] = []
    version = workspace_version(root)

    for manifest in sorted(root.glob("crates/*/Cargo.toml")):
        text = manifest.read_text(encoding="utf-8")
        for dep in INTERNAL_DEP.finditer(text):
            dep_version = DEP_VERSION.search(dep.group(0))
            rel = manifest.relative_to(root)
            if dep_version is None:
                errors.append(f"{rel}: {dep.group(1)} path dependency has no version")
            elif dep_version.group(1) != version:
                errors.append(
                    f"{rel}: {dep.group(1)} requires {dep_version.group(1)}, "
                    f"workspace is {version}"
                )

    releases = CHANGELOG_RELEASE.findall(
        (root / "CHANGELOG.md").read_text(encoding="utf-8")
    )
    if not releases:
        errors.append("CHANGELOG.md: no released '## X.Y.Z - YYYY-MM-DD' heading")
    elif releases[0] != version:
        errors.append(
            f"CHANGELOG.md: newest release heading is {releases[0]}, workspace is {version}"
        )

    scorecard = json.loads((root / "docs/scorecard.json").read_text(encoding="utf-8"))
    if scorecard.get("release") != version:
        errors.append(
            f"docs/scorecard.json: release is {scorecard.get('release')!r}, "
            f"workspace is {version}"
        )
    if len(releases) > 1 and scorecard.get("base_release") != releases[1]:
        errors.append(
            f"docs/scorecard.json: base_release is {scorecard.get('base_release')!r}, "
            f"previous CHANGELOG release is {releases[1]}"
        )

    roadmap = ROADMAP_VERSION.search((root / "ROADMAP.md").read_text(encoding="utf-8"))
    if roadmap is None:
        errors.append("ROADMAP.md: no 'Version X.Y.Z ...' baseline sentence")
    elif roadmap.group(1) != version:
        errors.append(f"ROADMAP.md: baseline names {roadmap.group(1)}, workspace is {version}")

    return errors


def main(argv: list[str] | None = None) -> int:
    parser = argparse.ArgumentParser(description=__doc__.splitlines()[0])
    parser.add_argument("--root", type=Path, default=Path("."))
    parser.add_argument(
        "--expect",
        help="also require the workspace version to equal this value (e.g. a tag without 'v')",
    )
    args = parser.parse_args(argv)
    root = args.root.resolve()
    try:
        errors = check(root)
        version = workspace_version(root)
    except (OSError, ValueError, json.JSONDecodeError) as error:
        print(f"release consistency check failed: {error}", file=sys.stderr)
        return 1
    if args.expect is not None and args.expect != version:
        errors.insert(0, f"expected version {args.expect}, workspace is {version}")
    for error in errors:
        print(f"error: {error}", file=sys.stderr)
    if errors:
        return 1
    print(f"release markers agree on {version}")
    return 0


if __name__ == "__main__":
    sys.exit(main())
