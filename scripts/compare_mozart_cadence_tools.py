#!/usr/bin/env python3
"""Compare Acorde and a pinned music21 10.5.2 cadence recipe on projected Mozart positions (#502).

Both tools are scored at the same eligible positions: part-0 note onsets of the
projected cadence CSV, minus each case's preflight unequal-timeline measures.
Gold PAC and IAC map to Authentic, HC to Half, DC to Deceptive, EC to Evaded.

The music21 recipe ("music21-dominant-tonic-outer-voices-v1") mirrors the
conditions of Acorde's opt-in vertical cadence rule using only music21 calls:

1. key = score.analyze("key") on the parsed prepared score (one global key);
2. chords = score.chordify(); for each Chord with pitches, in measure order,
   figure = roman.romanNumeralFromChord(chord, key).figure;
3. a chord whose figure is a plain I (I, I6, I64, ...; no applied "/") is an
   Authentic candidate when the nearest of the previous eight chords with a
   different figure is at most one measure earlier and is a plain V
   (V, V7, V65, ...), the lowest pitch rises five semitones modulo 12 from that
   chord, and the highest pitch of the I chord shares the lowest pitch's pitch
   class.

A candidate counts at the chord's measure index and offset. Candidates that do
not land on an eligible position are reported, not scored.

Usage: compare_mozart_cadence_tools.py PROJECTION PREFLIGHT INPUT_DIR ACORDE OUTPUT
"""

import argparse
from collections import Counter
from fractions import Fraction
import hashlib
import json
from pathlib import Path
import subprocess

import music21
from music21 import roman, stream

from score_mozart_cadence_acorde_development import GOLD_KIND, candidate_position

RECIPE = "music21-dominant-tonic-outer-voices-v1"


def sha256(path):
    return hashlib.sha256(path.read_bytes()).hexdigest()


def plain(figure, numeral):
    if "/" in figure or not figure.startswith(numeral):
        return False
    rest = figure[len(numeral):]
    return rest == "" or rest[0].isdigit()


def music21_cadences(path):
    score = music21.converter.parse(str(path), forceSource=True)
    key = score.analyze("key")
    measures = list(score.chordify().getElementsByClass(stream.Measure))
    sequence = []
    for index, measure in enumerate(measures):
        for chord in measure.recurse().getElementsByClass("Chord"):
            if chord.pitches:
                figure = roman.romanNumeralFromChord(chord, key).figure
                beat = Fraction(chord.offset).limit_denominator(10_000)
                sequence.append((index, beat, figure, [p.midi for p in chord.pitches]))
    found = []
    for position, (index, beat, figure, midis) in enumerate(sequence):
        if not plain(figure, "I"):
            continue
        previous = next((item for item in reversed(sequence[max(0, position - 8):position])
                         if index - item[0] <= 1 and item[2] != figure), None)
        if previous is None or not plain(previous[2], "V"):
            continue
        bass = min(midis)
        if (bass - min(previous[3])) % 12 != 5 or (max(midis) - bass) % 12 != 0:
            continue
        found.append((index, beat, "Authentic"))
    return str(key), found


def acorde_cadences(binary, path):
    analysis = json.loads(subprocess.check_output(
        [str(binary), "analyze-cadences", str(path), "--infer-key"]))
    report = json.loads(subprocess.check_output([str(binary), "report", str(path)]))
    found = []
    for candidate in analysis["cadence_candidates"]:
        part, measure, beat = candidate_position(report["score"], candidate)
        if part == 0:
            found.append((measure, beat, candidate["kind"]))
    return found


def score_tool(eligible, predictions):
    by_position = {}
    outside = 0
    for measure, beat, kind in predictions:
        if (measure, beat) in eligible:
            by_position.setdefault((measure, beat), set()).add(kind)
        else:
            outside += 1
    counts = Counter(outside_eligible=outside)
    for position, gold in eligible.items():
        kinds = by_position.get(position, set())
        expected = GOLD_KIND[gold] if gold else None
        if len(kinds) > 1:
            counts["ambiguous_prediction"] += 1
        if not kinds:
            counts["missed" if expected else "true_negative"] += 1
        elif expected is None:
            counts["false_positive"] += 1
        elif kinds == {expected}:
            counts["correct"] += 1
        else:
            counts["wrong_label"] += 1
    return counts, by_position


def main():
    parser = argparse.ArgumentParser(description=__doc__)
    for name in ("projection", "preflight", "input_dir", "acorde", "output"):
        parser.add_argument(name, type=Path)
    args = parser.parse_args()
    if music21.__version__ != "10.5.2":
        parser.error("requires music21 10.5.2")
    projection = json.loads(args.projection.read_text())
    preflight = json.loads(args.preflight.read_text())
    if (projection["source_commit"] != preflight["source_commit"]
            or projection["prepared_inputs_sha256"] != preflight["prepared_inputs_sha256"]):
        parser.error("projection/preflight provenance mismatch")
    splits = {case["split"] for case in projection["cases"]}
    if len(splits) != 1:
        parser.error("a projection must hold exactly one split")
    by_preflight = {case["id"]: case for case in preflight["cases"]}
    binary = args.acorde.resolve()
    cases = []
    for case in projection["cases"]:
        checked = by_preflight[case["id"]]
        name = "kv" + case["id"][1:].lower().replace("-", "_") + ".musicxml"
        path = args.input_dir / "scores" / name
        if sha256(path) != case["score_input_sha256"] or checked["input_sha256"] != case["score_input_sha256"]:
            parser.error(f"input hash mismatch: {case['id']}")
        skip = set(checked["unequal_timeline_measures"])
        eligible = {}
        for position in case["positions"]:
            if position["part"] != 0 or position["measure"] in skip:
                continue
            eligible[(position["measure"], Fraction(*position["offset_quarter"]))] = position["cadence"]
        result = {"id": case["id"], "split": case["split"], "input_sha256": case["score_input_sha256"],
                  "excluded_timeline_measures": sorted(skip), "eligible": len(eligible),
                  "gold_positive": sum(label is not None for label in eligible.values())}
        predictions = {}
        try:
            predictions["acorde"] = acorde_cadences(binary, path)
        except (subprocess.CalledProcessError, ValueError) as error:
            result["acorde_error"] = f"{type(error).__name__}: {error}"
        try:
            result["music21_key"], predictions["music21"] = music21_cadences(path)
        except Exception as error:  # music21 raises many unrelated exception types
            result["music21_error"] = f"{type(error).__name__}: {error}"
        placed = {}
        for tool, found in predictions.items():
            result[f"{tool}_counts"], placed[tool] = score_tool(eligible, found)
        rows = []
        for (measure, beat), gold in sorted(eligible.items()):
            entry = {tool: sorted(placed[tool].get((measure, beat), ())) for tool in placed}
            if gold or any(entry.values()):
                rows.append({"measure": measure, "offset_quarter": [beat.numerator, beat.denominator],
                             "gold": gold, "gold_coarse": GOLD_KIND[gold] if gold else None, **entry})
        result["rows"] = rows
        cases.append(result)
        print(case["id"], result.get("acorde_counts", {}).get("correct"),
              result.get("music21_counts", {}).get("correct"), flush=True)
    summary = {"cases": len(cases), "eligible": sum(c["eligible"] for c in cases),
               "gold_positive": sum(c["gold_positive"] for c in cases)}
    for tool in ("acorde", "music21"):
        total = Counter()
        for case in cases:
            total.update(case.get(f"{tool}_counts", {}))
        summary[tool] = dict(total)
        summary[f"{tool}_failures"] = sum(f"{tool}_error" in case for case in cases)
    output = {"schema_version": 1, "split": splits.pop(), "recipe": RECIPE,
              "source_commit": projection["source_commit"],
              "projection_sha256": sha256(args.projection), "preflight_sha256": sha256(args.preflight),
              "acorde_binary_sha256": sha256(binary), "acorde_command": "analyze-cadences INPUT --infer-key",
              "music21_version": music21.__version__,
              "script_sha256": sha256(Path(__file__)), "class_mapping": GOLD_KIND,
              "summary": summary, "cases": cases}
    args.output.write_text(json.dumps(output, ensure_ascii=False, indent=2) + "\n")
    print(json.dumps(summary))


if __name__ == "__main__":
    main()
