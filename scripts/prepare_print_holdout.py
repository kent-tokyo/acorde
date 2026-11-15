#!/usr/bin/env python3
"""Prepare shared MusicXML independently of Acorde after a candidate freeze.

Usage: prepare_print_holdout.py SELECTION SOURCES FREEZE OUT MUSESCORE
The source hash/Git-blob inventory is verified before any score is parsed.
MuseScore 4.7.5 exports MSCX and extracted parts. A known macOS teardown crash
is recorded, never treated as a clean exporter exit or a fidelity pass.
"""
import base64
import collections
import copy
import datetime
import hashlib
import json
import pathlib
import subprocess
import sys
import xml.etree.ElementTree as ET

from audit_print_musicxml import xml_events
from compare_print_corpus import source_root, ROOT
from fetch_print_holdout import validate_selection


def sha(path):
    return hashlib.sha256(path.read_bytes()).hexdigest()


def run_export(command, output, log):
    if output.exists():
        raise ValueError(f"refusing to reuse exporter output: {output}")
    result = subprocess.run(command, capture_output=True, timeout=180)
    log.write_bytes(result.stdout + result.stderr)
    exists = output.is_file() and output.stat().st_size > 0
    known_teardown = result.returncode in (-6, 134) and b"mutex lock failed: Invalid argument" in result.stderr
    if not exists or (result.returncode != 0 and not known_teardown):
        raise RuntimeError(f"export failed ({result.returncode}); see {log.name}")
    return {"command": command, "exit_code": result.returncode,
            "known_teardown_crash": known_teardown, "log": log.name,
            "log_sha256": sha(log), "output_sha256": sha(output),
            "fidelity": "not established by successful output parsing"}


def selected_events(root, index):
    selected = copy.deepcopy(root)
    for i, part in enumerate(list(selected.findall("part"))):
        if i != index:
            selected.remove(part)
    # MuseScore may renumber source voice IDs while preserving their lanes.
    return xml_events(selected)


def main():
    selection_path, sources, freeze_path, out, mscore = map(pathlib.Path, sys.argv[1:])
    selection = json.loads(selection_path.read_text())
    validate_selection(selection)
    freeze = json.loads(freeze_path.read_text())
    if freeze["selection_sha256"] != sha(selection_path):
        raise ValueError("selection changed since candidate freeze")
    for relative, digest in freeze["source_files_sha256"].items():
        if sha(ROOT / relative) != digest:
            raise ValueError(f"candidate source changed: {relative}")
    paths = []
    for work in selection["works"]:
        source = sources / work["group"] / (work["id"] + pathlib.Path(work["path"]).suffix)
        data = source.read_bytes()
        blob = hashlib.sha1(f"blob {len(data)}\0".encode() + data).hexdigest()
        if len(data) != work["bytes"] or blob != work["git_blob_sha1"]:
            raise ValueError(f"source identity mismatch: {source}")
        paths.append(source)
    out.mkdir(parents=True, exist_ok=True)
    report = {"classification": "reserved source preparation; no quality pass",
              "freeze_sha256": sha(freeze_path), "musescore_binary_sha256": sha(mscore),
              "opened_at_utc": datetime.datetime.now(datetime.timezone.utc).isoformat(), "rows": []}
    for work, source in zip(selection["works"], paths):
        name = work["group"] + "-" + work["id"]
        row = {"score": name, "group": work["group"], "id": work["id"],
               "source_sha256": sha(source), "source": str(source), "exports": []}
        shared = out / (name + ".musicxml")
        try:
            if work["group"] == "part":
                parts_json = out / (name + "-parts.json")
                row["exports"].append(run_export(["arch", "-arm64", str(mscore), "--score-parts",
                    "-o", str(parts_json), str(source)], parts_json, out / (name + "-parts.log")))
                parts = json.loads(parts_json.read_text())
                index = work["part_index"]
                if len(parts["parts"]) != len(source_root(source).findall("part")):
                    raise ValueError("exported part inventory differs from source")
                row["part_index"] = index
                row["part_name"] = parts["parts"][index]
                part_mscz = out / (name + ".mscz")
                part_mscz.write_bytes(base64.b64decode(parts["partsBin"][index], validate=True))
                row["exports"].append(run_export(["arch", "-arm64", str(mscore), "-o", str(shared), str(part_mscz)],
                    shared, out / (name + "-musicxml.log")))
                prepared = source_root(shared)
                expected = selected_events(source_root(source), index)
                actual = xml_events(prepared)
                row["pitched_event_audit"] = {"source": sum(expected.values()), "prepared": sum(actual.values()),
                    "missing": [{"event": e, "count": n} for e, n in (expected-actual).items()],
                    "extra": [{"event": e, "count": n} for e, n in (actual-expected).items()]}
            elif source.suffix == ".mscx":
                row["exports"].append(run_export(["arch", "-arm64", str(mscore), "-o", str(shared), str(source)],
                    shared, out / (name + "-musicxml.log")))
                prepared = source_root(shared)
                original = ET.parse(source).getroot()
                # This coarse inventory detects lost/added notes, not duration,
                # voice, spelling, expression, or geometry fidelity.
                pitches = collections.Counter(n.text for n in original.findall("./Score/Staff/Measure/voice/Chord/Note/pitch"))
                row["mscx_midi_pitch_inventory"] = dict(pitches)
                row["prepared_pitched_events"] = sum(xml_events(prepared).values())
            else:
                prepared = source_root(source)
                ET.indent(prepared)
                ET.ElementTree(prepared).write(shared, encoding="utf-8", xml_declaration=True)
                if xml_events(source_root(source)) != xml_events(source_root(shared)):
                    raise ValueError("MXL unpacking changed events")
            if prepared.tag != "score-partwise" or not prepared.findall("part"):
                raise ValueError("expected nonempty partwise MusicXML")
            row.update(status="prepared", shared_input=str(shared), shared_input_sha256=sha(shared),
                       parts=len(prepared.findall("part")),
                       measures=[len(p.findall("measure")) for p in prepared.findall("part")])
        except Exception as error:
            row.update(status="failed", error=str(error))
        report["rows"].append(row)
        (out / "preparation.json").write_text(json.dumps(report, indent=2) + "\n")
        print(name, row["status"], flush=True)


if __name__ == "__main__":
    main()
