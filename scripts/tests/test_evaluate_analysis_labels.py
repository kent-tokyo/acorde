"""The label evaluator must reject equal-count but musically wrong outputs."""

import importlib.util
import hashlib
from pathlib import Path
import tempfile
import unittest


MODULE = Path(__file__).resolve().parents[1] / "evaluate_analysis_labels.py"
SPEC = importlib.util.spec_from_file_location("evaluate_analysis_labels", MODULE)
EVALUATOR = importlib.util.module_from_spec(SPEC)
SPEC.loader.exec_module(EVALUATOR)


def event(beat):
    return {"part": 0, "staff": 0, "measure": 2, "voice": 0, "beat": beat}


def manifest(gold, predictions, *, other=None):
    return {
        "schema_version": 1,
        "system_profiles": {
            "acorde": {"version": "1.3.12", "command": "acorde analyze", "options": {}},
            "music21": {"version": "10.5.2", "command": "score.analyze", "options": {}},
        },
        "cases": [{
            "id": "piece-1", "input_sha256": "a" * 64, "split": "holdout",
            "gold": gold,
            "systems": {"acorde": predictions, "music21": other if other is not None else []},
        }],
    }


class EvaluateAnalysisLabelsTests(unittest.TestCase):
    def test_wrong_label_and_wrong_location_fail_despite_equal_counts(self):
        gold = [{"task": "roman", "at": event("1/2"), "labels": ["V7/V", "II7"]}]
        predictions = [{"task": "roman", "at": event("2/4"), "label": "V7/V"}]
        result = EVALUATOR.evaluate(manifest(gold, predictions))
        acorde = result["cases"][0]["systems"]["acorde"]
        self.assertEqual((acorde["correct"], acorde["f1"]), (1, 1.0))
        predictions[0]["label"] = "V7"
        acorde = EVALUATOR.evaluate(manifest(gold, predictions))["cases"][0]["systems"]["acorde"]
        self.assertEqual((acorde["correct"], acorde["wrong_label"], acorde["f1"]), (0, 1, 0.0))
        predictions[0]["at"] = event("3/4")
        acorde = EVALUATOR.evaluate(manifest(gold, predictions))["cases"][0]["systems"]["acorde"]
        self.assertEqual((acorde["correct"], acorde["missing"], acorde["extra"]), (0, 1, 1))

    def test_missing_error_and_empty_prediction_are_distinct(self):
        gold = [{"task": "key", "at": "score", "labels": ["C major"]}]
        result = EVALUATOR.evaluate(manifest(gold, {"error": "cannot parse"}))
        self.assertEqual(result["cases"][0]["systems"]["acorde"]["status"], "error")
        self.assertEqual(result["totals"]["acorde"]["holdout"]["failed"], 1)
        result = EVALUATOR.evaluate(manifest(gold, []))
        self.assertEqual(result["cases"][0]["systems"]["acorde"]["status"], "ok")
        self.assertEqual(result["totals"]["acorde"]["holdout"]["missing"], 1)
        self.assertEqual(result["totals"]["acorde"]["development"]["gold"], 0)

    def test_task_metrics_do_not_hide_weak_key_results_behind_roman_rows(self):
        gold = [{"task": "key", "at": "score", "labels": ["C major"]},
                {"task": "roman", "at": event("1/2"), "labels": ["V"]},
                {"task": "roman", "at": event("3/4"), "labels": ["I"]}]
        predictions = [{"task": "key", "at": "score", "label": "a minor"},
                       {"task": "roman", "at": event("1/2"), "label": "V"},
                       {"task": "roman", "at": event("3/4"), "label": "I"}]
        totals = EVALUATOR.evaluate(manifest(gold, predictions))["totals"]["acorde"]["holdout"]
        self.assertEqual(totals["tasks"]["key"]["f1"], 0.0)
        self.assertEqual(totals["tasks"]["roman"]["f1"], 1.0)
        self.assertEqual(totals["correct"], 2)

    def test_duplicate_location_and_invalid_fraction_are_rejected(self):
        gold = [{"task": "roman", "at": event("1/2"), "labels": ["V"]}]
        duplicate = [{"task": "roman", "at": event("1/2"), "label": "V"},
                     {"task": "roman", "at": event("2/4"), "label": "V"}]
        with self.assertRaisesRegex(ValueError, "duplicate"):
            EVALUATOR.evaluate(manifest(gold, duplicate))
        with self.assertRaisesRegex(ValueError, "denominator"):
            EVALUATOR.evaluate(manifest(gold, [{"task": "roman", "at": event("1/0"),
                                                "label": "V"}]))

    def test_spans_require_ordered_positions(self):
        gold = [{"task": "cadence", "at": {"start": event("1/2"), "end": event("3/4")},
                 "labels": ["authentic"]}]
        result = EVALUATOR.evaluate(manifest(gold, [{"task": "cadence",
            "at": {"start": event("2/4"), "end": event("6/8")},
            "label": "authentic"}]))
        self.assertEqual(result["cases"][0]["systems"]["acorde"]["correct"], 1)
        with self.assertRaisesRegex(ValueError, "ordered"):
            EVALUATOR.evaluate(manifest(gold, [{"task": "cadence",
                "at": {"start": event("3/4"), "end": event("1/2")},
                "label": "authentic"}]))
        later_part = event("1/2")
        later_part["part"] = 1
        with self.assertRaisesRegex(ValueError, "ordered"):
            EVALUATOR.evaluate(manifest(gold, [{"task": "cadence",
                "at": {"start": event("3/4"), "end": later_part},
                "label": "authentic"}]))

    def test_input_hash_is_verified_against_bytes(self):
        with tempfile.TemporaryDirectory() as directory:
            path = Path(directory) / "piece.musicxml"
            path.write_bytes(b"score bytes")
            data = manifest([], [])
            data["cases"][0]["input_path"] = path.name
            data["cases"][0]["input_sha256"] = hashlib.sha256(path.read_bytes()).hexdigest()
            report = EVALUATOR.evaluate(data, base_dir=Path(directory))
            self.assertTrue(report["cases"][0]["input_verified"])
            path.write_bytes(b"changed score")
            with self.assertRaisesRegex(ValueError, "SHA-256 mismatch"):
                EVALUATOR.evaluate(data, base_dir=Path(directory))


if __name__ == "__main__":
    unittest.main()
