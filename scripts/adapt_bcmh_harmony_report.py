#!/usr/bin/env python3
"""Project a complete BCMH harmony comparison into the label evaluator contract.

The source comparison must use contract 4, which retains every aligned gold
position and both predictions. Its older bounded error examples cannot be
used to reconstruct a label-level manifest. The pinned BCMH archive is read
again to verify every stripped musical input before a manifest is written.
"""

import argparse
from fractions import Fraction
import hashlib
import io
import json
import os
from pathlib import Path
import re
import zipfile

from compare_bcmh_harmony_music21 import canonical_roman
from compare_bcmh_keys import ARCHIVE_SHA256, remove_harmony_spine


TASK = "roman_at_source_annotated_onset"
MEMBER = re.compile(r"BCMH_dataset/annotated/(chor\d{3}_rev\.krn)\Z")


def digest(data):
    return hashlib.sha256(data).hexdigest()


def event_position(measure, beat):
    if type(measure) is not int or measure < 0 or type(beat) not in (int, float):
        raise ValueError("invalid source position")
    fraction = Fraction(str(beat))
    if fraction < 0:
        raise ValueError("negative source beat")
    return {"part": 0, "staff": 0, "measure": measure, "beat": str(fraction)}


def aligned_labels(row):
    source = row.get("aligned_rows")
    if not isinstance(source, list) or len(source) != row.get("labeled_rows"):
        raise ValueError(f"{row['score']}: incomplete aligned_rows")
    gold = []
    acorde = []
    music21 = []
    positions = set()
    for entry in source:
        at = event_position(entry.get("measure"), entry.get("beat"))
        position = (at["measure"], at["beat"])
        if position in positions:
            raise ValueError(f"{row['score']}: duplicate source position {position}")
        positions.add(position)
        expected = canonical_roman(entry.get("gold"))
        if not expected:
            raise ValueError(f"{row['score']}: missing gold label")
        gold.append({"task": TASK, "at": at, "labels": [expected]})
        for name, predictions in (("acorde", acorde), ("music21", music21)):
            label = canonical_roman(entry.get(name))
            if label:
                predictions.append({"task": TASK, "at": at, "label": label})
    if (len(acorde) != row.get("acorde_predicted_rows")
            or len(music21) != row.get("music21_predicted_rows")):
        raise ValueError(f"{row['score']}: prediction count mismatch")
    for name, predictions in (("acorde", acorde), ("music21", music21)):
        predicted = {(p["at"]["measure"], p["at"]["beat"]): p["label"]
                     for p in predictions}
        correct = sum(predicted.get((g["at"]["measure"], g["at"]["beat"]))
                      == g["labels"][0] for g in gold)
        if correct != row.get(f"{name}_canonical_exact"):
            raise ValueError(f"{row['score']}: canonical exact count mismatch for {name}")
    return gold, acorde, music21


def exclusion(row, name):
    if "error" in row:
        return f"comparison_error: {row['error']}"
    if f"{name}_error" in row:
        return f"parse_error: {row[f'{name}_error']}"
    if "music21_error" in row or "acorde_error" in row:
        return "comparison_excluded_after_peer_parse_error"
    if row.get("pitched_timeline_equal") is False:
        return "comparison_excluded: unequal_pitched_timeline"
    return "comparison_excluded: no_aligned_rows"


def adapt(report, archive, *, report_sha256, input_dir=None, manifest_dir=None):
    if report.get("contract_version") != 4:
        raise ValueError("BCMH source comparison must use contract_version 4")
    if report.get("archive_sha256") != ARCHIVE_SHA256 or digest(archive) != ARCHIVE_SHA256:
        raise ValueError("pinned BCMH archive SHA-256 mismatch")
    if report.get("music21_version") != "10.5.2":
        raise ValueError("expected music21 10.5.2")
    binary_sha = report.get("acorde_binary_sha256")
    if not isinstance(binary_sha, str) or not re.fullmatch(r"[0-9a-f]{64}", binary_sha):
        raise ValueError("missing Acorde binary SHA-256")
    rows = report.get("rows")
    if not isinstance(rows, list) or len(rows) != 100:
        raise ValueError("expected 100 BCMH comparison rows")
    by_name = {row["score"]: row for row in rows}
    if len(by_name) != len(rows):
        raise ValueError("duplicate comparison score")
    if input_dir is not None and manifest_dir is None:
        raise ValueError("manifest_dir is needed with input_dir")

    cases = []
    with zipfile.ZipFile(io.BytesIO(archive)) as package:
        members = sorted((name, match.group(1)) for name in package.namelist()
                         if (match := MEMBER.fullmatch(name)))
        if len(members) != 100 or {name for _, name in members} != set(by_name):
            raise ValueError("archive score list differs from source comparison")
        for member, name in members:
            row = by_name[name]
            raw = package.read(member)
            cleaned = remove_harmony_spine(raw.decode("utf-8-sig").splitlines()).encode()
            if row.get("source_sha256", digest(raw)) != digest(raw):
                raise ValueError(f"{name}: source SHA-256 mismatch")
            if row.get("notes_only_sha256", digest(cleaned)) != digest(cleaned):
                raise ValueError(f"{name}: stripped-input SHA-256 mismatch")
            case = {"id": name, "input_sha256": digest(cleaned),
                    "split": "development"}
            if input_dir is not None:
                path = input_dir / name
                path.parent.mkdir(parents=True, exist_ok=True)
                if path.exists() and path.read_bytes() != cleaned:
                    raise ValueError(f"{name}: input directory has different content")
                path.write_bytes(cleaned)
                case["input_path"] = os.path.relpath(path, manifest_dir)
            if row.get("pitched_timeline_equal") is True:
                gold, acorde, music21 = aligned_labels(row)
                case["gold"] = gold
                case["systems"] = {"acorde": acorde, "music21": music21}
            else:
                case["gold"] = []
                case["systems"] = {
                    name: {"error": exclusion(row, name)}
                    for name in ("acorde", "music21")
                }
            cases.append(case)
    summary = report.get("summary", {})
    eligible = [row for row in rows if row.get("pitched_timeline_equal") is True]
    for field, value in (
        ("equal_pitched_timelines", len(eligible)),
        ("labeled_rows_in_aligned_measures", sum(len(row["aligned_rows"]) for row in eligible)),
        ("acorde_canonical_exact", sum(row["acorde_canonical_exact"] for row in eligible)),
        ("music21_canonical_exact", sum(row["music21_canonical_exact"] for row in eligible)),
    ):
        if summary.get(field) != value:
            raise ValueError(f"summary mismatch: {field}")
    return {
        "schema_version": 1,
        "system_profiles": {
            "acorde": {
                "version": f"binary-sha256:{binary_sha}",
                "command": "acorde analyze-vertical <cleaned.kern> --infer-key"
                           + (" --arpeggiated-bars" if report.get("arpeggiated_bars") else ""),
                "options": {"key": "independently inferred", "input": "same stripped kern",
                            "arpeggiated_bars": bool(report.get("arpeggiated_bars"))},
            },
            "music21": {
                "version": "10.5.2",
                "command": "score.analyze('key'); roman.romanNumeralFromChord(active, key)",
                "options": {"key": "independently inferred", "input": "same stripped kern"},
            },
        },
        "provenance": {
            "corpus": report.get("corpus"),
            "archive_sha256": ARCHIVE_SHA256,
            "source_report_sha256": report_sha256,
            "task": TASK,
            "split_reason": "previously inspected development corpus",
            "eligibility": "equal pitched timelines and source beats matching music21 onsets",
            "normalization": "percent/half-diminished, degree/o, terminal 42/2 only",
            "reference_position": "part 0 staff 0 measure and rounded source beat",
            "excluded_source_rows_in_equal_timeline_scores":
                summary["source_labeled_rows"]
                - summary["labeled_rows_in_aligned_measures"],
            "comparison_summary": summary,
        },
        "cases": cases,
    }


def main():
    parser = argparse.ArgumentParser(description=__doc__)
    parser.add_argument("report", type=Path)
    parser.add_argument("archive", type=Path)
    parser.add_argument("output", type=Path)
    parser.add_argument("--input-dir", type=Path,
                        help="write stripped input files for evaluator SHA-256 readback")
    args = parser.parse_args()
    raw = args.report.read_bytes()
    manifest = adapt(json.loads(raw), args.archive.read_bytes(),
                     report_sha256=digest(raw), input_dir=args.input_dir,
                     manifest_dir=args.output.parent)
    args.output.write_text(json.dumps(manifest, indent=2) + "\n", encoding="utf-8")
    print(json.dumps({"cases": len(manifest["cases"]),
                      "gold_rows": sum(len(c["gold"]) for c in manifest["cases"]),
                      "verified_inputs": sum("input_path" in c for c in manifest["cases"])},
                     sort_keys=True))


if __name__ == "__main__":
    main()
