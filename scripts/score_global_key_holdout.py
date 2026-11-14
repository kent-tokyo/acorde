#!/usr/bin/env python3
"""Score the frozen three-style global-key task against independent labels."""

import argparse
import csv
import hashlib
import json
import re
import subprocess
from pathlib import Path

import lxml
import music21
from lxml import html

from compare_bcmh_keys import acorde_key_name
from compare_mozart_keys import music21_key_name
from fetch_global_key_holdout_scores import git_blob_sha1


SUPPORT_SCRIPTS = (
    "compare_bcmh_keys.py", "compare_mozart_keys.py",
    "fetch_global_key_holdout_scores.py",
)
INDEX_SHA256 = "d8849022d85f59ade61d84e911fd439fe8a42ad920c9a7748129c08863487d5e"
LABEL = re.compile(r"^([A-Ga-g])([#b-]?)$")
ROMAN_OPENING_KEY = re.compile(r"(?<![A-Za-z])([A-Ga-g][#b-]?):")
RIEMENSCHNEIDER = re.compile(r"(?<!\d)(\d{1,3})(\*)?(?!\d)")


def sha256(path):
    return hashlib.sha256(path.read_bytes()).hexdigest()


def normalize_label(raw):
    match = LABEL.fullmatch(raw.strip())
    if not match:
        raise ValueError(f"unsupported tonic label: {raw!r}")
    note, accidental = match.groups()
    return f"{note.upper()}{accidental.replace('-', 'b')} " + (
        "major" if note.isupper() else "minor")


def chorale_index_labels(data, numbers):
    doc = html.fromstring(data)
    rows = doc.xpath('//table[@id="sortable"]//tr')
    if len(rows) < 300:
        raise ValueError("chorale key-index table is incomplete")
    wanted = set(numbers)
    found = {number: [] for number in numbers}
    for row in rows:
        cells = row.xpath("./td")
        if len(cells) != 11:
            continue
        tokens = RIEMENSCHNEIDER.findall(cells[3].text_content())
        for number, starred in tokens:
            if number not in wanted:
                continue
            tonic = cells[6].text_content().strip()
            mode = cells[7].text_content().strip()
            ambiguous_markup = any(cell.xpath(".//i") for cell in cells[5:9])
            found[number].append({"tonic": tonic, "mode": mode,
                                  "starred": bool(starred),
                                  "ambiguous_markup": ambiguous_markup})
    return found


def opening_key(analysis):
    for line in analysis.splitlines():
        if not re.match(r"^m\d", line.strip()):
            continue
        match = ROMAN_OPENING_KEY.search(line)
        if match:
            return normalize_label(match.group(1))
    raise ValueError("no opening RomanText key")


def metadata_labels(path, expected_blob):
    data = path.read_bytes()
    if git_blob_sha1(data) != expected_blob:
        raise ValueError(f"metadata Git blob differs: {path}")
    with path.open(newline="") as stream:
        rows = list(csv.DictReader(stream, delimiter="\t"))
    if not rows or not {"piece", "annotated_key"}.issubset(rows[0]):
        raise ValueError("metadata columns differ")
    result = {}
    for row in rows:
        if row["piece"] in result:
            raise ValueError(f"duplicate metadata piece: {row['piece']}")
        result[row["piece"]] = row["annotated_key"]
    return result


def gold_for_work(work, index_labels, wir_root, metadata):
    piece = work["piece"]
    if work["group"] == "chorale":
        analysis = wir_root / work["corroboration_path"]
        data = analysis.read_bytes()
        if git_blob_sha1(data) != work["corroboration_git_blob"]:
            raise ValueError(f"RomanText Git blob differs: {piece}")
        labels = index_labels[piece]
        if len(labels) != 1:
            return None, "nonunique_chorale_index_row"
        item = labels[0]
        if item["starred"]:
            return None, "chorale_collection_transposition_flag"
        if item["ambiguous_markup"] or "?" in item["tonic"] + item["mode"]:
            return None, "ambiguous_chorale_index_key"
        try:
            tonic = normalize_label(item["tonic"])
            mode = item["mode"].lower()
            if mode not in {"major", "minor"}:
                raise ValueError("nonmajor_minor_index_mode")
            index_key = tonic.split(" ")[0] + " " + mode
        except ValueError:
            return None, "unsupported_chorale_index_key"
        try:
            corroborated = opening_key(data.decode("utf-8-sig"))
        except ValueError:
            return None, "missing_chorale_opening_key"
        if index_key != corroborated:
            return None, "chorale_reference_disagreement"
        return index_key, None
    raw = metadata[work["group"]].get(piece)
    if not raw:
        return None, "missing_metadata_key"
    try:
        return normalize_label(raw), None
    except ValueError:
        return None, "unsupported_metadata_key"


def acorde_prediction(binary, path):
    result = subprocess.run([str(binary), "analyze", str(path),
                             "--aarden-essen-keys"], capture_output=True,
                            text=True, timeout=120, check=False)
    if result.returncode:
        raise RuntimeError(result.stderr.strip()[-300:] or
                           f"Acorde exit status {result.returncode}")
    candidates = json.loads(result.stdout)["key_estimates"]
    return acorde_key_name(candidates[0]) if candidates else None


def music21_prediction(path):
    score = music21.converter.parse(str(path), forceSource=True)
    return music21_key_name(score.analyze("key"))


def pitch_class_mode(label):
    if label is None:
        return None
    tonic, mode = label.split(" ")
    steps = {"C": 0, "D": 2, "E": 4, "F": 5,
             "G": 7, "A": 9, "B": 11}
    return ((steps[tonic[0]] + sum(1 if x == "#" else -1
                                  for x in tonic[1:])) % 12, mode)


def score(selection, inputs, preflight, protocol, input_root, wir_root,
          index_path, metadata, binary):
    by_input = {(x["group"], x["piece"]): x for x in inputs["inputs"]}
    by_preflight = {(x["group"], x["piece"]): x for x in preflight["cases"]}
    if len(by_input) != len(selection["works"]) or len(by_preflight) != len(by_input):
        raise ValueError("selection/input/preflight counts differ")
    if sha256(index_path) != INDEX_SHA256:
        raise ValueError("chorale index SHA-256 differs")
    index_labels = chorale_index_labels(index_path.read_bytes(),
                                         [x["piece"] for x in selection["works"]
                                          if x["group"] == "chorale"])
    rows = []
    for work in selection["works"]:
        group, piece = work["group"], work["piece"]
        item = by_input[(group, piece)]
        checked = by_preflight[(group, piece)]
        path = input_root / item["input_path"]
        if sha256(path) != item["input_sha256"]:
            raise ValueError(f"prepared score hash differs: {group}/{piece}")
        if "error" in item or "error" in checked:
            raise ValueError(f"preflight failed: {group}/{piece}")
        gold, exclusion = gold_for_work(work, index_labels, wir_root, metadata)
        row = {"group": group, "piece": piece,
               "input_sha256": item["input_sha256"], "gold": gold,
               "exclusion": exclusion}
        if exclusion is None:
            for name, predict in (("acorde", lambda: acorde_prediction(binary, path)),
                                  ("music21", lambda: music21_prediction(path))):
                try:
                    row[name] = {"key": predict()}
                except Exception as error:
                    row[name] = {"error": f"{type(error).__name__}: {error}"}
            row["acorde_exact"] = row["acorde"].get("key") == gold
            row["music21_exact"] = row["music21"].get("key") == gold
            row["acorde_pc_mode"] = (pitch_class_mode(row["acorde"].get("key"))
                                     == pitch_class_mode(gold))
            row["music21_pc_mode"] = (pitch_class_mode(row["music21"].get("key"))
                                      == pitch_class_mode(gold))
        rows.append(row)
        print(f"{group}/{piece}: {'excluded' if exclusion else 'scored'}", flush=True)
    included = [row for row in rows if row["exclusion"] is None]
    paired = [row for row in included
              if row["acorde"].get("key") and row["music21"].get("key")]
    by_group = {}
    for group in ("chorale", "keyboard", "chamber"):
        group_rows = [row for row in included if row["group"] == group]
        group_paired = [row for row in paired if row["group"] == group]
        by_group[group] = {
            "eligible": len(group_rows), "paired": len(group_paired),
            "acorde_exact": sum(row["acorde_exact"] for row in group_paired),
            "music21_exact": sum(row["music21_exact"] for row in group_paired),
        }
    summary = {
        "selected": len(rows), "excluded": len(rows) - len(included),
        "eligible": len(included), "paired": len(paired),
        "acorde_available": sum(bool(x["acorde"].get("key")) for x in included),
        "music21_available": sum(bool(x["music21"].get("key")) for x in included),
        "acorde_exact": sum(x["acorde_exact"] for x in included),
        "music21_exact": sum(x["music21_exact"] for x in included),
        "acorde_exact_paired": sum(x["acorde_exact"] for x in paired),
        "music21_exact_paired": sum(x["music21_exact"] for x in paired),
        "acorde_pc_mode": sum(x["acorde_pc_mode"] for x in included),
        "music21_pc_mode": sum(x["music21_pc_mode"] for x in included),
    }
    gate = protocol["gate"]
    passed = (summary["selected"] == gate["required_scores"]
              and summary["excluded"] <= gate["max_excluded_scores"]
              and all(group["eligible"] >= gate["min_eligible_per_group"]
                      for group in by_group.values())
              and summary["acorde_available"] >= summary["music21_available"]
              and summary["acorde_exact_paired"] > summary["music21_exact_paired"]
              and all(group["acorde_exact"] >= group["music21_exact"]
                      for group in by_group.values()))
    return {"schema_version": 1, "classification": "fixed global-key label holdout",
            "gate_passed": passed, "summary": summary,
            "by_group": by_group, "rows": rows}


def main():
    parser = argparse.ArgumentParser(description=__doc__)
    parser.add_argument("selection", type=Path)
    parser.add_argument("inputs", type=Path)
    parser.add_argument("preflight", type=Path)
    parser.add_argument("protocol", type=Path)
    parser.add_argument("input_root", type=Path)
    parser.add_argument("wir_gold_root", type=Path)
    parser.add_argument("chorale_index", type=Path)
    parser.add_argument("cpe_metadata", type=Path)
    parser.add_argument("pleyel_metadata", type=Path)
    parser.add_argument("acorde", type=Path)
    parser.add_argument("output", type=Path)
    args = parser.parse_args()
    if music21.__version__ != "10.5.2" or lxml.__version__ != "6.0.2":
        parser.error("music21/lxml versions differ from frozen comparator")
    protocol = json.loads(args.protocol.read_text())
    pinned = protocol["pinned"]
    if pinned["chorale_key_index_sha256"] != INDEX_SHA256:
        parser.error("frozen chorale key-index hash differs")
    checks = (("selection_sha256", args.selection),
              ("inputs_sha256", args.inputs),
              ("preflight_sha256", args.preflight),
              ("acorde_binary_sha256", args.acorde),
              ("scorer_source_sha256", Path(__file__)))
    for key, path in checks:
        if sha256(path) != pinned[key]:
            parser.error(f"frozen {key} differs")
    for name in SUPPORT_SCRIPTS:
        if sha256(Path(__file__).with_name(name)) != pinned["support_script_sha256"][name]:
            parser.error(f"frozen support script differs: {name}")
    if sha256(Path(music21.analysis.discrete.__file__)) != pinned["music21_discrete_source_sha256"]:
        parser.error("music21 key-analyzer source differs")
    selection = json.loads(args.selection.read_text())
    inputs = json.loads(args.inputs.read_text())
    preflight = json.loads(args.preflight.read_text())
    if (len(selection["works"]) != 12 or inputs["selection_sha256"] != sha256(args.selection)
            or preflight["inputs_sha256"] != sha256(args.inputs)):
        parser.error("selection/input/preflight chain differs")
    metadata = {
        "keyboard": metadata_labels(args.cpe_metadata,
                                    pinned["cpe_metadata_git_blob"]),
        "chamber": metadata_labels(args.pleyel_metadata,
                                   pinned["pleyel_metadata_git_blob"]),
    }
    result = score(selection, inputs, preflight, protocol, args.input_root,
                   args.wir_gold_root, args.chorale_index, metadata,
                   args.acorde.resolve())
    result["protocol_sha256"] = sha256(args.protocol)
    result["selection_sha256"] = sha256(args.selection)
    result["inputs_sha256"] = sha256(args.inputs)
    result["preflight_sha256"] = sha256(args.preflight)
    result["acorde_binary_sha256"] = sha256(args.acorde)
    result["scorer_source_sha256"] = sha256(Path(__file__))
    args.output.write_text(json.dumps(result, ensure_ascii=False, indent=2) + "\n")
    print(json.dumps(result["summary"], sort_keys=True))
    print(f"gate_passed={result['gate_passed']}")


if __name__ == "__main__":
    main()
