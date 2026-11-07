#!/usr/bin/env python3
"""Compare Acorde and music21 harmony at source-derived BCMH beats.

Both analyzers receive the same cleaned **kern source, without the annotation
spine or tonic/mode interpretations. Source rhythm independently locates each
gold annotation. A score needs equal pitched timelines in both imports, and a
measure needs source data-row beats equal to its pitched onset beats. Unsupported
source rhythm and missing Acorde labels count separately rather than shifting
the remaining rows.
"""

import argparse
import hashlib
import json
import re
import tempfile
import zipfile
from collections import defaultdict
from fractions import Fraction
from pathlib import Path

from compare_bcmh_keys import (
    ARCHIVE_SHA256,
    acorde_key_name,
    acorde_measure_timeline,
    acorde_output,
    music21_measure_timeline,
    key_name,
    remove_harmony_spine,
    same_measure_events,
)
from compare_bcmh_vertical_harmony import clean_label


def kern_duration(token):
    """Return a simple **kern reciprocal duration in quarter-note beats."""
    match = re.match(r"^(\d+)(\.*)", token)
    if not match or not int(match.group(1)):
        raise ValueError(f"unsupported **kern duration: {token!r}")
    base = Fraction(4, int(match.group(1)))
    return base * sum((Fraction(1, 2**i) for i in range(len(match.group(2)) + 1)), Fraction())


def source_beats(lines):
    """Locate **harm rows using the four **kern spines, without either importer."""
    rows = defaultdict(list)
    errors = {}
    measure = 0
    beat = Fraction()
    remaining = None
    seen_data = False
    for line in lines:
        if line.startswith("="):
            if seen_data:
                if any(remaining):
                    errors.setdefault(measure, "duration_crosses_barline")
                measure += 1
            beat = Fraction()
            remaining = None
            seen_data = False
            continue
        if not line or line.startswith(("!", "*")):
            continue
        fields = line.split("\t")
        if remaining is None:
            remaining = [Fraction()] * (len(fields) - 1)
        if len(fields) != len(remaining) + 1:
            errors.setdefault(measure, "spine_count_changed")
            continue
        rows[measure].append((fields[0], round(float(beat), 6)))
        seen_data = True
        for index, token in enumerate(fields[1:]):
            if token == ".":
                if remaining[index] <= 0:
                    errors.setdefault(measure, "null_without_active_duration")
                continue
            if remaining[index]:
                errors.setdefault(measure, "event_before_previous_duration_end")
            try:
                remaining[index] = kern_duration(token)
            except ValueError:
                errors.setdefault(measure, "unsupported_duration")
        positive = [duration for duration in remaining if duration > 0]
        if not positive:
            errors.setdefault(measure, "no_active_duration")
            continue
        step = min(positive)
        beat += step
        remaining = [duration - step for duration in remaining]
    return rows, errors


def canonical_roman(label):
    """Normalize only equivalent Roman glyphs and third-inversion figures."""
    if label is None:
        return None
    label = clean_label(label).replace("%", "ø").replace("°", "o")
    return re.sub(r"42(?=/|$)", "2", label)


def music21_measure_events(measures, index):
    events = []
    for part_measures in measures:
        measure = part_measures[index]
        for event in measure.recurse().notes:
            pitches = list(event.pitches)
            if not pitches:
                continue
            onset = round(float(event.getOffsetInHierarchy(measure)), 6)
            duration = float(event.duration.quarterLength)
            if duration > 0:
                events.append((onset, onset + duration, pitches))
    return events


def compare_one(archive, name, directory, acorde, converter, note, chord, roman, stream):
    source = archive.read(name)
    lines = source.decode("utf-8-sig").splitlines()
    cleaned = remove_harmony_spine(lines)
    path = directory / Path(name).name
    path.write_text(cleaned, encoding="utf-8")
    row = {
        "score": path.name,
        "source_sha256": hashlib.sha256(source).hexdigest(),
        "notes_only_sha256": hashlib.sha256(cleaned.encode()).hexdigest(),
    }
    try:
        m_score = converter.parse(str(path), forceSource=True)
        m_key = m_score.analyze("key")
        m_timeline = music21_measure_timeline(m_score, note, chord)
        measures = [list(part.getElementsByClass(stream.Measure)) for part in m_score.parts]
        row["music21_key"] = key_name(m_key.tonic.name, m_key.mode)
    except Exception as error:
        row["music21_error"] = f"{type(error).__name__}: {error}"
    try:
        a_report = json.loads(acorde_output(acorde, "analyze-vertical", path, "--infer-key"))
        a_import = json.loads(acorde_output(acorde, "report", path))
        a_timeline = acorde_measure_timeline(a_import["score"])
        a_key = a_report.get("global_key")
        row["acorde_key"] = acorde_key_name({"key": a_key}) if a_key else None
    except Exception as error:
        row["acorde_error"] = f"{type(error).__name__}: {error}"
    if "music21_error" in row or "acorde_error" in row:
        return row

    row["pitched_timeline_equal"] = same_measure_events(
        m_timeline, a_timeline, "pitched", with_onset=True
    )
    if not row["pitched_timeline_equal"]:
        return row

    source_rows, source_errors = source_beats(lines)
    a_sonorities = defaultdict(dict)
    for sonority in a_report["sonorities"]:
        beat = round(sonority["beat"], 6)
        a_sonorities[sonority["measure"]][beat] = sonority["label"]
    row["source_labeled_rows"] = sum(
        label not in {".", "*", ""} for values in source_rows.values() for label, _ in values
    )
    row["aligned_measures"] = 0
    row["unaligned_measures"] = 0
    row["unaligned_reasons"] = defaultdict(int)
    row["labeled_rows"] = 0
    row["acorde_predicted_rows"] = 0
    row["missing_sonority_rows"] = 0
    row["unlabeled_sonority_rows"] = 0
    row["missing_by_pitch_class_count"] = defaultdict(int)
    row["unlabeled_by_chord_kind"] = defaultdict(int)
    row["music21_predicted_rows"] = 0
    row["acorde_exact"] = 0
    row["music21_exact"] = 0
    row["both_exact"] = 0
    row["acorde_canonical_exact"] = 0
    row["music21_canonical_exact"] = 0
    row["both_canonical_exact"] = 0
    row["errors"] = []
    for index, gold_rows in source_rows.items():
        if any(index >= len(part_measures) for part_measures in measures):
            row["unaligned_measures"] += 1
            row["unaligned_reasons"]["missing_import_measure"] += 1
            continue
        events = music21_measure_events(measures, index)
        onsets = sorted({start for start, _, _ in events})
        source_onsets = [beat for _, beat in gold_rows]
        reason = source_errors.get(index)
        if reason is None and source_onsets != onsets:
            reason = "source_vs_music21_onset_beats"
        if reason is not None:
            row["unaligned_measures"] += 1
            row["unaligned_reasons"][reason] += 1
            if len(row["errors"]) < 12:
                row["errors"].append({
                    "measure": index, "reason": reason,
                    "source_beats": source_onsets, "music21_onsets": onsets,
                })
            continue
        row["aligned_measures"] += 1
        for ordinal, (gold, beat) in enumerate(gold_rows):
            if gold in {".", "*", ""}:
                continue
            gold = clean_label(gold)
            row["labeled_rows"] += 1
            active = [pitch for start, end, pitches in events
                      if start <= beat + 1e-6 < end for pitch in pitches]
            try:
                m_label = roman.romanNumeralFromChord(chord.Chord(active), m_key).figure
            except Exception:
                m_label = None
            a_entry = a_sonorities[index].get(beat)
            a_label = a_entry.get("roman_numeral") if a_entry else None
            if a_label is None:
                if a_entry:
                    row["unlabeled_sonority_rows"] += 1
                    row["unlabeled_by_chord_kind"][a_entry["chord"]["kind"]] += 1
                else:
                    row["missing_sonority_rows"] += 1
                    row["missing_by_pitch_class_count"][str(len({p.pitchClass for p in active}))] += 1
            row["acorde_predicted_rows"] += a_label is not None
            row["music21_predicted_rows"] += m_label is not None
            a_exact = a_label == gold
            m_exact = m_label == gold
            row["acorde_exact"] += a_exact
            row["music21_exact"] += m_exact
            row["both_exact"] += a_exact and m_exact
            a_canonical = canonical_roman(a_label) == canonical_roman(gold)
            m_canonical = canonical_roman(m_label) == canonical_roman(gold)
            row["acorde_canonical_exact"] += a_canonical
            row["music21_canonical_exact"] += m_canonical
            row["both_canonical_exact"] += a_canonical and m_canonical
            if (not a_exact or not m_exact) and len(row["errors"]) < 12:
                example = {
                    "measure": index, "row": ordinal, "beat": beat,
                    "gold": gold, "acorde": a_label, "music21": m_label,
                }
                if a_label is None:
                    example["active_pitches"] = [p.nameWithOctave for p in active]
                    if a_entry:
                        example["acorde_chord"] = {
                            field: a_entry["chord"].get(field)
                            for field in ("root", "kind", "bass")
                        }
                row["errors"].append(example)
    return row


def main():
    parser = argparse.ArgumentParser(description=__doc__)
    parser.add_argument("archive", type=Path)
    parser.add_argument("acorde", type=Path)
    parser.add_argument("output", type=Path)
    args = parser.parse_args()
    from music21 import chord, converter, note, roman, stream
    import music21

    if music21.__version__ != "10.5.2":
        parser.error(f"expected music21 10.5.2, got {music21.__version__}")
    if hashlib.sha256(args.archive.read_bytes()).hexdigest() != ARCHIVE_SHA256:
        parser.error("BCMH archive SHA-256 does not match the pinned version")
    with zipfile.ZipFile(args.archive) as archive:
        names = sorted(name for name in archive.namelist()
                       if re.fullmatch(r"BCMH_dataset/annotated/chor\d{3}_rev\.krn", name))
        if len(names) != 100:
            parser.error(f"expected 100 annotated chorales, found {len(names)}")
        with tempfile.TemporaryDirectory(prefix="acorde-bcmh-harmony-paired-") as temporary:
            rows = []
            for name in names:
                try:
                    rows.append(compare_one(
                        archive, name, Path(temporary), args.acorde,
                        converter, note, chord, roman, stream,
                    ))
                except Exception as error:
                    rows.append({"score": Path(name).name,
                                 "error": f"{type(error).__name__}: {error}"})
    paired = [row for row in rows if "error" not in row
              and "music21_error" not in row and "acorde_error" not in row]
    eligible = [row for row in paired if row["pitched_timeline_equal"]]
    summary = {
        "total_scores": len(rows),
        "music21_parsed": sum("music21_error" not in row and "error" not in row for row in rows),
        "acorde_parsed": sum("acorde_error" not in row and "error" not in row for row in rows),
        "paired_parses": len(paired),
        "equal_pitched_timelines": len(eligible),
        "equal_estimated_keys": sum(row["acorde_key"] == row["music21_key"] for row in eligible),
        "aligned_measures": sum(row["aligned_measures"] for row in eligible),
        "unaligned_measures": sum(row["unaligned_measures"] for row in eligible),
        "unaligned_reasons": dict(sorted((
            (reason, sum(row["unaligned_reasons"].get(reason, 0) for row in eligible))
            for reason in {reason for row in eligible for reason in row["unaligned_reasons"]}
        ))),
        "source_labeled_rows": sum(row["source_labeled_rows"] for row in eligible),
        "labeled_rows_in_aligned_measures": sum(row["labeled_rows"] for row in eligible),
        "acorde_predicted_rows": sum(row["acorde_predicted_rows"] for row in eligible),
        "missing_sonority_rows": sum(row["missing_sonority_rows"] for row in eligible),
        "unlabeled_sonority_rows": sum(row["unlabeled_sonority_rows"] for row in eligible),
        "missing_by_pitch_class_count": dict(sorted((
            (count, sum(row["missing_by_pitch_class_count"].get(count, 0) for row in eligible))
            for count in {count for row in eligible for count in row["missing_by_pitch_class_count"]}
        ))),
        "unlabeled_by_chord_kind": dict(sorted((
            (kind, sum(row["unlabeled_by_chord_kind"].get(kind, 0) for row in eligible))
            for kind in {kind for row in eligible for kind in row["unlabeled_by_chord_kind"]}
        ))),
        "music21_predicted_rows": sum(row["music21_predicted_rows"] for row in eligible),
        "acorde_exact": sum(row["acorde_exact"] for row in eligible),
        "music21_exact": sum(row["music21_exact"] for row in eligible),
        "both_exact": sum(row["both_exact"] for row in eligible),
        "acorde_canonical_exact": sum(row["acorde_canonical_exact"] for row in eligible),
        "music21_canonical_exact": sum(row["music21_canonical_exact"] for row in eligible),
        "both_canonical_exact": sum(row["both_canonical_exact"] for row in eligible),
    }
    report = {
        "contract_version": 3,
        "corpus": "MusicCogCompLab/BCMH@97dfc249b90a8739457d83a2702644173c7eb4a8",
        "archive_sha256": ARCHIVE_SHA256,
        "acorde_binary_sha256": hashlib.sha256(args.acorde.read_bytes()).hexdigest(),
        "music21_version": music21.__version__,
        "conditions": "same cleaned kern; independently estimated global keys; gold row beats derived directly from **kern reciprocal durations and matched to music21 pitched onsets",
        "summary": summary,
        "rows": rows,
    }
    args.output.write_text(json.dumps(report, indent=2) + "\n", encoding="utf-8")
    print(json.dumps(summary, sort_keys=True))


if __name__ == "__main__":
    main()
