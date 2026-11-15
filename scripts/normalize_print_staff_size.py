#!/usr/bin/env python3
"""Create a separate shared MusicXML input for an equal-staff-size comparison.

Only explicit staff-details/staff-size percentages become 100. Keep the authored
input and its evaluation separately. This is a physical-size control, not an
import-fidelity fix or permission to remove notation from a comparison.
Usage: normalize_print_staff_size.py INPUT OUTPUT RECEIPT
"""
import copy
import hashlib
import json
import math
from pathlib import Path
import sys
import xml.etree.ElementTree as ET

from audit_print_musicxml import xml_events
from compare_print_corpus import source_root


def normalize(source, output):
    if source.resolve() == output.resolve():
        raise ValueError("the authored source must remain separate")
    original = source_root(source)
    root = copy.deepcopy(original)
    changes = []
    for part in root.findall("part"):
        for measure in part.findall("measure"):
            for details in measure.findall("attributes/staff-details"):
                for size in details.findall("staff-size"):
                    value = float(size.text or "")
                    if not math.isfinite(value) or value <= 0:
                        raise ValueError("invalid authored staff-size")
                    changes.append({"part": part.get("id"), "measure": measure.get("number"),
                                    "staff": details.get("number", "1"),
                                    "authored_percent": value, "comparison_percent": 100})
                    size.text = "100"
    if xml_events(original) != xml_events(root):
        raise ValueError("physical-size normalization changed pitched events")
    # Restore just the permitted text edits and compare the complete parsed tree.
    restored = copy.deepcopy(root)
    before = original.findall("part/measure/attributes/staff-details/staff-size")
    after = restored.findall("part/measure/attributes/staff-details/staff-size")
    if len(before) != len(after):
        raise ValueError("staff-size structure changed")
    for old, new in zip(before, after):
        new.text = old.text
    if ET.tostring(original) != ET.tostring(restored):
        raise ValueError("normalization changed other source content")
    data = ET.tostring(root, encoding="utf-8", xml_declaration=True)
    if output.exists() and output.read_bytes() != data:
        raise ValueError("refusing to overwrite different comparison input")
    output.parent.mkdir(parents=True, exist_ok=True)
    output.write_bytes(data)
    return {"classification": "shared equal-staff-size comparison input; authored output remains separate",
            "source": str(source), "source_sha256": hashlib.sha256(source.read_bytes()).hexdigest(),
            "output": str(output), "output_sha256": hashlib.sha256(data).hexdigest(),
            "staff_size_changes": changes, "all_other_parsed_content_unchanged": True}


if __name__ == "__main__":
    source, output, receipt = map(Path, sys.argv[1:])
    receipt.write_text(json.dumps(normalize(source, output), ensure_ascii=False, indent=2) + "\n")
