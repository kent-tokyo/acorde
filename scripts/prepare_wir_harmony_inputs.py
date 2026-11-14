#!/usr/bin/env python3
"""Remove MusicXML harmony answers from pinned MXL scores before analysis."""

import argparse
import hashlib
import json
from pathlib import Path
import re
import xml.etree.ElementTree as ET
import zipfile


HARMONY = re.compile(rb"<harmony(?=[\s>])[^>]*>.*?</harmony\s*>", re.DOTALL)


def sha256(data):
    return hashlib.sha256(data).hexdigest()


def count_harmony(xml):
    root = ET.fromstring(xml)
    return sum(node.tag.rsplit("}", 1)[-1] == "harmony" for node in root.iter())


def strip_mxl(data):
    from io import BytesIO

    source = BytesIO(data)
    output = BytesIO()
    removed = 0
    with zipfile.ZipFile(source) as original, zipfile.ZipFile(output, "w") as cleaned:
        for info in original.infolist():
            payload = original.read(info.filename)
            if (info.filename.endswith((".xml", ".musicxml"))
                    and not info.filename.startswith("META-INF/")):
                expected = count_harmony(payload)
                payload, found = HARMONY.subn(b"", payload)
                if found != expected or count_harmony(payload):
                    raise ValueError(f"cannot safely strip harmony from {info.filename}")
                removed += found
            cleaned.writestr(info, payload)
    return output.getvalue(), removed


def prepare(selection, hashes, source_dir, output_dir):
    if selection["commit"] != hashes["commit"]:
        raise ValueError("commit mismatch")
    by_work = {row["work"]: row for row in hashes["files"]}
    if len(by_work) != len(selection["works"]):
        raise ValueError("work count mismatch")
    inputs = []
    for work in selection["works"]:
        name = work["work"]
        if name not in by_work:
            raise ValueError(f"missing hash record: {name}")
        relative = Path(work["score_path"])
        if relative.is_absolute() or ".." in relative.parts:
            raise ValueError(f"invalid score path: {relative}")
        source_bytes = (source_dir / relative).read_bytes()
        if sha256(source_bytes) != by_work[name]["score_sha256"]:
            raise ValueError(f"source score hash mismatch: {name}")
        cleaned, removed = strip_mxl(source_bytes)
        if not removed:
            cleaned = source_bytes
        target = output_dir / relative
        target.parent.mkdir(parents=True, exist_ok=True)
        target.write_bytes(cleaned)
        entry = {"work": name, "path": str(relative),
                 "source_sha256": sha256(source_bytes),
                 "input_sha256": sha256(cleaned),
                 "removed_harmony_elements": removed}
        if "gold_sha256" in by_work[name]:
            entry["gold_sha256"] = by_work[name]["gold_sha256"]
        inputs.append(entry)
    return {"schema_version": 1, "source_commit": selection["commit"],
            "selection_sha256": hashes["selection_sha256"], "inputs": inputs}


def main():
    parser = argparse.ArgumentParser(description=__doc__)
    parser.add_argument("selection", type=Path)
    parser.add_argument("hashes", type=Path)
    parser.add_argument("source_dir", type=Path)
    parser.add_argument("output_dir", type=Path)
    parser.add_argument("manifest", type=Path)
    args = parser.parse_args()
    selection_bytes = args.selection.read_bytes()
    selection = json.loads(selection_bytes)
    hashes = json.loads(args.hashes.read_bytes())
    if sha256(selection_bytes) != hashes["selection_sha256"]:
        parser.error("selection hash mismatch")
    result = prepare(selection, hashes, args.source_dir, args.output_dir)
    args.manifest.write_text(json.dumps(result, ensure_ascii=False, indent=2) + "\n")
    print(f"prepared {len(result['inputs'])} score inputs; removed "
          f"{sum(item['removed_harmony_elements'] for item in result['inputs'])} harmony elements")


if __name__ == "__main__":
    main()
