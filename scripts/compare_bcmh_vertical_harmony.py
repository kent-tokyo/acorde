#!/usr/bin/env python3
"""Screen vertical Roman labels against the pinned BCMH annotation spine.

Only measures with one Acorde sonority per source data row are scored. Other
measures remain in the denominator as unaligned, rather than shifting labels.
This is a development-corpus screening, not a music21 accuracy comparison.
"""

import argparse
import hashlib
import json
import re
import tempfile
import zipfile
from collections import defaultdict
from pathlib import Path

from compare_bcmh_keys import ARCHIVE_SHA256, acorde_output, remove_harmony_spine


def source_harmony_rows(lines):
    rows = defaultdict(list)
    measure = 0
    seen_data = False
    for line in lines:
        if line.startswith("="):
            if seen_data:
                measure += 1
            seen_data = False
            continue
        if not line or line.startswith(("!", "*")):
            continue
        rows[measure].append(line.split("\t", 1)[0])
        seen_data = True
    return rows


def clean_label(label):
    return label.rstrip(";,")


def compare_one(archive, name, directory, acorde, infer_key):
    source = archive.read(name)
    lines = source.decode("utf-8-sig").splitlines()
    cleaned = remove_harmony_spine(lines)
    path = directory / Path(name).name
    path.write_text(cleaned, encoding="utf-8")
    row = {
        "score": path.name,
        "source_sha256": hashlib.sha256(source).hexdigest(),
        "notes_only_sha256": hashlib.sha256(cleaned.encode()).hexdigest(),
    }
    try:
        options = ("--infer-key",) if infer_key else ()
        report = json.loads(acorde_output(acorde, "analyze-vertical", path, *options))
    except Exception as error:
        row["error"] = f"{type(error).__name__}: {error}"
        return row
    source_rows = source_harmony_rows(lines)
    predictions = defaultdict(list)
    for sonority in report["sonorities"]:
        predictions[sonority["measure"]].append(sonority["label"].get("roman_numeral"))
    measures = sorted(set(source_rows) | set(predictions))
    row["source_rows"] = sum(map(len, source_rows.values()))
    row["source_labeled_rows"] = sum(
        label not in {".", "*", ""} for values in source_rows.values() for label in values
    )
    row["predicted_sonorities"] = sum(map(len, predictions.values()))
    row["aligned_measures"] = 0
    row["unaligned_measures"] = 0
    row["labeled_rows"] = 0
    row["exact_labels"] = 0
    row["errors"] = []
    for measure in measures:
        expected = source_rows[measure]
        predicted = predictions[measure]
        if len(expected) != len(predicted):
            row["unaligned_measures"] += 1
            if len(row["errors"]) < 12:
                row["errors"].append({
                    "measure": measure, "reason": "row_count",
                    "source_rows": len(expected), "predicted_sonorities": len(predicted),
                })
            continue
        row["aligned_measures"] += 1
        for index, (gold, actual) in enumerate(zip(expected, predicted)):
            if gold in {".", "*", ""}:
                continue
            row["labeled_rows"] += 1
            gold = clean_label(gold)
            if actual == gold:
                row["exact_labels"] += 1
            elif len(row["errors"]) < 12:
                row["errors"].append({
                    "measure": measure, "row": index,
                    "gold": gold, "acorde": actual,
                })
    return row


def main():
    parser = argparse.ArgumentParser(description=__doc__)
    parser.add_argument("archive", type=Path)
    parser.add_argument("acorde", type=Path)
    parser.add_argument("output", type=Path)
    parser.add_argument("--infer-key", action="store_true",
                        help="use Acorde's first Aarden-Essen global-key candidate")
    args = parser.parse_args()
    archive_bytes = args.archive.read_bytes()
    if hashlib.sha256(archive_bytes).hexdigest() != ARCHIVE_SHA256:
        parser.error("BCMH archive SHA-256 does not match the pinned version")
    with zipfile.ZipFile(args.archive) as archive:
        names = sorted(name for name in archive.namelist()
                       if re.fullmatch(r"BCMH_dataset/annotated/chor\d{3}_rev\.krn", name))
        if len(names) != 100:
            parser.error(f"expected 100 chorales, found {len(names)}")
        with tempfile.TemporaryDirectory(prefix="acorde-bcmh-harmony-") as temporary:
            rows = []
            for name in names:
                try:
                    rows.append(compare_one(
                        archive, name, Path(temporary), args.acorde, args.infer_key
                    ))
                except Exception as error:
                    rows.append({"score": Path(name).name,
                                 "error": f"{type(error).__name__}: {error}"})
    parsed = [row for row in rows if "error" not in row]
    summary = {
        "total_scores": len(rows),
        "parsed_scores": len(parsed),
        "source_rows": sum(row["source_rows"] for row in parsed),
        "source_labeled_rows": sum(row["source_labeled_rows"] for row in parsed),
        "predicted_sonorities": sum(row["predicted_sonorities"] for row in parsed),
        "aligned_measures": sum(row["aligned_measures"] for row in parsed),
        "unaligned_measures": sum(row["unaligned_measures"] for row in parsed),
        "labeled_rows_in_aligned_measures": sum(row["labeled_rows"] for row in parsed),
        "exact_labels_in_aligned_measures": sum(row["exact_labels"] for row in parsed),
    }
    report = {
        "contract_version": 1,
        "corpus": "MusicCogCompLab/BCMH@97dfc249b90a8739457d83a2702644173c7eb4a8",
        "archive_sha256": ARCHIVE_SHA256,
        "acorde_binary_sha256": hashlib.sha256(args.acorde.read_bytes()).hexdigest(),
        "selection": "source rows in measures with equal sonority and data-row counts",
        "key_source": "aarden-essen-first-candidate" if args.infer_key else "written",
        "summary": summary,
        "rows": rows,
    }
    args.output.write_text(json.dumps(report, indent=2) + "\n", encoding="utf-8")
    print(json.dumps(summary, sort_keys=True))


if __name__ == "__main__":
    main()
