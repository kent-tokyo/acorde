#!/usr/bin/env python3
"""Score one frozen WIR local-key holdout after verifying every pinned input."""

import argparse
from collections import Counter
import hashlib
import json
from pathlib import Path
import subprocess

import music21

from compare_wir_harmony_music21 import gold_rows, music21_measures, score_has_harmony
from compare_wir_local_keys_music21 import counts, key_name, music21_window_grid, score_rows
from diagnose_wir_local_keys import digest
from fetch_wir_harmony_holdout import git_blob_sha1

SUPPORT_SCRIPTS = (
    "compare_wir_harmony_music21.py",
    "compare_wir_local_keys_music21.py",
    "diagnose_wir_local_keys.py",
    "compare_bcmh_keys.py",
    "fetch_wir_harmony_holdout.py",
)


def score_case(work, prepared, preflight, clean_dir, gold_dir, binary, music21_window):
    input_path = clean_dir / prepared["path"]
    gold_path = gold_dir / work["gold_path"]
    if digest(input_path) != prepared["input_sha256"]:
        raise ValueError("prepared score hash mismatch")
    if score_has_harmony(input_path):
        raise ValueError("embedded harmony answer remains")
    gold_bytes = gold_path.read_bytes()
    if git_blob_sha1(gold_bytes) != work["gold_git_blob"]:
        raise ValueError("gold Git blob mismatch")
    gold_sha256 = hashlib.sha256(gold_bytes).hexdigest()
    source = music21.converter.parse(str(input_path), forceSource=True)
    m_measures = music21_measures(source, music21.stream)
    measure_count, grid = music21_window_grid(source, (music21_window,))
    competitor = grid[music21_window]
    a_import = json.loads(subprocess.check_output([str(binary), "report", str(input_path)]))["score"]
    a_measures = a_import["parts"][0]["staves"][0]["measures"]
    a_report = json.loads(subprocess.check_output([
        str(binary), "analyze-local-keys", str(input_path), "--radius", "4"
    ]))
    acorde = {item["measure"]: key_name(item["key"]) for item in a_report["local_keys"]}
    if len(acorde) != len(a_report["local_keys"]):
        raise ValueError("duplicate Acorde estimate measure")
    gold = music21.converter.parse(str(gold_path), format="romanText")
    annotations = gold_rows(gold, music21.roman, music21.stream)
    source_labels = sum(map(len, annotations.values()))
    excluded = []
    accepted = []
    unequal = set(preflight["unequal_timeline_measures"])
    for number, rows in sorted(annotations.items()):
        beats = Counter(beat for beat, _, _ in rows)
        ambiguous = {beat for beat, count in beats.items() if count > 1}
        if ambiguous:
            excluded.append({"measure": number,
                             "labels": sum(beats[beat] for beat in ambiguous),
                             "reason": "ambiguous_gold_position"})
            rows = [row for row in rows if row[0] not in ambiguous]
        if not rows:
            continue
        indices = [index for index, measure in enumerate(m_measures[0])
                   if measure.number == number]
        if len(indices) != 1:
            excluded.append({"measure": number, "labels": len(rows),
                             "reason": "missing_or_ambiguous_source_measure"})
            continue
        index = indices[0]
        if index >= measure_count or index >= len(a_measures) or a_measures[index]["number"] != number:
            excluded.append({"measure": number, "labels": len(rows),
                             "reason": "acorde_measure_number_mismatch"})
            continue
        if index in unequal:
            excluded.append({"measure": number, "labels": len(rows),
                             "reason": "predeclared_unequal_pitched_timeline"})
            continue
        accepted.extend({"measure": number, "measure_index": index, "beat": beat,
                         "gold_local_key": local_key}
                        for beat, _, local_key in rows)
    scored = score_rows(accepted, acorde, competitor)
    return {"work": work["work"], "input_sha256": prepared["input_sha256"],
            "gold_git_blob": work["gold_git_blob"], "gold_sha256": gold_sha256,
            "music21_measures": measure_count, "music21_estimates": len(competitor),
            "acorde_estimates": len(acorde), "source_labels": source_labels,
            "excluded_labels": sum(item["labels"] for item in excluded),
            "exclusions": excluded, "counts": counts(scored), "rows": scored}


def main():
    parser = argparse.ArgumentParser(description=__doc__)
    parser.add_argument("protocol", type=Path)
    parser.add_argument("selection", type=Path)
    parser.add_argument("inputs", type=Path)
    parser.add_argument("preflight", type=Path)
    parser.add_argument("clean_dir", type=Path)
    parser.add_argument("gold_dir", type=Path)
    parser.add_argument("acorde", type=Path)
    parser.add_argument("output", type=Path)
    args = parser.parse_args()
    protocol = json.loads(args.protocol.read_text())
    selection = json.loads(args.selection.read_text())
    inputs = json.loads(args.inputs.read_text())
    preflight = json.loads(args.preflight.read_text())
    binary = args.acorde.resolve()
    pinned = protocol["pinned"]
    if (protocol["acorde_radius_measures"] != 4
            or digest(args.selection) != pinned["selection_sha256"]
            or digest(args.inputs) != pinned["inputs_sha256"]
            or digest(args.preflight) != pinned["preflight_sha256"]
            or digest(binary) != pinned["acorde_binary_sha256"]
            or digest(Path(__file__)) != pinned["scorer_source_sha256"]
            or music21.__version__ != pinned["music21_version"]
            or digest(Path(music21.analysis.floatingKey.__file__))
            != pinned["music21_floating_key_source_sha256"]
            or digest(Path(music21.analysis.discrete.__file__))
            != pinned["music21_discrete_source_sha256"]):
        parser.error("frozen candidate, input, scorer, or music21 version drift")
    for name in SUPPORT_SCRIPTS:
        if digest(Path(__file__).with_name(name)) != pinned["support_script_sha256"][name]:
            parser.error(f"frozen support script drift: {name}")
    if (selection["commit"] != inputs["source_commit"]
            or selection["commit"] != preflight["source_commit"]
            or selection["split"] != "local-key label holdout"
            or preflight["classification"] != "input-only, no gold content read"):
        parser.error("holdout source/preflight mismatch")
    by_input = {entry["work"]: entry for entry in inputs["inputs"]}
    by_preflight = {entry["work"]: entry for entry in preflight["cases"]}
    if (set(by_input) != set(by_preflight)
            or set(by_input) != {work["work"] for work in selection["works"]}):
        parser.error("holdout work sets differ")
    results = []
    for work in selection["works"]:
        name = work["work"]
        prepared = by_input[name]
        checked = by_preflight[name]
        if ("error" in checked or checked["input_sha256"] != prepared["input_sha256"]):
            parser.error(f"input preflight failed: {name}")
        try:
            results.append(score_case(work, prepared, checked, args.clean_dir,
                                      args.gold_dir, binary, protocol["music21_window_measures"]))
        except (ValueError, OSError, KeyError, subprocess.CalledProcessError,
                music21.exceptions21.Music21Exception) as error:
            results.append({"work": name, "input_sha256": prepared["input_sha256"],
                            "error": f"{type(error).__name__}: {error}"})
    summary = Counter()
    for result in results:
        if "counts" in result:
            summary.update(result["counts"])
        summary["source_labels"] += result.get("source_labels", 0)
        summary["excluded_labels"] += result.get("excluded_labels", 0)
    summary["works"] = len(results)
    summary["failures"] = sum("error" in result for result in results)
    summary["acorde_work_wins"] = sum(result["counts"]["acorde_exact"]
                                      > result["counts"]["music21_exact"]
                                      for result in results if "counts" in result)
    summary["music21_work_wins"] = sum(result["counts"]["music21_exact"]
                                       > result["counts"]["acorde_exact"]
                                       for result in results if "counts" in result)
    summary["work_ties"] = sum(result["counts"]["acorde_exact"]
                                == result["counts"]["music21_exact"]
                                for result in results if "counts" in result)
    eligible_fraction = (summary["excluded_labels"] / summary["source_labels"]
                         if summary["source_labels"] else 1.0)
    gate = (summary["failures"] == 0
            and summary["works"] == protocol["gate"]["required_works"]
            and eligible_fraction <= protocol["gate"]["max_excluded_fraction"]
            and summary["acorde_available"] >= summary["music21_available"]
            and summary["acorde_exact_paired"] > summary["music21_exact_paired"]
            and summary["acorde_work_wins"] >= protocol["gate"]["min_acorde_work_wins"])
    rows_path = args.output.with_name(args.output.stem + "-rows.jsonl")
    with rows_path.open("w") as row_file:
        for result in results:
            for row in result.pop("rows", []):
                row_file.write(json.dumps({"work": result["work"], **row},
                                          ensure_ascii=False, separators=(",", ":")) + "\n")
    output = {"schema_version": 1, "classification": "frozen local-key label holdout",
              "protocol_sha256": digest(args.protocol),
              "selection_sha256": digest(args.selection),
              "inputs_sha256": digest(args.inputs),
              "preflight_sha256": digest(args.preflight),
              "acorde_binary_sha256": digest(binary),
              "scorer_source_sha256": digest(Path(__file__)),
              "music21_version": music21.__version__,
              "music21_window_measures": protocol["music21_window_measures"],
              "acorde_radius_measures": 4,
              "summary": dict(summary), "gate_passed": gate,
              "rows_file": rows_path.name, "rows_sha256": digest(rows_path),
              "works": results}
    args.output.write_text(json.dumps(output, ensure_ascii=False, indent=2) + "\n")
    print({"summary": dict(summary), "gate_passed": gate})


if __name__ == "__main__":
    main()
