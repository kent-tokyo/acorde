"""The harmony adapter preserves every scored source onset and rejects gaps."""

import sys
from pathlib import Path
import unittest

sys.path.insert(0, str(Path(__file__).resolve().parents[1]))
from adapt_bcmh_harmony_report import adapt, aligned_labels


def row():
    return {
        "score": "chor001_rev.krn",
        "labeled_rows": 2,
        "acorde_predicted_rows": 1,
        "music21_predicted_rows": 2,
        "acorde_canonical_exact": 1,
        "music21_canonical_exact": 2,
        "aligned_rows": [
            {"measure": 0, "row": 0, "beat": 0.5,
             "gold": "IV42", "acorde": "IV2", "music21": "IV42"},
            {"measure": 0, "row": 1, "beat": 1.0,
             "gold": "V7", "acorde": None, "music21": "V7"},
        ],
    }


class AdaptBcmhHarmonyTests(unittest.TestCase):
    def test_all_gold_positions_and_missing_prediction_survive(self):
        gold, acorde, music21 = aligned_labels(row())
        self.assertEqual(len(gold), 2)
        self.assertEqual(gold[0]["at"]["beat"], "1/2")
        self.assertEqual(gold[0]["labels"], ["IV2"])
        self.assertEqual(len(acorde), 1)
        self.assertEqual(len(music21), 2)

    def test_missing_or_duplicate_source_rows_are_rejected(self):
        incomplete = row()
        incomplete["aligned_rows"].pop()
        with self.assertRaisesRegex(ValueError, "incomplete aligned_rows"):
            aligned_labels(incomplete)

        duplicated = row()
        duplicated["aligned_rows"][1]["beat"] = 0.5
        with self.assertRaisesRegex(ValueError, "duplicate source position"):
            aligned_labels(duplicated)

    def test_bounded_error_report_cannot_be_projected(self):
        with self.assertRaisesRegex(ValueError, "contract_version 4"):
            adapt({"contract_version": 3}, b"", report_sha256="a" * 64)


if __name__ == "__main__":
    unittest.main()
