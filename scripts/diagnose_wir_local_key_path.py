#!/usr/bin/env python3
"""Development sweep behind the local-key path method (#501).

Uses only inspected When in Rome development works: the three harmony cohorts
and the failed first local-key holdout (48 works). For each work it strips
MusicXML harmony, reads Acorde's parsed score, aligns RomanText local keys to
physical measures as the holdout scorers do, and records music21 10.5.2's
window-2 floating-key estimates. It then reimplements the path method in
Python, sweeps radius, change cost and fifth cost, and reports pooled totals
and a leave-one-cohort-out check. With --check-rust it also requires
`analyze-local-keys --method path` to match the Python path at every measure.

Usage: diagnose_wir_local_key_path.py WIR_DIR CLEAN_DIR ACORDE OUTPUT [--check-rust]
WIR_DIR holds the pinned score/analysis files fetched with
fetch_wir_harmony_holdout.py for the four selections below.
"""

import argparse
from collections import Counter, defaultdict
import json
from pathlib import Path
import subprocess

import music21

from compare_wir_harmony_music21 import gold_rows, music21_measures
from compare_wir_local_keys_music21 import key_name, music21_window_grid
from diagnose_wir_local_keys import KEYS, digest, measure_histograms, profile_score, written_fifths
from prepare_wir_harmony_inputs import strip_mxl

ROOT = Path(__file__).resolve().parents[1]
COHORTS = ("harmony-holdout-selection-v1", "harmony-holdout-2-selection-v1",
           "harmony-holdout-3-selection-v1", "local-key-holdout-selection-v1")
GRID = [(radius, change, fifth) for radius in (0, 1, 2, 3)
        for change in (0.75, 1.0, 1.5, 2.0, 3.0) for fifth in (0.0, 0.1, 0.2)]
CHOSEN = (1, 1.5, 0.1)


def scores_matrix(hist, radius):
    rows = []
    for index in range(len(hist)):
        observed = [sum(hist[j][pc] for j in range(max(0, index - radius),
                                                   min(len(hist), index + radius + 1)))
                    for pc in range(12)]
        row = [profile_score(observed, key[4], key[3]) for key in KEYS]
        rows.append([-1.0 if value == float("-inf") or value != value else value for value in row])
    return rows


def key_path(matrix, spelling, change, fifth):
    def tonic(key):
        return key[0] + (3 if key[1] == "minor" else 0)
    cost = [[0.0 if a == b else change + fifth * abs(tonic(KEYS[a]) - tonic(KEYS[b]))
             for b in range(len(KEYS))] for a in range(len(KEYS))]
    def emit(index, key):
        return matrix[index][key] - 0.001 * abs(KEYS[key][0] - spelling[index])
    totals = [emit(0, key) for key in range(len(KEYS))]
    back = []
    for index in range(1, len(matrix)):
        pointers = [max(range(len(KEYS)), key=lambda a: totals[a] - cost[a][b])
                    for b in range(len(KEYS))]
        totals = [totals[a] - cost[a][b] + emit(index, b) for b, a in enumerate(pointers)]
        back.append(pointers)
    state = max(range(len(KEYS)), key=lambda key: totals[key])
    path = [state]
    for pointers in reversed(back):
        state = pointers[state]
        path.append(state)
    return [KEYS[key][2] for key in reversed(path)]


def window(hist, radius, spelling):
    out = []
    for index in range(len(hist)):
        observed = [sum(hist[j][pc] for j in range(max(0, index - radius),
                                                   min(len(hist), index + radius + 1)))
                    for pc in range(12)]
        out.append(max(KEYS, key=lambda k: (profile_score(observed, k[4], k[3]),
                                            -abs(k[0] - spelling[index]), -abs(k[0]),
                                            -k[0], k[1]))[2])
    return out


def load_work(work, wir, clean, binary):
    source = (wir / work["score_path"]).read_bytes()
    cleaned, removed = strip_mxl(source)
    path = clean / work["score_path"]
    path.parent.mkdir(parents=True, exist_ok=True)
    path.write_bytes(cleaned if removed else source)
    score = json.loads(subprocess.check_output([str(binary), "report", str(path)]))["score"]
    parsed = music21.converter.parse(str(path), forceSource=True)
    measures = music21_measures(parsed, music21.stream)
    _, grid = music21_window_grid(parsed, (2,))
    gold = music21.converter.parse(str(wir / work["gold_path"]), format="romanText")
    a_measures = score["parts"][0]["staves"][0]["measures"]
    rows = []
    for number, labels in sorted(gold_rows(gold, music21.roman, music21.stream).items()):
        beats = Counter(beat for beat, _, _ in labels)
        indices = [i for i, m in enumerate(measures[0]) if m.number == number]
        if len(indices) != 1 or indices[0] >= len(a_measures) or a_measures[indices[0]]["number"] != number:
            continue
        rows += [(indices[0], beat, key) for beat, _, key in labels if beats[beat] == 1]
    return path, score, rows, grid[2]


def main():
    parser = argparse.ArgumentParser(description=__doc__)
    parser.add_argument("wir", type=Path)
    parser.add_argument("clean", type=Path)
    parser.add_argument("acorde", type=Path)
    parser.add_argument("output", type=Path)
    parser.add_argument("--check-rust", action="store_true")
    args = parser.parse_args()
    if music21.__version__ != "10.5.2":
        parser.error("requires music21 10.5.2")
    binary = args.acorde.resolve()
    grid = defaultdict(Counter)
    baseline = defaultdict(Counter)
    works = []
    all_rows = []
    for cohort in COHORTS:
        selection = json.loads((ROOT / f"docs/archive/analysis-wir-{cohort}.json").read_text())
        for work in selection["works"]:
            path, score, rows, music21_keys = load_work(work, args.wir, args.clean, binary)
            hist = measure_histograms(score)
            spelling = written_fifths(score, len(hist))
            def hits(pred):
                return sum(pred[index] == gold for index, _, gold in rows)
            m21 = sum(music21_keys.get(index) == gold for index, _, gold in rows)
            w4 = hits(window(hist, 4, spelling))
            matrices = {radius: scores_matrix(hist, radius) for radius in (0, 1, 2, 3)}
            for radius, change, fifth in GRID:
                predicted = key_path(matrices[radius], spelling, change, fifth)
                grid[(radius, change, fifth)][cohort] += hits(predicted)
                if (radius, change, fifth) == CHOSEN:
                    chosen = predicted
                    chosen_hits = hits(predicted)
            if args.check_rust:
                report = json.loads(subprocess.check_output(
                    [str(binary), "analyze-local-keys", str(path), "--method", "path"]))
                rust = {entry["measure"]: key_name(entry["key"]) for entry in report["local_keys"]}
                if [rust.get(i) for i in range(len(chosen))] != chosen:
                    raise ValueError(f"Rust/Python key path differs: {work['work']}")
            baseline[cohort].update({"positions": len(rows), "window4": w4, "music21_w2": m21})
            works.append({"cohort": cohort, "work": work["work"], "input_sha256": digest(path),
                          "positions": len(rows),
                          "path": chosen_hits, "window4": w4, "music21_w2": m21})
            all_rows += [{"work": work["work"], "measure_index": index, "beat": beat, "gold": gold,
                          "acorde": chosen[index], "music21": music21_keys.get(index)}
                         for index, beat, gold in rows]
            print(cohort, work["work"], len(rows), chosen_hits, w4, m21, flush=True)
    loco = []
    for held in COHORTS:
        best = max(GRID, key=lambda g: sum(v for c, v in grid[g].items() if c != held))
        loco.append({"held_out": held, "chosen_on_rest": list(best),
                     "held_out_path": grid[best][held], **dict(baseline[held])})
    pooled = Counter()
    for value in baseline.values():
        pooled.update(value)
    rows_path = args.output.with_name(args.output.stem + "-rows.jsonl")
    with rows_path.open("w") as handle:
        for row in all_rows:
            handle.write(json.dumps(row, ensure_ascii=False, separators=(",", ":")) + "\n")
    output = {"schema_version": 1,
              "rows_file": rows_path.name, "rows_sha256": digest(rows_path),
              "classification": "development diagnostic on inspected works, not a holdout",
              "acorde_binary_sha256": digest(binary), "music21_version": music21.__version__,
              "rust_parity_checked": args.check_rust, "chosen": list(CHOSEN),
              "pooled": dict(pooled), "pooled_path_chosen": sum(grid[CHOSEN].values()),
              "grid": [{"radius": g[0], "change_cost": g[1], "fifth_cost": g[2],
                        "exact": sum(grid[g].values()), "by_cohort": dict(grid[g])} for g in GRID],
              "leave_one_cohort_out": loco, "works": works}
    args.output.write_text(json.dumps(output, ensure_ascii=False, indent=2) + "\n")


if __name__ == "__main__":
    main()
