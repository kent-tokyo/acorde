#!/usr/bin/env python3
"""Paired local-key diagnostic on inspected When in Rome development cohorts."""

import argparse
import json
import subprocess
import sys
from collections import Counter
from pathlib import Path

import music21

from compare_wir_harmony_music21 import score_has_harmony
from diagnose_wir_local_keys import (MAJOR_NAMES, MINOR_NAMES, digest,
                                     key_pitch_class_and_mode)


REQUIRED_MUSIC21_VERSION = "10.5.2"
WINDOW_SIZE = 4
WINDOW_GRID = (0, 1, 2, 3, 4, 6, 8, 12)


def key_name(signature):
    mode = signature["mode"]
    names = MINOR_NAMES if mode == "minor" else MAJOR_NAMES
    return names[signature["fifths"]] + " " + mode


def score_rows(gold_rows, acorde_keys, music21_keys):
    scored = []
    for gold_row in gold_rows:
        index = gold_row["measure_index"]
        gold = gold_row["gold_local_key"]
        acorde = acorde_keys.get(index)
        competitor = music21_keys.get(index)
        scored.append({"measure": gold_row["measure"], "measure_index": index,
                       "beat": gold_row["beat"], "gold": gold,
                       "acorde": acorde, "music21": competitor,
                       "acorde_exact": acorde == gold,
                       "music21_exact": competitor == gold,
                       "acorde_pc_mode": acorde is not None and
                           key_pitch_class_and_mode(acorde) == key_pitch_class_and_mode(gold),
                       "music21_pc_mode": competitor is not None and
                           key_pitch_class_and_mode(competitor) == key_pitch_class_and_mode(gold)})
    return scored


def counts(rows):
    return {"positions": len(rows),
            "acorde_available": sum(row["acorde"] is not None for row in rows),
            "music21_available": sum(row["music21"] is not None for row in rows),
            "paired_available": sum(row["acorde"] is not None and row["music21"] is not None
                                    for row in rows),
            "acorde_exact": sum(row["acorde_exact"] for row in rows),
            "music21_exact": sum(row["music21_exact"] for row in rows),
            "acorde_exact_paired": sum(row["acorde_exact"] and row["music21"] is not None
                                       for row in rows),
            "music21_exact_paired": sum(row["music21_exact"] and row["acorde"] is not None
                                        for row in rows),
            "acorde_pc_mode": sum(row["acorde_pc_mode"] for row in rows),
            "music21_pc_mode": sum(row["music21_pc_mode"] for row in rows)}


def align_music21_estimates(raw_keys, smoothed_keys, measure_count):
    present_indices = [index for index, raw in enumerate(raw_keys) if raw is not None]
    if len(raw_keys) != measure_count or len(smoothed_keys) != len(present_indices):
        raise ValueError("music21 raw/smoothed local-key positions do not align")
    return {index: str(key) for index, key in zip(present_indices, smoothed_keys)}


def music21_window_grid(score, windows):
    analyzer = music21.analysis.floatingKey.KeyAnalyzer(score)
    raw_keys = analyzer.getRawKeyByMeasure()
    predictions = {}
    for window in windows:
        # KeyAnalyzer caches mutable interpretation dictionaries during smoothing.
        # Reuse only the immutable raw key objects, not a prior window's cache.
        window_analyzer = music21.analysis.floatingKey.KeyAnalyzer(score)
        window_analyzer.rawKeyByMeasure = raw_keys
        window_analyzer.windowSize = window
        predictions[window] = align_music21_estimates(
            raw_keys, window_analyzer.smoothInterpretationByMeasure(),
            window_analyzer.numMeasures)
    return analyzer.numMeasures, predictions


def compare_work(work, source, root, binary):
    path = root / source["score_path"]
    if digest(path) != work["score_sha256"]:
        raise ValueError(f"input hash mismatch: {path}")
    if score_has_harmony(path):
        raise ValueError(f"score contains harmony answers: {path}")
    score = music21.converter.parse(str(path), forceSource=True)
    measure_count, competitor_grid = music21_window_grid(score, WINDOW_GRID)
    competitor = competitor_grid[WINDOW_SIZE]
    result = json.loads(subprocess.check_output([
        str(binary), "analyze-local-keys", str(path), "--radius", str(WINDOW_SIZE)
    ]))
    acorde = {item["measure"]: key_name(item["key"]) for item in result["local_keys"]}
    if len(acorde) != len(result["local_keys"]):
        raise ValueError("duplicate Acorde local-key measure")
    if any(row["measure_index"] >= measure_count for row in work["rows"]):
        raise ValueError("gold position outside music21 physical measures")
    rows = score_rows(work["rows"], acorde, competitor)
    window_scores = {}
    for window, predictions in competitor_grid.items():
        checked = score_rows(work["rows"], acorde, predictions)
        result_counts = counts(checked)
        window_scores[str(window)] = {
            "available": result_counts["music21_available"],
            "exact": result_counts["music21_exact"],
            "pc_mode": result_counts["music21_pc_mode"],
        }
    return {"work": work["work"], "score_sha256": work["score_sha256"],
            "music21_measures": measure_count,
            "music21_estimates": len(competitor),
            "acorde_estimates": len(acorde), "counts": counts(rows),
            "music21_window_scores": window_scores, "rows": rows}


def main():
    parser = argparse.ArgumentParser(description=__doc__)
    parser.add_argument("--cohort", nargs=3, action="append", required=True,
                        metavar=("REPORT", "SELECTION", "INPUT_DIR"))
    parser.add_argument("--acorde", type=Path, required=True)
    parser.add_argument("--output", type=Path, required=True)
    args = parser.parse_args()
    if music21.__version__ != REQUIRED_MUSIC21_VERSION:
        parser.error(f"requires music21 {REQUIRED_MUSIC21_VERSION}; found {music21.__version__}")
    binary = args.acorde.resolve()
    if not binary.is_file():
        parser.error(f"missing Acorde binary: {binary}")
    cohorts = []
    works = []
    for report_name, selection_name, input_name in args.cohort:
        report_path = Path(report_name)
        selection_path = Path(selection_name)
        source = json.loads(report_path.read_text())
        selection = json.loads(selection_path.read_text())
        if source["classification"] != "development":
            parser.error("only inspected development cohorts are allowed")
        if source["selection_sha256"] != digest(selection_path):
            parser.error(f"selection hash mismatch: {selection_path}")
        cohort = {"report_sha256": digest(report_path),
                  "selection_sha256": digest(selection_path),
                  "input_dir": input_name,
                  "works": len(source["works"])}
        cohorts.append(cohort)
        by_name = {item["work"]: item for item in selection["works"]}
        cohort_index = len(cohorts) - 1
        for work in source["works"]:
            try:
                compared = compare_work(work, by_name[work["work"]], Path(input_name), binary)
                compared["cohort_index"] = cohort_index
                works.append(compared)
            except (ValueError, KeyError, subprocess.CalledProcessError,
                    music21.exceptions21.Music21Exception) as error:
                works.append({"work": work["work"], "score_sha256": work["score_sha256"],
                              "cohort_index": cohort_index,
                              "eligible_positions": len(work["rows"]),
                              "error": f"{type(error).__name__}: {error}"})
    combined = Counter()
    for work in works:
        if "counts" in work:
            combined.update(work["counts"])
        else:
            combined["positions"] += work["eligible_positions"]
    cohort_summaries = []
    for index in range(len(cohorts)):
        cohort_counts = Counter()
        for work in works:
            if work["cohort_index"] != index:
                continue
            if "counts" in work:
                cohort_counts.update(work["counts"])
            else:
                cohort_counts["positions"] += work["eligible_positions"]
        cohort_summaries.append(dict(cohort_counts))
    window_summary = {}
    for window in WINDOW_GRID:
        window_summary[str(window)] = {
            metric: sum(work["music21_window_scores"][str(window)][metric]
                        for work in works if "music21_window_scores" in work)
            for metric in ("available", "exact", "pc_mode")
        }
    best_window = max(WINDOW_GRID, key=lambda window: (
        window_summary[str(window)]["exact"], -window))
    rows_path = args.output.with_name(args.output.stem + "-rows.jsonl")
    with rows_path.open("w") as row_file:
        for work in works:
            for row in work.pop("rows", []):
                row_file.write(json.dumps({"work": work["work"], **row},
                                          ensure_ascii=False, separators=(",", ":")) + "\n")
    result = {"schema_version": 1,
              "classification": "paired development diagnostic, not a frozen local-key holdout",
              "unit": "existing aligned RomanText positions",
              "acorde_method": "analyze-local-keys --radius 4",
              "music21_method": "music21.analysis.floatingKey.KeyAnalyzer; getRawKeyByMeasure then smoothInterpretationByMeasure with windowSize=4",
              "music21_window_grid": list(WINDOW_GRID),
              "music21_window_scores": window_summary,
              "music21_best_window_on_development": best_window,
              "music21_version": music21.__version__,
              "python_version": sys.version.split()[0],
              "music21_floating_key_source_sha256": digest(Path(music21.analysis.floatingKey.__file__)),
              "acorde_binary_sha256": digest(binary),
              "comparator_source_sha256": digest(Path(__file__)),
              "rows_file": rows_path.name, "rows_sha256": digest(rows_path),
              "cohorts": cohorts, "cohort_summaries": cohort_summaries,
              "summary": dict(combined),
              "works": works,
              "failures": sum("error" in work for work in works)}
    args.output.write_text(json.dumps(result, indent=2, ensure_ascii=False) + "\n")


if __name__ == "__main__":
    main()
