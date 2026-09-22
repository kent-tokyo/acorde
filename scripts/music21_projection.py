#!/usr/bin/env python3
"""Emit a bounded, reproducible music21 projection for one score fixture.

This is an optional Phase 17 evidence helper, not a dependency of acorde or its test suite.
It intentionally records only fields with a clear counterpart in a score interoperability
review: parsed part/measure/note/rest/chord counts and music21's inferred key.  It does not
claim engraving, playback, or general analysis equivalence.
"""

from __future__ import annotations

import argparse
import hashlib
import json
from pathlib import Path


def main() -> int:
    parser = argparse.ArgumentParser(description=__doc__)
    parser.add_argument("input", type=Path, help="MusicXML or another music21-readable score")
    args = parser.parse_args()

    try:
        from music21 import chord, converter, note
        import music21
    except ImportError as error:
        parser.error(f"music21 is required: {error}")

    source = args.input.read_bytes()
    score = converter.parse(str(args.input))
    recursive = score.recurse()
    note_count = len(list(recursive.getElementsByClass(note.Note)))
    rest_count = len(list(score.recurse().getElementsByClass(note.Rest)))
    chord_count = len(list(score.recurse().getElementsByClass(chord.Chord)))
    measure_count = len(list(score.recurse().getElementsByClass("Measure")))
    inferred_key = str(score.analyze("key"))
    report = {
        "contract_version": 1,
        "tool": {"name": "music21", "version": music21.__version__},
        "fixture": {
            "path": args.input.as_posix(),
            "sha256": hashlib.sha256(source).hexdigest(),
        },
        "options": {"key_estimator": "music21.score.analyze('key')"},
        "projection": {
            "parts": len(score.parts),
            "measures": measure_count,
            "pitched_notes": note_count,
            "rests": rest_count,
            "chord_elements": chord_count,
            "inferred_key": inferred_key,
        },
    }
    print(json.dumps(report, indent=2, sort_keys=True))
    return 0


if __name__ == "__main__":
    raise SystemExit(main())
