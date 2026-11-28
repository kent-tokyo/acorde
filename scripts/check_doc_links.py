#!/usr/bin/env python3
"""Check relative Markdown links and heading anchors in tracked documentation."""

from __future__ import annotations

import argparse
import re
import subprocess
import sys
import unicodedata
from pathlib import Path
from urllib.parse import unquote

LINK = re.compile(r"(?<!!)\[(?:[^\[\]]|\[[^\]]*\])*\]\(\s*<?([^)\s>]+)>?(?:\s+\"[^\"]*\")?\s*\)")
IMAGE = re.compile(r"!\[[^\]]*\]\(\s*<?([^)\s>]+)>?(?:\s+\"[^\"]*\")?\s*\)")
REFERENCE = re.compile(r"^\s{0,3}\[[^\]]+\]:\s*<?(\S+?)>?(?:\s+.*)?$")
HEADING = re.compile(r"^\s{0,3}(#{1,6})\s+(.*?)\s*#*\s*$")
HTML_ID = re.compile(r"<a\s+[^>]*(?:id|name)=\"([^\"]+)\"", re.IGNORECASE)
FENCE = re.compile(r"^\s{0,3}(```|~~~)")
INLINE_CODE = re.compile(r"`[^`]*`")
EXTERNAL = re.compile(r"^[a-zA-Z][a-zA-Z0-9+.-]*:")


def slug(text: str) -> str:
    """Approximate GitHub's heading anchor algorithm."""
    text = re.sub(r"<[^>]+>", "", text)
    text = re.sub(r"!?\[([^\]]*)\]\([^)]*\)", r"\1", text)
    text = text.replace("`", "").strip().lower()
    kept = []
    for char in text:
        category = unicodedata.category(char)
        if char in "-_ " or category[0] in "LN" or category == "Mn":
            kept.append(char)
    return "".join(kept).replace(" ", "-")


def strip_code(lines: list[str]) -> list[str]:
    out, fenced = [], False
    for line in lines:
        if FENCE.match(line):
            fenced = not fenced
            out.append("")
            continue
        out.append("" if fenced else INLINE_CODE.sub("", line))
    return out


def anchors(path: Path, cache: dict[Path, set[str]]) -> set[str]:
    if path not in cache:
        found: set[str] = set()
        counts: dict[str, int] = {}
        for line in strip_code(path.read_text(encoding="utf-8").splitlines()):
            found.update(HTML_ID.findall(line))
            match = HEADING.match(line)
            if match:
                base = slug(match.group(2))
                n = counts.get(base, 0)
                counts[base] = n + 1
                found.add(base if n == 0 else f"{base}-{n}")
        cache[path] = found
    return cache[path]


def targets(lines: list[str]):
    for number, line in enumerate(strip_code(lines), 1):
        for pattern in (LINK, IMAGE):
            for match in pattern.finditer(line):
                yield number, match.group(1)
        match = REFERENCE.match(line)
        if match:
            yield number, match.group(1)


def check(root: Path, files: list[Path]) -> list[str]:
    errors, cache = [], {}
    for source in files:
        lines = source.read_text(encoding="utf-8").splitlines()
        for number, target in targets(lines):
            if EXTERNAL.match(target) or target.startswith("//"):
                continue
            raw_path, _, fragment = target.partition("#")
            raw_path = unquote(raw_path.split("?", 1)[0])
            if raw_path.startswith("/"):
                resolved = root / raw_path.lstrip("/")
            else:
                resolved = (source.parent / raw_path) if raw_path else source
            where = f"{source.relative_to(root)}:{number}"
            if not resolved.exists():
                errors.append(f"{where}: missing target {target}")
                continue
            if fragment and resolved.is_file() and resolved.suffix == ".md":
                if unquote(fragment).lower() not in {a.lower() for a in anchors(resolved, cache)}:
                    errors.append(f"{where}: missing anchor {target}")
    return errors


def tracked_markdown(root: Path) -> list[Path]:
    listed = subprocess.run(
        ["git", "ls-files", "-z", "--", "*.md"], cwd=root, check=True, capture_output=True
    ).stdout.decode()
    return [root / name for name in listed.split("\0") if name]


def main() -> int:
    parser = argparse.ArgumentParser(description=__doc__)
    parser.add_argument("files", nargs="*", type=Path, help="Markdown files (default: all tracked)")
    args = parser.parse_args()
    root = Path(__file__).resolve().parent.parent
    files = [p.resolve() for p in args.files] or tracked_markdown(root)
    errors = check(root, files)
    for error in errors:
        print(error, file=sys.stderr)
    print(f"checked {len(files)} Markdown files; {len(errors)} broken links")
    return 1 if errors else 0


if __name__ == "__main__":
    sys.exit(main())
