#!/usr/bin/env python3
"""Diagnose windowed local keys on inspected When in Rome development works."""

import argparse
import hashlib
import json
import math
import subprocess
from collections import Counter
from pathlib import Path

from compare_wir_harmony_music21 import acorde_fraction_beats
from compare_bcmh_keys import STEP_SEMITONES

MAJOR = [17.7661, 0.145624, 14.9265, 0.160186, 19.8049, 11.3587,
         0.291248, 22.062, 0.145624, 8.15494, 0.232998, 4.95122]
MINOR = [18.2648, 0.737619, 14.0499, 16.8599, 0.702494, 14.4362,
         0.702494, 18.6161, 4.56621, 1.93186, 7.37619, 1.75623]
MAJOR_NAMES = {-7: "C-", -6: "G-", -5: "D-", -4: "A-", -3: "E-", -2: "B-", -1: "F",
               0: "C", 1: "G", 2: "D", 3: "A", 4: "E", 5: "B", 6: "F#", 7: "C#"}
MINOR_NAMES = {-7: "a-", -6: "e-", -5: "b-", -4: "f", -3: "c", -2: "g", -1: "d",
               0: "a", 1: "e", 2: "b", 3: "f#", 4: "c#", 5: "g#", 6: "d#", 7: "a#"}


def profile_score(observed, weights, tonic):
    mean = sum(observed) / 12
    expected_mean = sum(weights) / 12
    variance = sum((v - mean) ** 2 for v in observed)
    expected_variance = sum((v - expected_mean) ** 2 for v in weights)
    if variance == 0:
        return float("-inf")
    covariance = sum((v - mean) * (weights[(i - tonic) % 12] - expected_mean)
                     for i, v in enumerate(observed))
    return covariance / math.sqrt(variance * expected_variance)


KEYS = [(fifths, mode, (MAJOR_NAMES if mode == "major" else MINOR_NAMES)[fifths] + " " + mode,
         (fifths * 7 + (9 if mode == "minor" else 0)) % 12,
         MAJOR if mode == "major" else MINOR)
        for fifths in range(-7, 8) for mode in ("major", "minor")]
SUPPORTED_LABELS = {key[2] for key in KEYS}


def key_pitch_class_and_mode(label):
    tonic, mode = label.split(" ")
    if mode not in ("major", "minor") or any(mark not in "#-b" for mark in tonic[1:]):
        raise ValueError(f"unsupported key label: {label}")
    offset = sum(1 if mark == "#" else -1 for mark in tonic[1:])
    return ((STEP_SEMITONES[tonic[0].upper()] + offset) % 12, mode)


def measure_histograms(score):
    count = max((len(s["measures"]) for p in score["parts"] for s in p["staves"]), default=0)
    hist = [[0.0] * 12 for _ in range(count)]
    for part in score["parts"]:
        for staff in part["staves"]:
            for index, measure in enumerate(staff["measures"]):
                for voice in measure["voices"]:
                    for note in voice:
                        if note["is_rest"] or note["is_unpitched"]:
                            continue
                        duration = float(acorde_fraction_beats(note))
                        for pitch in note["pitches"]:
                            if pitch["microtone_cents"]:
                                continue
                            semitone = STEP_SEMITONES[pitch["step"]] + pitch["alter"]
                            if int(semitone) == semitone:
                                hist[index][int(semitone) % 12] += duration
    return hist


def written_fifths(score, count):
    reference = 0
    out = []
    first_staff = score["parts"][0]["staves"][0]["measures"]
    for index in range(count):
        if index < len(first_staff) and first_staff[index]["key_sig"]:
            reference = first_staff[index]["key_sig"]["fifths"]
        out.append(reference)
    return out


def predict(hist, radius, spelling_fifths):
    out = []
    for index in range(len(hist)):
        observed = [sum(hist[j][pc] for j in range(max(0, index - radius),
                         min(len(hist), index + radius + 1))) for pc in range(12)]
        out.append(max(KEYS, key=lambda key: (profile_score(observed, key[4], key[3]),
                                              -abs(key[0] - spelling_fifths[index]),
                                              -abs(key[0]), -key[0], key[1]))[2])
    return out


def digest(path):
    return hashlib.sha256(path.read_bytes()).hexdigest()


def main():
    parser = argparse.ArgumentParser(description=__doc__)
    parser.add_argument("--cohort", nargs=3, action="append", required=True,
                        metavar=("REPORT", "SELECTION", "INPUT_DIR"))
    parser.add_argument("--acorde", type=Path, required=True)
    parser.add_argument("--output", type=Path, required=True)
    parser.add_argument("--check-rust", action="store_true",
                        help="require the opt-in Rust radius-4 local keys to match this prototype")
    args = parser.parse_args()
    radii = (0, 1, 2, 3, 4, 6, 8, 12)
    totals = {radius: Counter() for radius in radii}
    baseline = Counter()
    records = []
    cohorts = []
    binary = args.acorde.resolve()
    if not binary.is_file():
        parser.error(f"missing Acorde binary: {binary}")
    binary_hash = digest(binary)
    if len(KEYS) != 30:
        raise AssertionError("unexpected key candidate set")
    for report_name, selection_name, root_name in args.cohort:
        report_path = Path(report_name)
        selection_path = Path(selection_name)
        root = Path(root_name)
        report = json.loads(report_path.read_text())
        selection = json.loads(selection_path.read_text())
        if report["classification"] != "development":
            parser.error("only inspected development cohorts are allowed")
        if report["selection_sha256"] != digest(selection_path):
            parser.error(f"selection hash mismatch: {selection_path}")
        cohorts.append({"report_sha256": digest(report_path),
                        "selection_sha256": digest(selection_path),
                        "input_dir": str(root)})
        by_work = {item["work"]: item for item in selection["works"]}
        for work in report["works"]:
            path = root / by_work[work["work"]]["score_path"]
            if digest(path) != work["score_sha256"]:
                raise ValueError(f"input hash mismatch: {path}")
            score = json.loads(subprocess.check_output([str(binary), "report", str(path)]))["score"]
            hist = measure_histograms(score)
            spelling = written_fifths(score, len(hist))
            counts = {radius: Counter() for radius in radii}
            for row in work["rows"]:
                gold = row["gold_local_key"]
                baseline["positions"] += 1
                baseline["gold_spelling_outside_candidate_set"] += gold not in SUPPORTED_LABELS
                baseline["acorde_global_correct"] += (
                    (MAJOR_NAMES if work["acorde_key"]["mode"] == "major" else MINOR_NAMES)[
                        work["acorde_key"]["fifths"]] + " " + work["acorde_key"]["mode"] == gold)
                baseline["music21_global_correct"] += work["music21_key"] == gold
            for radius in totals:
                predictions = predict(hist, radius, spelling)
                if radius == 4 and args.check_rust:
                    actual = json.loads(subprocess.check_output([
                        str(binary), "analyze-local-keys", str(path), "--radius", "4"
                    ]))["local_keys"]
                    actual_by_measure = {entry["measure"]: entry["key"] for entry in actual}
                    for measure, expected in enumerate(predictions):
                        key = actual_by_measure.get(measure)
                        if key is None:
                            raise ValueError(f"Rust local key missing: {work['work']} {measure}")
                        name = (MAJOR_NAMES if key["mode"] == "major" else MINOR_NAMES)[
                            key["fifths"]] + " " + key["mode"]
                        if name != expected:
                            raise ValueError(
                                f"Rust/Python local key differs: {work['work']} {measure} "
                                f"{name} != {expected}")
                for row in work["rows"]:
                    index = row["measure_index"]
                    if index >= len(predictions):
                        raise ValueError(f"measure index missing: {work['work']} {index}")
                    totals[radius]["positions"] += 1
                    totals[radius]["correct"] += predictions[index] == row["gold_local_key"]
                    totals[radius]["pc_mode_correct"] += (
                        key_pitch_class_and_mode(predictions[index])
                        == key_pitch_class_and_mode(row["gold_local_key"]))
                    counts[radius]["positions"] += 1
                    counts[radius]["correct"] += predictions[index] == row["gold_local_key"]
                    counts[radius]["pc_mode_correct"] += (
                        key_pitch_class_and_mode(predictions[index])
                        == key_pitch_class_and_mode(row["gold_local_key"]))
            records.append({"work": work["work"], "score_sha256": work["score_sha256"],
                            "measures": len(hist), "positions": len(work["rows"]),
                            "distinct_gold_local_keys": len({row["gold_local_key"] for row in work["rows"]}),
                            "radii": {str(radius): dict(value) for radius, value in counts.items()}})
    output = {"schema_version": 1, "classification": "development diagnostic, not a paired local-key comparison",
              "method": "centered physical-measure window, duration-weighted Aarden-Essen correlation; written-key-signature spelling tie-break, no smoothing",
              "radii": list(radii), "cohorts": cohorts, "acorde_binary_sha256": binary_hash,
              "rust_radius_4_parity_checked": args.check_rust,
              "global_baselines": dict(baseline),
              "summary": {str(radius): dict(value) for radius, value in totals.items()},
              "works": records}
    args.output.write_text(json.dumps(output, indent=2, ensure_ascii=False) + "\n")


if __name__ == "__main__":
    main()
