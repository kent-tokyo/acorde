"""Projection preserves every selected movement and its system outcome."""

import importlib.util
from pathlib import Path
import unittest


MODULE = Path(__file__).resolve().parents[1] / "adapt_beethoven_key_report.py"
SPEC = importlib.util.spec_from_file_location("adapt_beethoven_key_report", MODULE)
ADAPTER = importlib.util.module_from_spec(SPEC)
SPEC.loader.exec_module(ADAPTER)


def report(rows):
    return {
        "corpus": "example", "archive_sha256": "a" * 64,
        "music21_version": "10.5.2", "acorde_binary_sha256": "b" * 64,
        "mscore_version": "MuseScore4 4.7.5", "selection": "all annotated",
        "rows": rows,
    }


class AdaptBeethovenTests(unittest.TestCase):
    def test_key_predictions_keep_source_hash_and_failed_conversion(self):
        rows = [
            {"piece": "01-1", "gold": "C major", "source_sha256": "c" * 64,
             "analysis_xml_sha256": "d" * 64,
             "acorde": {"key": "C major"}, "music21": {"key": "a minor"}},
            {"piece": "01-2", "gold": "F major", "source_sha256": "e" * 64,
             "export_error": "converter failed"},
        ]
        manifest = ADAPTER.projection(report(rows))
        self.assertEqual([case["split"] for case in manifest["cases"]],
                         ["development", "development"])
        self.assertEqual(manifest["cases"][0]["input_sha256"], "d" * 64)
        self.assertEqual(manifest["cases"][0]["systems"]["music21"][0]["label"], "a minor")
        self.assertEqual(manifest["cases"][1]["input_sha256"], "e" * 64)
        self.assertIn("conversion", manifest["cases"][1]["systems"]["acorde"]["error"])
        self.assertTrue(manifest["provenance"]["historical_holdout_reused_for_development"])

    def test_missing_system_observation_is_rejected(self):
        rows = [{"piece": "01-1", "gold": "C major", "source_sha256": "c" * 64,
                 "acorde": {"key": "C major"}}]
        with self.assertRaisesRegex(ValueError, "invalid music21"):
            ADAPTER.projection(report(rows))


if __name__ == "__main__":
    unittest.main()
