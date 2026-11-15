#!/usr/bin/env python3
"""Fetch/verify pinned print sources without parsing scores or rendering the holdout."""

import argparse
import hashlib
import json
import re
import subprocess
from pathlib import Path, PurePosixPath
from urllib.parse import quote

REPOSITORIES = {
    "OpenScore/Lieder", "OpenScore/StringQuartets", "DCMLab/cpe_bach_keyboard",
    "DCMLab/chopin_mazurkas", "DCMLab/schumann_kinderszenen",
}


def validate_selection(selection):
    works = selection["works"]
    counts = {group: 0 for group in ("piano", "vocal", "chamber", "part")}
    identities = set()
    for work in works:
        if work["group"] not in counts or work["repository"] not in REPOSITORIES:
            raise ValueError("unexpected group or repository")
        path = PurePosixPath(work["path"])
        if path.is_absolute() or ".." in path.parts or path.suffix not in (".mxl", ".mscx"):
            raise ValueError("unsafe source path")
        if not re.fullmatch(r"[A-Za-z0-9_-]+", work["id"]):
            raise ValueError("unsafe source id")
        for key in ("revision", "git_blob_sha1"):
            if not re.fullmatch(r"[0-9a-f]{40}", work[key]):
                raise ValueError("source must use a full pinned Git hash")
        identity = (work["repository"], work["work_id"])
        if identity in identities:
            raise ValueError("a work appears more than once")
        identities.add(identity)
        counts[work["group"]] += 1
    if set(counts.values()) != {6}:
        raise ValueError(f"expected six inputs per group: {counts}")


def fetch(selection_path, output_dir, allow_fetch=False):
    data = selection_path.read_bytes()
    selection = json.loads(data)
    validate_selection(selection)
    records = []
    for work in selection["works"]:
        target = output_dir / work["group"] / (work["id"] + Path(work["path"]).suffix)
        if target.is_symlink():
            raise ValueError(f"source target is a symlink: {target}")
        url = (f"https://raw.githubusercontent.com/{work['repository']}/{work['revision']}/"
               + quote(work["path"]))
        if target.exists():
            content = target.read_bytes()
        elif allow_fetch:
            response = subprocess.run(
                ["curl", "--fail", "--location", "--silent", "--show-error",
                 "--max-time", "60", "--max-filesize", "16777216", url],
                capture_output=True, check=True)
            content = response.stdout
        else:
            raise FileNotFoundError(target)
        blob = hashlib.sha1(f"blob {len(content)}\0".encode() + content).hexdigest()
        if blob != work["git_blob_sha1"] or len(content) != work["bytes"]:
            raise ValueError(f"source hash or length mismatch: {target}")
        target.parent.mkdir(parents=True, exist_ok=True)
        if not target.exists():
            target.write_bytes(content)
        records.append({"group": work["group"], "id": work["id"], "source_url": url,
                        "license": work["license"], "git_blob_sha1": blob,
                        "sha256": hashlib.sha256(content).hexdigest(), "bytes": len(content),
                        "local_path": str(target.relative_to(output_dir))})
        print(f"verified {work['group']}/{work['id']} ({len(content)} bytes)", flush=True)
    return {"schema_version": 1, "selection_sha256": hashlib.sha256(data).hexdigest(),
            "source_contents_inspected": False, "renders_inspected": False, "files": records}


def main():
    parser = argparse.ArgumentParser(description=__doc__)
    parser.add_argument("selection", type=Path)
    parser.add_argument("output_dir", type=Path)
    parser.add_argument("record", type=Path)
    parser.add_argument("--fetch", action="store_true", help="allow downloading absent files")
    args = parser.parse_args()
    result = fetch(args.selection, args.output_dir, args.fetch)
    args.record.write_text(json.dumps(result, ensure_ascii=False, indent=2) + "\n")


if __name__ == "__main__":
    main()
