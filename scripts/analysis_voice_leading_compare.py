#!/usr/bin/env python3
"""Compare Acorde's located quartets with music21 10.5.2 on identical notes."""

from __future__ import annotations

import argparse
import json
import subprocess
import sys
from pathlib import Path

import music21
from music21 import note, voiceLeading


PINNED_MUSIC21 = "10.5.2"
FIELDS = (
    "parallel_fifth", "parallel_octave", "contrary_motion", "similar_motion",
    "proper_resolution_without_key",
)


def music21_note(data: dict) -> note.Note:
    pitch_data = data["pitch"]
    if pitch_data["microtone_cents"]:
        raise ValueError("music21 quartet comparator requires integer-semitone pitches")
    result = note.Note()
    result.pitch.step = pitch_data["step"]
    result.pitch.octave = pitch_data["octave"]
    result.pitch.accidental = pitch_data["alter"]
    result.quarterLength = data["duration_beats"]
    result.offset = data["onset_beats"]
    return result


def reference_labels(quartet: dict) -> dict[str, bool]:
    vlq = voiceLeading.VoiceLeadingQuartet(
        music21_note(quartet["first_before"]),
        music21_note(quartet["first_after"]),
        music21_note(quartet["second_before"]),
        music21_note(quartet["second_after"]),
    )
    return {
        "parallel_fifth": vlq.parallelFifth(),
        "parallel_octave": vlq.parallelOctave(),
        "contrary_motion": vlq.contraryMotion(),
        "similar_motion": vlq.similarMotion(),
        "proper_resolution_without_key": vlq.isProperResolution(),
    }


def main() -> int:
    parser = argparse.ArgumentParser(description=__doc__)
    parser.add_argument("score", type=Path, help="MusicXML input for both quartet extraction and review")
    parser.add_argument("--acorde-bin", type=Path, default=Path("target/debug/acorde"))
    args = parser.parse_args()
    if music21.__version__ != PINNED_MUSIC21:
        parser.error(f"music21 {PINNED_MUSIC21} required, found {music21.__version__}")
    output = subprocess.check_output(
        [str(args.acorde_bin.resolve()), "analyze-voice-leading", str(args.score.resolve())],
        text=True,
    )
    report = json.loads(output)
    mismatches = []
    for index, quartet in enumerate(report["quartets"]):
        expected = reference_labels(quartet)
        actual = {field: quartet[field] for field in FIELDS}
        if actual != expected:
            mismatches.append({
                "quartet_index": index,
                "evidence": quartet["evidence"],
                "acorde": actual,
                "music21": expected,
            })
    print(json.dumps({
        "score": str(args.score),
        "score_fingerprint": report["score_fingerprint"],
        "music21_version": music21.__version__,
        "quartets": len(report["quartets"]),
        "mismatches": mismatches,
    }, indent=2))
    return 1 if mismatches else 0


if __name__ == "__main__":
    sys.exit(main())
