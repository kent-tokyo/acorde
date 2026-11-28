#!/usr/bin/env python3
"""Freeze independent consecutive-fifth/octave labels for Bach chorales (#502).

The labels transcribe Luke Dahn's list "Consecutive Fifths & Octaves in the
Chorales" (https://www.bach-chorales.com/ConsecutivesInChorales.htm,
copyright 2018, read 2026-11-28): 49 rows in its fermata, non-structural and
structural tables. Its six manuscript-only Dietel additions are left out,
since the scores here follow the printed edition. Each row is expanded to one
label per voice pair and interval. Chorales are matched to music21 10.5.2's
bundled corpus by BWV number; two have no corpus file and are listed as
unmatched. Chorales are split into development and holdout halves by a
seeded SHA-256 rank, before either tool is run on them.

Usage: freeze_voice_leading_dahn_labels.py ROWS_TXT OUTPUT
"""

import argparse
import hashlib
import json
import re
from pathlib import Path

import music21
from music21 import corpus

SOURCE = "https://www.bach-chorales.com/ConsecutivesInChorales.htm"
SEED = "acorde-voice-leading-dahn-split-20261128"
VOICES = {"S": 0, "A": 1, "T": 2, "B": 3}
CORPUS_ALIASES = {"248.33": "bwv248.33-3"}


def sha256(data):
    return hashlib.sha256(data).hexdigest()


def measures(spec):
    """'4', '4–5', '2/6', '11/25–12/26' -> sorted measure numbers."""
    numbers = set()
    for part in re.split(r"[–/-]", spec):
        numbers.add(int(part))
    return sorted(numbers)


def expand(voices, description):
    names = voices.split("/")
    kinds = []
    pairs = re.findall(r"P([58])s?[^()]*\(([SATB])/([SATB])\)", description)
    if pairs:
        for interval, upper, lower in pairs:
            kinds.append(("fifth" if interval == "5" else "octave", upper, lower))
    else:
        if len(names) != 2:
            raise ValueError(f"cannot assign interval to voices: {voices} {description}")
        found = re.search(r"P5|P8|fifth|octave", description)
        if not found:
            raise ValueError(f"no interval in: {description}")
        # The first interval named is the consecutive one ("P5s (melodically up a P8)").
        interval = "fifth" if found.group() in ("P5", "fifth") else "octave"
        kinds.append((interval, names[0], names[1]))
    labels = []
    for interval, first, second in kinds:
        upper, lower = sorted((first, second), key=VOICES.get)
        labels.append({"interval": interval, "upper": upper, "lower": lower})
    return labels


def main():
    parser = argparse.ArgumentParser(description=__doc__)
    parser.add_argument("rows", type=Path)
    parser.add_argument("output", type=Path)
    args = parser.parse_args()
    if args.output.exists():
        parser.error("refusing to overwrite frozen labels")
    if music21.__version__ != "10.5.2":
        parser.error("requires music21 10.5.2")
    available = {path.name.rsplit(".", 1)[0]: path for path in corpus.getComposer("bach")}
    rows_bytes = args.rows.read_bytes()
    chorales = {}
    unmatched = []
    for number, line in enumerate(rows_bytes.decode().splitlines(), 1):
        table, bwv, riemenschneider, spec, voices, description = [c.strip() for c in line.split(" | ")]
        name = CORPUS_ALIASES.get(bwv, "bwv" + bwv)
        row = {"row": number, "table": table, "riemenschneider": riemenschneider,
               "measure_text": spec, "measures": measures(spec), "voices": voices,
               "description": description, "labels": expand(voices, description)}
        if name not in available:
            unmatched.append({"bwv": bwv, **row})
            continue
        entry = chorales.setdefault(bwv, {"bwv": bwv, "corpus_file": available[name].name,
                                          "corpus_sha256": sha256(available[name].read_bytes()),
                                          "rows": []})
        entry["rows"].append(row)
    ranked = sorted(chorales.values(), key=lambda c: sha256(f"{SEED}\n{c['bwv']}".encode()))
    half = len(ranked) // 2
    for index, chorale in enumerate(ranked):
        chorale["split"] = "development" if index < half else "holdout"
    result = {"schema_version": 1,
              "classification": "independent consecutive fifth/octave labels; split frozen before either tool ran",
              "source": SOURCE, "source_author": "Luke Dahn", "source_copyright": "2018",
              "read_on": "2026-11-28", "rows_sha256": sha256(rows_bytes), "seed": SEED,
              "music21_version": music21.__version__,
              "not_exhaustive": "The source does not claim to list every consecutive; predictions outside it are reported as unlisted, not as confirmed errors.",
              "excluded": "the six Dietel manuscript variants",
              "unmatched_in_corpus": unmatched,
              "chorales": sorted(ranked, key=lambda c: [int(p) for p in c["bwv"].split(".")])}
    args.output.write_text(json.dumps(result, ensure_ascii=False, indent=2) + "\n")
    print({split: sum(c["split"] == split for c in ranked) for split in ("development", "holdout")},
          "unmatched", [u["bwv"] for u in unmatched])


if __name__ == "__main__":
    main()
