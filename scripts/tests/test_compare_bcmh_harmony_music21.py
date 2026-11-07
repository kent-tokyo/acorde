"""Check source-derived beats and the limited Roman-figure normalization."""

import sys
import unittest
from pathlib import Path

sys.path.insert(0, str(Path(__file__).resolve().parents[1]))
from compare_bcmh_harmony_music21 import canonical_roman, source_beats


class SourceBeatTests(unittest.TestCase):
    def test_pickup_staggered_voices_and_dotted_duration(self):
        rows, errors = source_beats([
            "**harm\t**kern\t**kern",
            "I\t4c\t2e",
            "V\t4d\t.",
            "=1\t=1\t=1",
            "I\t4.c\t4.e",
            "V6\t8d\t8f",
        ])
        self.assertEqual(dict(rows), {
            0: [("I", 0.0), ("V", 1.0)],
            1: [("I", 0.0), ("V6", 1.5)],
        })
        self.assertEqual(errors, {})

    def test_unsupported_duration_is_excluded(self):
        _, errors = source_beats([
            "I\t4c\t4e", "V\tf#\t4f",
        ])
        self.assertEqual(errors[0], "unsupported_duration")

    def test_roman_normalization_is_narrow(self):
        self.assertEqual(canonical_roman("IV42"), "IV2")
        self.assertEqual(canonical_roman("vii%7"), "viiø7")
        self.assertNotEqual(canonical_roman("I7"), canonical_roman("V7/IV"))


if __name__ == "__main__":
    unittest.main()
