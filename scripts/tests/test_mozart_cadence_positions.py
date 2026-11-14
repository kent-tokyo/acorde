"""Check folded repeat IDs, exact offsets, and conflicting labels."""

import sys
import unittest
from pathlib import Path

sys.path.insert(0, str(Path(__file__).resolve().parents[1]))
from mozart_cadence_positions import extract


XML = b"""<score-partwise><part id="P1"><measure number="1">
  <attributes><divisions>2</divisions></attributes>
  <note id="n1"><pitch><step>C</step><octave>4</octave></pitch><duration>1</duration></note>
  <note id="n2"><pitch><step>D</step><octave>4</octave></pitch><duration>1</duration></note>
</measure></part></score-partwise>"""
HEADER = "id,xml_mn,pitch,cadence,cadence_type\n"


class MozartCadencePositionsTests(unittest.TestCase):
    def test_repeats_fold_to_source_note_positions(self):
        csv = HEADER + "n1-1,1,60,PAC,1\nn2-1,1,62,,0\nn1-2,1,60,PAC,1\n"
        result = extract(XML, csv)
        self.assertEqual(3, result["source_rows"])
        self.assertEqual(2, result["eligible_positions"])
        self.assertEqual(1, result["positive_positions"])
        self.assertEqual([0, 1], result["positions"][0]["offset_quarter"])
        self.assertEqual([1, 2], result["positions"][1]["offset_quarter"])

    def test_conflicting_repeat_labels_are_excluded(self):
        csv = HEADER + "n1-1,1,60,PAC,1\nn1-2,1,60,HC,2\n"
        result = extract(XML, csv)
        self.assertEqual(0, result["eligible_positions"])
        self.assertEqual("conflicting_repeat_labels", result["exclusions"][0]["reason"])


if __name__ == "__main__":
    unittest.main()
