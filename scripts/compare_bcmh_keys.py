#!/usr/bin/env python3
"""Compare global-key estimates against the independently annotated BCMH chorales.

Usage: uv run --no-project --with music21==10.5.2 python3 \
    scripts/compare_bcmh_keys.py BCMH_dataset.zip /path/to/acorde OUT.json

The caller supplies the pinned BCMH archive. The script removes its leading
annotation spine and tonic/mode interpretations before either analyzer reads a
score, so neither receives the answer.
It reports both Acorde's first candidate and whether the answer is anywhere
in its tied candidate list. These are different measures.
"""

import argparse
import hashlib
import json
import re
import subprocess
import tempfile
import zipfile
from collections import Counter
from pathlib import Path


ARCHIVE_SHA256 = "9aa8ac78052156932a04431fd56d7a217fe4cd07d0dc726dac1b9dbf85870b1c"
MAJOR = ["Cb", "Gb", "Db", "Ab", "Eb", "Bb", "F", "C", "G", "D", "A", "E", "B", "F#", "C#"]
MINOR = ["Ab", "Eb", "Bb", "F", "C", "G", "D", "A", "E", "B", "F#", "C#", "G#", "D#", "A#"]
KEY_TOKEN = re.compile(r"^\*([A-Ga-g][#-]?):$")
TONAL_INTERPRETATION = re.compile(r"^\*[A-Ga-g][#-]?:[a-z]*$")
HARM_KEY = re.compile(r"^([A-Ga-g][#-]?):[^\t]*$")
NOTE_COUNT = re.compile(r"^notes:\s+(\d+)", re.MULTILINE)
STEP_SEMITONES = {"C": 0, "D": 2, "E": 4, "F": 5, "G": 7, "A": 9, "B": 11}
DURATION_BEATS = {
    "Longa": 16, "Breve": 8, "Whole": 4, "Half": 2,
    "Quarter": 1, "Eighth": 0.5, "Sixteenth": 0.25,
    "ThirtySecond": 0.125, "SixtyFourth": 0.0625,
    "HundredTwentyEighth": 0.03125,
}


def sha256(data):
    return hashlib.sha256(data).hexdigest()


def key_name(tonic, mode):
    return f"{tonic.replace('-', 'b').upper()[0]}{tonic.replace('-', 'b')[1:]} {mode}"


def gold_key(lines):
    labels = {
        match.group(1)
        for line in lines
        for field in line.split("\t")
        for match in [KEY_TOKEN.fullmatch(field)]
        if match
    }
    source = "key_interpretation"
    if not labels:
        first_harmonic_key = next((
            match.group(1)
            for line in lines
            for match in [HARM_KEY.fullmatch(line.split("\t", 1)[0])]
            if match
        ), None)
        labels = {first_harmonic_key} if first_harmonic_key else set()
        source = "first_harmony_key"
    if len(labels) != 1:
        raise ValueError(f"expected one corpus global-key label, found {sorted(labels)}")
    label = labels.pop()
    return key_name(label, "major" if label[0].isupper() else "minor"), source


def remove_harmony_spine(lines):
    if not any(
        line.split("\t")[0] in {"**harm", "**"}
        and line.count("**kern") == 4
        for line in lines
    ):
        raise ValueError("expected a leading annotation spine and four **kern spines")
    cleaned = []
    for line in lines:
        if "\t" not in line or line.startswith("!!"):
            cleaned.append(line)
            continue
        music_spines = line.split("\t")[1:]
        cleaned.append("\t".join(
            "*" if TONAL_INTERPRETATION.fullmatch(field) else field
            for field in music_spines
        ))
    return "\n".join(cleaned) + "\n"


def acorde_key_name(candidate):
    key = candidate["key"]
    fifths = key["fifths"]
    if not -7 <= fifths <= 7:
        raise ValueError(f"unsupported fifths: {fifths}")
    mode = key["mode"]
    tonic = MAJOR[fifths + 7] if mode == "major" else MINOR[fifths + 7]
    return f"{tonic} {mode}"


def acorde_output(acorde, command, score_path, *extra_args):
    result = subprocess.run(
        [str(acorde), command, str(score_path), *extra_args],
        capture_output=True,
        text=True,
        timeout=15,
        check=False,
    )
    if result.returncode:
        raise RuntimeError(result.stderr.strip()[:300] or f"exit status {result.returncode}")
    return result.stdout


def music21_events(score, note, chord):
    """One normalized event for each pitched note or chord, ignoring order."""
    events = []
    for event in score.recurse().notes:
        if isinstance(event, note.Note):
            pitches = (round(event.pitch.midi, 6),)
        elif isinstance(event, chord.Chord):
            pitches = tuple(sorted(round(pitch.midi, 6) for pitch in event.pitches))
        else:
            continue
        events.append((pitches, round(float(event.duration.quarterLength), 6)))
    return events


def music21_measure_timeline(score, note, chord):
    """Preserve each part, measure, onset and rest in the parsed score."""
    parts = []
    for part in score.parts:
        measures = []
        for measure in part.getElementsByClass("Measure"):
            events = []
            for event in measure.recurse().notesAndRests:
                if isinstance(event, note.Rest):
                    kind = "hidden_rest" if event.style.hideObjectOnPrint else "rest"
                    pitches = ()
                elif isinstance(event, note.Note):
                    kind, pitches = "pitched", (round(event.pitch.midi, 6),)
                elif isinstance(event, chord.Chord):
                    kind = "pitched"
                    pitches = tuple(sorted(round(p.midi, 6) for p in event.pitches))
                else:
                    continue
                events.append((
                    round(float(event.getOffsetInHierarchy(measure)), 6),
                    kind,
                    pitches,
                    round(float(event.duration.quarterLength), 6),
                ))
            measures.append(events)
        parts.append(measures)
    return parts


def acorde_duration_beats(event):
    beats = DURATION_BEATS[event["duration"]]
    beats *= 2 - 0.5 ** event["dot_count"]
    if event["tuplet"]:
        beats *= (event["tuplet"]["normal_notes"]
                  / event["tuplet"]["actual_notes"])
    return 0 if event["is_grace"] else round(beats, 6)


def acorde_measure_timeline(score):
    parts = []
    for part in score["parts"]:
        if len(part["staves"]) != 1:
            raise ValueError("BCMH comparison expects one staff per part")
        measures = []
        for measure in part["staves"][0]["measures"]:
            events = []
            for voice in measure["voices"]:
                onset = 0.0
                for event in voice:
                    beats = acorde_duration_beats(event)
                    pitches = () if event["is_rest"] else tuple(sorted(round(
                        (pitch["octave"] + 1) * 12
                        + STEP_SEMITONES[pitch["step"]]
                        + pitch["alter"] + pitch["microtone_cents"] / 100, 6
                    ) for pitch in event["pitches"]))
                    events.append((round(onset, 6),
                                   ("hidden_rest" if event.get("hidden", False) else "rest")
                                   if event["is_rest"] else "pitched",
                                   pitches, beats))
                    onset += beats
            measures.append(events)
        parts.append(measures)
    return parts


def same_measure_events(left, right, kind, *, with_onset):
    kinds = {kind} if isinstance(kind, str) else set(kind)
    if len(left) != len(right) or any(len(a) != len(b) for a, b in zip(left, right)):
        return False
    for left_part, right_part in zip(left, right):
        for left_measure, right_measure in zip(left_part, right_part):
            project = lambda events: Counter(
                (onset, pitches, duration) if with_onset else (pitches, duration)
                for onset, event_kind, pitches, duration in events if event_kind in kinds
            )
            if project(left_measure) != project(right_measure):
                return False
    return True


def acorde_events(score):
    events = []
    for part in score["parts"]:
        for staff in part["staves"]:
            for measure in staff["measures"]:
                for voice in measure["voices"]:
                    for event in voice:
                        if event["is_rest"]:
                            continue
                        pitches = tuple(sorted(round(
                            (pitch["octave"] + 1) * 12
                            + STEP_SEMITONES[pitch["step"]]
                            + pitch["alter"] + pitch["microtone_cents"] / 100, 6
                        ) for pitch in event["pitches"]))
                        events.append((pitches, acorde_duration_beats(event)))
    return events


def pitch_bag(events):
    return Counter(pitch for pitches, _ in events for pitch in pitches)


def compare_one(archive, name, temporary_dir, acorde, converter, note, chord, key_option):
    source = archive.read(name)
    lines = source.decode("utf-8-sig").splitlines()
    gold, gold_source = gold_key(lines)
    notes_only = remove_harmony_spine(lines).encode("utf-8")
    score_path = temporary_dir / Path(name).name
    score_path.write_bytes(notes_only)
    row = {
        "score": Path(name).name,
        "source_sha256": sha256(source),
        "notes_only_sha256": sha256(notes_only),
        "gold": gold,
        "gold_source": gold_source,
    }
    music21_projection = None
    music21_timeline = None
    try:
        score = converter.parse(str(score_path), forceSource=True)
        music21_key = score.analyze("key")
        music21_name = key_name(music21_key.tonic.name, music21_key.mode)
        music21_projection = music21_events(score, note, chord)
        music21_timeline = music21_measure_timeline(score, note, chord)
        row["music21"] = {
            "key": music21_name,
            "notes": len(music21_projection),
            "exact": music21_name == gold,
        }
    except Exception as error:
        row["music21"] = {"error": f"{type(error).__name__}: {error}"}
    try:
        info = acorde_output(acorde, "info", score_path)
        match = NOTE_COUNT.search(info)
        if not match:
            raise ValueError("Acorde info did not report a note count")
        extra_args = (key_option,) if key_option else ()
        analysis = json.loads(acorde_output(acorde, "analyze", score_path, *extra_args))
        import_report = json.loads(acorde_output(acorde, "report", score_path))
        acorde_projection = acorde_events(import_report["score"])
        acorde_timeline = acorde_measure_timeline(import_report["score"])
        candidates = [acorde_key_name(candidate) for candidate in analysis["key_estimates"]]
        row["acorde"] = {
            "candidates": candidates,
            "notes": int(match.group(1)),
            "projection_event_count_matches_info": len(acorde_projection) == int(match.group(1)),
            "first_exact": bool(candidates) and candidates[0] == gold,
            "candidate_contains_gold": gold in candidates,
        }
    except Exception as error:
        row["acorde"] = {"error": f"{type(error).__name__}: {error}"}
    if "error" not in row["music21"] and "error" not in row["acorde"]:
        row["note_counts_equal"] = row["music21"]["notes"] == row["acorde"]["notes"]
        row["pitch_bag_equal"] = pitch_bag(music21_projection) == pitch_bag(acorde_projection)
        row["pitch_duration_bag_equal"] = (
            Counter(music21_projection) == Counter(acorde_projection)
        )
        row["part_measure_counts_equal"] = (
            len(music21_timeline) == len(acorde_timeline)
            and all(len(left) == len(right)
                    for left, right in zip(music21_timeline, acorde_timeline))
        )
        row["part_measure_pitched_events_equal"] = same_measure_events(
            music21_timeline, acorde_timeline, "pitched", with_onset=False
        )
        row["part_measure_pitched_timeline_equal"] = same_measure_events(
            music21_timeline, acorde_timeline, "pitched", with_onset=True
        )
        row["part_measure_visible_rest_timeline_equal"] = same_measure_events(
            music21_timeline, acorde_timeline, "rest", with_onset=True
        )
        row["part_measure_all_rest_timeline_equal"] = same_measure_events(
            music21_timeline, acorde_timeline, {"rest", "hidden_rest"}, with_onset=True
        )
    return row


def main():
    parser = argparse.ArgumentParser(description=__doc__)
    parser.add_argument("archive", type=Path)
    parser.add_argument("acorde", type=Path)
    parser.add_argument("output", type=Path)
    key_options = parser.add_mutually_exclusive_group()
    key_options.add_argument("--rank-tied-keys", action="store_true",
                             help="rank only equal-coverage key candidates")
    key_options.add_argument("--aarden-essen-keys", action="store_true",
                             help="rank all key candidates by the Aarden-Essen profile")
    args = parser.parse_args()
    from music21 import chord, converter, note
    import music21

    if music21.__version__ != "10.5.2":
        parser.error(f"expected music21 10.5.2, got {music21.__version__}")
    if sha256(args.archive.read_bytes()) != ARCHIVE_SHA256:
        parser.error("BCMH archive SHA-256 does not match the pinned version")
    key_option = ("--aarden-essen-keys" if args.aarden_essen_keys else
                  "--rank-tied-keys" if args.rank_tied_keys else None)
    with zipfile.ZipFile(args.archive) as archive:
        names = sorted(
            name for name in archive.namelist()
            if re.fullmatch(r"BCMH_dataset/annotated/chor\d{3}_rev\.krn", name)
        )
        if len(names) != 100:
            parser.error(f"expected 100 annotated chorales, found {len(names)}")
        with tempfile.TemporaryDirectory(prefix="acorde-bcmh-") as tmp:
            rows = []
            for name in names:
                try:
                    rows.append(compare_one(
                        archive, name, Path(tmp), args.acorde, converter, note, chord,
                        key_option,
                    ))
                except Exception as error:
                    rows.append({"score": Path(name).name,
                                 "error": f"{type(error).__name__}: {error}"})
    valid = [row for row in rows if "error" not in row]
    music21_valid = [row for row in valid if "error" not in row["music21"]]
    acorde_valid = [row for row in valid if "error" not in row["acorde"]]
    both = [row for row in music21_valid if "error" not in row["acorde"]]
    same_notes = [row for row in both if row["note_counts_equal"]]
    explicit_same_notes = [
        row for row in same_notes if row["gold_source"] == "key_interpretation"
    ]
    report = {
        "corpus": "MusicCogCompLab/BCMH@97dfc249b90a8739457d83a2702644173c7eb4a8",
        "license": "CC BY 4.0",
        "archive_sha256": ARCHIVE_SHA256,
        "acorde_binary_sha256": sha256(args.acorde.read_bytes()),
        "music21_version": music21.__version__,
        "input": "annotated **kern notes with leading annotation spine and tonic/mode interpretations removed",
        "summary": {
            "total": len(rows),
            "gold_from_key_interpretation": sum(row["gold_source"] == "key_interpretation" for row in valid),
            "gold_from_first_harmony_key": sum(row["gold_source"] == "first_harmony_key" for row in valid),
            "music21_parsed": len(music21_valid),
            "acorde_parsed": len(acorde_valid),
            "parsed_by_both": len(both),
            "equal_note_counts": sum(row["note_counts_equal"] for row in both),
            "equal_pitch_bags": sum(row["pitch_bag_equal"] for row in both),
            "equal_pitch_duration_bags": sum(row["pitch_duration_bag_equal"] for row in both),
            "equal_part_measure_counts": sum(row["part_measure_counts_equal"] for row in both),
            "equal_part_measure_pitched_events": sum(
                row["part_measure_pitched_events_equal"] for row in both
            ),
            "equal_part_measure_pitched_timelines": sum(
                row["part_measure_pitched_timeline_equal"] for row in both
            ),
            "equal_part_measure_visible_rest_timelines": sum(
                row["part_measure_visible_rest_timeline_equal"] for row in both
            ),
            "equal_part_measure_all_rest_timelines": sum(
                row["part_measure_all_rest_timeline_equal"] for row in both
            ),
            "acorde_projection_counts_match_info": sum(
                row["acorde"]["projection_event_count_matches_info"] for row in acorde_valid
            ),
            "music21_exact_on_both": sum(row["music21"]["exact"] for row in both),
            "acorde_first_exact_on_both": sum(row["acorde"]["first_exact"] for row in both),
            "acorde_contains_gold_on_both": sum(row["acorde"]["candidate_contains_gold"] for row in both),
            "acorde_tied_on_both": sum(len(row["acorde"]["candidates"]) > 1 for row in both),
            "same_note_subset": {
                "count": len(same_notes),
                "music21_exact": sum(row["music21"]["exact"] for row in same_notes),
                "acorde_first_exact": sum(row["acorde"]["first_exact"] for row in same_notes),
                "acorde_contains_gold": sum(row["acorde"]["candidate_contains_gold"] for row in same_notes),
            },
            "explicit_key_same_note_subset": {
                "count": len(explicit_same_notes),
                "music21_exact": sum(row["music21"]["exact"] for row in explicit_same_notes),
                "acorde_first_exact": sum(row["acorde"]["first_exact"] for row in explicit_same_notes),
                "acorde_contains_gold": sum(row["acorde"]["candidate_contains_gold"] for row in explicit_same_notes),
            },
        },
        "rows": rows,
    }
    if key_option:
        report["acorde_analyze_option"] = key_option
    args.output.write_text(json.dumps(report, indent=2, ensure_ascii=False) + "\n")
    print(json.dumps(report["summary"], sort_keys=True))


if __name__ == "__main__":
    main()
