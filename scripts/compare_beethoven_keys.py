#!/usr/bin/env python3
"""Untouched Beethoven sonata key holdout for Acorde and music21 10.5.2.

Usage: uv run --no-project --with music21==10.5.2 python3 \
    scripts/compare_beethoven_keys.py ARCHIVE.zip /path/to/mscore /path/to/acorde \
    WORK_DIR OUT.json

The pinned v2.3 archive's 64 movements with an annotated key are fixed before
any result is inspected. Parse/export failures remain in the denominator.
"""

import argparse
import csv
import hashlib
import io
import json
import subprocess
import zipfile
from collections import Counter
from pathlib import Path

from compare_bcmh_keys import acorde_events, acorde_key_name, music21_events
from compare_mozart_keys import (
    export_score, gold_key, music21_key_name, remove_harmony_annotations,
    valid_musicxml,
)


COMMIT = "a10cb43e229db57e3c5e3649d325bee08ffe4d0f"
ROOT = "beethoven_piano_sonatas-2.3"
ARCHIVE_SHA256 = "078e6af64e0fe56075e0220d522a5cf38778f2fbb15c93851f5ff070eb9861a1"


def sha256(data):
    return hashlib.sha256(data).hexdigest()


def run(command):
    result = subprocess.run(command, capture_output=True, text=True, timeout=180)
    if result.returncode:
        raise RuntimeError(result.stderr.strip()[-400:] or f"exit status {result.returncode}")
    return result.stdout


def compare_one(archive, metadata, mscore, acorde, work_dir, converter, note, chord,
                reuse_valid_musicxml):
    piece = metadata["piece"]
    source = archive.read(f"{ROOT}/MS3/{piece}.mscx")
    mscx = work_dir / f"{piece}.mscx"
    musicxml = work_dir / f"{piece}.musicxml"
    analysis_xml = work_dir / f"{piece}.analysis.musicxml"
    mscx.write_bytes(source)
    row = {"piece": piece, "gold": gold_key(metadata["annotated_key"]),
           "source_sha256": sha256(source)}
    try:
        if reuse_valid_musicxml and valid_musicxml(musicxml):
            row["export_origin"] = "existing_validated"
        else:
            row["export_origin"] = "fresh_export"
            row["export_exit_status"] = export_score([str(mscore)], mscx, musicxml)
        row["musicxml_sha256"] = sha256(musicxml.read_bytes())
        row["harmony_elements_removed"] = remove_harmony_annotations(
            musicxml, analysis_xml
        )
        row["analysis_xml_sha256"] = sha256(analysis_xml.read_bytes())
    except Exception as error:
        row["export_error"] = f"{type(error).__name__}: {error}"
        return row

    reference_events = None
    try:
        score = converter.parse(str(analysis_xml), forceSource=True)
        key = music21_key_name(score.analyze("key"))
        reference_events = music21_events(score, note, chord)
        row["music21"] = {"key": key, "exact": key == row["gold"],
                          "events": len(reference_events)}
    except Exception as error:
        row["music21"] = {"error": f"{type(error).__name__}: {error}"}
    try:
        analysis = json.loads(run([str(acorde), "analyze", str(analysis_xml),
                                   "--aarden-essen-keys"]))
        report = json.loads(run([str(acorde), "report", str(analysis_xml)]))
        candidates = [acorde_key_name(item) for item in analysis["key_estimates"]]
        own_events = acorde_events(report["score"])
        row["acorde"] = {"key": candidates[0] if candidates else None,
                         "exact": bool(candidates) and candidates[0] == row["gold"],
                         "events": len(own_events),
                         "import_diagnostics": len(report["diagnostics"]),
                         "diagnostics_by_code": dict(sorted(Counter(
                             item["code"] for item in report["diagnostics"]
                         ).items()))}
        if reference_events is not None:
            row["same_event_bag"] = Counter(reference_events) == Counter(own_events)
    except Exception as error:
        row["acorde"] = {"error": f"{type(error).__name__}: {error}"}
    return row


def summarize(rows):
    common = [row for row in rows if row.get("music21", {}).get("key")
              and row.get("acorde", {}).get("key")]
    return {
        "total": len(rows),
        "exported": sum("musicxml_sha256" in row for row in rows),
        "music21_parsed": sum(bool(row.get("music21", {}).get("key")) for row in rows),
        "acorde_parsed": sum(bool(row.get("acorde", {}).get("key")) for row in rows),
        "paired": len(common),
        "same_event_bag": sum(row.get("same_event_bag", False) for row in common),
        "same_key": sum(row["music21"]["key"] == row["acorde"]["key"] for row in common),
        "music21_exact": sum(row["music21"]["exact"] for row in common),
        "acorde_exact": sum(row["acorde"]["exact"] for row in common),
    }


def main():
    parser = argparse.ArgumentParser(description=__doc__)
    parser.add_argument("archive", type=Path)
    parser.add_argument("mscore", type=Path)
    parser.add_argument("acorde", type=Path)
    parser.add_argument("work_dir", type=Path)
    parser.add_argument("output", type=Path)
    parser.add_argument("--reuse-valid-musicxml", action="store_true")
    args = parser.parse_args()
    from music21 import chord, converter, note
    import music21

    if music21.__version__ != "10.5.2":
        parser.error(f"expected music21 10.5.2, got {music21.__version__}")
    if sha256(args.archive.read_bytes()) != ARCHIVE_SHA256:
        parser.error("Beethoven archive SHA-256 does not match v2.3")
    mscore_version = run([str(args.mscore), "--version"]).strip()
    if mscore_version != "MuseScore4 4.7.5":
        parser.error(f"expected MuseScore4 4.7.5, got {mscore_version}")
    args.work_dir.mkdir(parents=True, exist_ok=True)
    with zipfile.ZipFile(args.archive) as archive:
        metadata = list(csv.DictReader(io.StringIO(
            archive.read(f"{ROOT}/metadata.tsv").decode("utf-8")), delimiter="\t"))
        selected = sorted((row for row in metadata if row["annotated_key"]),
                          key=lambda row: row["piece"])
        if len(metadata) != 91 or len(selected) != 64:
            parser.error("expected 91 movements and 64 annotated keys")
        rows = []
        for item in selected:
            try:
                row = compare_one(archive, item, args.mscore, args.acorde, args.work_dir,
                                  converter, note, chord, args.reuse_valid_musicxml)
            except Exception as error:
                row = {"piece": item["piece"], "error": f"{type(error).__name__}: {error}"}
            rows.append(row)
            report = {"corpus": f"DCMLab/beethoven_piano_sonatas@{COMMIT}",
                      "license": "CC BY-NC-SA 4.0", "archive_sha256": ARCHIVE_SHA256,
                      "mscore_version": mscore_version,
                      "music21_version": music21.__version__,
                      "acorde_binary_sha256": sha256(args.acorde.read_bytes()),
                      "selection": "all 64 movements with annotated_key in v2.3 metadata.tsv",
                      "summary": summarize(rows), "rows": rows}
            args.output.write_text(json.dumps(report, indent=2) + "\n")
            print(f"{item['piece']}: {len(rows)}/{len(selected)}", flush=True)
    print(json.dumps(report["summary"], sort_keys=True))


if __name__ == "__main__":
    main()
