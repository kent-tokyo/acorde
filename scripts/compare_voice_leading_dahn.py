#!/usr/bin/env python3
"""Score Acorde and music21 consecutive fifths/octaves against Dahn's chorale labels (#502).

Inputs are music21 10.5.2's bundled corpus files, hash-checked against the
frozen label file. Voices are the four corpus parts in order (S, A, T, B).

* Acorde: `analyze-voice-leading`; every quartet flagged `parallel_fifth` or
  `parallel_octave` gives (interval, upper part, lower part, measure numbers of
  the two states). Antiparallel motion counts, as in music21.
* music21 ("music21-figuredBass-checker-v1"): figuredBass.checker's
  voice-leading moments (`extractHarmonies` + `generalNoteToPitch`) with
  `checker.parallelFifths` / `checker.parallelOctaves` on each consecutive pair
  of moments; measure numbers come from part 0 at each moment's offset.

A detection covers a gold label when interval and voice pair match and either
state's measure number is among the label's listed measure numbers. Positions
for the label evaluator are (upper, lower, measure): a covering detection takes
the label's first listed measure; any other detection keeps its arrival
measure and counts as unlisted (the source list is not exhaustive).

Usage: compare_voice_leading_dahn.py LABELS SPLIT ACORDE OUTPUT [--work-dir DIR]
"""

import argparse
import hashlib
import json
from pathlib import Path
import shutil
import subprocess
import tempfile

import music21
from music21 import corpus, stream
from music21.figuredBass import checker

RECIPE = "music21-figuredBass-checker-v1"
VOICES = "SATB"


def sha256(path):
    return hashlib.sha256(Path(path).read_bytes()).hexdigest()


def music21_detections(path):
    score = music21.converter.parse(str(path))
    part0 = score.parts[0]

    def number(offset):
        measure = part0.getElementAtOrBefore(offset, [stream.Measure])
        return measure.number if measure is not None else None

    found = set()
    moments = sorted(checker.extractHarmonies(score).items())
    previous_offsets, previous_notes = moments[0]
    before = [checker.generalNoteToPitch(note) for note in previous_notes]
    for offsets, notes in moments[1:]:
        after = [checker.generalNoteToPitch(note) for note in notes]
        for interval, rule in (("fifth", checker.parallelFifths), ("octave", checker.parallelOctaves)):
            for upper, lower in rule(before, after):
                found.add((interval, upper - 1, lower - 1,
                           number(previous_offsets[0]), number(offsets[0])))
        before, previous_offsets = after, offsets
    return len(score.parts), sorted(found, key=lambda x: (x[4], x[3], x[1], x[2], x[0]))


def acorde_detections(binary, path):
    report = json.loads(subprocess.check_output([str(binary), "report", str(path)]))["score"]
    numbers = [[m["number"] for m in part["staves"][0]["measures"]] for part in report["parts"]]
    analysis = json.loads(subprocess.check_output([str(binary), "analyze-voice-leading", str(path)]))
    found = set()
    for quartet in analysis["quartets"]:
        first = quartet["first_before"]["address"]
        second = quartet["second_before"]["address"]
        before = numbers[first["part"]][first["measure"]]
        after = numbers[quartet["first_after"]["address"]["part"]][quartet["first_after"]["address"]["measure"]]
        upper, lower = sorted((first["part"], second["part"]))
        for interval, flag in (("fifth", "parallel_fifth"), ("octave", "parallel_octave")):
            if quartet[flag]:
                found.add((interval, upper, lower, before, after))
    return len(report["parts"]), sorted(found, key=lambda x: (x[4], x[3], x[1], x[2], x[0]))


def place(labels, detections):
    """Map detections to evaluator positions; return (positions, covered label ids)."""
    positions, covered = {}, set()
    for interval, upper, lower, before, after in detections:
        hit = next((label for label in labels
                    if label["interval"] == interval
                    and (VOICES.index(label["upper"]), VOICES.index(label["lower"])) == (upper, lower)
                    and ({before, after} & set(label["measures"]))), None)
        if hit is not None:
            covered.add(hit["id"])
            key = (interval, upper, lower, hit["measures"][0])
        else:
            key = (interval, upper, lower, after)
        positions[key] = hit["id"] if hit is not None else None
    return positions, covered


def main():
    parser = argparse.ArgumentParser(description=__doc__)
    parser.add_argument("labels", type=Path)
    parser.add_argument("split", choices=("development", "holdout"))
    parser.add_argument("acorde", type=Path)
    parser.add_argument("output", type=Path)
    args = parser.parse_args()
    if music21.__version__ != "10.5.2":
        parser.error("requires music21 10.5.2")
    frozen = json.loads(args.labels.read_text())
    binary = args.acorde.resolve()
    available = {path.name: path for path in corpus.getComposer("bach")}
    cases = []
    with tempfile.TemporaryDirectory() as work:
        for chorale in frozen["chorales"]:
            if chorale["split"] != args.split:
                continue
            source = available[chorale["corpus_file"]]
            if sha256(source) != chorale["corpus_sha256"]:
                parser.error(f"corpus file changed: {chorale['corpus_file']}")
            path = Path(work) / chorale["corpus_file"]
            shutil.copyfile(source, path)
            labels = []
            for row in chorale["rows"]:
                for index, label in enumerate(row["labels"]):
                    labels.append({"id": f"{row['row']}.{index}", "measures": row["measures"], **label})
            case = {"bwv": chorale["bwv"], "corpus_file": chorale["corpus_file"],
                    "input_sha256": chorale["corpus_sha256"], "labels": labels}
            for tool, run in (("acorde", lambda: acorde_detections(binary, path)),
                              ("music21", lambda: music21_detections(path))):
                try:
                    parts, detections = run()
                except Exception as error:  # record every tool failure
                    case[f"{tool}_error"] = f"{type(error).__name__}: {error}"
                    continue
                if parts != 4:
                    case[f"{tool}_error"] = f"expected 4 parts, found {parts}"
                    continue
                positions, covered = place(labels, detections)
                case[tool] = {"detections": [list(d) for d in detections],
                              "positions": [{"interval": k[0], "upper": VOICES[k[1]], "lower": VOICES[k[2]],
                                             "measure": k[3], "label": v} for k, v in sorted(positions.items())],
                              "covered": sorted(covered),
                              "unlisted": sum(v is None for v in positions.values())}
            cases.append(case)
            print(chorale["bwv"], len(labels), *(len(case[t]["covered"]) if t in case else "error"
                                                    for t in ("acorde", "music21")), flush=True)
    summary = {"chorales": len(cases), "labels": sum(len(c["labels"]) for c in cases)}
    for tool in ("acorde", "music21"):
        summary[tool] = {"failures": sum(f"{tool}_error" in c for c in cases),
                         "covered": sum(len(c[tool]["covered"]) for c in cases if tool in c),
                         "unlisted": sum(c[tool]["unlisted"] for c in cases if tool in c)}
    output = {"schema_version": 1, "split": args.split, "recipe": RECIPE,
              "labels_sha256": sha256(args.labels), "acorde_binary_sha256": sha256(binary),
              "acorde_command": "analyze-voice-leading INPUT",
              "music21_version": music21.__version__, "script_sha256": sha256(__file__),
              "summary": summary, "cases": cases}
    args.output.write_text(json.dumps(output, ensure_ascii=False, indent=2) + "\n")
    print(json.dumps(summary))


if __name__ == "__main__":
    main()
