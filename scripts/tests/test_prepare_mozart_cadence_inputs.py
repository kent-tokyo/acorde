"""Check that answer removal preserves pitched score events."""

import sys
import unittest
import xml.etree.ElementTree as ET
from pathlib import Path

sys.path.insert(0, str(Path(__file__).resolve().parents[1]))
from prepare_mozart_cadence_inputs import prepare


class PrepareMozartCadenceTests(unittest.TestCase):
    def test_removes_answers_and_preserves_notes(self):
        source = b"""<score-partwise><credit/><part id="P1"><measure number="1">
          <attributes><divisions>1</divisions></attributes>
          <harmony><root><root-step>C</root-step></root></harmony>
          <direction><direction-type><words>cadence</words></direction-type></direction>
          <note><pitch><step>C</step><octave>4</octave></pitch><duration>1</duration>
            <lyric><text>answer</text></lyric></note></measure></part></score-partwise>"""
        clean, removed, repaired = prepare(source)
        root = ET.fromstring(clean)
        self.assertEqual({"credit": 1, "direction": 1, "harmony": 1, "lyric": 1}, removed)
        self.assertEqual({}, repaired)
        self.assertEqual("C", root.findtext(".//note/pitch/step"))
        self.assertEqual("1", root.findtext(".//note/duration"))
        self.assertFalse(any(element.tag in {"harmony", "direction", "lyric", "credit"}
                             for element in root.iter()))

    def test_repairs_input_only_import_errors(self):
        source = b"""<score-partwise><part id="P1"><measure number="1">
          <note><pitch><step>C</step><octave>4</octave></pitch><duration>1</duration>
            <type>eigth</type></note><backup><duration>1</duration></backup>
          <note><chord/><pitch><step>E</step><octave>4</octave></pitch>
            <duration>1</duration><type>quarter</type></note>
        </measure></part></score-partwise>"""
        clean, removed, repaired = prepare(source)
        root = ET.fromstring(clean)
        self.assertEqual({}, removed)
        self.assertEqual({"eigth_to_eighth": 1, "orphan_chord_removed": 1}, repaired)
        self.assertEqual(["C", "E"], [note.findtext("pitch/step") for note in root.findall(".//note")])
        self.assertEqual("eighth", root.findtext(".//note/type"))


if __name__ == "__main__":
    unittest.main()
