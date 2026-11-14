"""The WIR projection preserves source positions and exact rhythmic fractions."""

import hashlib
import json
from fractions import Fraction
from pathlib import Path
import sys
import unittest

sys.path.insert(0, str(Path(__file__).resolve().parents[1]))
from adapt_wir_harmony_report import adapt
from compare_wir_harmony_music21 import acorde_fraction_beats
from evaluate_analysis_labels import evaluate


class WirHarmonyAdapterTests(unittest.TestCase):
    def test_triplet_onsets_do_not_accumulate_rounded_durations(self):
        note = {"duration": "Sixteenth", "dot_count": 0, "is_grace": False,
                "tuplet": {"normal_notes": 2, "actual_notes": 3}}
        duration = acorde_fraction_beats(note)
        self.assertEqual(duration, Fraction(1, 6))
        self.assertEqual(duration * 20, Fraction(10, 3))

    def test_projection_matches_common_evaluator_and_rejects_changed_gold(self):
        work = "Corpus/Example/Score/1"
        selected = {"commit": "pinned", "works": [{"work": work,
                    "score_path": work + "/score.mxl"}]}
        selection_bytes = json.dumps(selected).encode()
        selection_sha = hashlib.sha256(selection_bytes).hexdigest()
        hashes = {"commit": "pinned", "selection_sha256": selection_sha,
                  "files": [{"work": work, "score_sha256": "a" * 64,
                             "gold_sha256": "b" * 64}]}
        row = {"measure": 1, "measure_index": 0, "beat": 0.333333,
               "gold": "IV42", "acorde": "IV2", "music21": None,
               "acorde_correct": True, "music21_correct": False}
        observed = {"work": work, "score_sha256": "a" * 64,
                    "gold_sha256": "b" * 64, "source_labels": 1,
                    "rows": [row], "exclusions": []}
        report = {"schema_version": 1, "classification": "development",
                  "commit": "pinned", "selection_sha256": selection_sha,
                  "acorde_binary_sha256": "c" * 64, "music21_version": "10.5.2",
                  "summary": {"scored_labels": 1, "acorde_predicted": 1,
                              "music21_predicted": 0}, "works": [observed]}
        report_bytes = json.dumps(report).encode()
        manifest = adapt(report, selected, hashes, report_bytes=report_bytes,
                         selection_bytes=selection_bytes)
        self.assertEqual(manifest["cases"][0]["gold"][0]["at"]["beat"], "1/3")
        result = evaluate(manifest)
        self.assertEqual(result["totals"]["acorde"]["development"]["correct"], 1)
        self.assertEqual(result["totals"]["music21"]["development"]["missing"], 1)

        observed["gold_sha256"] = "d" * 64
        with self.assertRaisesRegex(ValueError, "gold hash mismatch"):
            adapt(report, selected, hashes, report_bytes=report_bytes,
                  selection_bytes=selection_bytes)


if __name__ == "__main__":
    unittest.main()
