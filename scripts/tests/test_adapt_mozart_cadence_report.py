import json
import sys
import tempfile
import unittest
from pathlib import Path

sys.path.insert(0, str(Path(__file__).resolve().parents[1]))

import adapt_mozart_cadence_report as adapter  # noqa: E402


def report(split, rows, gold_positive, **extra):
    case = {"id": "K1-1", "input_sha256": "0" * 64, "rows": rows,
            "gold_positive": gold_positive, **extra}
    return {"split": split, "acorde_binary_sha256": "a", "acorde_command": "analyze-cadences INPUT",
            "music21_version": "10.5.2", "recipe": "r", "cases": [case]}


class CadenceAdapterTest(unittest.TestCase):
    def write(self, tmp, name, value):
        path = Path(tmp) / name
        path.write_text(json.dumps(value))
        return path

    def test_gold_and_predictions_keep_positions_and_splits(self):
        rows = [{"measure": 2, "offset_quarter": [3, 2], "gold": "PAC", "gold_coarse": "Authentic",
                 "acorde": ["Authentic"], "music21": []},
                {"measure": 4, "offset_quarter": [0, 1], "gold": None, "gold_coarse": None,
                 "acorde": [], "music21": ["Authentic"]}]
        with tempfile.TemporaryDirectory() as tmp:
            dev = self.write(tmp, "dev.json", report("development", rows, 1))
            hold = self.write(tmp, "hold.json", report("holdout", rows[:1], 1,
                                                       music21_error="ParseError"))
            manifest = adapter.adapt([dev, hold])
        first, second = manifest["cases"]
        self.assertEqual((first["split"], second["split"]), ("development", "holdout"))
        self.assertEqual(first["gold"], [{"task": adapter.TASK,
                                          "at": {"part": 0, "staff": 0, "measure": 2, "beat": "3/2"},
                                          "labels": ["Authentic"]}])
        self.assertEqual(len(first["systems"]["acorde"]), 1)
        self.assertEqual(first["systems"]["music21"][0]["at"]["measure"], 4)
        self.assertEqual(second["systems"]["music21"], {"error": "ParseError"})

    def test_rejects_gold_count_drift_and_ambiguous_predictions(self):
        row = {"measure": 0, "offset_quarter": [0, 1], "gold": "HC", "gold_coarse": "Half",
               "acorde": ["Authentic", "Half"], "music21": []}
        with tempfile.TemporaryDirectory() as tmp:
            with self.assertRaises(ValueError):
                adapter.adapt([self.write(tmp, "a.json", report("development", [row], 2))])
            with self.assertRaises(ValueError):
                adapter.adapt([self.write(tmp, "b.json", report("development", [row], 1))])


if __name__ == "__main__":
    unittest.main()
