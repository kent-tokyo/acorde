#!/usr/bin/env python3
"""Independently compare MusicXML pitched events with imported Score JSON.

Audits written pitch spelling, physical staff, source voice, onset, duration,
grace/hidden state and per-pitch ties. This is not a rest, text or layout proof.
Usage: audit_print_musicxml.py COMPARISON.json WORKDIR ACORDE OUT.json
"""
import collections
from fractions import Fraction as F
import hashlib
import json
import pathlib
import subprocess
import sys

from compare_print_corpus import source_root, ROOT

BASE = dict(zip(["Longa", "Breve", "Whole", "Half", "Quarter", "Eighth", "Sixteenth",
                 "ThirtySecond", "SixtyFourth", "HundredTwentyEighth"], [F(16, 2**i) for i in range(10)]))


def xml_events(root):
    events = collections.Counter()
    for pi, part in enumerate(root.findall("part")):
        divisions = 1
        for mi, measure in enumerate(part.findall("measure")):
            cursor = previous = F(0)
            for node in measure:
                if node.tag == "attributes":
                    divisions = int(node.findtext("divisions", divisions))
                elif node.tag in ("backup", "forward"):
                    cursor += (-1 if node.tag == "backup" else 1) * F(node.findtext("duration", "0")) / divisions
                elif node.tag == "note":
                    grace = node.find("grace") is not None
                    duration = F(0) if grace else F(node.findtext("duration", "0")) / divisions
                    chord = node.find("chord") is not None
                    onset = previous if chord else cursor
                    if not chord:
                        previous = cursor
                        cursor += duration
                    pitch = node.find("pitch")
                    if pitch is None:
                        continue
                    ties = {n.get("type") for n in node.findall("notations/tied")}
                    events[(pi, mi, int(node.findtext("staff", "1"))-1,
                            int(node.findtext("voice", "1")), str(onset), str(duration),
                            pitch.findtext("step"), int(pitch.findtext("octave")),
                            str(F(pitch.findtext("alter", "0"))*100), grace, node.find("cue") is not None,
                            node.get("print-object") == "no", "start" in ties, "stop" in ties)] += 1
    return events


def score_events(score):
    events = collections.Counter()
    for pi, part in enumerate(score["parts"]):
        for si, staff in enumerate(part["staves"]):
            for mi, measure in enumerate(staff["measures"]):
                for vi, voice in enumerate(measure["voices"]):
                    cursor = F(0)
                    source_voices = measure.get("source_voice_numbers", [])
                    source_voice = (source_voices[vi] if vi < len(source_voices) else None) or vi+1
                    for note in voice:
                        dots = note["dot_count"]
                        duration = BASE[note["duration"]] * sum((F(1, 2**d) for d in range(dots+1)), F(0))
                        if note.get("tuplet"):
                            t = note["tuplet"]
                            duration *= F(t["normal_notes"], t["actual_notes"])
                        if note["is_grace"]:
                            duration = F(0)
                        physical = note.get("cross_staff") or {}
                        staff_number = physical.get("target_staff", si)
                        for index, pitch in enumerate(note["pitches"]):
                            starts, ends = note.get("pitch_tie_starts", []), note.get("pitch_tie_ends", [])
                            start = starts[index] if index < len(starts) else note["tie_start"]
                            end = ends[index] if index < len(ends) else note["tie_end"]
                            events[(pi, mi, staff_number, source_voice, str(cursor), str(duration),
                                    pitch["step"], pitch["octave"], str(F(pitch["alter"]*100 + pitch.get("microtone_cents", 0))),
                                    note["is_grace"], note["is_cue"], note.get("hidden", False), start, end)] += 1
                        # Imported cue durations are carried by a hidden timeline spacer;
                        # audit their written duration while retaining the model's onset.
                        if not note["is_cue"]:
                            cursor += duration
    return events


def main():
    comparison, work, binary, output = map(pathlib.Path, sys.argv[1:])
    rows = []
    for row in json.loads(comparison.read_text()):
        source = ROOT / row["source"]
        shared = source if source.suffix in (".mxl", ".musicxml") else work / f'{row["score"]}.musicxml'
        report = json.loads(subprocess.run([str(binary.resolve()), "report", str(shared)], check=True, capture_output=True, text=True).stdout)
        expected, actual = xml_events(source_root(shared)), score_events(report["score"])
        rows.append({"score": row["score"], "shared_input_sha256": hashlib.sha256(shared.read_bytes()).hexdigest(),
                     "source_pitches": sum(expected.values()), "imported_pitches": sum(actual.values()),
                     "missing": [{"event": k, "count": n} for k,n in (expected-actual).items()],
                     "extra": [{"event": k, "count": n} for k,n in (actual-expected).items()]})
    output.write_text(json.dumps({"event_fields": ["part", "measure", "staff", "source_voice", "onset_beats", "written_duration_beats", "step", "octave", "alter_cents", "grace", "cue", "hidden", "tie_start", "tie_stop"], "rows": rows}, indent=2)+"\n")
    for row in rows:
        print(row["score"], row["source_pitches"], row["imported_pitches"], "missing", sum(x["count"] for x in row["missing"]), "extra", sum(x["count"] for x in row["extra"]))


if __name__ == "__main__":
    main()
