#!/usr/bin/env python3
"""Compare bounded channel-event projections from Mido and Acorde for one MIDI fixture."""

from __future__ import annotations

import argparse
import hashlib
import json
import subprocess
import sys
from collections import Counter
from importlib.metadata import version as package_version
from pathlib import Path
from typing import Any


CONTRACT_VERSION = 1


def sha256(path: Path) -> str:
    return hashlib.sha256(path.read_bytes()).hexdigest()


def canonical(values: list[dict[str, int]]) -> list[dict[str, int]]:
    return sorted(values, key=lambda value: tuple(value[key] for key in sorted(value)))


def main() -> int:
    parser = argparse.ArgumentParser(description=__doc__)
    parser.add_argument("input", type=Path)
    parser.add_argument("--acorde", default="acorde")
    parser.add_argument("--fixture-manifest", type=Path, default=Path("tests/fixtures/manifest.json"))
    parser.add_argument("--root", type=Path, default=Path.cwd())
    parser.add_argument("--output", type=Path)
    args = parser.parse_args()
    try:
        import mido

        source = args.input.resolve()
        root = args.root.resolve()
        manifest = json.loads(args.fixture_manifest.read_text(encoding="utf-8"))
        fixture_name = source.relative_to(args.fixture_manifest.resolve().parent).as_posix()
        fixture = next((item for item in manifest.get("fixtures", []) if item.get("path") == fixture_name), None)
        if not fixture or fixture.get("format") != "midi" or fixture.get("sha256") != sha256(source):
            raise ValueError("input must be a registered, checksum-matching MIDI fixture")
        completed = subprocess.run(
            [args.acorde, "report", str(source)], check=False, capture_output=True, text=True
        )
        if completed.returncode:
            raise ValueError(completed.stderr.strip() or "acorde report failed")
        acorde = json.loads(completed.stdout)["score"]
        midi = mido.MidiFile(source)
        mido_events: dict[str, list[dict[str, int]]] = {"pitch_bends": [], "control_changes": [], "program_changes": [], "aftertouch": []}
        note_on_count = percussion_note_on_count = 0
        tempo_events: list[dict[str, int]] = []
        time_signature = None
        for track in midi.tracks:
            tick = 0
            for message in track:
                tick += message.time
                if message.type == "pitchwheel":
                    mido_events["pitch_bends"].append({"tick": tick, "channel": message.channel, "value": message.pitch})
                elif message.type == "control_change":
                    mido_events["control_changes"].append({"tick": tick, "channel": message.channel, "controller": message.control, "value": message.value})
                elif message.type == "program_change":
                    mido_events["program_changes"].append({"tick": tick, "channel": message.channel, "program": message.program})
                elif message.type == "aftertouch":
                    mido_events["aftertouch"].append({"tick": tick, "channel": message.channel, "value": message.value})
                elif message.type == "note_on" and message.velocity:
                    note_on_count += 1
                    percussion_note_on_count += int(message.channel == 9)
                elif message.type == "set_tempo":
                    tempo_events.append({"tick": tick, "bpm": round(60_000_000 / message.tempo)})
                elif message.type == "time_signature" and time_signature is None:
                    time_signature = {"numerator": message.numerator, "denominator": message.denominator}
        acorde_events = {"pitch_bends": [], "control_changes": [], "program_changes": [], "aftertouch": []}
        note_count = unpitched_count = 0
        for part in acorde["parts"]:
            for key, score_key in (("pitch_bends", "midi_pitch_bends"), ("control_changes", "midi_control_changes"), ("program_changes", "midi_program_changes"), ("aftertouch", "midi_aftertouch")):
                acorde_events[key].extend(part[score_key])
            for staff in part["staves"]:
                for measure in staff["measures"]:
                    for voice in measure["voices"]:
                        for note in voice:
                            if not note["is_rest"]:
                                note_count += 1
                                unpitched_count += int(note["is_unpitched"])
        event_matches = {key: canonical(mido_events[key]) == canonical(acorde_events[key]) for key in mido_events}
        settings = acorde["settings"]
        projection = {
            "mido": {
                "file_type": midi.type,
                "ticks_per_beat": midi.ticks_per_beat,
                "note_on_count": note_on_count,
                "percussion_note_on_count": percussion_note_on_count,
                "tempo_events": tempo_events,
                "time_signature": time_signature,
                **{key: canonical(value) for key, value in mido_events.items()},
            },
            "acorde": {
                "part_count": len(acorde["parts"]),
                "note_count": note_count,
                "unpitched_note_count": unpitched_count,
                "initial_tempo_bpm": settings["tempo_bpm"],
                "time_signature": settings["time_signature"],
                **{key: canonical(value) for key, value in acorde_events.items()},
            },
            "comparison": {
                "channel_events_match": event_matches,
                "all_channel_events_match": all(event_matches.values()),
                "note_count_match": note_on_count == note_count,
                "percussion_note_count_match": percussion_note_on_count == unpitched_count,
                "initial_tempo_bpm_match": bool(tempo_events) and tempo_events[0]["bpm"] == settings["tempo_bpm"],
                "time_signature_match": time_signature == settings["time_signature"],
            },
        }
        result = {
            "contract_version": CONTRACT_VERSION,
            "evidence_kind": "midi-event-observation",
            "tool": {"name": "Mido", "version": package_version("mido")},
            "fixture": {
                "path": source.relative_to(root).as_posix(), "format": "midi", "sha256": fixture["sha256"],
                **{key: fixture[key] for key in ("license", "source_url") if key in fixture},
            },
            "options": {"projection": "channel events, initial tempo/time signature, note and percussion counts"},
            "execution_status": "success",
            "projection": projection,
        }
        text = json.dumps(result, indent=2, sort_keys=True) + "\n"
        if args.output:
            args.output.write_text(text, encoding="utf-8")
        else:
            print(text, end="")
    except (ImportError, OSError, ValueError, json.JSONDecodeError) as error:
        print(f"MIDI comparison invalid: {error}", file=sys.stderr)
        return 1
    return 0


if __name__ == "__main__":
    raise SystemExit(main())
