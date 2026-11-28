import json
from pathlib import Path
import sys
import tempfile
import unittest

sys.path.insert(0, str(Path(__file__).resolve().parents[1]))

import adapt_voice_leading_dahn_report as adapter  # noqa: E402
import evaluate_analysis_labels as evaluator  # noqa: E402


def report(split="development"):
    return {"split": split, "acorde_binary_sha256": "0" * 64, "acorde_command": "analyze-voice-leading INPUT",
            "music21_version": "10.5.2", "recipe": "music21-figuredBass-checker-v1",
            "cases": [{"bwv": "1.6", "input_sha256": "1" * 64,
                       "labels": [{"id": "1.0", "measures": [3, 4], "interval": "fifth", "upper": "S", "lower": "B"},
                                  {"id": "2.0", "measures": [7], "interval": "octave", "upper": "A", "lower": "T"}],
                       "acorde": {"positions": [
                           {"interval": "fifth", "upper": "S", "lower": "B", "measure": 3, "label": "1.0"},
                           {"interval": "fifth", "upper": "T", "lower": "B", "measure": 9, "label": None}]},
                       "music21_error": "boom"}]}


class AdaptVoiceLeadingTest(unittest.TestCase):
    def test_scores_covered_unlisted_and_failure(self):
        with tempfile.TemporaryDirectory() as work:
            path = Path(work) / "dev.json"
            path.write_text(json.dumps(report()))
            manifest = adapter.adapt([path])
        case = manifest["cases"][0]
        self.assertEqual(case["gold"][0]["at"], {"part": 0, "staff": 0, "measure": 3, "beat": "0", "voice": 3})
        self.assertEqual(case["systems"]["music21"], {"error": "boom"})
        totals = evaluator.evaluate(manifest)["totals"]
        acorde = totals["acorde"]["development"]
        self.assertEqual((acorde["correct"], acorde["missing"], acorde["extra"]), (1, 1, 1))
        self.assertEqual(totals["music21"]["development"]["failed"], 1)

    def test_rejects_duplicate_gold_position(self):
        data = report()
        data["cases"][0]["labels"].append(dict(data["cases"][0]["labels"][0], id="3.0"))
        with tempfile.TemporaryDirectory() as work:
            path = Path(work) / "dev.json"
            path.write_text(json.dumps(data))
            with self.assertRaises(ValueError):
                adapter.adapt([path])


if __name__ == "__main__":
    unittest.main()
