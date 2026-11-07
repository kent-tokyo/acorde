#!/usr/bin/env python3
"""Frozen Mozart sonata key holdout, separate from the BCMH development corpus.

Usage: uv run --no-project --with music21==10.5.2 python3 \
    scripts/compare_mozart_keys.py SONATAS.zip /path/to/mscore /path/to/acorde \
    WORK_DIR OUT.json

All 54 annotated MS3 movements in the pinned archive are processed. K279-1 was
used to check the export command before this protocol was fixed, so the other
53 movements form the primary untouched holdout. Do not select scores or tune
the key ranker after inspecting this report.
"""

import argparse
import csv
import hashlib
import io
import json
import platform
import subprocess
import sys
import zipfile
import xml.etree.ElementTree as ET
from collections import Counter
from pathlib import Path

from compare_bcmh_keys import acorde_events, acorde_key_name, music21_events, pitch_bag


COMMIT = "5337257a5318711e6302cfe85c3f1a6ade3c6271"
ROOT = f"mozart_piano_sonatas-{COMMIT}"
ARCHIVE_SHA256 = "32c9698aecc398a7a155c4b25c6da848e88d34ed6f0ade4cead3f6a2a0477101"
SMOKE_SCORE = "K279-1"


def sha256(data):
    return hashlib.sha256(data).hexdigest()


def run(command, timeout=90):
    result = subprocess.run(command, capture_output=True, text=True, timeout=timeout, check=False)
    if result.returncode:
        raise RuntimeError(result.stderr.strip()[-400:] or f"exit status {result.returncode}")
    return result.stdout


def export_score(command, source, output):
    """Accept complete MusicXML despite a nonzero MuseScore exit status."""
    failures = []
    for _ in range(3):
        output.unlink(missing_ok=True)
        result = subprocess.run(
            [*command, "--force", "--export-to", str(output), str(source)],
            capture_output=True, text=True, timeout=90, check=False,
        )
        if output.is_file() and output.stat().st_size:
            try:
                root = ET.parse(output).getroot()
                if root.tag in {"score-partwise", "score-timewise"}:
                    return result.returncode
                failures.append(f"unexpected MusicXML root {root.tag}")
            except ET.ParseError as error:
                failures.append(f"invalid MusicXML: {error}")
        else:
            failures.append(f"no MusicXML; exporter exit status {result.returncode}")
    raise RuntimeError("; ".join(failures))


def valid_musicxml(path):
    if not path.is_file() or not path.stat().st_size:
        return False
    try:
        return ET.parse(path).getroot().tag in {"score-partwise", "score-timewise"}
    except ET.ParseError:
        return False


def remove_harmony_annotations(source, output):
    """Keep sounding score data but remove analyst-provided chord labels."""
    tree = ET.parse(source)
    removed = 0
    for measure in tree.iterfind(".//measure"):
        for child in list(measure):
            if child.tag == "harmony":
                measure.remove(child)
                removed += 1
    tree.write(output, encoding="utf-8", xml_declaration=True)
    return removed


def gold_key(label):
    mode = "major" if label[0].isupper() else "minor"
    return f"{label[0].upper()}{label[1:]} {mode}"


def music21_key_name(key):
    tonic = key.tonic.name.replace("-", "b")
    return f"{tonic[0].upper()}{tonic[1:]} {key.mode}"


def compare_one(archive, metadata, mscore_command, acorde, work_dir, converter, note, chord,
                reuse_valid_musicxml):
    piece = metadata["piece"]
    source = archive.read(f"{ROOT}/MS3/{piece}.mscx")
    mscx = work_dir / f"{piece}.mscx"
    musicxml = work_dir / f"{piece}.musicxml"
    analysis_xml = work_dir / f"{piece}.analysis.musicxml"
    mscx.write_bytes(source)
    row = {
        "piece": piece,
        "primary": piece != SMOKE_SCORE,
        "gold": gold_key(metadata["annotated_key"]),
        "source_sha256": sha256(source),
    }
    try:
        if reuse_valid_musicxml and valid_musicxml(musicxml):
            row["export_origin"] = "existing_validated"
        else:
            row["export_origin"] = "fresh_export"
            row["export_exit_status"] = export_score(mscore_command, mscx, musicxml)
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
        # MusicXML harmony labels become zero-pitch ChordSymbol objects in
        # music21's .notes iterator. They are annotations, not sounded events.
        reference_events = [
            event for event in music21_events(score, note, chord) if event[0]
        ]
        row["music21"] = {
            "key": key,
            "exact": key == row["gold"],
            "events": len(reference_events),
        }
    except Exception as error:
        row["music21"] = {"error": f"{type(error).__name__}: {error}"}

    acorde_projection = None
    try:
        analysis = json.loads(run([str(acorde), "analyze", str(analysis_xml)]))
        ranked = json.loads(run([str(acorde), "analyze", str(analysis_xml), "--rank-tied-keys"]))
        import_report = json.loads(run([str(acorde), "report", str(analysis_xml)]))
        acorde_projection = acorde_events(import_report["score"])
        candidates = [acorde_key_name(candidate) for candidate in analysis["key_estimates"]]
        ranked_candidates = [
            acorde_key_name(candidate) for candidate in ranked["key_estimates"]
        ]
        row["acorde"] = {
            "candidates": candidates,
            "ranked_candidates": ranked_candidates,
            "default_first_exact": bool(candidates) and candidates[0] == row["gold"],
            "ranked_first_exact": bool(ranked_candidates)
            and ranked_candidates[0] == row["gold"],
            "candidate_contains_gold": row["gold"] in candidates,
            "events": len(acorde_projection),
            "import_diagnostic_codes": dict(sorted(Counter(
                diagnostic["code"] for diagnostic in import_report["diagnostics"]
            ).items())),
        }
    except Exception as error:
        row["acorde"] = {"error": f"{type(error).__name__}: {error}"}

    if reference_events is not None and acorde_projection is not None:
        row["event_counts_equal"] = len(reference_events) == len(acorde_projection)
        row["pitch_bags_equal"] = pitch_bag(reference_events) == pitch_bag(acorde_projection)
        row["notehead_pitch_duration_bags_equal"] = (
            Counter((pitch, duration) for pitches, duration in reference_events
                    for pitch in pitches)
            == Counter((pitch, duration) for pitches, duration in acorde_projection
                       for pitch in pitches)
        )
        row["pitch_duration_bags_equal"] = (
            Counter(reference_events) == Counter(acorde_projection)
        )
    return row


def summarize(rows):
    primary = [row for row in rows if row["primary"]]
    exported = [row for row in primary if "export_error" not in row]
    music21_parsed = [row for row in exported if "error" not in row["music21"]]
    acorde_parsed = [row for row in exported if "error" not in row["acorde"]]
    paired = [row for row in music21_parsed if "error" not in row["acorde"]]
    return {
        "total": len(rows),
        "smoke_excluded": SMOKE_SCORE,
        "primary": len(primary),
        "exported": len(exported),
        "reused_valid_musicxml": sum(
            row["export_origin"] == "existing_validated" for row in exported
        ),
        "harmony_elements_removed": sum(
            row["harmony_elements_removed"] for row in exported
        ),
        "fresh_exports_with_nonzero_exit": sum(
            row.get("export_exit_status", 0) != 0 for row in exported
        ),
        "music21_parsed": len(music21_parsed),
        "acorde_parsed": len(acorde_parsed),
        "paired": len(paired),
        "music21_exact_on_primary": sum(row["music21"]["exact"] for row in music21_parsed),
        "acorde_default_exact_on_primary": sum(
            row["acorde"]["default_first_exact"] for row in acorde_parsed
        ),
        "acorde_ranked_exact_on_primary": sum(
            row["acorde"]["ranked_first_exact"] for row in acorde_parsed
        ),
        "acorde_contains_gold_on_primary": sum(
            row["acorde"]["candidate_contains_gold"] for row in acorde_parsed
        ),
        "equal_event_counts_on_paired": sum(row["event_counts_equal"] for row in paired),
        "equal_pitch_bags_on_paired": sum(row["pitch_bags_equal"] for row in paired),
        "equal_notehead_pitch_duration_bags_on_paired": sum(
            row["notehead_pitch_duration_bags_equal"] for row in paired
        ),
        "equal_pitch_duration_bags_on_paired": sum(
            row["pitch_duration_bags_equal"] for row in paired
        ),
    }


def main():
    parser = argparse.ArgumentParser(description=__doc__)
    parser.add_argument("archive", type=Path)
    parser.add_argument("mscore", type=Path)
    parser.add_argument("acorde", type=Path)
    parser.add_argument("work_dir", type=Path)
    parser.add_argument("output", type=Path)
    parser.add_argument("--reuse-valid-musicxml", action="store_true",
                        help="reuse validated exports from earlier identical pinned runs")
    args = parser.parse_args()
    from music21 import chord, converter, note
    import music21

    if music21.__version__ != "10.5.2":
        parser.error(f"expected music21 10.5.2, got {music21.__version__}")
    if sha256(args.archive.read_bytes()) != ARCHIVE_SHA256:
        parser.error("Mozart archive SHA-256 does not match the frozen version")
    # Pin the native architecture of the universal MuseScore bundle on macOS ARM.
    mscore_command = [str(args.mscore)]
    if sys.platform == "darwin" and platform.machine() == "arm64":
        mscore_command = ["arch", "-arm64", str(args.mscore)]
    mscore_version = run([*mscore_command, "--version"]).strip()
    if mscore_version != "MuseScore4 4.7.5":
        parser.error(f"expected MuseScore4 4.7.5, got {mscore_version}")
    args.work_dir.mkdir(parents=True, exist_ok=True)
    with zipfile.ZipFile(args.archive) as archive:
        metadata = list(csv.DictReader(io.StringIO(
            archive.read(f"{ROOT}/metadata.tsv").decode("utf-8")
        ), delimiter="\t"))
        if len(metadata) != 54 or not all(row["annotated_key"] for row in metadata):
            parser.error("expected all 54 annotated Mozart movements")
        if {row["piece"] for row in metadata} != {
            Path(name).stem for name in archive.namelist()
            if name.startswith(f"{ROOT}/MS3/")
            and name.endswith(".mscx")
            and not Path(name).name.startswith("ambiguous_measure_numbers_for_")
        }:
            parser.error("annotated score files do not match metadata")
        rows = [compare_one(
            archive, row, mscore_command, args.acorde, args.work_dir,
            converter, note, chord, args.reuse_valid_musicxml,
        ) for row in metadata]
    report = {
        "corpus": f"DCMLab/mozart_piano_sonatas@{COMMIT}",
        "license": "CC BY-NC-SA 4.0",
        "archive_sha256": ARCHIVE_SHA256,
        "mscore_version": mscore_version,
        "mscore_command": mscore_command,
        "acorde_binary_sha256": sha256(args.acorde.read_bytes()),
        "music21_version": music21.__version__,
        "primary_selection": "all 54 annotated movements except K279-1 setup smoke, fixed before scoring",
        "reuse_valid_musicxml": args.reuse_valid_musicxml,
        "summary": summarize(rows),
        "rows": rows,
    }
    args.output.write_text(json.dumps(report, indent=2, ensure_ascii=False) + "\n")
    print(json.dumps(report["summary"], sort_keys=True))


if __name__ == "__main__":
    main()
