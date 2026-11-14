#!/usr/bin/env python3
"""Compare source-positioned analysis labels with an independent gold manifest.

Input is one JSON object with `system_profiles` and `cases`. Each system profile
records `version`, `command`, and `options`. Each case contains `id`, `input_sha256`,
`split` (development, calibration, or holdout), `gold`, and `systems`. Each
system is either a list of predictions or {"error": "..."}. Gold rows have
`task`, `at`, and `labels` (accepted alternatives); predictions have `task`,
`at`, and `label`. `at` is "score" or a structured object:

    {"part": 0, "staff": 0, "measure": 2, "beat": "1/2", "voice": 0}

Spans use {"start": <position>, "end": <position>}. Beat fractions are reduced
before comparison. Inputs and outputs are data; this script never parses a score.
"""

from __future__ import annotations

import argparse
from collections import Counter
from fractions import Fraction
import hashlib
import json
from pathlib import Path
import re
import sys


SCHEMA_VERSION = 1
SPLITS = {"development", "calibration", "holdout"}
SHA256 = re.compile(r"[0-9a-f]{64}\Z")


def position_key(value):
    if value == "score":
        return ("score",)
    if not isinstance(value, dict):
        raise ValueError("position must be 'score' or an object")
    if set(value) == {"start", "end"}:
        start, end = position_key(value["start"]), position_key(value["end"])
        if (start[0] != "event" or end[0] != "event"
                or (start[3], Fraction(start[4], start[5]))
                >= (end[3], Fraction(end[4], end[5]))):
            raise ValueError("span must contain ordered event positions")
        return ("span", start, end)
    required = {"part", "staff", "measure", "beat"}
    if not required <= set(value) or set(value) - required - {"voice"}:
        raise ValueError("event position needs part, staff, measure, beat, optional voice")
    numbers = [value[field] for field in ("part", "staff", "measure")]
    if any(type(number) is not int or number < 0 for number in numbers):
        raise ValueError("part, staff, and measure must be nonnegative integers")
    voice = value.get("voice")
    if voice is not None and (type(voice) is not int or voice < 0):
        raise ValueError("voice must be a nonnegative integer")
    beat = value["beat"]
    if not isinstance(beat, str) or not re.fullmatch(r"\d+(?:/\d+)?", beat):
        raise ValueError("beat must be a nonnegative integer or fraction string")
    try:
        beat = Fraction(beat)
    except ZeroDivisionError as error:
        raise ValueError("beat denominator must be positive") from error
    return ("event", *numbers, beat.numerator, beat.denominator, voice)


def label_key(row, *, gold):
    if not isinstance(row, dict) or not isinstance(row.get("task"), str) or not row["task"]:
        raise ValueError("each label needs a nonempty task")
    key = (row["task"], position_key(row.get("at")))
    if gold:
        labels = row.get("labels")
        if not isinstance(labels, list) or not labels or any(
            not isinstance(label, str) or not label for label in labels
        ) or len(set(labels)) != len(labels):
            raise ValueError("gold labels must be a nonempty list of unique strings")
        return key, frozenset(labels)
    label = row.get("label")
    if not isinstance(label, str) or not label:
        raise ValueError("prediction needs a nonempty label")
    return key, label


def indexed(rows, *, gold):
    if not isinstance(rows, list):
        raise ValueError("labels must be an array")
    result = {}
    for row in rows:
        key, label = label_key(row, gold=gold)
        if key in result:
            raise ValueError(f"duplicate task and position: {key!r}")
        result[key] = label
    return result


def measure(gold, predictions):
    matched = sorted(
        (key for key, labels in gold.items() if predictions.get(key) in labels),
        key=repr,
    )
    correct = len(matched)
    return {
        "gold": len(gold),
        "predicted": len(predictions),
        "correct": correct,
        "precision": correct / len(predictions) if predictions else None,
        "recall": correct / len(gold) if gold else None,
        "f1": 2 * correct / (len(predictions) + len(gold))
        if predictions or gold else None,
        "wrong_label": sum(key in predictions and predictions[key] not in labels
                           for key, labels in gold.items()),
        "missing": sum(key not in predictions for key in gold),
        "extra": sum(key not in gold for key in predictions),
    }


def task_measures(gold, predictions):
    tasks = sorted({key[0] for key in gold} | {key[0] for key in predictions})
    return {
        task: measure({key: value for key, value in gold.items() if key[0] == task},
                      {key: value for key, value in predictions.items() if key[0] == task})
        for task in tasks
    }


def verified_input(case, base_dir):
    path = case.get("input_path")
    if path is None:
        return False
    if not isinstance(path, str) or not path:
        raise ValueError("input_path must be a nonempty string")
    if base_dir is None:
        raise ValueError("base_dir is required to verify input_path")
    target = (base_dir / path).resolve()
    digest = hashlib.sha256()
    with target.open("rb") as source:
        for chunk in iter(lambda: source.read(1024 * 1024), b""):
            digest.update(chunk)
    if digest.hexdigest() != case["input_sha256"]:
        raise ValueError(f"{case['id']}: input SHA-256 mismatch: {target}")
    return True


def evaluate(manifest, *, base_dir=None):
    if not isinstance(manifest, dict) or manifest.get("schema_version") != SCHEMA_VERSION:
        raise ValueError("unsupported evaluation schema_version")
    cases = manifest.get("cases")
    if not isinstance(cases, list) or not cases:
        raise ValueError("cases must be a nonempty array")
    profiles = manifest.get("system_profiles")
    if not isinstance(profiles, dict) or not profiles:
        raise ValueError("system_profiles must be a nonempty object")
    for name, profile in profiles.items():
        if (not isinstance(name, str) or not name or not isinstance(profile, dict)
                or any(not isinstance(profile.get(field), str) or not profile[field]
                       for field in ("version", "command"))
                or not isinstance(profile.get("options"), dict)):
            raise ValueError("each system profile needs version, command, and options")
    provenance = manifest.get("provenance", {})
    if not isinstance(provenance, dict):
        raise ValueError("provenance must be an object")
    seen = set()
    systems = set()
    reports = []
    for case in cases:
        if not isinstance(case, dict):
            raise ValueError("case must be an object")
        case_id, digest, split = (case.get(field) for field in ("id", "input_sha256", "split"))
        if not isinstance(case_id, str) or not case_id or case_id in seen:
            raise ValueError("case ids must be unique nonempty strings")
        seen.add(case_id)
        if not isinstance(digest, str) or not SHA256.fullmatch(digest):
            raise ValueError(f"{case_id}: input_sha256 must be a lowercase SHA-256 hex digest")
        input_verified = verified_input(case, base_dir)
        if split not in SPLITS:
            raise ValueError(f"{case_id}: invalid split")
        gold = indexed(case.get("gold"), gold=True)
        case_systems = case.get("systems")
        if not isinstance(case_systems, dict) or not case_systems:
            raise ValueError(f"{case_id}: systems must be a nonempty object")
        if set(case_systems) != set(profiles):
            raise ValueError("every case must name exactly the profiled systems")
        systems = set(case_systems)
        results = {}
        for name, result in sorted(case_systems.items()):
            if not isinstance(name, str) or not name:
                raise ValueError("system names must be nonempty strings")
            if isinstance(result, dict) and set(result) == {"error"}:
                if not isinstance(result["error"], str) or not result["error"]:
                    raise ValueError("parse error must be a nonempty string")
                results[name] = {"status": "error", "error": result["error"], "gold": len(gold)}
                continue
            predictions = indexed(result, gold=False)
            results[name] = {"status": "ok", **measure(gold, predictions),
                             "tasks": task_measures(gold, predictions)}
        reports.append({"id": case_id, "input_sha256": digest,
                        "input_verified": input_verified, "split": split,
                        "systems": results})
    totals = {}
    for name in sorted(systems):
        by_split = {}
        for split in sorted(SPLITS):
            rows = [case["systems"][name] for case in reports if case["split"] == split]
            eligible = [row for row in rows if row["status"] == "ok"]
            counts = Counter()
            for row in eligible:
                counts.update({field: row[field] for field in
                               ("gold", "predicted", "correct", "wrong_label", "missing", "extra")})
            by_split[split] = {
                "cases": len(rows), "analyzed": len(eligible), "failed": len(rows) - len(eligible),
                **measure_from_counts(counts),
                "tasks": aggregate_tasks(eligible),
            }
        totals[name] = by_split
    return {"schema_version": SCHEMA_VERSION, "system_profiles": profiles,
            "provenance": provenance,
            "cases": reports, "totals": totals}


def measure_from_counts(counts):
    gold, predicted, correct = (counts[field] for field in ("gold", "predicted", "correct"))
    fields = ("gold", "predicted", "correct", "wrong_label", "missing", "extra")
    return {**{field: counts[field] for field in fields},
            "precision": correct / predicted if predicted else None,
            "recall": correct / gold if gold else None,
            "f1": 2 * correct / (predicted + gold) if predicted + gold else None}


def aggregate_tasks(rows):
    tasks = sorted({task for row in rows for task in row["tasks"]})
    return {task: measure_from_counts(Counter({
        field: sum(row["tasks"].get(task, {}).get(field, 0) for row in rows)
        for field in ("gold", "predicted", "correct", "wrong_label", "missing", "extra")
    })) for task in tasks}


def main():
    parser = argparse.ArgumentParser(description=__doc__)
    parser.add_argument("manifest", type=Path)
    parser.add_argument("--output", type=Path, help="write JSON here instead of stdout")
    args = parser.parse_args()
    try:
        report = evaluate(json.loads(args.manifest.read_text(encoding="utf-8")),
                          base_dir=args.manifest.parent)
    except (OSError, ValueError, TypeError) as error:
        parser.error(str(error))
    output = json.dumps(report, ensure_ascii=False, indent=2) + "\n"
    if args.output:
        args.output.write_text(output, encoding="utf-8")
    else:
        sys.stdout.write(output)


if __name__ == "__main__":
    main()
