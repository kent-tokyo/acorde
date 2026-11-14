"""Check cadence location scoring against positive and negative onsets."""

import sys
import unittest
from pathlib import Path

sys.path.insert(0, str(Path(__file__).resolve().parents[1]))
from score_mozart_cadence_acorde_development import score_case


NOTE = {"is_grace": False, "duration": "Quarter", "dot_count": 0, "tuplet": None}


class ScoreMozartCadenceTests(unittest.TestCase):
    def test_located_predictions_count_false_positives(self):
        score = {"parts": [{"staves": [{"measures": [{"voices": [[NOTE, NOTE, NOTE]]}]}]}]}
        gold = {"positions": [
            {"part": 0, "measure": 0, "offset_quarter": [0, 1], "cadence": "PAC"},
            {"part": 0, "measure": 0, "offset_quarter": [1, 1], "cadence": None},
            {"part": 0, "measure": 0, "offset_quarter": [2, 1], "cadence": "HC"},
        ]}
        candidates = [
            {"to": {"part": 0, "staff": 0, "measure": 0, "voice": 0, "note": 0},
             "kind": "Authentic"},
            {"to": {"part": 0, "staff": 0, "measure": 0, "voice": 0, "note": 1},
             "kind": "Half"},
        ]
        counts, _ = score_case(gold, candidates, score, set())
        self.assertEqual(1, counts["correct"])
        self.assertEqual(1, counts["false_positive"])
        self.assertEqual(1, counts["missed"])
        self.assertEqual(0, counts["wrong_label"])


if __name__ == "__main__":
    unittest.main()
