#!/usr/bin/env python3
"""Score a frozen When in Rome RomanText holdout against two score analyzers."""

import argparse
from collections import Counter, defaultdict
from fractions import Fraction
import hashlib
import json
from pathlib import Path
import re
import subprocess
import xml.etree.ElementTree as ET
import zipfile

from compare_bcmh_keys import DURATION_BEATS, STEP_SEMITONES


def canonical(label):
    if label is None:
        return None
    label = re.sub(r"\s+", "", label).replace("%", "ø").replace("°", "o")
    return re.sub(r"42(?=/|$)", "2", label)


def checked_file(root, path, expected_sha):
    file = root / path
    data = file.read_bytes()
    if hashlib.sha256(data).hexdigest() != expected_sha:
        raise ValueError(f"SHA-256 mismatch: {path}")
    return file


def score_has_harmony(path):
    with zipfile.ZipFile(path) as archive:
        for name in archive.namelist():
            if name.startswith("META-INF/") or not name.endswith((".xml", ".musicxml")):
                continue
            xml = ET.fromstring(archive.read(name))
            if any(element.tag.rsplit("}", 1)[-1] == "harmony" for element in xml.iter()):
                return True
    return False


def music21_measures(score, stream):
    return [list(part.getElementsByClass(stream.Measure)) for part in score.parts]


def music21_events(measures, index):
    events = []
    for part in measures:
        if index >= len(part):
            return None
        measure = part[index]
        for note in measure.recurse().notes:
            if not note.pitches or note.duration.quarterLength <= 0:
                continue
            events.append((round(float(note.getOffsetInHierarchy(measure)), 6),
                           round(float(note.duration.quarterLength), 6),
                           tuple(sorted(round(p.midi, 6) for p in note.pitches)),
                           tuple(note.pitches)))
    return events


def acorde_fraction_beats(note):
    if note["is_grace"]:
        return Fraction()
    base = Fraction(str(DURATION_BEATS[note["duration"]]))
    dotted = sum((Fraction(1, 2**i) for i in range(note["dot_count"] + 1)), Fraction())
    duration = base * dotted
    if note["tuplet"]:
        duration *= Fraction(note["tuplet"]["normal_notes"], note["tuplet"]["actual_notes"])
    return duration


def acorde_events(score, index):
    events = []
    for part in score["parts"]:
        for staff in part["staves"]:
            if index >= len(staff["measures"]):
                return None
            for voice in staff["measures"][index]["voices"]:
                onset = Fraction()
                for note in voice:
                    duration = acorde_fraction_beats(note)
                    if not note["is_rest"] and duration > 0:
                        pitches = tuple(sorted(round(
                            (p["octave"] + 1) * 12 + STEP_SEMITONES[p["step"]]
                            + p["alter"] + p["microtone_cents"] / 100, 6
                        ) for p in note["pitches"]))
                        events.append((round(float(onset), 6), round(float(duration), 6), pitches))
                    onset += duration
    return events


def timeline_equal(m_events, a_events):
    return Counter((start, duration, pitches) for start, duration, pitches, _ in m_events) == Counter(
        (start, duration, pitches) for start, duration, pitches in a_events)


def gold_rows(gold, roman, stream):
    rows = defaultdict(list)
    for item in gold.recurse().getElementsByClass(roman.RomanNumeral):
        measure = item.getContextByClass(stream.Measure)
        if measure is None:
            raise ValueError("RomanNumeral outside measure")
        beat = round(float(item.getOffsetInHierarchy(measure)), 6)
        rows[measure.number].append((beat, item.figure, str(item.key)))
    return rows


def run_acorde(binary, action, file, *options):
    result = subprocess.run([str(binary), action, str(file), *options],
                            check=True, capture_output=True, text=True)
    return json.loads(result.stdout)


def compare_one(work, hashes, root, binary, converter, chord, roman, stream,
                arpeggiated_bars=False, prepared_dir=None, prepared=None):
    input_hash = prepared["input_sha256"] if prepared is not None else hashes["score_sha256"]
    path = checked_file(prepared_dir or root, work["score_path"], input_hash)
    gold_path = checked_file(root, work["gold_path"], hashes["gold_sha256"])
    result = {"work": work["work"], "score_sha256": input_hash,
              "gold_sha256": hashes["gold_sha256"], "rows": [], "exclusions": []}
    if score_has_harmony(path):
        result["error"] = "score contains harmony answers"
        return result
    try:
        gold = converter.parse(str(gold_path), format="romanText")
        source_rows = gold_rows(gold, roman, stream)
    except Exception as error:
        result["error"] = f"gold_parse: {type(error).__name__}: {error}"
        return result
    result["source_labels"] = sum(map(len, source_rows.values()))
    try:
        m_score = converter.parse(str(path), forceSource=True)
        m_key = m_score.analyze("key")
        m_measures = music21_measures(m_score, stream)
        result["music21_key"] = str(m_key)
    except Exception as error:
        result["music21_error"] = f"{type(error).__name__}: {error}"
    try:
        a_import = run_acorde(binary, "report", path)
        a_options = ("--infer-key", "--arpeggiated-bars") if arpeggiated_bars else ("--infer-key",)
        a_report = run_acorde(binary, "analyze-vertical", path, *a_options)
        result["acorde_key"] = a_report.get("global_key")
        a_measures = a_import["score"]["parts"][0]["staves"][0]["measures"]
    except Exception as error:
        result["acorde_error"] = f"{type(error).__name__}: {error}"
    if "music21_error" in result or "acorde_error" in result:
        return result
    sonorities = defaultdict(dict)
    for entry in a_report["sonorities"]:
        sonorities[entry["measure"]][round(entry["beat"], 6)] = entry["label"].get("roman_numeral")
    for number, annotations in sorted(source_rows.items()):
        beat_counts = Counter(beat for beat, _, _ in annotations)
        ambiguous = {beat for beat, count in beat_counts.items() if count > 1}
        if ambiguous:
            result["exclusions"].append({"measure": number,
                                         "labels": sum(beat_counts[beat] for beat in ambiguous),
                                         "reason": "ambiguous_gold_position"})
            annotations = [row for row in annotations if row[0] not in ambiguous]
        if not annotations:
            continue
        indices = [i for i, measure in enumerate(m_measures[0]) if measure.number == number]
        if len(indices) != 1:
            result["exclusions"].append({"measure": number, "labels": len(annotations),
                                         "reason": "missing_or_ambiguous_source_measure"})
            continue
        index = indices[0]
        if index >= len(a_measures) or a_measures[index]["number"] != number:
            result["exclusions"].append({"measure": number, "labels": len(annotations),
                                         "reason": "acorde_measure_number_mismatch"})
            continue
        m_events = music21_events(m_measures, index)
        a_events = acorde_events(a_import["score"], index)
        if m_events is None or a_events is None or not timeline_equal(m_events, a_events):
            result["exclusions"].append({"measure": number, "labels": len(annotations),
                                         "reason": "unequal_pitched_timeline"})
            continue
        for beat, label, local_key in annotations:
            active = [pitch for start, duration, _, pitches in m_events
                      if start <= beat + 1e-6 < start + duration for pitch in pitches]
            try:
                m_label = roman.romanNumeralFromChord(chord.Chord(active), m_key).figure if active else None
            except Exception:
                m_label = None
            a_label = sonorities[index].get(beat)
            result["rows"].append({"measure": number, "measure_index": index, "beat": beat,
                                   "gold": label, "gold_local_key": local_key,
                                   "acorde": a_label, "music21": m_label,
                                   "acorde_correct": canonical(a_label) == canonical(label),
                                   "music21_correct": canonical(m_label) == canonical(label)})
    result["scored_labels"] = len(result["rows"])
    result["excluded_labels"] = sum(item["labels"] for item in result["exclusions"])
    return result


def main():
    parser = argparse.ArgumentParser(description=__doc__)
    parser.add_argument("selection", type=Path)
    parser.add_argument("hashes", type=Path)
    parser.add_argument("source_dir", type=Path)
    parser.add_argument("acorde", type=Path)
    parser.add_argument("output", type=Path)
    parser.add_argument("--classification", choices=("holdout", "development"), default="development")
    parser.add_argument("--arpeggiated-bars", action="store_true")
    parser.add_argument("--prepared-inputs", type=Path)
    parser.add_argument("--prepared-input-dir", type=Path)
    args = parser.parse_args()
    from music21 import chord, converter, roman, stream
    import music21

    if music21.__version__ != "10.5.2":
        parser.error(f"requires music21 10.5.2; found {music21.__version__}")
    selected = json.loads(args.selection.read_text())
    hashes = json.loads(args.hashes.read_text())
    if hashlib.sha256(args.selection.read_bytes()).hexdigest() != hashes["selection_sha256"]:
        parser.error("selection hash mismatch")
    if selected["commit"] != hashes["commit"]:
        parser.error("commit mismatch")
    by_work = {x["work"]: x for x in hashes["files"]}
    if set(by_work) != {x["work"] for x in selected["works"]}:
        parser.error("work set mismatch")
    if (args.prepared_inputs is None) != (args.prepared_input_dir is None):
        parser.error("--prepared-inputs and --prepared-input-dir must be used together")
    prepared_bytes = args.prepared_inputs.read_bytes() if args.prepared_inputs else None
    prepared = json.loads(prepared_bytes) if prepared_bytes else None
    prepared_by_work = {x["work"]: x for x in prepared["inputs"]} if prepared else {}
    if prepared:
        if (prepared.get("source_commit") != selected["commit"]
                or prepared.get("selection_sha256") != hashes["selection_sha256"]
                or set(prepared_by_work) != set(by_work)):
            parser.error("prepared input set or provenance mismatch")
        for work in selected["works"]:
            item = prepared_by_work[work["work"]]
            source = by_work[work["work"]]
            if (item["path"] != work["score_path"]
                    or item["source_sha256"] != source["score_sha256"]
                    or item["gold_sha256"] != source["gold_sha256"]):
                parser.error(f"prepared input provenance mismatch: {work['work']}")
    binary = args.acorde.resolve()
    results = [compare_one(work, by_work[work["work"]], args.source_dir, binary,
                           converter, chord, roman, stream, args.arpeggiated_bars,
                           args.prepared_input_dir, prepared_by_work.get(work["work"]))
               for work in selected["works"]]
    rows = [row for result in results for row in result["rows"]]
    summary = {
        "works": len(results),
        "source_labels": sum(result.get("source_labels", 0) for result in results),
        "scored_labels": len(rows),
        "excluded_labels": sum(result.get("excluded_labels", 0) for result in results),
        "music21_parse_failures": sum("music21_error" in result for result in results),
        "acorde_parse_failures": sum("acorde_error" in result for result in results),
        "acorde_predicted": sum(row["acorde"] is not None for row in rows),
        "music21_predicted": sum(row["music21"] is not None for row in rows),
        "acorde_correct": sum(row["acorde_correct"] for row in rows),
        "music21_correct": sum(row["music21_correct"] for row in rows),
    }
    report = {"schema_version": 1, "classification": args.classification,
              "arpeggiated_bars": args.arpeggiated_bars,
              "prepared_inputs_sha256": hashlib.sha256(prepared_bytes).hexdigest()
              if prepared_bytes else None,
              "selection_sha256": hashes["selection_sha256"],
              "commit": selected["commit"], "acorde_binary_sha256": hashlib.sha256(binary.read_bytes()).hexdigest(),
              "music21_version": music21.__version__, "summary": summary, "works": results}
    args.output.write_text(json.dumps(report, ensure_ascii=False, indent=2) + "\n")
    print(json.dumps(summary))


if __name__ == "__main__":
    main()
