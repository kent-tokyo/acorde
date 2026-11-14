#!/usr/bin/env python3
"""Prepare common, answer-free MusicXML inputs for the global-key holdout."""

import argparse
import hashlib
import json
import subprocess
import xml.etree.ElementTree as ET
import zipfile
from pathlib import Path

from compare_mozart_keys import export_score, valid_musicxml
from prepare_mozart_cadence_inputs import prepare


def sha256(data):
    return hashlib.sha256(data).hexdigest()


def unpack_mxl(data):
    from io import BytesIO

    with zipfile.ZipFile(BytesIO(data)) as archive:
        names = [name for name in archive.namelist()
                 if name.endswith((".xml", ".musicxml"))
                 and not name.startswith("META-INF/")]
        if len(names) != 1:
            raise ValueError(f"expected one MusicXML score in MXL: {names}")
        score = archive.read(names[0])
    if ET.fromstring(score).tag.rsplit("}", 1)[-1] not in {
        "score-partwise", "score-timewise"
    }:
        raise ValueError("MXL contains no MusicXML score")
    return score


def prepare_selection(selection, selection_sha, hashes, source_dir, export_dir,
                      output_dir, mscore, reuse_valid_exports=False):
    if selection_sha != hashes["selection_sha256"]:
        raise ValueError("source-hash manifest belongs to another selection")
    by_id = {(row["group"], row["piece"]): row for row in hashes["files"]}
    if len(by_id) != len(selection["works"]):
        raise ValueError("source-hash record count differs")
    records = []
    for work in selection["works"]:
        group, piece = work["group"], work["piece"]
        record = {"group": group, "piece": piece}
        try:
            source_record = by_id[(group, piece)]
            source = source_dir / source_record["local_path"]
            source_bytes = source.read_bytes()
            if sha256(source_bytes) != source_record["source_sha256"]:
                raise ValueError("source SHA-256 differs")
            if group == "chorale":
                exported = unpack_mxl(source_bytes)
                record["export_method"] = "single XML member from pinned MXL"
                record["export_exit_status"] = 0
            else:
                export_path = export_dir / group / f"{piece}.musicxml"
                export_path.parent.mkdir(parents=True, exist_ok=True)
                if reuse_valid_exports and valid_musicxml(export_path):
                    record["export_method"] = "existing validated MuseScore export"
                    record["export_exit_status"] = None
                else:
                    record["export_method"] = "MuseScore 4.7.5 export"
                    record["export_exit_status"] = export_score(
                        [str(mscore)], source, export_path)
                exported = export_path.read_bytes()
            cleaned, removed, repaired = prepare(exported)
            target = output_dir / group / f"{piece}.musicxml"
            target.parent.mkdir(parents=True, exist_ok=True)
            target.write_bytes(cleaned)
            record.update({
                "source_git_blob": work["score_git_blob"],
                "source_sha256": sha256(source_bytes),
                "export_sha256": sha256(exported),
                "input_path": str(target.relative_to(output_dir)),
                "input_sha256": sha256(cleaned),
                "removed_elements": removed,
                "repaired_elements": repaired,
            })
        except Exception as error:
            record["error"] = f"{type(error).__name__}: {error}"
        records.append(record)
        print(f"{group}/{piece}: {'error' if 'error' in record else 'prepared'}", flush=True)
    return {"schema_version": 1, "selection_sha256": selection_sha,
            "input_rule": ("Unpack pinned MXL or export pinned MSCX with MuseScore "
                           "4.7.5. Remove harmony, direction, lyric, and credit "
                           "elements; apply recorded input-only syntax repairs. "
                           "Give identical cleaned MusicXML bytes to both analyzers."),
            "inputs": records}


def main():
    parser = argparse.ArgumentParser(description=__doc__)
    parser.add_argument("selection", type=Path)
    parser.add_argument("source_hashes", type=Path)
    parser.add_argument("source_dir", type=Path)
    parser.add_argument("export_dir", type=Path)
    parser.add_argument("output_dir", type=Path)
    parser.add_argument("record", type=Path)
    parser.add_argument("--mscore", type=Path, required=True)
    parser.add_argument("--reuse-valid-exports", action="store_true")
    args = parser.parse_args()
    version = subprocess.run([str(args.mscore), "--version"], capture_output=True,
                             text=True, check=True).stdout.strip()
    if version != "MuseScore4 4.7.5":
        parser.error(f"expected MuseScore4 4.7.5, got {version}")
    selection_bytes = args.selection.read_bytes()
    selection = json.loads(selection_bytes)
    hashes = json.loads(args.source_hashes.read_text())
    result = prepare_selection(selection, sha256(selection_bytes), hashes,
                               args.source_dir, args.export_dir,
                               args.output_dir, args.mscore,
                               args.reuse_valid_exports)
    result["source_hashes_sha256"] = sha256(args.source_hashes.read_bytes())
    result["mscore_version"] = version
    args.record.write_text(json.dumps(result, ensure_ascii=False, indent=2) + "\n")
    print(f"prepared {sum('error' not in row for row in result['inputs'])}/"
          f"{len(result['inputs'])} common inputs")


if __name__ == "__main__":
    main()
